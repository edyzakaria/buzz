# buzz (edyzakaria fork)

## Objective
A personal fork of [block/buzz](https://github.com/block/buzz) used to explore
combining buzz's protocol-first, chat-native human+agent collaboration model
(Nostr identity, signed multi-party channels, encrypted engram memory) with
execution and memory depth ideas drawn from adjacent projects (kirocrew, bub),
adopted where they genuinely fit buzz's multi-human-multi-agent architecture
rather than ported wholesale.

## Tech Stack
Rust workspace: `buzz-relay` (WebSocket relay + git/huddle hosting),
`buzz-core`, `buzz-db` (Postgres), `buzz-auth`, `buzz-pubsub` (Redis),
`buzz-search`, `buzz-audit` (hash-chained audit log), `buzz-media`
(Blossom/S3), `buzz-acp`/`buzz-agent` (ACP agent harness), `buzz-persona`
(OPS-compatible persona packs), `buzz-workflow` (YAML + evalexpr
automations), `buzz-dev-mcp` (shell/file/search tools), `buzz-cli`,
`buzz-sdk`, `buzz-admin`, `sprig`. Desktop: Tauri 2 + React 19 + Vite +
Tailwind. Web: browser repo client. Mobile: Flutter + Riverpod. Storage:
Postgres, Redis, MinIO. Deployed via Docker Compose
(`docker/buzz/deploy/compose/`).

## Plan
See [`../Implementation/implementation.md`](../Implementation/implementation.md)
for the phased adoption plan — Phase 0 (pre-launch hardening: HMAC-keyed
audit chain, sandbox-exposure decision) → Phase 1 (go live) → Phase 2 (cheap
wins that ship regardless of signal) → Phase 3 (signal-driven items: trigger
matching, memory layering, retry/replan ladder, subagent spawning — each
gated on real operational demand, not speculation).

See [`../Improvements/`](../Improvements/) for the underlying research:
`wishlist.md` (kirocrew → buzz adoption ideas, each verified against actual
buzz code), `buzz-to-kirocrew.md` (the reverse direction), and
`related-initiatives.md` (a prior-art survey of similar efforts elsewhere).

## Self-Learning
This project draws on a cross-project lessons file maintained at
`~/.config/agent-skills/lessons/lesson_learned.md`. Running `/wrapup` in this
project keeps a `LESSON_LEARNED:START` / `LESSON_LEARNED:END` block in this
project's CLAUDE.md in sync with it. Use `/learn-lesson` any time during a
session to record something worth remembering project-wide.
