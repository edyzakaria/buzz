# The reverse direction: what kirocrew (or similar solo-dev agent gateways) could adopt from buzz

Companion to [`wishlist.md`](./wishlist.md), which looks at kirocrew → buzz.
This looks the other way: buzz is a chat-native, multi-human-multi-agent
platform built on signed Nostr events; kirocrew is a Python gateway
projecting one developer's agent session across many chat surfaces. Buzz's
protocol-first, cryptographically-signed architecture solves several
problems kirocrew currently solves with gateway-local, trust-the-process
mechanisms.

---

## 1. True multi-party group channels, not 1:1 bridging

Kirocrew treats a conversation as one user's session projected onto
Slack/Discord/Telegram/etc — there's no native concept of several humans
*and* several agents co-present in one shared, addressable space. Buzz's
channels (NIP-29 `h`-tag scoping, kind:39002 membership events) give every
participant — human or agent — the same shared thread by construction.
A real group/channel primitive, rather than N per-platform 1:1 bridges,
is the difference between "my agent, mirrored to many chat apps" and
actual team collaboration.

## 2. Cryptographic per-agent identity instead of a name string

A buzz agent has its own keypair; everything it posts is a signed event —
authorship and integrity are independently verifiable by anyone, without
trusting the gateway process. A kirocrew agent is just an entry in a JSON
config with no signing — "who said what" is only as trustworthy as the
gateway's own bookkeeping. Giving each agent a signing key (even without
adopting Nostr wholesale) would make multi-agent output independently
verifiable and portable across gateway restarts or migrations.

## 3. Encrypted, portable memory objects instead of local files

Buzz engrams are NIP-44-encrypted, addressable, tombstoneable *events* —
they can be synced, backed up, or relayed without ever touching the
gateway's disk, and only the agent↔owner keypair can decrypt them.
Kirocrew's memory (SQLite + markdown) sits unencrypted on the gateway
host with no portability story. Their `lessons.jsonl`/semantic-KV design is
good — wrapping it in an owner-scoped encryption envelope like buzz's would
fix "anyone with disk access reads all your agent's memory" without giving
up the layered-memory idea itself.

## 4. Protocol-first interoperability over per-platform connectors

Buzz exposes one relay protocol; any Nostr-aware client speaks it without
bespoke integration code. Kirocrew hand-codes a connector per surface
(Slack, Discord, Telegram, Webex, WeCom, Teams, Weixin) — seven separate
bridges to build and maintain, and every new surface is another one.
An event-protocol-first design collapses "N connectors" into "1 protocol,
N thin adapters," which scales much better as surfaces are added.

## 5. Workflow state as chat-visible objects, not gateway-internal logs

Buzz's workflow runs are just more events in the same channel everyone's
already watching — inherently observable by every participant, human or
agent. Kirocrew's task/run state lives in internal JSON snapshots invisible
unless you go dig through a dashboard. Surfacing task/plan/replan state as
first-class posts in the conversation (the way buzz naturally does) would
make kirocrew's autonomous runs auditable by the humans actually present,
not just by log-readers after the fact.

## 6. Asymmetric, per-agent audit signatures over one shared HMAC secret

Kirocrew's audit chain trust model rests on protecting a single gateway-held
HMAC key — compromise that key and the whole chain is forgeable
retroactively. Buzz's model (each event signed by its own keypair) means no
single secret, if leaked, can forge everyone else's history — verification
is distributed across every participant's own key, not centralized in one
log-writer's secret.

---

These are architectural, not code-level, ports — they'd mean kirocrew
adopting event-signing and a shared-channel primitive as first-class
concepts, which is a bigger lift than any single item in `wishlist.md`.
Flagged here as direction, not a scoped task.
