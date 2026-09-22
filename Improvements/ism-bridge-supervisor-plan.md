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

## What's still open — deliberately not designed yet

1. **The Supervisor/AI-team execution engine itself.** Maps onto
   `implementation.md`'s Phase 3.4 ("subagent spawning + consolidated
   fan-in") — already flagged there as the largest, most structurally
   important unbuilt item, with real new failure modes (hung children,
   runaway spawn depth) and the hard design constraint carried from day
   one: consolidate sibling completions into **one** digest reply, never
   "each subagent posts its own message."
2. **How a Buzz discussion formally becomes a "decision."** What role gate,
   what command/event marks a thread as ratified and hands it to the
   Supervisor. Not specified.
3. **How much of ISM's structured ticket fields need mirroring vs. just
   referencing by ID.** Not decided — depends on how much of ISM's
   reporting/custom-field machinery actually matters for this workflow
   day to day.
4. **Trigger direction and shape.** Does Buzz poll ISM, does ISM eventually
   grow real webhook-out (currently unbuilt — n8n is deployed on ISM's host
   but has zero integration code today), or is the "decision" always
   Buzz-originated so no ISM→Buzz trigger is even needed?

Nothing here should be treated as scoped-to-build until those four are
answered.
