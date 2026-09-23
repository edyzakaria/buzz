# CI policy: manual full-suite trigger (this fork)

Fork-specific operational policy. Not upstream `block/buzz` convention — named
`fork-*` to avoid ambiguity with this repo's existing `docs/` convention (see
`fork-overview.md`).

## Background

PR #3 (`ci-defer-desktop-e2e-to-manual`) started this by making
`Desktop Domain`/`Desktop macOS Domain` manual-only for PRs. Follow-up work
(2026-09-23) broadened it: **every heavy CI domain is now manual-only on
PRs** — `Desktop Domain`, `Desktop macOS Domain`, `Relay Artifact Producer`,
`PostgreSQL Domain`, `Relay and PostgreSQL`, `Clients` (web/mobile),
`Mobile Swift Domain`. Only `Rust Lint`, `Unit Tests`, `Windows Rust`,
`Rust Cross-Compile`, and `Security` still run automatically on every PR —
these are the fast (\<5min), code-correctness checks. A push to `main` or
`release` still always runs everything, full stop — this policy only affects
open PRs.

**Deliberate, accepted trade-off:** none of the E2E/integration/desktop/
client domains auto-catch a regression pre-merge anymore, even a real one
(e.g. an actual `desktop/**` change, or a relay-behavior regression a Relay
E2E test would have caught). This doc replaces "GitHub's path-filter
decides" with "the assisting agent (Claude) decides and says so explicitly,
every time" as the safety net in place of automatic triggering.

## How to trigger it manually

```
gh workflow run ci.yml --repo edyzakaria/buzz --ref <branch-name> -f run_full_suite=true
```

Or via the GitHub UI: Actions tab → "CI" workflow → "Run workflow" → pick the
branch → tick `run_full_suite` → Run workflow. This one switch runs
everything — Desktop, Desktop macOS, Relay, Postgres, Clients — together;
there's no per-domain manual toggle.

## The policy going forward

When an assisting agent (Claude, via a standing subagent or directly) makes
or reviews a change touching `crates/**`, `Cargo.toml`, `Cargo.lock`,
`desktop/**`, `web/**`, or `mobile/**` on this fork, it must explicitly state
a recommendation — not silently skip the judgment:

1. **State whether the change plausibly needs the full suite.** Concrete
   triggers for "yes, run it": a shared/workspace dependency was added,
   removed, or version-bumped; a crate the desktop Tauri backend depends on
   (directly or transitively — `buzz-core`, `buzz-sdk`, `buzz-persona`,
   `buzz-agent`, `buzz-voice`, `buzz-ws-client`, `buzz-media`, or
   `desktop/src-tauri/**` itself) changed; relay wire-protocol or event-kind
   behavior changed in a way a Relay E2E test would exercise; the change
   touches `web/**` or `mobile/**` directly. Concrete triggers for "no, skip
   it": a change is scoped to a crate desktop doesn't import (e.g. `buzz-cli`,
   `buzz-acp`, `buzz-ism`, `buzz-dev-mcp`) with no shared-dependency-graph
   changes and no relay wire-behavior change.
2. **Say so to the user, with the reasoning**, before merging — not just a
   yes/no, the concrete "why" (which crate/dependency, why it does or
   doesn't need the full suite).
3. **If the recommendation is to run it**, trigger it — dispatch
   `buzziro-tester` (this fork's standing test subagent) to run
   `gh workflow run ci.yml --ref <branch> -f run_full_suite=true` and then
   poll/report the result via `gh run list`/`gh run view` once it completes.
   `buzziro-tester` can also run a **local** smoke check first for faster
   signal while the remote CI run is in flight — `just ci`, `just
   desktop-dev`, or `cd desktop && pnpm test:e2e:smoke` — but only covers
   what this dev box can actually run (Linux/web-capable paths, not macOS/
   Windows canary builds); say so explicitly rather than implying local
   parity with the full CI matrix.
4. **If the recommendation is to skip it**, still say so, so the decision is
   visible and reversible — the user can always override and ask for it
   anyway.

This is a judgment call, not a mechanical rule — when genuinely unsure
whether a change needs the full suite, the default is to recommend running
it (cheap insurance against a real trade-off), not to guess silently.
