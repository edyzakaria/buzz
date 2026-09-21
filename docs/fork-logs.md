# buzz (edyzakaria fork) — Logs

Decisions, changes, and notes worth keeping. One entry per item, newest first.

---

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
