# Related initiatives: others bringing buzz-style and kirocrew-style ideas together

A GitHub/web survey (2026-09-17) for prior art combining buzz's chat-native,
signed-event, multi-human-multi-agent model with kirocrew-style agent
capability/memory/orchestration depth. No project directly forks or merges
buzz and kirocrew specifically. The closest hit found (Quorum, below) turned
out on inspection to be a thin, unverified, zero-traction repo — its README
overclaims relative to its actual signals (see caveat under Quorum) — so
treat this whole section as "the same idea occurred to someone else," not
"someone has proven this out."

---

## Quorum ([superirale/Quorum](https://github.com/superirale/Quorum), MIT)

> **Caveat, checked after initial write-up:** on closer look this repo reads
> as thin and self-reported rather than proven. Signals: 0 stars/forks, ~32
> commits total, yet its own README claims all 11 milestones complete, "823
> tests" across six suites, a working Go relay + TypeScript SDK + web
> console — a large maturity claim resting on essentially zero external
> validation or community traction. No screenshots or UI were evident on
> inspection despite the README describing one. That gap between claimed
> completeness and observable signal is itself the tell — likely a
> solo/AI-scaffolded spec-and-boilerplate project, not a working system on
> par with either buzz or kirocrew. Read the rest of this section as "one
> person had a similar idea and wrote a spec for it," not "this validates
> the direction."

"An agent-first messaging protocol built on Nostr: agents hold their own
keys, carry scoped capabilities, ask humans for signed consent, and account
for what they cost." Reference relay + client + agent SDK (per its own
README — unverified beyond that).

Its own framing is close to the buzz↔kirocrew comparison: *"Slack and its
clones are human-first systems with bots bolted on — a bot is a webhook with
an avatar: no durable identity, no scoped permissions, no protocol-level way
to ask a human for consent, and no accounting for what it costs."* Quorum
inverts that: agents are members with their own keys, scoped/revocable
capabilities, a consent channel to humans, and a context API built for
them — humans and agents read/write the same signed event log. This is
architecturally the same bet buzz makes (protocol-level identity over
gateway-local trust), pursued as a protocol-first project rather than a
full chat platform.

**Notable recent work — M10 (MLS channels):** adds Messaging Layer Security
on top of the Nostr envelope, so forward secrecy protects a channel's memory
while addressing, rate-limiting, and Merkle-checkpoint audit still work at
the relay layer — the relay stays the transport, each member is the record.
Two gaps are stated openly rather than glossed over: no MLS Remove yet, and
no `createAgent` path builds an `MlsCrypto` yet. Relevant to both sides of
our comparison: it's a candidate answer to "how do you get e2e-encrypted
group/shared memory without losing relay-side accountability" — bearing on
buzz's engram encryption model (§1/§3 in the two wishlist docs) and on
kirocrew's memory-at-rest gap (wishlist.md §1, buzz-to-kirocrew.md §3).

### Companion spec: NIP-XX "Agent-First Messaging"

[PR #2466 on nostr-protocol/nips](https://github.com/nostr-protocol/nips/pull/2466),
by superirale (co-authored with Claude Opus 5). Proposes new Nostr event
kinds covering almost exactly the layer buzz and kirocrew each solve
separately and incompatibly:

- **8101–8112** — actions, approvals, summaries, errors, artifacts, thread
  management (a unified "action" kind with a lifecycle, not ad hoc
  tool-call/result pairs)
- **28101–28103** — interrupts, thread leases, presence tracking
- **38101–38107** — thread state, capability grants, agent manifests,
  **agent memory**, agent cursors, delegation, channel encryption policy

Four core mechanisms: addressable thread-tasks carrying status and budgets;
a unified action kind with lifecycle; cryptographic consent via input
digests (an approval signs exactly the arguments it approves); and layered
ordering via counters plus Merkle checkpoints.

**Status:** closed/draft after community pushback — one reviewer (`@staab`)
called it over-engineered, arguing agents should use existing human-designed
protocol features rather than special-cased kinds. So this is contested,
not settled or adopted — but it's the most direct existing attempt to
standardize "agent memory + capability + consent" at the protocol level,
which is exactly the gap between buzz's chat-native event model and
kirocrew's gateway-local memory/security model. Worth watching, and worth
buzz maintainers weighing in on if the identity/capability/memory kind
ranges overlap with anything buzz would want to standardize itself
(`buzz-core/src/kind.rs` already defines its own kind ranges independently).

---

## Other adjacent, narrower projects

- **[wassname/agent-nostr-relay](https://github.com/wassname/agent-nostr-relay)**
  ("The Rusty Claw") — a public Nostr relay purpose-built for agent
  coordination: agents post tasks, status, reproducibility notes, benchmark
  debugging notes, and capability ads over plain Nostr, with search. Narrower
  than buzz (no chat/desktop/mobile surface, no persona-pack/workflow layer)
  but the same "agents as first-class relay participants" bet.
- **[AustinKelsay/nostr-agent-interface](https://github.com/AustinKelsay/nostr-agent-interface)**
  — a generic interface/API+CLI letting AI agents use Nostr as a substrate.
  A library, not a platform — useful as a reference for a minimal
  agent-on-Nostr integration surface, not a competing platform.

## Not a match, despite name collisions

Web/GitHub search surfaces several unrelated projects sharing keywords —
JPMorgan/Consensys's `quorum` (an Ethereum blockchain platform, unrelated),
`supermemoryai/supermemory` (a general AI memory API, no Nostr/chat angle),
and a Solidity security-tooling `quorum` — none relevant here.

---

**Bottom line:** nobody has published a proven, working buzz+kirocrew merge.
The closest thing found is a *proposal*, not a proof: the NIP-XX draft (and
the Quorum repo backing it, itself unverified — see caveat above) argues
"agent memory and capability should be signed Nostr event kinds," which is
the same bet buzz already made and actually shipped, with a full chat
platform, desktop/mobile/web clients, persona packs, and a workflow engine
on top. Buzz is materially further along than anything this search
surfaced — this section documents that someone else had a similar thought,
not that they got there first or better.
