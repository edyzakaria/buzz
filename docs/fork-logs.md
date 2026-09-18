# buzz (edyzakaria fork) — Logs

Decisions, changes, and notes worth keeping. One entry per item, newest first.

---

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
