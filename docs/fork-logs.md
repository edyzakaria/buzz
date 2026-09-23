# buzz (edyzakaria fork) — Logs

Decisions, changes, and notes worth keeping. One entry per item, newest first.

---

- 2026-09-23 — **PR #3 merged (`d9cda98a`).** Includes a second commit
  (`e9d4432d`) beyond the original manual-trigger change: found and fixed a
  real, pre-existing bug in the `desktop:` path-filter itself — a standalone
  negation pattern (`'!desktop/src-tauri/**'`) matched independently under
  `predicate-quantifier: some`, so `outputs.desktop` was `true` for nearly
  any changed file repo-wide (confirmed via the actual paths-filter step log
  on PR #3's own first run: `.github/workflows/ci.yml` alone matched
  `desktop`). This had been silently defeating the intended narrowing the
  whole time, predating this PR. Fixed by dropping the exclusion (verified
  no consumer depended on it). Not yet empirically re-verified with a fresh
  PR touching only non-desktop files — the mechanism and fix are confirmed
  by reading the log and the filter logic, but a live confirming run is
  still outstanding.
- 2026-09-23 — **CI policy: manual Desktop/E2E trigger, agent-judged.**
  Following PR #3 (auto-trigger removed for backend-only changes), wrote
  `docs/fork-ci-policy.md`: the assisting agent now makes and states the
  "does this change plausibly affect desktop" judgment explicitly on every
  `crates/**`/`Cargo.toml`/`Cargo.lock` change, rather than leaving it to
  GitHub's path-filter alone. If the call is "run it," dispatch
  `buzziro-tester` to fire `gh workflow run ci.yml -f run_desktop_e2e=true`
  and report the real result — never silently skip the judgment, never
  claim a result before the run finishes. Wired the same into
  `buzziro-tester`'s own agent definition so it doesn't require re-explaining
  per dispatch.
- 2026-09-23 — **Phase 4 built and merged (PR #2, `90cb3c6b`).** ISM-bridged
  Supervisor execution moved from planning-only to real, working code:
  `buzz-ism` client + `buzz discuss thread` (pulls ISM tickets into a channel
  as Markdown); Supervisor persona + 4-state decision-gating
  (`mentioned → drafted → confirmed → executing`) tracked via Nostr event
  tags with best-effort ISM status write-back; a new `buzz-acp run-task`
  one-shot agent-execution primitive (didn't exist before — required for
  real, non-simulated execution); and real 2-child dev/tester fan-out via
  `run-task`, replacing three earlier attempts that each disguised a
  placeholder as real execution (caught by reading the actual code, not
  trusting subagent self-reports — see PR #2's commit history). All
  independently verified: `cargo fmt`/`clippy -D warnings`/`test` clean
  across `buzz-ism`, `buzz-cli`, `buzz-acp`. **Known, disclosed gap:**
  `run-task --tool-scope` doesn't enforce anything yet — `buzz-dev-mcp` has
  no tool-gating mechanism, so dev and tester currently share the same
  (bypass) permission mode. Tracked as Phase 3.5, not hidden debt. **Not yet
  done:** end-to-end exercise against a live relay + real LLM credentials.
  Also opened PR #3 (separate, unmerged as of this entry): stops
  backend-only Rust PRs from auto-triggering the ~19min Desktop/E2E CI
  suite, adds `workflow_dispatch` to run it manually when wanted.
- 2026-09-22 — Resolved Phase 4's four open design questions (full detail
  in `Improvements/ism-bridge-supervisor-plan.md`): tickets originate in
  ISM, pulled into Buzz discussion on demand via a human-run command (no
  ISM webhook/poll needed for v1 — proactive notification deferred to the
  plan's own "deferred" list); decision-gating is
  mention-Supervisor → drafted summary → human confirms → execution;
  Buzz holds only the ISM ticket ID, never mirrors ISM's field model,
  summarizes to plain Markdown when needed; execution authority defaults
  to actual implementation (code + PR), deploy/merge always a separate
  explicit ask, mirroring `buzziro-dev`'s existing discipline; fan-out
  capped at exactly two children (dev + test, mirroring ISM's own
  Sonnet→Andy→Rose and this fork's `buzziro-dev`/`buzziro-tester`), one
  consolidated reply, ISM credentials held only by the Supervisor, one
  level of spawn depth max. Still planning only — not scoped for a build.

- 2026-09-22 — Added Phase 4 (planning only, not scoped for build):
  ISM-bridged Supervisor execution. Responds to the shared-agent-identity
  gap found while testing Phase 1 (`block/buzz#2910`/`#4174`, confirmed via
  `Improvements/shared-agent-reference-survey.md` that no one — Slack,
  Discord, Matrix, Nostr, LangChain, kirocrew, or Jira/Rovo's own approach
  — has a reusable answer for "one shared agent identity, many independent
  clients"). Chosen direction (full comparison in
  `Improvements/ism-bridge-supervisor-plan.md`): Buzz hosts discussion +
  decision-gating + AI-team execution; the issue-management project
  ("ISM," `/home/blade/projects/issue-management`) is a data source/sink
  only, never the executor — verified ISM's own "Sonnet/Andy/Rose" flow is
  100% manually triggered today, so the execution-engine cost is identical
  regardless of which system hosts it. Feasibility verified live: ISM
  reachable at `192.168.0.200:8080` (plain HTTP; port 80/443 on that host
  belongs to an unrelated `traefik` proxy), real OpenAPI schema pulled and
  checked (JWT auth, issues/comments/worklogs/transition endpoints all
  sufficient). Four open design questions block scoping this as a real
  phase — see the plan doc.

- 2026-09-22 — Phase 2.1 (wishlist 7c, soft form): added an explicit
  "comment only — never merge or approve" rule to the two example
  code-review personas (`examples/meadow-core/agents/lev.persona.md`,
  `bana.persona.md`). Zero code, as scoped — the harness still has no
  tool-level allow/deny mechanism to enforce this (verified prior session);
  the enforced version stays tracked under Phase 3's 7c-hard, bundled with
  5b. User flagged Phase 2.1 may need revisiting later (no specifics given
  yet) — don't treat it as closed.

- 2026-09-21 — Phase 0.2 decided: buzz-dev-mcp's shell/file/search tools are
  reachable only by a small trusted team, so full OS-level sandboxing (Linux
  namespaces/Seatbelt) is deferred — documented, not silently skipped, per
  the exit criteria in `Implementation/implementation.md`.
- 2026-09-21 — Phase 1 go-live: built and deployed our own relay image
  (`buzziro:cc22167f`, tag = merge commit of PR #1) in place of the removed
  `buzz-prod` stack. Relay on `ws://192.168.0.186:3003` (moved off 3001/3000
  — both taken by other host containers), Postgres/Redis/MinIO on fresh
  volumes, real `BUZZ_AUDIT_HMAC_SECRET` generated for `.env` (never the
  Phase 0.1 dev-only fallback key). Audit service confirmed live in relay
  logs (`audit_enabled: true`, `"Audit service ready"`). Hit a real external
  blocker along the way: Docker Hub now blocks anonymous pulls of
  `minio/minio`/`minio/mc` — the repo's own `deploy/compose/compose.yml` had
  already fixed this (digest-pinned `quay.io/minio/*` mirrors, dated before
  this session) but the separately-maintained deployed copy at
  `/home/blade/docker/buzz/deploy/compose/` was stale and needed
  re-syncing from the repo.

- 2026-09-18 — **Phase 0.1 complete: HMAC-keyed audit chain.** Swapped
  `compute_hash` from unkeyed SHA-256 to HMAC-SHA256 with relay-held key
  (loaded from `BUZZ_AUDIT_HMAC_SECRET` env, falls back to fixed dev-only key if unset).
  The key lives in relay config, never in the chain itself; read access to the
  log alone no longer permits forging a valid continuation. Field order and hash
  construction are unchanged — only the digest becomes keyed. **Migration:**
  this is pre-launch work with no production chain; existing local dev/test
  chain history remains valid under the old construction since no real audits
  yet exist. The relay enforces the keyed construction from day one (Phase 1
  go-live) — no fallback, no rotation logic needed yet. All unit tests pass,
  including a new regression test proving hash mismatches when using the wrong
  key (`hmac_key_is_required_for_hash_verification`). Relay can be started
  with `BUZZ_AUDIT_HMAC_SECRET=<hex-key>` (32+ bytes, hex-encoded) or will use
  a fixed dev-only key when unset — do not use in production without setting BUZZ_AUDIT_HMAC_SECRET.
  Updated `crates/buzz-audit/src/hash.rs`, `crates/buzz-relay/src/config.rs`, and threaded the key through 4 relay
  codepaths (main.rs, state.rs, router.rs, workflow_sink.rs).
- 2026-09-18 — Session end: attempted to dispatch `buzziro-dev` for Phase 0.1
  (HMAC audit chain) but the running session's agent registry doesn't yet
  recognize `buzziro-dev`/`buzziro-tester`. Diagnosed against
  `issue-management`'s own `20260811-Andy_and_Rose_issue.md` precedent: our
  frontmatter is structurally correct (unlike their original missing-
  `description` bug), so this is purely a session-freshness issue — their
  own history confirms custom agents only become invokable in a genuinely
  fresh session. Phase 0.1 has not started; pick up there next session.
- 2026-09-18 — Adopted the name "Buzziro" for this fork in docs/conversation
  only (no code, crate, binary, or package rename). Added standing subagents
  `.claude/agents/buzziro-dev.md` and `buzziro-tester.md`, mirroring the
  `issue-management` project's `andy.md`/`rose.md` safety pattern (hard rules
  against `docker compose down/up/restart`, `rm -rf`, and destructive git ops
  without explicit per-task authorization), to replace ad-hoc general-purpose
  subagent dispatch for implementation/verification work going forward.
- 2026-09-18 — Bootstrapped fork-specific project docs (`docs/fork-overview.md`,
  this file) via the `init-project` template, named to avoid ambiguity with
  buzz's existing upstream `docs/` convention.
- 2026-09-18 — Removed the outdated `buzz-prod` Docker stack (relay, Postgres,
  Redis, MinIO — `ghcr.io/block/buzz:main`) and all its volumes; our own build
  will replace it at Phase 1 go-live (`Implementation/implementation.md`).
- 2026-09-17 — Wrote `Implementation/implementation.md`: phased adoption plan
  sequencing the wishlist by a deploy-first decision — harden the audit chain
  before launch, defer everything speculative until real operational signal
  justifies it.
- 2026-09-17 — Re-verified several `Improvements/wishlist.md` claims directly
  against the running code and found real inaccuracies: `buzz-persona`'s
  `keywords`/`all_messages` trigger fields are fully parsed but never
  consulted by `buzz-acp` at runtime (dead config, not a live matcher to
  upgrade); `buzz-workflow` already has a proven resume-from-checkpoint
  primitive (`execute_from_step`, built for approval-resume) that a
  retry/replan ladder can reuse; `buzz-acp` has no subagent-spawning
  mechanism at all today. Lesson: verify wishlist items against current code
  before treating them as scoped, not just against the original kirocrew
  research.
- 2026-09-17 — Wrote `Improvements/related-initiatives.md`: surveyed GitHub
  for prior art combining buzz's protocol-first model with kirocrew-style
  execution depth. Found Quorum (`superirale/Quorum`) and its companion
  NIP-XX "Agent-First Messaging" draft; on closer inspection Quorum's README
  overclaims relative to its actual signals (0 stars/forks, ~32 commits) —
  corrected the write-up rather than presenting it as validated prior art.
- 2026-09-17 — Wrote `Improvements/buzz-to-kirocrew.md`: the reverse
  direction — what a solo-dev agent gateway like kirocrew could adopt from
  buzz's signed-event, protocol-first architecture (cryptographic per-agent
  identity, real multi-party group channels, encrypted portable memory).
- 2026-09-17 — Wrote `Improvements/wishlist.md`: kirocrew → buzz adoption
  ideas across memory layering, task retry/replan, subagent fan-out,
  deterministic skill triggering, security hardening, session lifecycle, and
  dev-work automation — framed as adopting open-source ideas, not "salvage,"
  since both projects are permissively licensed.
