# ISM-bridged Supervisor execution — planning notes

Status: **planning only, nothing built.** Captures the 2026-09-22 discussion
that produced a concrete, verified-feasible direction. Read alongside
[shared-agent-reference-survey.md](shared-agent-reference-survey.md), which
this plan responds to.

## The problem this starts from

Desktop-spawned personas (e.g. "Fizz") are local per-install, not shared —
two people in the same channel each get their own independently-keyed
instance, no shared identity or memory. Confirmed as a known, still-open
upstream issue (`block/buzz#2910`) and RFC (`#4174`); ~10 prior upstream
attempts never converged, because there's no protocol-level way to mint one
canonical agent identity regardless of how it's created. Not something to
solve head-on speculatively (see the survey's conclusion: nobody has solved
"one shared identity, many independent clients" cleanly, including
well-funded incumbents).

## The reframe that produced a workable direction

Rather than solve general multi-party shared-agent identity, narrow the goal:
**humans collaborate normally in a Buzz channel → a role-gated decision hands
work to one centrally-deployed Supervisor agent → Supervisor executes (solo
or via internal sub-agent fan-out) → one consolidated result posts back.**
This sidesteps the identity-collision problem by construction (one Supervisor
identity, deployed once, never per-desktop-auto-provisioned) rather than
trying to generalize it away.

Checked against the industry: this is the same shape Atlassian's Rovo
Service and "Agents in Jira" (GA since May 2026) already ship — ticket in,
supervisor agent breaks it down, executes, closes it. That's not a reason to
avoid building it; convergent architecture under real constraints is a
validation signal. It is a reason **not** to try to out-build Atlassian's
ticket-AI product — our reason to do this is Buzz's own value (sovereignty,
audit-provability, no context-switch out of the team's existing chat), not
market novelty.

## Three architecture options considered

| | A. ISM builds its own live discussion (replaces comments) | B. Buzz discusses + full bidirectional bridge to ISM (ISM = executor) | **C. Buzz discusses + Buzz hosts the Supervisor/AI team (chosen direction)** |
|---|---|---|---|
| New real-time chat infra needed | Yes — presence, WS delivery, threading, notifications from scratch | No | No |
| AI execution/trigger engine | Build fresh (ISM's Andy/Rose flow has zero auto-trigger today) | Build fresh (same gap) | Build fresh — this is Phase 3.4 in `implementation.md`, already flagged as the largest unbuilt item |
| Ticket structure (custom fields, formal workflow, reporting) | Native | Native (in ISM) | Weak on its own — not attempting to replicate |
| Audit trail | ISM's own DB | ISM's own DB | Buzz's HMAC-keyed chain (Phase 0.1, already shipped) |
| Systems to operate | 1 | 2 | 1 (ISM touched only as a data source/sink, not a dependency for execution) |
| Redundancy | Duplicates Buzz's mature chat infra | None | None, once ISM's role is narrowed to "record," not "executor" |

**Decision: Option C.** Buzz hosts discussion, decision-gating, and AI-team
execution end-to-end. ISM is not the executor — it is where supporting
docs/context get pulled from, and where progress/status/closure get written
back, purely for human-facing tracking and reporting.

Key finding that removed the case for B: ISM's own "Sonnet orchestrates, Andy
codes, Rose tests" is **100% manually triggered today** — a human opens a
Claude Code session and decides to dispatch it. No webhook, scheduler, or
ticket-state listener exists on ISM's side either. The automation engine has
to be built regardless of which system hosts it, so that cost doesn't favor
ISM.

## Feasibility check performed (not simulated — verified live)

- ISM is deployed and healthy at `192.168.0.200:8080` (plain HTTP — port
  80/443 on that host belongs to a separate `traefik` proxy for unrelated
  apps, not ISM's own `nginx`, which is bound to 8080/8443).
- Network path confirmed reachable from Buzz's host (`192.168.0.186`) —
  ping and HTTP both respond; `/api/v1/auth/login` returns a real FastAPI
  validation response, not a routing 404.
- Live OpenAPI schema pulled from `http://192.168.0.200:8080/openapi.json`
  confirms simple, sufficient endpoints for the bridge:
  - **Auth**: `POST /api/v1/auth/login` — `{email, password}` → JWT
    (service-account style; no purpose-built API-key mechanism exists, but
    adding a service-account user is normal, small work).
  - **Pull context**: `GET /issues/{id}`, `/comments`, `/attachments`,
    `/history`.
  - **Push status back**: `POST /comments` (`{body}`), `POST /worklogs`
    (`{time_spent_minutes, description}`), `POST /transition`
    (`{transition_id, comment}` — validated against ISM's own workflow
    state machine, so Buzz cannot corrupt ISM's ticket lifecycle even from
    outside), `POST /issues` if Buzz needs to originate a ticket.

## Design decisions (2026-09-22 follow-up pass)

The four open questions below are now answered at the design level. None of
this is built — these are decisions to build *against*, not a build log.

### 1. Ticket origin and pull direction (was: "how a discussion becomes a decision" + "trigger shape")

Tickets originate in ISM normally (opened by ISM's own users, ISM's own UI)
— unrelated to Buzz at creation time. A role-gated Buzz member pulls one into
discussion on demand (e.g. `/discuss ISM-123`), which fetches that ticket's
context live (`GET /issues/{id}` + comments + attachments) into a thread.
Separately, Buzz teams can discuss freely with **no ISM ticket at all** —
not everything is ticket-bound.

**Trigger shape is therefore human-initiated pull, not system-triggered
push.** No polling loop, no webhook-out needed from ISM for v1. A "hey, a
new/updated ISM ticket exists" proactive notification is a real, wanted
feature but explicitly **deferred** — noted below, not scoped now.

### 2. Decision-gating mechanism

1. A lead/maintainer (role-gated) `@mentions` the Supervisor in the thread.
2. Supervisor reads the thread and posts a **drafted summary** — scope,
   acceptance criteria, links — as a proposal, not an action.
3. The same role reviews: confirms as-is, asks for a redraft, or edits
   directly.
4. Only on confirmation does Supervisor transition into execution mode.

Four auditable states: mentioned → drafted → confirmed → executing. If the
thread is linked to an ISM ticket (via step 1's `/discuss`), status writes
back to it as execution proceeds. If it's a free discussion that gets
confirmed anyway, confirmation is also the moment Buzz calls `POST /issues`
to create one retroactively, if the team wants it tracked.

### 3. ISM field mirroring

**Don't mirror.** Buzz holds only the ISM ticket ID as a reference and
fetches live when needed. When Buzz needs to present ticket info for
discussion or as a knowledge-base entry, it summarizes into plain Markdown —
never replicates ISM's typed field/schema model. Revisit only if a concrete
case emerges needing to filter/act on a specific ISM field without a live
fetch (none identified yet).

### 4. Execution engine scope and mechanics (Phase 3.4)

**Authority:** default is actual implementation (write code, commit, open a
PR) — not analysis-only. **Deploy/merge is always a separate, explicitly
requested action** by whoever's in charge, never automatic on completion.
Mirrors the `buzziro-dev` discipline already proven in this fork (implement
+ PR, ask before merge, ask before deploy).

**Fan-out:** exactly two canonical children per execution — a developer role
(implements) and a tester role (verifies) — mirroring the pattern already
proven twice (ISM's Sonnet→Andy→Rose; this fork's own
`buzziro-dev`→`buzziro-tester`). Not an open-ended "AI team" for v1.
Extending beyond two roles (e.g. an architecture/security reviewer) is a
future step, only once real usage shows the two-role split isn't enough —
same discipline, not a redesign.

**Consolidation:** exactly one reply per execution, posted only once both
children finish — what was done, test results, PR link. Matches Phase 3.4's
own pre-existing design constraint; hold this strictly.

**Tooling access:** dev child gets `buzz-dev-mcp` shell/file access under
Phase 0.2's already-decided trusted-team-only exposure model (no new
exposure decision needed). Test child gets the same tools, scoped
read/verify-only, mirroring `buzziro-tester`'s existing safety rules. **ISM
credentials live only with the Supervisor**, never the children — one
identity holds the ISM-facing write surface.

**Failure handling:** fan-out capped at one level (children cannot spawn
their own children — eliminates runaway spawn depth by construction, not by
runtime policing). One overall timeout/budget per execution; on breach,
Supervisor posts a status update rather than going silent. A child quiet
past its own bound is treated as failed and reported honestly in the digest.

## Deferred — noted, not scoped

- **ISM → Buzz notification/webhook.** Proactively surfacing new/updated
  ISM tickets in Buzz, instead of relying on a human to `/discuss` them
  manually. Wanted, real, but requires building the webhook-out ISM doesn't
  have today (n8n is deployed on ISM's host with zero integration code) —
  or a polling loop. Revisit once the human-pull flow is actually in use and
  the gap is felt, not before.
