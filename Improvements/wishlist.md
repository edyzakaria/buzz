# Wishlist: ideas worth adopting from kirocrew

Both `block/buzz` and `kirodotdev/kirocrew` are open source under permissive
licenses. This is not "salvage from a competitor" — it's picking up good,
publicly-available ideas and combining them with what buzz already does well.
Nothing here is a wholesale port: kirocrew is a Python solo-dev gateway
wrapping an external CLI agent runtime; buzz is a Rust, chat-native,
multi-human-multi-agent platform built on Nostr events. Each item below is
reframed for that difference, and mismatches are called out explicitly rather
than glossed over.

Buzz's own agent stack for reference: `buzz-acp` (harness + session
management), `buzz-agent` (ACP agent), `buzz-persona` (OPS-compatible persona
packs), `buzz-workflow` (YAML + evalexpr channel-scoped automations),
`buzz-dev-mcp` (shell/file/search/todo tools), `buzz-core::engram` (encrypted
NIP-AE agent memory).

---

## 1. Memory: layer the flat KV store

Buzz's `engram` memory today is one flat, encrypted key-value tier (`core`
identity slot + `mem/<slug>` entries, tombstones, LWW head selection).
kirocrew splits memory into six layers: preferences.md / projects.md
(LLM-consolidated), decaying daily history, confidence-gated semantic KV,
vector episodic (FAISS + MMR), and `lessons.jsonl` (explicit user corrections,
confidence 1.0, unconditionally injected).

Adopt:
- **Lessons tier** — a slug namespace (or new `Body` variant) for explicit
  user corrections, always injected into the prompt regardless of query,
  separate from inferred/consolidated memory. Cheapest, highest-value item
  here — no vector store required.
- **History decay renderer** — day-tiered context: full detail (0–13d) →
  header + count (14–60d) → one-line marker (61–180d) → off-context but
  on-disk (181–364d) → pruned (365d+). A pure function over existing engram
  events, plugged into `buzz-acp`'s prompt assembly to bound context growth
  without deleting anything prematurely.
- **Confidence tagging** — distinguish "model inferred this" (needs a
  threshold before persisting) from "user explicitly said this" (always
  persisted/injected). Add as a field on the engram `Body::Memory` variant.

Skip: the vector/FAISS episodic tier, the V1/V2 per-agent private memory
store migration, and the vendored embedding runtime — that's infrastructure
solving multi-tenant/hot-swap problems buzz doesn't have yet. Take the shape
of the layering, not the storage weight.

---

## 2. Task execution: a real retry-vs-replan ladder

`buzz-workflow` runs steps sequentially with conditions but has no notion of
graduated failure handling. kirocrew's `TaskRunner.execute_task()` uses four
independent, non-interchangeable retry budgets in one loop:

- logic/test failure → `attempt` counter (`MAX_RETRIES=3`)
- process crash → `recoveries` counter (`MAX_RECOVERIES=2`), doesn't spend `attempt`
- stalled/recovering turn → a separate recovery-ladder gate
- dependency failure (429/5xx/auth) → classified and parked as
  "waiting_dependency", released without spending an attempt

Replan is a level above retry: it only fires when a task is FAILED after
exhausting its ladder, capped at `MAX_REPLAN=2` and a runaway-task ceiling
(`MAX_TOTAL_TASKS=50`), with a repeated-identical-error short-circuit
straight to FAILED.

Adopt:
- The four-budget ladder shape (attempt / recovery / stall / dependency,
  each independently capped) for `buzz-workflow` step execution.
- The dual-format checkpoint: a durable JSON run snapshot (atomic write +
  fsync + sequence number) *plus* a human-readable progress doc, re-parsed
  on resume. Cheap, and the human-readable half is a natural fit for posting
  progress into a buzz channel.

Reframe, don't copy: kirocrew's LLM-decomposes-into-steps model is a
background batch run with no human in the loop. Buzz's chat-native model
should make the plan a channel-visible, human-interruptible object, not a
silent background job — post the plan, let a human redirect mid-run.

---

## 3. Subagent fan-out/fan-in: consolidate, don't spam

kirocrew's `WaveDigestCoordinator` batches N sibling subagent spawns under a
shared `batch_id` and delivers **one consolidated digest** ("N✅·M❌") to the
parent instead of N separate completion messages. Delivery itself is a
shielded, claim-based handoff (`TerminalCoordinator`) so a cancelled reporter
can't silently drop a result. Concurrency is fairness-scheduled per root
session (weighted round-robin + reserved child slots), not a flat semaphore.

Adopt:
- **Consolidated fan-in messaging** — when `buzz-acp` fans a task out to
  multiple subagents in a channel/thread, post one summary message on
  completion, not one per subagent.
- **Per-session fairness scheduling** (lanes + reserved child slots) as the
  concurrency model for multi-agent channels, instead of a single global cap.
- **Claim-based result delivery** so an in-flight cancellation can't drop a
  result silently.

Mismatch to flag: kirocrew's whole model assumes one parent session waiting
on results. Buzz's channels have no single "parent" — multiple humans and
agents may be watching the same thread. The fan-in target should be "post to
thread," not "inject into a parent's context."

---

## 4. Skills: deterministic trigger matching, not vibes

kirocrew's skill matcher (`trigger_match.py`) is pure, stdlib-only, and
shared by two different callers (skill injection and crew routing) so they
can't drift apart: tokenize the message, score each trigger phrase as
`|phrase_words ∩ message_words| / |phrase_words|`, require max score ≥ 0.7,
support a `!`-prefixed negative veto phrase. No LLM call for matching.

Adopt directly — this is the cleanest, most portable idea in the whole
research pass:
- Deterministic word-overlap trigger matching for `buzz-workflow` conditions
  or `buzz-persona` pack/skill selection, as a single shared, testable
  function rather than per-caller ad hoc logic.
- The **rubber-duck pattern**: a Presenter reconstructs its reasoning, spawns
  a cross-model-blinded Listener subagent (memory/lessons excluded) under a
  fixed adversarial charter, tags objections `[OVERCLAIM]/[GAP]/[INCONSISTENCY]`,
  iterates until no new objection surfaces, reconciles into a disposition
  ledger (HELD/DOWNGRADED/GAP-FLAGGED/CONTRADICTED) — proposes edits only,
  human sign-off required to apply. Strong template for a `buzz-persona`
  "critic" pack in code-review channels.

No real mismatch here — skill triggering on a channel message is a natural
fit for buzz's chat-native model.

---

## 5. Security: an unweakenable deny floor + a stronger audit chain

- `buzz-audit`'s hash chain should be checked against kirocrew's: their
  chain is **HMAC-SHA256** (keyed) over canonical sort-keys JSON, not plain
  SHA256, so read access to the log alone can't forge a valid continuation.
  If buzz's chain isn't keyed, that's a concrete hardening item.
- kirocrew's deny-list is structurally un-weakenable: base patterns are
  `@final`, a plugin can only *union in* extra deny patterns (no subtract
  method exists), and a boot-time `assert_security_floor()` check raises if
  anything final was overridden. Rust can enforce this even more strongly
  than Python (sealed traits, no `pub` mutation path) — worth applying to
  `buzz-dev-mcp`'s shell/file tool gate, and worth auditing whether that gate
  is currently reachable from anything the agent process can rewrite (their
  explicit design principle: never trust agent-writable config for deny
  rules).

Real gap, not a quick port: buzz currently has no OS-level sandbox
(namespaces on Linux, Seatbelt on macOS) around `buzz-dev-mcp` shell/file
execution the way kirocrew does. Worth scoping as real, separate work if
wanted — not something to bolt on casually.

---

## 6. Session lifecycle: cheap patterns worth taking as-is

- **String-prefix session keying** (`cron:{id}`, `subagent:{parent}:...`,
  `dashboard:{slot}`, bare channel id) resolved through one fold function —
  simple, and directly reusable for `buzz-acp` session routing across
  channel/cron/subagent callers.
- **Circuit breaker as counter+reset**, not a full open/half-open state
  machine: 5 consecutive failures → destructive session reset. Deliberately
  unsophisticated — buzz shouldn't build more than this either.
- **Auto-compaction by destroy-and-recreate** as the fallback path for
  non-native backends, worth noting if `buzz-acp` ever supports a backend
  that can't compact in place.

---

## 7. Real dev-work patterns worth taking

- **"Prove the regression test"**: before trusting a fix, revert it in a
  throwaway worktree and rerun the test to confirm it actually catches the
  regression. Concrete, adoptable as a `buzz-workflow` PR-automation step.
- **Generic bounded worker pool** (`PoolWorker` protocol, semaphore-bounded,
  self-healing dead-worker replacement) — good shape for any bounded-
  concurrency agent-fleet feature behind `buzz-acp`.
- **Review-only reviewer as a hard rule**: an agent persona that
  auto-approves its own tool calls but is explicitly forbidden from calling
  the merge/approve action itself — read-and-comment only, human keeps
  final judgment. Good default for any buzz code-review persona.

Out of scope for now: computer-use and browser-automation modules are
solo-desktop-agent features (OS accessibility APIs, screenshot-driven
clicking) with no analog to "acting in a shared chat channel" — would need
significant reframing before they're even a sensible fit, if ever.
