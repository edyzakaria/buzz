# CI policy: manual Desktop/E2E trigger (this fork)

Fork-specific operational policy. Not upstream `block/buzz` convention — named
`fork-*` to avoid ambiguity with this repo's existing `docs/` convention (see
`fork-overview.md`).

## Background

As of PR #3 (`ci-defer-desktop-e2e-to-manual`), `Desktop Domain` and
`Desktop macOS Domain` (Tauri build + Playwright, ~19min) no longer
auto-trigger on plain backend `crates/**` changes — only on actual
`desktop/**`/`desktop/src-tauri/**` changes, a push to `main`/`release`, or an
explicit manual request. This was a deliberate trade-off: a workspace-wide
dependency bump in a shared crate *could* in principle still break the
desktop Tauri build without CI catching it automatically pre-merge.

This doc replaces "GitHub's path-filter decides" with "the assisting agent
(Claude) decides and says so explicitly" for that judgment call.

## How to trigger it manually

```
gh workflow run ci.yml --repo edyzakaria/buzz --ref <branch-name> -f run_desktop_e2e=true
```

Or via the GitHub UI: Actions tab → "CI" workflow → "Run workflow" → pick the
branch → tick `run_desktop_e2e` → Run workflow.

## The policy going forward

When an assisting agent (Claude, via a standing subagent or directly) makes
or reviews a change touching `crates/**`, `Cargo.toml`, or `Cargo.lock` on
this fork, it must explicitly state a recommendation — not silently skip the
judgment:

1. **State whether the change plausibly affects the desktop build.** Concrete
   triggers for "yes, run it": a shared/workspace dependency was added,
   removed, or version-bumped; a crate the desktop Tauri backend depends on
   (directly or transitively) changed its public API or behavior; the change
   touches anything under `desktop/src-tauri/` even indirectly (e.g. via a
   shared internal crate). Concrete triggers for "no, skip it": a change is
   scoped to a single non-desktop-facing crate (e.g. `buzz-cli`, `buzz-acp`,
   `buzz-ism`, `buzz-relay`) with no dependency-graph or shared-crate
   changes, and desktop doesn't import that crate.
2. **Say so to the user, with the reasoning**, before merging — not just a
   yes/no, the concrete "why" (which crate/dependency, why it does or
   doesn't reach desktop).
3. **If the recommendation is to run it**, trigger it — dispatch
   `buzziro-tester` (this fork's standing test subagent) to run
   `gh workflow run ci.yml --ref <branch> -f run_desktop_e2e=true` and then
   poll/report the result via `gh run list`/`gh run view` once it completes.
   `buzziro-tester` can also run a **local** smoke check first for faster
   signal while the remote CI run is in flight — `just desktop-dev` or
   `desktop && pnpm test:e2e:smoke` — but only covers the Linux/web-capable
   path on this dev box, not the macOS/Windows canary builds; say so
   explicitly rather than implying local parity with the full CI matrix.
4. **If the recommendation is to skip it**, still say so, so the decision is
   visible and reversible — the user can always override and ask for it
   anyway.

This is a judgment call, not a mechanical rule — when genuinely unsure
whether a dependency change reaches desktop, the default is to recommend
running it (cheap insurance against a real trade-off), not to guess silently.
