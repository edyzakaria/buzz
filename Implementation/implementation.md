# Implementation plan: our buzz fork

Sequencing for adopting ideas from [`Improvements/wishlist.md`](../Improvements/wishlist.md)
into this fork. Order follows the deploy-first decision: harden what
gets more expensive to fix later, go live, then let real operational
signal — not speculation — drive everything after that. Item numbers
below match `wishlist.md`'s numbering.

---

## Phase 0 — pre-launch hardening

Do these before wide deployment. Both get more expensive the longer
real usage runs on the weaker version.

### 0.1 — HMAC-keyed audit chain (wishlist item 5a)

- **Where**: `crates/buzz-audit/src/hash.rs`, `entry.rs`.
- **Current state (verified)**: `compute_hash` is plain `Sha256` over
  `community_id → seq → created_at → action → actor_pubkey → object_id
  → detail(canonical JSON)`. Well-designed chain, just unkeyed — anyone
  with read access to the log can forge a valid continuation.
- **Work**:
  1. Decide where the HMAC key lives (relay-held secret, env/config —
     not derivable by any tenant, not stored alongside the chain it
     protects).
  2. Swap `Sha256::new()` for a keyed HMAC-SHA256 construction over the
     same field set (field order unchanged, so this only affects the
     digest, not the schema).
  3. Decide the migration story for any chain history that already
     exists locally in dev/test — a genesis re-anchor or a documented
     "chain validity starts at key rotation" cutover.
- **Exit criteria**: `AuditService::log` uses the keyed construction;
  a test proves an entry hashed without the key does not verify.

### 0.2 — Decide the sandbox exposure model (wishlist item 5b)

- **Where**: `crates/buzz-dev-mcp` (shell/file/search tools).
- **Not a build task yet** — a decision task. Before going live, answer:
  who can reach `buzz-dev-mcp`'s shell/file tools, and how trusted are
  they? If it's a small trusted team only, the full OS-level sandbox
  (Linux namespaces / macOS Seatbelt) can wait. If exposure is broader,
  it belongs in Phase 0 as real, separate security engineering — not
  something to bolt on after the fact.
- **Exit criteria**: a written answer to "what's our exposure model,"
  and if the answer requires sandboxing, a follow-up task is opened
  (not silently deferred).

---

## Phase 1 — go live

Deploy our fork's own build in place of the removed `buzz-prod` stack.
No new features from the wishlist ship in this phase — it's the
baseline against which every later phase measures real impact.

- Rebuild and tag our own image (replacing `ghcr.io/block/buzz:main`).
- Redeploy the compose stack at `/home/blade/docker/buzz/deploy/compose/`
  (relay + Postgres + Redis + MinIO), fresh volumes.
- Confirm health checks pass and the audit chain is live under the
  Phase 0.1 keyed construction from day one — never falls back to
  unkeyed.

---

## Phase 2 — cheap wins, ship regardless of signal

These cost close to nothing and don't need operational data to justify.

### 2.1 — Review-only reviewer, as a persona convention (wishlist item 7c, soft form)

- **Where**: whichever persona pack is used for code-review personas.
- **Work**: add the explicit system-prompt rule — "you may comment,
  you may never call merge/approve" — to the persona's instructions.
  Zero code. Ships immediately.
- **Note**: this is *not* enforced by the harness (verified: no
  tool-level allow/deny mechanism exists in the persona spec or
  `buzz-acp`'s `permission_mode` today). The enforced version is
  tracked separately in Phase 3 under 7c-hard, bundled with 5b.

---

## Phase 3 — signal-driven, build only once operational usage justifies it

Do not start these speculatively. Each has a stated trigger — the
observation that should prompt picking it up.

### 3.1 — Trigger matching for persona dispatch (wishlist item 4)

- **Where**: `crates/buzz-persona/src/persona.rs` (schema, already
  built), new consumer needed in `crates/buzz-acp` dispatch path.
- **Verified reality**: `keywords`/`all_messages` are fully parsed,
  merged, and displayed by `buzz-cli pack ...` — but **nothing in
  `buzz-acp` reads them today**. Runtime dispatch is currently only
  `RespondTo` (all/mentions/allowlist) + literal `event_mentions_agent()`.
  This is not an upgrade, it's building the first live consumer.
- **Trigger to build**: real channel usage shows personas need
  content-based triggering beyond @mentions (e.g. a persona that should
  jump in on certain keywords without being tagged).
- **When built**: implement as deterministic word-overlap scoring
  (kirocrew's `trigger_score()` shape) from day one, since there's no
  existing algorithm to migrate off.

### 3.2 — Memory layering in engram (wishlist item 1)

- **Where**: `crates/buzz-core/src/engram.rs` (`Body` enum), `crates/buzz-acp`
  prompt assembly (`engram_fetch.rs`, `prompt_framing.rs`).
- **Trigger to build**: real instances of "the agent forgot something a
  human explicitly corrected it on" or context windows visibly bloating
  with old engram history.
- **Work when triggered**:
  1. Lessons tier — new `Body` variant (or slug namespace) for explicit
     corrections, unconditionally injected, confidence 1.0. Needs
     backward-compatible serde handling since bodies are NIP-44
     encrypted/signed and existing decoders must not break.
  2. History decay renderer — pure function over existing engram events
     (day-tiered: full → header+count → marker → pruned), no schema
     change required, plug into `buzz-acp` prompt assembly.
  3. Confidence tagging — field on `Body::Memory` distinguishing
     inferred vs. explicitly-stated.

### 3.3 — Retry/replan ladder (wishlist item 2)

- **Where**: `crates/buzz-workflow` (`executor.rs`, `error.rs`).
- **Verified reality**: `execute_from_step` (resume-from-index) and
  `PartialProgress` (failure trace) already exist and are proven live
  for approval-resume. Missing: failure classification, an attempt
  counter per step, wiring the existing resume call into the failure
  path, and a human-escalation post once budget is exhausted.
- **Trigger to build**: real workflow runs failing in ways a human has
  to manually re-trigger — and enough of them to reveal what failure
  categories and thresholds actually matter. Do not import kirocrew's
  constants (`MAX_RETRIES=3`, etc.) — they were tuned for kirocrew's
  failure modes, not buzz's.

### 3.4 — Subagent spawning + consolidated fan-in (wishlist item 3)

- **Where**: new surface in `crates/buzz-acp` (session pool, currently
  one session per persona per channel) + a new tool in
  `crates/buzz-dev-mcp`.
- **Verified reality**: nothing exists today — no spawn tool, no
  sibling-session tracking, no batch/wave concept, no delivery path.
  This is the largest, most structurally important item on the list for
  "agents execute in parallel, humans assemble the result," but it's
  genuinely new surface area (new failure modes: hung children, runaway
  spawn depth; new UX question: who in a shared thread owns reviewing a
  spawned agent's output).
- **Trigger to build**: real, repeated demand for parallel agent work in
  a single channel — not speculative parity with kirocrew.
- **Design constraint carried forward from day one**: consolidate
  sibling completions into one digest reply to the thread — do not
  ship "each subagent posts its own message" first and retrofit batching
  later; that forces a redesign of the delivery path after the fact.
- **Sequencing note**: build after 3.3 — the classify → capped-budget →
  escalate pattern proven there is the template for this item's failure
  handling, even though the two don't share code (different crates,
  different failure domains).

### 3.5 — Enforced review-only rule (wishlist item 7c, hard form)

- Bundle with 5b's scoping, not with 2.1. Requires the same tool-level
  deny-list infrastructure — don't scope it standalone.

### 3.6 — Session lifecycle patterns (wishlist item 6)

- **Where**: `crates/buzz-acp` session keying.
- **Status**: unverified against current code (not yet audited this
  session). Low urgency — reassess only if real session-management pain
  shows up post-launch (e.g. cron/subagent/dashboard session collisions
  once 3.4 exists).

---

## Phase 4 — ISM-bridged Supervisor execution (built, merged 2026-09-23)

Full design discussion, option comparison, and verified feasibility in
[Improvements/ism-bridge-supervisor-plan.md](../Improvements/ism-bridge-supervisor-plan.md).
Direct response to the real signal from testing Phase 1: desktop-spawned
personas don't share identity across users (a known, still-open upstream
issue, `block/buzz#2910`/`#4174`). Chosen direction is a superset of 3.4
(Supervisor = the one centrally-deployed identity that fans out internally,
sidestepping the identity-collision problem by construction), bridged to the
issue-management project ("ISM") as a data source/sink only — ISM is never
the executor. Feasibility verified live (ISM reachable at
`192.168.0.200:8080`, real OpenAPI schema pulled and checked against).

**Built and merged into `main` (PR #2, `90cb3c6b`, 2026-09-23):** `buzz-ism`
client + `buzz discuss thread` pull command; Supervisor persona + 4-state
decision-gating; a new `buzz-acp run-task` one-shot execution primitive
(this crate had none before — genuinely new surface, as flagged under 3.4
below); real 2-child dev/tester fan-out via `run-task`. Verified via
`cargo fmt`/`clippy -D warnings`/`test` across `buzz-ism`, `buzz-cli`,
`buzz-acp` — not yet exercised end-to-end against a live relay + real LLM
credentials. **Known, disclosed gap:** `run-task --tool-scope` (full vs.
read-only) doesn't enforce anything yet — `buzz-dev-mcp` has no tool-gating
mechanism, so dev and tester currently share the same permission mode. This
is exactly Phase 3.5's scope below; not new debt, just now has a concrete
caller waiting on it.

---

## What's deliberately not on this plan

- Kirocrew's vector/FAISS episodic memory, V1/V2 private-memory-store
  migration, vendored embedding runtime — infrastructure solving
  multi-tenant/hot-swap problems this fork doesn't have.
- Computer-use / browser-automation — solo-desktop-agent features with
  no analog to acting in a shared chat channel.
- Bub's hook-pipeline pattern — noted as a nice-to-have architectural
  reference for `buzz-acp`'s turn handling, not a scoped task; revisit
  only if `buzz-acp`'s turn pipeline becomes hard to extend in practice.
