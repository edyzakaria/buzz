# Shared-agent-identity reference survey

Research question: how do other real systems give a bot/agent **one canonical
identity reachable identically by every client**, especially where clients are
independent/decentralized. Findings only — no recommendation.

## 1. Centralized chat platforms (Slack, Discord, Matrix)

**Slack.** A Slack app installed into a workspace gets exactly one bot token
per install: "you get one bot token per workspace install... a single bot
identity is shared across an entire workspace." The bot's identity is
retrievable via `auth.test`, which returns one `bot_user_id` for that
token — there is no per-client bot identity, only one process (or process
pool) that holds the one token and receives all events for that workspace.
Slack explicitly favors the bot token over a user token because "bot tokens
survive the installing user leaving" — identity persistence is a stated design
goal, not an accident. (Source: Slack bot/user token architecture, summarized
from Slack developer docs and Bolt SDK guidance — dev.to/sapotacorp
"Slack Bot Token vs User Token Scopes"; agenticfabriq.com "Slack for AI Agents:
User Tokens vs. Bot Tokens".)

**Discord.** "A single bot token works across every server the bot has been
invited to, and there is no per-server token." The bot maintains one WebSocket
Gateway connection (or a fixed, self-managed set of *shards* of the same
identity — sharding splits guilds across connections of the **same** bot
application, never spawns independent bot identities) and "receives events
from all servers simultaneously." Sharding is explicitly framed by Discord as
a scaling mechanism for one identity, not multiple identities: each shard
"requires no state-sharing between separate connections" but they are still
the same application/token. (Source: Discord Developer Docs, Gateway —
docs.discord.com/developers/events/gateway.)

**Matrix (maubot).** Maubot's default mode is dynamic/multi-tenant management,
but for single-bot production deployments it ships a dedicated **standalone
mode**: "a separate entrypoint that runs a single maubot plugin with a
predefined Matrix account," optionally receiving events via appservice
transactions instead of `/sync` for high-traffic instances. The Matrix
Application Service spec generalizes this further: an application service
registers with the homeserver and is granted control of a whole namespace of
user IDs (including bot/bridge users), with the homeserver itself enforcing
that only the registered AS can act as those identities — the server, not any
client, is the arbiter of "who owns this identity." (Sources:
github.com/mautrix/docs `maubot/usage/standalone.md`; matrix.org "Application
Services" older docs, matrix.org/docs/older/application-services/.)

**Common thread:** all three tie one bot identity to one authoritative
credential (token, or homeserver-granted namespace) held by one running
process (or a coordinated shard set of the *same* process), and the platform
itself — not each client — is what resolves "who is this bot" for every
viewer.

## 2. Nostr-specific prior art

No NIP (accepted or the merged base spec set) currently defines a general
"agent identity + single execution owner" model. Two relevant open pull
requests against `nostr-protocol/nips` exist, both unmerged/draft as of this
research:

- **PR #2466, "NIP-XX: Agent-First Messaging."** Proposes event kinds/tag
  conventions for a NIP-29 group where autonomous agents hold their own keys
  with scoped, revocable permissions; argues an agent's identity is its
  keypair, so "no server issues it a bot token" and no server can forge or
  silently rotate it. It builds on existing NIP-29 groups + NIP-C7/NIP-7D/
  NIP-22 chat kinds rather than defining new ones. It does **not** address the
  collision problem (many holders of "the same logical agent" minting
  different keypairs) — it assumes one already-provisioned keypair per agent.
  Reference implementation: github.com/superirale/Quorum. (Source:
  github.com/nostr-protocol/nips/pull/2466.)
- **Proposed NIP-100, "Sovereign Agent Identity Network (SNIN)."** A more
  comprehensive draft covering agent passports, discovery, tasks, marketplace,
  invoices — economic/identity infrastructure for many independent agents, not
  specifically the "one shared bot identity across N unaffiliated clients"
  problem.
- Existing shipped NIPs touch adjacent but narrower ground: **NIP-24** already
  reserves a `bot: true` field on kind:0 profile metadata to mark automated
  content, and **NIP-39** lets a profile attach externally-verified identity
  claims (`i` tags) — neither prescribes execution ownership.

Buzz's own `NIP-OA` (owner attestation, `Schnorr(SHA256("nostr:agent-auth:" ||
agent_pubkey || ":" || conditions), owner_secret)`, per
`crates/buzz-sdk/src/nip_oa.rs` per issue #2859's description) is a
Buzz-invented convention, not a published/accepted NIP — no NIP number exists
for it. **Conclusion: there is no existing widely-adopted Nostr bot framework
or accepted NIP that already solves "one bot identity, centrally hosted,
multi-client-visible."** The protocol layer leaves this entirely to
application convention, and Buzz's own collision bug (#2910) is evidence no
convention has emerged yet even informally.

## 3. Multi-agent orchestration frameworks (server-side agent, many callers)

- **OpenAI/Azure OpenAI Assistants API.** The documented pattern is one
  Assistant object (server-side, holds instructions/tools/model) with **one
  Thread per end-user/conversation** appended to over time; Threads are
  explicitly isolated from each other so "stuff that a user writes in their
  own thread isn't shared with other threads." Multiple simultaneous callers
  share the one Assistant configuration but never share or contend over
  identity — the platform (OpenAI's servers) is the single host, and the
  "identity" (Assistant ID) is meaningless without it; no client can spin up
  its own local Assistant with the same ID. (Source: Microsoft Learn, Azure
  OpenAI Assistants concepts, learn.microsoft.com/.../concepts/assistants;
  corroborated by OpenAI developer-community threads on one-assistant/many-
  threads concurrency, e.g. community.openai.com/t/multiple-threads-per-
  assistant/1079806.)
- **LangChain.** LangServe (now maintenance-mode) served this exact model over
  HTTP: `add_routes()` binds one runnable/agent to one path on one running
  server process; "efficient /invoke, /batch and /stream endpoints with
  support for many concurrent requests on a single server." Its
  successor, **Agent Server** (docs.langchain.com/langsmith/agent-server), and
  the `deployment-cookbook` examples keep the same shape but add a
  **checkpointer** (Redis/Postgres/SQLite) so conversation state survives
  "beyond a single instance" — i.e., the identity/state is externalized to a
  shared store precisely so that *multiple server processes* can serve the
  same logical agent without forking its state. (Source:
  github.com/langchain-ai/langserve README; github.com/langchain-ai/
  deployment-cookbook README.)
- AutoGen and CrewAI's hosted/enterprise modes were not directly documented in
  primary sources retrieved this session; the LangChain pattern (one hosted
  agent process/cluster + externalized shared state, addressed by a stable
  server-side ID) is the representative documented example found and is
  consistent with how OpenAI Assistants does it.

**Common thread:** every one of these frameworks puts the agent's identity and
state on a server the clients call into — never lets a calling client
instantiate its own independent copy of "the same" agent.

## 4. block/buzz's own prior attempts (why none converged)

Reconstructed from GitHub issues/PRs (block/buzz):

- **#2859 (`buzz-spawner` proposal) → PR #3113 (implementation) → split into
  PR stack #3185–#3188.** Proposed a standalone daemon watching the relay for
  agent-definition events (`kind:30178` spec / `30179` status / `24201`
  attestation handshake) and reconciling them into one Docker container per
  agent, with the private key minted server-side and never leaving the host.
  This is the closest thing to "one canonical execution owner" in the whole
  history. A later comment (siddhartpai) reports it actually built and
  extended with **identity relocation** — moving an *existing* local agent's
  keypair to a spawner so pubkey/history/DMs survive — and found via live
  testing that a naive hand-off caused **two runners racing on one identity**
  (local auto-restart resurrecting the process after relocation), fixed only
  by treating "relocated" as persisted state checked at every spawn choke
  point. It was never merged to `main`; the RFC (#4174) still treats it as "a
  strong foundation," not a shipped design.
- **#3449 (SSH/provider-based remote agents).** Specified a `BackendKind::
  Provider` stdin/stdout JSON contract and shipped `buzz-backend-ssh` as a
  reference implementation — no daemon, one process per operation, credential
  and workspace concerns handled per-op. Review comments flagged real gaps
  left unresolved: no working-directory/repo-checkout projection for the
  remote agent (each long-running agent ends up in a separately provisioned
  checkout, not Buzz's native project workspace), and team instructions not
  carried to the remote record — both filed as known limitations rather than
  fixed, because closing them "means a protocol field the desktop states
  rather than the provider guessing" (out of scope for that PR).
- **#3551 (self-hosted backend framework, docs+systemd only).** Deliberately
  scoped down to documentation and a `systemd` unit template — explicitly "no
  protocol fork," positioned as guidance for operators, not a new mechanism.
  Its own author frames it as intentionally the smallest mergeable slice
  ("happy to trim wording... framework doc vs quick-start split is
  intentional"), i.e. it never tried to solve identity collision at all — it
  assumes one operator, one host, one agent set.
- **#3554 (machine enrollment, many agents per host).** An "umbrella / product
  shape" request explicitly built **on top of** #2859/spawner rather than a
  competing design; it asks maintainers to "confirm whether #2859 is the
  intended vehicle" — i.e., its author self-identifies as blocked on the same
  unmerged spawner work, not offering an alternative.
- **#3628 (headless self-hosted persona seats).** Not an identity-collision
  proposal at all — it's a question about how personas/skills reach a headless
  seat after `buzz-persona` pack-loading support was removed from `buzz-acp`
  (PR history: #297 added it, #1846 removed it in favor of `TeamRecord` +
  snapshots). Comments reveal the portable `AgentSnapshot`/`TeamSnapshot`
  format has no fields for skills, env, or MCP config today — a real gap, but
  orthogonal to shared identity.
- **#2663 (external non-ACP agents, e.g. OpenClaw).** Different problem
  entirely: giving non-ACP external systems a realtime listen/respond path
  (`buzz listen`, webhooks, or a generic stdio bridge) — about connectivity,
  not identity ownership.
- **#4174 (the RFC, still open).** Maintainer (dspury) states the target
  principle directly: **"A Buzz agent should be the relay-native identity, not
  the process currently running it,"** with custody/lifecycle ownership
  ("Buzz owns the key and launches the body" vs. "a self-hosted harness owns
  the key and runtime") as the axis that should unify all the prior attempts.
  A contributor (Onnokh) is described as actively building a five-layer
  execution-node/runtime-adapter architecture, but it is explicitly
  unfinished ("not ready to share the implementation yet"). Another commenter
  (Shipitrealgood) identifies the actual blocking primitive: **there is no
  single, protocol-level, attestable way to mint an agent identity regardless
  of whether it's created from Desktop or a VM** — every attempt so far
  hand-rolls its own minting/attestation path, so none of them compose. That
  is the stated reason none of the ~10 related PRs converged: they solved
  different sub-problems (container hosting, SSH deploy, docs, persona
  delivery, external bridging) without a shared foundation for "what is an
  agent identity and who is allowed to create/move one" — the RFC is asking
  for that foundation, and as of this research it does not exist yet.

**None of these attempts, including the most advanced one (spawner), directly
target the #2910 collision bug** — they're all about *where an agent's process
runs*, not about *preventing two different people's independently-minted
"default Fizz" identities from both existing in one channel*. The RFC's
custody/lifecycle framing would make the collision fixable (default personas
could require single-mint-per-community rather than one-per-user-by-default)
but no linked issue or PR proposes that specific change.

## 5. kirocrew (`kirodotdev/KiroCrew`)

kirocrew is a Python solo-dev "gateway" wrapping an external CLI agent runtime
(per our own fork's `Improvements/wishlist.md`), not a chat platform — it's
architecturally a single local daemon serving one operator's tools/MCP
servers, not a shared multi-user bot. Its concurrency-control primitive is a
**lock file**, not identity federation:

- The gateway "mints ONE secret and writes it to two files: `run/gateway-
  <port>.secret` (always) and `.local_secret` (the single-instance
  fallback)" — i.e., a single-instance guarantee enforced by a locally-held
  secret/lock, not a network-resolvable identity. (Source: kirocrew issue
  #9175, "security: split the gateway credential by capability.")
- Issue #12673 states plainly: **"Today Kiro Crew is effectively
  single-tenant: one gateway bound to one operator identity. Workspaces
  isolate memory/tools but not people."** The filer explicitly requests
  multi-user/SSO/shared-instance support as unbuilt future work, confirming
  kirocrew has not solved "one shared identity, many independent callers" —
  it has solved "one identity, one owner, everything else is out of scope."

**Kirocrew has no answer to our problem.** It assumes a single trusted
operator per gateway instance; multi-tenant/shared access is an open feature
request (#12673), and its lock/secret mechanism exists to prevent *accidental
duplicate local processes for the same operator*, not to arbitrate identity
across independent, mutually distrusting clients the way Buzz's per-user
Desktop installs are.

## Implications for us (descriptive only — no recommendation)

- **Adopting the Slack/Discord/Matrix-appservice pattern** (one canonical
  process/credential per bot identity, resolved by the server) would require
  Buzz's relay (or a relay-trusted service) to become the arbiter of "who may
  publish as this agent's pubkey," analogous to a Matrix homeserver's
  application-service registration — a capability the relay does not have
  today (it currently treats any correctly-signed event as legitimate
  regardless of who created the identity).
- **Adopting the OpenAI-Assistants/LangChain pattern** (one hosted
  agent + externalized shared state, many callers) would require Buzz to pick
  one authoritative execution host per agent identity and route all
  `@mention`s to it over the relay, plus a durable, shared conversation/memory
  store keyed by agent pubkey rather than per-Desktop-install local state —
  which is close to what the unmerged `buzz-spawner` work (#2859/#3113)
  already attempted for hosting, but was never extended to cover *default,
  auto-provisioned* personas like Fizz.
- **Adopting the Matrix application-service model specifically** would
  require a namespace/ownership grant step (something registers "this pubkey
  range/persona name belongs to this one execution owner for this
  community") enforced server-side, which doesn't exist in Buzz's current
  purely-cryptographic (signature-only) trust model.
- **Following Buzz's own RFC #4174 direction** (agent = relay-native identity,
  decoupled from any one process) would require resolving the identity-
  minting primitive gap that Shipitrealgood's comment identifies: a single,
  protocol-level way to mint/attest an agent identity that is the same
  regardless of whether Desktop, a VM, or a default-persona bootstrap path
  creates it — without that, any single-instance-per-agent guarantee has to
  be re-implemented per creation path (as the spawner's relocation-guard
  fix and the SSH provider's known-limitations list both show happening in
  practice).
- **Adopting kirocrew's model** would not by itself address multi-client
  identity collision at all, since kirocrew's own single-tenant assumption
  is the thing its users are asking it to relax (#12673) — it would only be
  informative for the "single-instance-per-process" lock/secret mechanics
  (issue #9175), not for cross-client shared identity.
