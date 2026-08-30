# Relay: Product Requirements and User Flows

Document status: normative product specification for Relay.
Last revised: 2026-08-30.

Implementation status: nothing is built. Every requirement, flow, limit, and
guarantee in this document is `planned`: specified, not implemented. No gate
has passed; no test exists; no benchmark has run. The only `accepted`
artifacts in this repository are the architecture decision records in
[./decisions/](./decisions/), which record decisions rather than working
code. The status vocabulary used throughout is exactly: `accepted`
(implemented on mainline, backed by its named automated gate), `in progress`
(present on a branch, not a claim), `planned` (specified, not implemented),
and `deferred` (outside the named phase; forbidden as completion evidence). A
package, type, stub, or happy-path unit test is never completion. Conflict
and status precedence between this document and its companions is fixed in
[./README.md](./README.md).

## 1. Product Definition

Relay is a self-hosted message queue and pub/sub service. A backend engineer
runs one static binary, `relayd`, points an application at it through the
`relay-client` library or the `relayctl` CLI, and gets durable at-least-once
queues, strictly ordered FIFO message groups, content deduplication,
dead-letter queues with redrive, and topic fanout with attribute filter
policies — on hardware the operator controls, with no managed-cloud
dependency.

What distinguishes Relay is not the feature list, which is deliberately
small, but how its guarantees are established. Every delivery guarantee Relay
states is machine-checked: the entire broker state machine is a pure,
deterministic function driven through deterministic simulation (virtual
clock, simulated network, simulated disk, seeded randomness) and checked
against a reference model by a linearizability oracle over recorded operation
histories. Crash, torn-write, disk-full, partition, and failover behavior are
not tested by hopeful integration runs against live infrastructure; they are
explored systematically in simulated time, and every failure ever found
replays exactly from a seed. The claim is not "we tested it a lot"; the claim
is "this specific property is enforced by this specific named check, and here
is the seed corpus that once broke it."

The verification apparatus — the simulation harness, the reference model, the
history checker, the crash-injection rig, and the failing-seed corpus — is a
first-class product surface, not internal scaffolding. Users are expected to
run it, reviewers are expected to audit it, and no user-visible guarantee may
exist without a named check inside it. The properties Relay proves and the
non-guarantees it refuses to imply are owned by
[./CORRECTNESS.md](./CORRECTNESS.md); this document defines the product
behavior those properties protect.

### 1.1 One-sentence pitch

Relay is a self-hosted message queue and pub/sub service whose delivery
guarantees are machine-checked by deterministic simulation and model
checking, not asserted in documentation.

### 1.2 Flagship demonstration

The flagship demonstration kills the cluster leader with `kill -9` in the
middle of a live benchmark and then replays a discovered failure from its
seed:

1. The operator boots a 3-node Relay cluster on commodity hardware with
   `relayd` and confirms membership and leadership with
   `relayctl cluster members`.
2. The benchmark harness drives sustained send, receive, and delete traffic
   against a standard queue while recording every acknowledged send in a
   client-side ledger.
3. Mid-run, the operator issues `kill -9` against the current leader process.
   No shutdown handler runs; the process gets no chance to flush anything.
4. Clients observe `NOT_LEADER` and `UNAVAILABLE` errors for a bounded
   window, follow the leader hint, and resume against the new leader.
5. The run completes. The harness reconciles its ledger against received
   messages and reports zero lost acknowledged sends, zero invented
   messages, and every remaining message either delivered or dead-lettered —
   the properties P-01, P-09, and P-10 of
   [./CORRECTNESS.md](./CORRECTNESS.md), measured, not asserted.
6. The presenter then opens the simulation corpus, picks a historical seed
   that once produced a double-lease across a partition, and runs the
   deterministic simulator with that seed. The failure reproduces
   byte-for-byte on the first try, in milliseconds, on a laptop — followed by
   the same seed passing against the fixed code.
7. The demonstration closes on the traceability matrix in
   [./BUILD_PLAN.md](./BUILD_PLAN.md) §16: every claim shown maps to a
   requirement ID, a terminal gate, and a named automated check.

The same engine must support the deterministic simulation environment in CI
and real hardware in production; the state machine code is identical in both.

### 1.3 Product hierarchy

When priorities conflict, the product is ordered as follows:

1. correct core queue semantics, proven against the reference model;
2. the durability contract: an acknowledged send survives crash;
3. deterministic reproducibility: any failure replays from a seed;
4. a bounded, fuzzed, authenticated wire API honest about its errors;
5. replication safety: no lost ack and no double-lease across failover;
6. operability: metrics, tracing, runbook, backup, and restore;
7. published, statistically honest performance evidence;
8. marketing and launch collateral, bounded by that evidence.

Verification work that protects a higher item precedes feature work on a
lower item. No feature ships ahead of the checks that would catch it lying.

## 2. Primary Users and Jobs

### 2.1 Backend engineer

The primary user builds services that must not lose work: job queues, order
pipelines, webhook fanout, audit trails. This engineer has been burned by
queues whose durability claims dissolved under a crash and wants a queue
whose guarantees are legible and checkable.

Required jobs:

- create a queue and move real traffic through it within minutes of install;
- send, receive, and delete messages with an obvious client library;
- recover cleanly when a consumer crashes mid-processing;
- keep strict per-key ordering where the domain requires it;
- suppress duplicate submissions inside a known window;
- route poison messages to a dead-letter queue and redrive them later;
- read exactly what is and is not guaranteed without decoding marketing.

### 2.2 Platform and SRE operator

This user installs, upgrades, monitors, and repairs Relay clusters. They need
a single static binary, fail-fast configuration, honest health and readiness
endpoints, Prometheus metrics under a stated cardinality budget, structured
logs, a rehearsed backup-and-restore procedure, a rolling-upgrade window with
rollback, and a runbook that was written by someone who has broken the
system on purpose.

### 2.3 Correctness-focused evaluator

This user — a staff engineer or architect deciding whether to depend on
Relay — audits the gap between claims and evidence. They need the property
list and non-guarantee list in [./CORRECTNESS.md](./CORRECTNESS.md), the
mapping from every requirement to its terminal gate, the simulation corpus,
the model-checker output, and benchmark methodology in
[./BENCHMARK_PLAN.md](./BENCHMARK_PLAN.md) complete enough to reproduce.

### 2.4 Systems-engineering hiring reviewer

This user evaluates Relay as a portfolio artifact. They need to see real
systems work: a hand-rolled segmented write-ahead log, an in-house Raft
implementation, a custom bounded binary protocol with a fuzz corpus, a
deterministic simulation harness, a linearizability checker, and the
discipline of a project that never claims more than its tests prove.

## 3. Product Principles

### 3.1 Machine-checked over asserted

A guarantee exists only if a named automated check enforces it. Documentation
may describe a property; only the check makes it true. Any statement about
delivery, durability, ordering, or deduplication in any Relay document must
name the property in [./CORRECTNESS.md](./CORRECTNESS.md) and, through it,
the test family that proves it.

### 3.2 At-least-once, honestly stated

Relay delivers each message at least once and says so everywhere it matters.
It never implies exactly-once delivery, never hides redelivery behind vague
language, and states plainly that consumers must be idempotent (NG-01).
Deduplication narrows the window of duplicate acceptance on send; it does not
change the delivery guarantee.

### 3.3 Determinism over live infrastructure

Correctness evidence comes from deterministic simulation in virtual time, not
from flaky orchestration of live processes. Simulated suites are zero-flake
by policy: a flake is a bug. Live-cluster suites exist for what simulation
cannot represent and carry explicit quarantine rules in
[./OPERATIONS_TEST_PLAN.md](./OPERATIONS_TEST_PLAN.md).

### 3.4 One state machine everywhere

All broker semantics live in one pure, deterministic state machine
(`relay-core`) with no clock, randomness, IO, or thread access. Production,
simulation, and the model checker drive the same code through the same
`apply` function. Time enters only as explicit `AdvanceTime` log entries per
[./decisions/ADR-0005-injected-time-and-log-applied-clock.md](./decisions/ADR-0005-injected-time-and-log-applied-clock.md).

### 3.5 Bounded parsers at every boundary

Every byte that crosses a trust boundary — wire frames, WAL records,
snapshots, configuration — is parsed by a bounded parser that checks every
length against a limit before allocating, and every such parser has fuzz
targets whose corpus gates CI. There is no general-purpose serialization
framework on the wire per
[./decisions/ADR-0004-rwp-binary-protocol-no-sqs-compat.md](./decisions/ADR-0004-rwp-binary-protocol-no-sqs-compat.md).

### 3.6 Crash-only software

Relay's only supported stop model is the crash. Recovery from `kill -9` is
the normal startup path, not a special case; graceful shutdown is an
optimization, not a correctness requirement. Consequently, an fsync failure
is fatal — the process aborts rather than retrying into corruption — per
[./decisions/ADR-0008-fsync-before-ack-durability-contract.md](./decisions/ADR-0008-fsync-before-ack-durability-contract.md).

### 3.7 No unearned claims

Documentation labels planned work as planned and names the gate behind every
claim. In-memory behavior is never promoted to durability, single-node
behavior is never promoted to replication, and a simulated fault is never
promoted to production hardening. Reversals of recorded decisions require a
new ADR.

### 3.8 Small surface, deep verification

Relay ships fewer operations than incumbent queues and verifies each one to a
depth incumbents do not attempt. Feature requests compete against
verification depth, and verification wins. The explicit non-goals in §4.2 are
product positions, not backlog.

### 3.9 Operator-honest observability

Metrics, logs, traces, health endpoints, and `DescribeQueue` counts tell the
operator the truth, including the unflattering truth: approximate counts are
labeled approximate with staleness, readiness reflects actual Raft
membership and leadership, and backpressure is visible before it becomes an
outage.

### 3.10 Marketing may never outrun evidence

Every public claim about Relay cites a verified property, a named
non-guarantee, or a benchmark result with hardware and statistical
treatment. [./MARKETING.md](./MARKETING.md) sits below
[./CORRECTNESS.md](./CORRECTNESS.md) and
[./THREAT_MODEL.md](./THREAT_MODEL.md) in precedence and may never
strengthen a claim beyond them. A claims audit gates every release
announcement.

## 4. Scope

### 4.1 Required for Relay 1.0

Relay 1.0 is one cumulative bundle produced only after every gate R0–R10 in
[./BUILD_PLAN.md](./BUILD_PLAN.md) passes. Intermediate gates are evidence
checkpoints, not earlier supported releases. The 1.0 surface is:

- standard queues with create, delete, attribute update, describe, list,
  purge, send, batch send, receive, delete, and visibility change;
- the durability contract: fsync-before-ack with bounded group commit,
  crash-safe WAL recovery, torn-write detection, and disk-full safety;
- per-message and per-queue delay, per-queue retention, typed message
  attributes, and per-queue in-flight caps;
- dead-letter queues with automatic move on receive-count exhaustion and
  redrive back to the source queue with progress reporting;
- FIFO queues with strict per-group ordering, parallel groups, content and
  explicit deduplication inside a fixed 300 s window;
- topics, subscriptions, filter policies with exact, anything-but, prefix,
  numeric-range, and exists matching, and fanout into standard and FIFO
  queues;
- the RWP/1 framed binary protocol with bounded parsing, per-tenant HMAC
  authentication, per-queue and per-topic ACLs, quotas, rate limits, long
  polling, TLS 1.3, and version negotiation;
- 3-node and 5-node Raft replication with pre-vote elections, majority
  commit, linearized lease grants, ReadIndex reads, snapshot install,
  single-server membership change, and leader hints;
- the `relayctl` admin CLI covering every administrative operation with
  human and JSON output, cluster administration, and an audit log of every
  administrative mutation;
- health and readiness endpoints, Prometheus metrics under a named
  cardinality budget, OTLP trace spans, structured JSON logs, and a
  `relayctl diagnose` support bundle;
- packaging as one static binary with embedded provenance, fail-fast TOML
  plus environment plus flag configuration, backup and restore rehearsed by
  a scripted drill, rolling upgrade and rollback within a one-minor-version
  window, uninstall and purge, and an incident-response procedure;
- published benchmarks, a failure-injection report, a capacity model, and
  evidence-bound launch collateral gated by a claims audit;
- the complete verification apparatus: deterministic simulation with a
  checked-in failing-seed corpus, the reference model and linearizability
  checker, crash and fault injection, fuzzing, and mutation testing at the
  thresholds in §9.5.

### 4.2 Explicit non-goals

The following are product positions. Each either restates a permanent
non-guarantee from [./CORRECTNESS.md](./CORRECTNESS.md) (NG-01 through
NG-10) or defers a surface with a recorded fail-closed default and a reopen
trigger in [./OPEN_QUESTIONS.md](./OPEN_QUESTIONS.md).

- No exactly-once delivery. Relay is at-least-once; consumers must be
  idempotent. No roadmap item weakens this honesty (NG-01).
- No cross-queue or cross-operation atomicity. A batch is not a
  transaction; fanout is per-subscription, never atomic across queues
  (NG-02).
- No global ordering across groups, queues, or topics. Ordering is a
  per-message-group property of FIFO queues only (NG-03).
- No exact-instant visibility expiry. Expiry is "not before" the timeout;
  scheduling jitter is permitted and documented (NG-04).
- No bounded delivery latency. Long-poll wakeup and delivery-latency numbers
  are benchmark goals, never contracts (NG-05).
- No message bodies over 256 KiB, and no claim that larger payloads belong
  in a queue (NG-06).
- No Byzantine fault tolerance. Relay tolerates crash-stop faults only; a
  malicious or arbitrarily corrupting node is out of scope (NG-07).
- No multi-region or geo replication. A Relay cluster is one
  failure-domain-local Raft group (NG-08).
- No durability for unacknowledged sends. The acknowledgment is the
  durability boundary; a send whose ack never arrived may be lost (NG-09).
- No promise that FIFO throughput matches standard-queue throughput
  (NG-10).
- No SQS wire compatibility. Relay speaks RWP/1 only, per
  [./decisions/ADR-0004-rwp-binary-protocol-no-sqs-compat.md](./decisions/ADR-0004-rwp-binary-protocol-no-sqs-compat.md);
  Relay refuses to fake semantics it does not implement.
- No HTTP/JSON gateway at 1.0. The gateway is deferred with its fail-closed
  default and reopen trigger recorded in
  [./OPEN_QUESTIONS.md](./OPEN_QUESTIONS.md).
- No multi-tenancy isolation beyond authentication, authorization, and
  quotas. Tenants share one process, one store, and one failure domain;
  Relay does not claim workload isolation between tenants at 1.0.
- No encryption at rest at 1.0. Operators requiring it must use filesystem
  or block-level encryption; the first-party feature is deferred in
  [./OPEN_QUESTIONS.md](./OPEN_QUESTIONS.md).
- No hosted or managed Relay service, no embedded-library deployment mode,
  and no message transformation, scheduling DSL, or workflow engine inside
  the broker.

## 5. Command and API Surface

Relay exposes one wire protocol, RWP/1 (frame format fixed in
[./ARCHITECTURE.md](./ARCHITECTURE.md)), consumed by `relay-client` and
`relayctl`. Default ports: 7414 (API), 7415 (metrics and health), 7416 (Raft
inter-node). Opcode assignments and byte-level body layouts are owned by
[./ARCHITECTURE.md](./ARCHITECTURE.md); this section fixes the user-visible
operation set and semantics.

### 5.1 RWP/1 operations

Queue data plane:

| Operation | Required behavior |
| --- | --- |
| `SendMessage` | Append one message (body ≤ 256 KiB, ≤ 10 typed attributes, optional `DelaySeconds` 0–900 s) and acknowledge only after the durability contract is satisfied. |
| `SendMessageBatch` | Accept up to 10 entries with independent per-entry validation, application, and results. |
| `ReceiveMessage` | Lease up to 10 messages under a visibility timeout (0 s–12 h, default 30 s) with optional `WaitTimeSeconds` 0–20 s long polling; return opaque single-use receipt handles. |
| `DeleteMessage` | Permanently remove a leased message identified by a valid receipt handle; idempotent for the same handle. |
| `ChangeMessageVisibility` | Extend or shorten an active lease; a value of 0 returns the message to the available set immediately. |

Queue control plane:

| Operation | Required behavior |
| --- | --- |
| `CreateQueue` | Create an empty standard or FIFO queue after full configuration validation; no partial state on failure. |
| `DeleteQueue` | Terminally delete a queue: invalidate receipt handles, remove subscriptions, free storage. |
| `SetQueueAttributes` | Validate and apply configuration changes with bounded propagation to in-flight behavior. |
| `DescribeQueue` | Return configuration plus approximate message counts labeled with staleness. |
| `ListQueues` | List queues with prefix filter and cursor pagination. |
| `PurgeQueue` | Remove every message including in-flight; reject a concurrent purge while one is active. |
| `StartRedriveTask` | Begin moving messages from a DLQ back to the source queue. |
| `DescribeRedriveTask` | Report redrive progress: moved, remaining, and failed counts, and terminal status. |

FIFO queues use the same operations with FIFO-specific fields:
`MessageGroupId` (required on send, ≤ 128 bytes) and optional
`MessageDeduplicationId`. Topic operations:

| Operation | Required behavior |
| --- | --- |
| `CreateTopic` | Create a topic after name and configuration validation. |
| `DeleteTopic` | Remove the topic and its subscriptions; subscribed queues and their messages are unaffected. |
| `Subscribe` | Bind a queue to a topic with an optional filter policy validated and recorded at subscribe time. |
| `Unsubscribe` | Stop future delivery for one subscription without affecting delivered copies. |
| `Publish` | Deliver an independent copy of the message to every matching subscription. |
| `DescribeTopic` | Return configuration, subscription list, and approximate counters labeled with staleness. |
| `ListTopics` | List topics with prefix filter and cursor pagination. |

Administrative and protocol operations:

| Operation | Required behavior |
| --- | --- |
| `Hello` | Negotiate protocol version before any state change; reject unknown versions with a stable error. |
| `TagResource` / `UntagResource` | Add or remove tags on a queue or topic. |
| `ListResourcesByTag` | Enumerate resources carrying a tag, with cursor pagination. |
| `ClusterMembers` | Return member list, roles, and per-member health. |
| `TransferLeadership` | Request an orderly leadership transfer to a named member. |

### 5.2 relayctl command tree

`relayctl` covers every administrative operation (FR-ADMIN-006) with `--output
human|json` on every command, an `--endpoint` flag, and named credential
profiles. The required tree:

| Command | Required behavior |
| --- | --- |
| `relayctl queue create <name>` | Create a queue with flags for every configuration attribute; print the resulting configuration. |
| `relayctl queue delete / describe / list / set-attributes / purge` | Invoke the matching control-plane operation with exact server-side semantics. |
| `relayctl queue send / receive / delete-message / change-visibility` | Data-plane operations for debugging and scripting, never a replacement for `relay-client` in applications. |
| `relayctl queue redrive start / status` | Start a DLQ redrive task and report its progress. |
| `relayctl topic create / delete / describe / list` | Topic control plane. |
| `relayctl topic subscribe / unsubscribe / publish` | Subscription management and test publishing. |
| `relayctl tag add / remove / list` | Resource tagging and tag-based listing. |
| `relayctl cluster members / health / transfer-leader` | Cluster administration surface. |
| `relayctl backup create / restore` | Consistent snapshot plus WAL-archive backup, and scripted restore. |
| `relayctl diagnose` | Produce a redacted support bundle with an exact content inventory. |
| `relayctl config validate` | Validate a configuration file offline with field-level errors and the effective merged result. |
| `relayctl version` | Print client version, and server version plus build provenance when reachable. |

Unknown subcommands fail with a suggestion; no command silently degrades to a
different operation than the one named.

### 5.3 Long-poll semantics

1. `ReceiveMessage` carries `WaitTimeSeconds` from 0 (immediate return) to
   20 s inclusive; values outside the range fail with `INVALID_ARGUMENT`.
2. When no message is available, the server holds the request and returns
   early as soon as a matching message becomes available — by new send,
   delay expiry, visibility expiry, or redrive arrival.
3. On timeout with nothing available, the server returns an empty success,
   not an error.
4. A held long poll never blocks other requests multiplexed on the same
   connection; responses are correlated by request ID and may return out of
   request order (FR-API-007).
5. Long-poll wakeup latency targets are benchmark goals under NG-05, never
   contracts.

### 5.4 Error taxonomy summary

Every failure returns exactly one stable machine-readable code, a
human-readable message, and a retryability class. The full registry with
wire encoding is owned by [./ARCHITECTURE.md](./ARCHITECTURE.md); the
user-visible classes and their anchor codes are fixed here:

| Code | Retryable | Meaning |
| --- | --- | --- |
| `MALFORMED_FRAME` | no | Frame failed magic, length, CRC32C, or body-layout validation. |
| `FRAME_TOO_LARGE` | no | Declared frame length exceeds the 1 MiB wire limit. |
| `UNSUPPORTED_VERSION` | no | Protocol version negotiation failed before any state change. |
| `UNAUTHENTICATED` | no | Missing or invalid per-tenant HMAC authentication. |
| `ACCESS_DENIED` | no | ACL authorization denied the operation; deny takes precedence. |
| `THROTTLED` | yes, after hint | Tenant quota or rate limit exceeded; carries a retry-after hint. |
| `INVALID_ARGUMENT` | no | A field failed validation; carries field-level detail. |
| `QUEUE_NOT_FOUND` / `TOPIC_NOT_FOUND` / `SUBSCRIPTION_NOT_FOUND` | no | The named resource does not exist. |
| `QUEUE_ALREADY_EXISTS` / `TOPIC_ALREADY_EXISTS` | no | Creation conflicts with an existing resource of different configuration. |
| `MESSAGE_TOO_LARGE` | no | Body exceeds 256 KiB. |
| `RECEIPT_INVALID` | no | Receipt handle is malformed, foreign, or fails HMAC verification. |
| `RECEIPT_SUPERSEDED` | no | The lease epoch in the handle is no longer current. |
| `RECEIPT_EXPIRED` | no | The lease behind the handle has expired. |
| `PURGE_IN_PROGRESS` | yes, after completion | A purge is already active on this queue. |
| `REDRIVE_IN_PROGRESS` | yes, after completion | A redrive task is already active on this DLQ. |
| `INFLIGHT_LIMIT_EXCEEDED` | yes, with backoff | The per-queue in-flight cap (120,000 standard / 20,000 FIFO) is reached. |
| `FILTER_POLICY_INVALID` | no | A filter policy failed validation; carries field-level detail. |
| `NOT_LEADER` | yes, at hinted leader | This node cannot serve the write; carries a leader hint. |
| `UNAVAILABLE` | yes, with backoff | No leader, quorum lost, or the node is draining. |
| `INTERNAL` | yes, with backoff | A server invariant failed; details are logged server-side, never leaked. |

Retryability classes are exactly: retryable with backoff, retryable after a
server-provided hint, retryable at the hinted leader, and not retryable.
`relay-client` exposes the class programmatically; callers never parse
message text to decide behavior.

## 6. Core User Flows

### 6.1 Create a queue and send, receive, delete

1. The engineer runs `relayctl queue create orders --visibility 60s` against
   a running `relayd`; validation passes and the effective configuration is
   printed, including every defaulted value.
2. The application sends a message through `relay-client`. The client blocks
   until the server acknowledges; the acknowledgment means the message and
   its attributes are durable per the contract in
   [./decisions/ADR-0008-fsync-before-ack-durability-contract.md](./decisions/ADR-0008-fsync-before-ack-durability-contract.md).
3. The response carries a ULID message ID usable in logs and support
   requests.
4. A consumer calls `ReceiveMessage` and receives the message with a receipt
   handle; the message enters `InFlight` under the 60 s visibility timeout.
5. The consumer processes the message and calls `DeleteMessage` with the
   handle; the message is permanently removed.
6. A second `DeleteMessage` with the same handle succeeds idempotently; a
   `DeleteMessage` with a corrupted handle fails with `RECEIPT_INVALID`.
7. `relayctl queue describe orders` shows configuration and approximate
   counts labeled with their staleness.

### 6.2 Visibility-timeout recovery after a consumer crash

1. A consumer receives a message and begins processing under the default
   30 s visibility timeout.
2. The consumer process is killed before deleting the message. No cleanup
   code runs; the broker learns nothing from the crash itself.
3. Not before the visibility timeout elapses, the broker returns the message
   to `Available` and increments its receive count (NG-04: "not before",
   never "exactly at").
4. A healthy consumer receives the same message with a new receipt handle
   carrying a new lease epoch. The dead consumer's stale handle is now
   rejected with `RECEIPT_SUPERSEDED` if replayed.
5. If a slow-but-alive consumer needs more time, it calls
   `ChangeMessageVisibility` to extend the lease before expiry; if it
   decides to give up, it sets visibility to 0 and the message returns to
   `Available` immediately.
6. Throughout, no second consumer ever holds a live lease on the same
   message (P-02 in [./CORRECTNESS.md](./CORRECTNESS.md)).

### 6.3 Long polling

1. A consumer calls `ReceiveMessage` with `WaitTimeSeconds: 20` on an empty
   queue.
2. The server holds the request without spinning and without blocking other
   requests on the same connection.
3. A producer sends a message. The held request completes with that message
   as soon as it is durably acknowledged and available.
4. If nothing arrives within 20 s, the consumer receives an empty success
   and immediately re-polls; the idle cost is one round trip per 20 s per
   consumer, not a busy loop.
5. The consumer's error handling never conflates an empty poll with a
   failure.

### 6.4 FIFO ordered processing with message groups

1. The engineer creates `payments.fifo`; the `.fifo` suffix selects FIFO
   semantics, and sends without a `MessageGroupId` fail with
   `INVALID_ARGUMENT`.
2. The application sends events for account A under group `acct-A` and
   account B under group `acct-B`.
3. Two consumers poll concurrently. One receives the head message of
   `acct-A`, the other the head of `acct-B`; distinct groups flow in
   parallel.
4. While the `acct-A` message is in flight, no later `acct-A` message is
   deliverable to anyone; `acct-B` is unaffected.
5. When the `acct-A` message is deleted, the next `acct-A` message becomes
   deliverable. Within each group, delivery order equals acknowledged send
   order (P-04), including across redeliveries after visibility expiry.

### 6.5 Deduplication window

1. The engineer enables content-based deduplication on `payments.fifo`.
2. A producer sends a message, receives an ack with message ID M, then
   crashes before recording the ack and retries the identical send.
3. The retry lands inside the 300 s deduplication window; the broker
   returns message ID M again and enqueues nothing (P-05). The consumer
   sees one message.
4. A different producer sends a byte-identical body but sets an explicit
   `MessageDeduplicationId`; the explicit ID overrides content hashing, so
   deduplication is governed by that ID alone.
5. At exactly the window boundary the behavior is exact, not approximate:
   a duplicate at 299.999 s is suppressed; the same send at 300.001 s is a
   new message. Boundary behavior is enforced by named boundary tests, not
   left to interpretation.

### 6.6 DLQ exhaustion and redrive back

1. The engineer attaches a redrive policy to `orders`: DLQ `orders-dlq`,
   `maxReceiveCount: 5`.
2. A poison message fails processing repeatedly; each visibility expiry
   increments its receive count.
3. When the receive count exceeds 5, the broker moves the message to
   `orders-dlq` automatically. The moved message preserves its body and
   attributes byte-identically and records the source queue, final receive
   count, and move time.
4. The on-call engineer inspects the DLQ, ships a fix, and runs
   `relayctl queue redrive start orders-dlq`.
5. `relayctl queue redrive status` reports moved, remaining, and failed
   counts until the task completes; a second concurrent redrive on the same
   DLQ is rejected with `REDRIVE_IN_PROGRESS`.
6. Redriven messages return to `orders` as ordinary available messages with
   reset receive counts; for FIFO queues, group ordering guarantees are
   preserved through both the DLQ move and the redrive back.

### 6.7 Topic fanout with filter policies

1. The engineer creates topic `order-events` and subscribes three queues:
   `billing` with filter `{"type": ["order.paid"]}`, `shipping` with
   `{"type": ["order.paid"], "weight_kg": [{"numeric": [">", 20]}]}`, and
   `audit` with no filter.
2. An invalid fourth filter policy is rejected at subscribe time with
   field-level errors; nothing is partially subscribed.
3. A publish with attributes `type=order.paid, weight_kg=25` delivers
   independent copies to all three queues; a publish with
   `type=order.cancelled` reaches only `audit`.
4. Each delivered copy is an ordinary queue message with its own lifecycle;
   fanout is per-subscription and never atomic across queues (NG-02).
5. Unsubscribing `shipping` stops its future deliveries without touching
   copies already delivered; deleting the topic removes remaining
   subscriptions while every queue and its messages survive intact.

### 6.8 Operator installs and boots a 3-node cluster

1. The operator places the `relayd` static binary on three hosts, writes
   `/etc/relay/relay.toml` on each with node identity, peer addresses, data
   directory, and TLS material, and starts the service.
2. Startup validation is fail-fast: a bad configuration exits nonzero with
   field-level errors before binding any port; the data directory is
   created with 0700 permissions and refused if wider.
3. The nodes elect a leader; `relayctl cluster members` shows all three
   members, their roles, and health.
4. Readiness on port 7415 reports ready only when the node is a member of a
   quorum-holding cluster; the load balancer admits traffic accordingly.
5. The operator scrapes Prometheus metrics, confirms the dashboard from the
   runbook renders, and creates the first production queue.

### 6.9 Leader failover during traffic

1. A 3-node cluster serves steady traffic; clients are connected across all
   nodes.
2. The leader host loses power. Followers detect heartbeat loss and, after
   the randomized election timeout, elect a new leader using pre-vote.
3. During the window, writes fail fast with `UNAVAILABLE` or `NOT_LEADER`
   with a leader hint; clients retry per the taxonomy in §5.4 and converge
   on the new leader.
4. Every send acknowledged before the failure remains present after it
   (P-09); every lease decision remains exclusive — the partition cannot
   mint a double-lease (P-08).
5. The old leader restarts, rejoins as a follower, truncates any
   uncommitted tail, and catches up from the log or a snapshot.
6. The operator reads the incident from metrics and structured logs:
   election events, term changes, and the unavailability window are all
   directly observable.

### 6.10 Backup and restore

1. A scheduled job runs `relayctl backup create --dest /backups/relay`,
   producing a consistent snapshot plus archived WAL segments; the backup
   is verified by checksum at creation time.
2. The primary datacenter's storage is lost.
3. The operator provisions fresh hosts and runs the scripted restore drill
   from the runbook: `relayctl backup restore` rebuilds the data directory,
   and `relayd` recovers to the exact state captured by the backup.
4. The restore report states precisely what was recovered — every message
   acknowledged before the backup point — and what was not: messages
   acknowledged after it. The recovery point is stated, not implied.
5. This drill is rehearsed by an automated scripted disaster-recovery test
   (FR-OPS-007); a restore path that has never been exercised is treated as
   nonexistent.

### 6.11 Reproduce a bug from a simulation seed

1. A nightly simulation run reports a violation: under seed `0x9C41F2A7`,
   a partition during snapshot install produced a lease-exclusivity
   violation at virtual time 84,213 ms.
2. The engineer runs the simulator with that seed on a laptop. The failure
   reproduces identically — same schedule, same fault timing, same
   violating history — because every source of nondeterminism (clock,
   network, disk, randomness) is virtualized and seeded.
3. The checker emits the violating JSONL operation history and the minimal
   property it breaks (P-02), pointing at the exact operations involved.
4. The engineer fixes the bug, re-runs the seed to green, and commits the
   seed to the failing-seed corpus, where CI replays it forever
   (NFR-MAINT-002).
5. No step in this flow involves timing luck, retries, or a live cluster.

### 6.12 Benchmark run and claims audit

1. A release candidate is benchmarked on the reference hardware named in
   [./BENCHMARK_PLAN.md](./BENCHMARK_PLAN.md) (8 vCPU / 16 GiB / local
   NVMe, Linux 6.x) with pinned workload definitions.
2. The harness measures sustained send+receive+delete throughput, p99
   send-to-ack latency under fsync-before-ack, long-poll wakeup latency,
   crash-recovery time, and failover time, each with statistical treatment.
3. Results are published with hardware, workload, duration, and percentile
   provenance; no number appears anywhere without them (NFR-PERF-005).
4. The marketing claims audit walks every public statement — site copy,
   README badges, comparison table — and binds each to a verified property,
   a named non-guarantee, or a benchmark ID (FR-MKT-002, FR-MKT-004).
5. A claim that cannot be bound is deleted, not softened. The audit
   checklist result is release-gating evidence, archived with the release.

## 7. Functional Requirements

Every requirement below is `planned`. Each carries the terminal gate at
which its evidence completes; earlier gates may begin it, and the
requirement is accepted only when its terminal gate passes
([./BUILD_PLAN.md](./BUILD_PLAN.md) §16 lists the identical mapping). All
limits are normative and appear verbatim wherever they are cited.

### 7.1 Core queue semantics (FR-QUEUE)

- `FR-QUEUE-001`: `CreateQueue` creates an empty standard queue only after
  validating the complete configuration: name against
  `^[A-Za-z0-9_-]{1,80}$`, visibility timeout within 0 s–12 h, retention
  within 60 s–14 d, default delay within 0–900 s, and any redrive policy
  against an existing DLQ. Any invalid field fails the whole operation with
  field-level errors and creates no partial state; re-creating an existing
  name with a different configuration fails with `QUEUE_ALREADY_EXISTS`.
  Terminal gate: R1.
- `FR-QUEUE-002`: `SendMessage` returns its acknowledgment only after the
  durability contract of
  [./decisions/ADR-0008-fsync-before-ack-durability-contract.md](./decisions/ADR-0008-fsync-before-ack-durability-contract.md)
  is satisfied: the record and its fsync are complete, within the adaptive
  group-commit window capped at 2 ms. A crash at any instant after the ack
  loses no acknowledged message. Terminal gate: R2.
- `FR-QUEUE-003`: `SendMessageBatch` accepts up to 10 entries, validates and
  applies each independently, and returns per-entry results; one invalid
  entry never fails a valid sibling, and a batch is never a transaction.
  Batches over 10 entries fail whole with `INVALID_ARGUMENT`. Terminal
  gate: R1.
- `FR-QUEUE-004`: `ReceiveMessage` leases up to 10 messages, each under the
  request's visibility timeout (0 s–12 h) or the queue default (30 s), and
  returns one opaque receipt handle per message. Leased messages are in
  `InFlight` and invisible to other consumers for the lease duration.
  Terminal gate: R1.
- `FR-QUEUE-005`: When a lease's visibility timeout expires without a
  delete, the message returns to the `Available` set not before the timeout
  instant, and its receive count increments exactly once per completed
  delivery. Terminal gate: R1.
- `FR-QUEUE-006`: `DeleteMessage` with a valid receipt handle permanently
  removes the message. Repeating the delete with the same handle succeeds
  idempotently and changes nothing (P-06). Terminal gate: R1.
- `FR-QUEUE-007`: A receipt handle that is expired, superseded by a newer
  delivery, or issued for another queue or message is rejected with the
  matching `RECEIPT_*` code. Handles are single-use per delivery: each
  delivery increments the lease epoch, and delete and visibility-change
  validate epoch equality. Terminal gate: R1.
- `FR-QUEUE-008`: `ChangeMessageVisibility` extends or shortens an active
  lease to any value within 0 s–12 h measured from the request; a value of
  0 returns the message to `Available` immediately. It fails on inactive
  leases with the matching `RECEIPT_*` code. Terminal gate: R1.
- `FR-QUEUE-009`: `ReceiveMessage` honors `WaitTimeSeconds` from 0 to 20 s:
  the server holds the request until a message is available or the wait
  expires, returns early on arrival, and returns an empty success on
  timeout. Held requests consume no busy-wait CPU. Terminal gate: R6.
- `FR-QUEUE-010`: A per-message `DelaySeconds` of 0–900 s keeps the message
  in `Delayed` until the delay elapses; it is not receivable, not counted
  in flight, and first becomes available not before the delay expires.
  Terminal gate: R4.
- `FR-QUEUE-011`: A per-queue default delay (0–900 s) applies exactly when a
  message omits its own `DelaySeconds`; an explicit per-message value,
  including 0, overrides the queue default. Terminal gate: R4.
- `FR-QUEUE-012`: A message carries at most 10 typed attributes of type
  String, Number, or Binary; attribute names are validated, duplicate names
  are rejected, and attributes are preserved byte-identically through
  delivery, DLQ moves, and redrive. Terminal gate: R1.
- `FR-QUEUE-013`: Message bodies are limited to 256 KiB. An oversized body
  fails with the stable `MESSAGE_TOO_LARGE` error before any state change,
  and no truncated variant is ever stored. Terminal gate: R1.
- `FR-QUEUE-014`: Each queue has a retention period of 60 s–14 d (default
  4 d). A message older than retention is removed from any lifecycle state,
  and removal is observable in metrics; expiry is "not before" the
  boundary. Terminal gate: R4.
- `FR-QUEUE-015`: `PurgeQueue` removes every message in the queue including
  in-flight ones and invalidates their receipt handles. A purge requested
  while another purge on the same queue is active fails with
  `PURGE_IN_PROGRESS`. Terminal gate: R1.
- `FR-QUEUE-016`: Each queue enforces an in-flight cap of 120,000 messages
  (standard) or 20,000 (FIFO). A `ReceiveMessage` that would exceed the cap
  fails with the stable `INFLIGHT_LIMIT_EXCEEDED` backpressure error rather
  than degrading service. Terminal gate: R1.
- `FR-QUEUE-017`: A queue with a redrive policy names a DLQ and a
  `maxReceiveCount` between 1 and 1,000. When a message's receive count
  exceeds the limit, the broker moves it to the DLQ automatically, with no
  consumer action required. Terminal gate: R4.
- `FR-QUEUE-018`: A dead-lettered message preserves its body and attributes
  byte-identically and additionally records its source queue, final receive
  count, and move time as queryable metadata. Terminal gate: R4.
- `FR-QUEUE-019`: `StartRedriveTask` moves messages from a DLQ back to its
  source queue asynchronously, reports progress (moved, remaining, failed)
  through `DescribeRedriveTask`, survives leader changes, and permits only
  one active task per DLQ. Terminal gate: R4.

### 7.2 FIFO queues (FR-FIFO)

- `FR-FIFO-001`: FIFO queues are selected by the `.fifo` name suffix
  (excluded from the 80-character name budget), and every send to a FIFO
  queue requires a `MessageGroupId` of at most 128 bytes; sends without one
  fail with `INVALID_ARGUMENT`. Terminal gate: R4.
- `FR-FIFO-002`: Within one message group, delivery order equals
  acknowledged send order (P-04). This holds across redeliveries: a message
  returned by visibility expiry is redelivered before any later message of
  its group. Terminal gate: R4.
- `FR-FIFO-003`: Distinct message groups are deliverable in parallel to
  distinct consumers; one group's backlog or in-flight state never blocks
  another group. Terminal gate: R4.
- `FR-FIFO-004`: While any message of a group is in flight, no later
  message of that group is deliverable to any consumer until the in-flight
  message is deleted or its visibility expires. Terminal gate: R4.
- `FR-FIFO-005`: When content-based deduplication is enabled, the
  deduplication key is the SHA-256 of the message body; two sends with
  byte-identical bodies inside the window deduplicate regardless of
  attribute differences. Terminal gate: R4.
- `FR-FIFO-006`: An explicit `MessageDeduplicationId` overrides
  content-based deduplication entirely: only the explicit ID participates
  in window matching for that send. Terminal gate: R4.
- `FR-FIFO-007`: The deduplication window is fixed at 300 s and is honored
  exactly at both boundaries (P-05). A duplicate send inside the window is
  acknowledged with the original message ID and enqueues nothing; the same
  send after the window creates a new message. Terminal gate: R4.
- `FR-FIFO-008`: FIFO ordering guarantees are preserved through DLQ move
  and redrive back: relative order within a group is maintained among
  messages moved to the DLQ and among messages redriven to the source.
  Terminal gate: R4.

### 7.3 Topics and fanout (FR-TOPIC)

- `FR-TOPIC-001`: `CreateTopic` validates the topic name against
  `^[A-Za-z0-9_-]{1,80}$` and the full configuration before creating the
  topic; failures produce field-level errors and no partial state.
  Terminal gate: R5.
- `FR-TOPIC-002`: `Subscribe` binds an existing queue to a topic with an
  optional filter policy that is validated and immutably recorded at
  subscribe time; the effective policy of a subscription never changes
  without an explicit re-subscribe. Terminal gate: R5.
- `FR-TOPIC-003`: `Publish` delivers an independent copy of the message to
  every subscription whose filter matches. Delivery is per-subscription and
  never atomic across queues (NG-02): one queue's failure or backpressure
  does not retract another queue's copy. Terminal gate: R5.
- `FR-TOPIC-004`: Filter policies match against message attributes with
  exactly these operators: exact value, anything-but, string prefix,
  numeric range, and attribute-exists. Multiple keys conjoin; multiple
  values per key disjoin. Terminal gate: R5.
- `FR-TOPIC-005`: An invalid filter policy — unknown operator, malformed
  numeric range, excessive size, or wrong structure — is rejected at
  subscribe time with `FILTER_POLICY_INVALID` and field-level errors,
  never at publish time. Terminal gate: R5.
- `FR-TOPIC-006`: `Unsubscribe` stops all future deliveries for that
  subscription without affecting copies already delivered to the queue.
  Terminal gate: R5.
- `FR-TOPIC-007`: `DeleteTopic` removes the topic and all of its
  subscriptions; subscribed queues and every message already in them are
  unaffected. Terminal gate: R5.
- `FR-TOPIC-008`: Fanout into a FIFO queue preserves FIFO semantics: the
  published message's group ID and deduplication behavior apply exactly as
  they would for a direct send to that queue. Terminal gate: R5.

### 7.4 Wire API (FR-API)

- `FR-API-001`: RWP/1 is a framed binary protocol; every frame carries the
  magic `RWP1`, a little-endian length (maximum 1 MiB), a CRC32C, an
  opcode, flags, and a request ID, with per-opcode fixed body layouts.
  Frames failing any structural check are rejected with `MALFORMED_FRAME`
  before any dispatch. Terminal gate: R6.
- `FR-API-002`: The parser is bounded: every declared length is checked
  against its limit before any allocation, and no input can cause
  allocation, recursion, or time proportional to attacker-chosen values
  beyond fixed bounds. Fuzz targets for the codec run with a checked-in
  corpus that gates CI. Terminal gate: R6.
- `FR-API-003`: Every frame is authenticated with the per-tenant HMAC
  scheme; unauthenticated or wrongly authenticated frames fail with
  `UNAUTHENTICATED` and are never partially processed. Terminal gate: R6.
- `FR-API-004`: Authorization evaluates per-queue and per-topic ACLs for
  every operation with deny precedence: an applicable deny rule defeats
  any allow. Denials return `ACCESS_DENIED` without leaking resource
  existence details beyond the ACL's own visibility. Terminal gate: R6.
- `FR-API-005`: Per-tenant quotas and rate limits are enforced on every
  operation; exceeding them fails with the stable `THROTTLED` error
  carrying a retry-after hint. Throttling is per-tenant and never degrades
  other tenants' admitted traffic. Terminal gate: R6.
- `FR-API-006`: Every failure surfaced on the wire maps to exactly one
  stable machine-readable code from the taxonomy summarized in §5.4;
  codes are append-only across versions, and no failure path returns
  free-text-only errors. Terminal gate: R6.
- `FR-API-007`: A long poll held on a connection never blocks unrelated
  requests multiplexed on that connection; responses correlate by request
  ID and may complete out of request order. Terminal gate: R6.
- `FR-API-008`: Transport is TLS 1.3. Plaintext operation is permitted only
  when explicitly configured for loopback addresses; a plaintext listener
  on a non-loopback address is a fail-fast startup error. Terminal gate:
  R6.
- `FR-API-009`: Connections negotiate the protocol version before any
  state-changing operation; an unknown or unsupported version is rejected
  with `UNSUPPORTED_VERSION` and no state change. Terminal gate: R6.
- `FR-API-010`: Wire limits are enforced: 1 MiB maximum frame, a bounded
  number of in-flight requests per connection, and read and write
  deadlines that defeat slowloris-style slow clients. Violations close or
  reject deterministically. Terminal gate: R6.

### 7.5 Replication (FR-REPL)

- `FR-REPL-001`: Leader election uses Raft with pre-vote enabled and
  randomized election timeouts (500–1000 ms against a 100 ms heartbeat, in
  simulated and configured real time), preventing disruptive candidacies
  from partitioned nodes. Terminal gate: R7.
- `FR-REPL-002`: A log entry is committed only after a majority of the
  cluster has durably appended it — each member's append satisfies the
  same fsync discipline as single-node writes. Terminal gate: R7.
- `FR-REPL-003`: An acknowledged write survives leader failover: any send
  acked to a client is present after any sequence of leader changes
  (P-09). Acks are issued only for committed entries. Terminal gate: R7.
- `FR-REPL-004`: Lease grants are linearized through the replicated log:
  the decision to grant, extend, or expire a lease is a log entry, so no
  two consumers can hold a live lease on one message across any partition
  (P-08). Terminal gate: R7.
- `FR-REPL-005`: Snapshot install brings a lagging or new follower to a
  verified state: snapshots transfer in 1 MiB chunks with per-chunk CRCs
  and a footer hash over the full state, and a follower serves nothing
  until verification passes. Terminal gate: R7.
- `FR-REPL-006`: Membership changes are single-server at a time; a change
  commits through the log, and a failure mid-change leaves the cluster in
  a safe, recoverable configuration. Terminal gate: R7.
- `FR-REPL-007`: A non-leader rejects writes with `NOT_LEADER` and a
  leader hint; `relay-client` follows the hint transparently and bounds
  redirect loops. Terminal gate: R7.
- `FR-REPL-008`: Linearizable reads are served via ReadIndex: a read
  reflects every write committed before it began, without writing a no-op
  entry per read. Terminal gate: R7.
- `FR-REPL-009`: A mixed-version cluster within the one-minor-version
  rolling-upgrade window operates correctly for all replicated semantics;
  behavior outside the window is refused at join time, not discovered by
  corruption. Terminal gate: R10.

### 7.6 Administration (FR-ADMIN)

- `FR-ADMIN-001`: `DescribeQueue` and `DescribeTopic` return the full
  effective configuration plus approximate counts (available, in-flight,
  delayed for queues; subscription count and matched-delivery counters for
  topics), each labeled with its staleness bound. Approximation is stated,
  never silently presented as exact. Terminal gate: R6.
- `FR-ADMIN-002`: `ListQueues` and `ListTopics` support a name-prefix
  filter and cursor pagination with a stable order; a cursor remains valid
  across leader changes or fails cleanly with a restartable error.
  Terminal gate: R6.
- `FR-ADMIN-003`: Queues and topics can be tagged and untagged with
  validated key-value pairs, and `ListResourcesByTag` enumerates resources
  by tag with cursor pagination. Terminal gate: R6.
- `FR-ADMIN-004`: `SetQueueAttributes` validates the requested delta
  against the same rules as `CreateQueue` and applies it with a bounded,
  documented propagation: existing leases keep their granted timeout, and
  new operations observe the new configuration within a stated bound.
  Terminal gate: R6.
- `FR-ADMIN-005`: `DeleteQueue` is terminal: all receipt handles for the
  queue become invalid, all subscriptions delivering to it are removed,
  and its storage is freed. A deleted queue name may be recreated as a
  fresh queue with no inherited state. Terminal gate: R6.
- `FR-ADMIN-006`: `relayctl` covers every administrative operation in §5.1
  with both human-readable and stable JSON output; the JSON schemas are
  versioned and tested. No administrative capability exists only through
  raw wire calls. Terminal gate: R8.
- `FR-ADMIN-007`: Cluster administration exposes the member list with
  roles and health, per-node health detail, and an orderly leadership
  transfer to a named member that completes or fails without an
  availability gap longer than an election. Terminal gate: R8.
- `FR-ADMIN-008`: Every administrative mutation — create, delete,
  attribute change, purge, redrive, tag, subscription change, membership
  change, leadership transfer — is recorded in an append-only audit log
  with principal, operation, parameters, result, and time. Terminal gate:
  R8.

### 7.7 Operations (FR-OPS)

- `FR-OPS-001`: `relayd` ships as one static binary per supported platform
  with embedded version, commit, build time, and build provenance,
  reported by `relayd --version` and over the admin surface. Terminal
  gate: R10.
- `FR-OPS-002`: Configuration merges the TOML file, environment variables,
  and command-line flags with a fixed, documented precedence (flags over
  environment over file over defaults). Startup validation is fail-fast:
  any invalid value exits nonzero with field-level errors before binding
  any port. Terminal gate: R6.
- `FR-OPS-003`: Health and readiness endpoints are served on port 7415.
  Liveness reflects the process; readiness reflects Raft membership and
  leadership knowledge, so an isolated node reports not-ready rather than
  silently serving stale reads. Terminal gate: R8.
- `FR-OPS-004`: Prometheus metrics cover queue depths, in-flight counts,
  lease expirations, DLQ moves, throughput, latency histograms, WAL and
  fsync health, Raft state, and quota rejections, all within a named
  cardinality budget owned by
  [./OPERATIONS_TEST_PLAN.md](./OPERATIONS_TEST_PLAN.md); exceeding the
  budget is a release blocker. Terminal gate: R8.
- `FR-OPS-005`: OTLP trace spans cover the request lifecycle from frame
  decode through log append, commit, apply, and response, with span
  attributes sufficient to correlate a client request to its log entry.
  Terminal gate: R8.
- `FR-OPS-006`: Logs are structured JSON with stable field conventions
  (timestamp, level, event, tenant, queue, request ID, node, term) and are
  machine-parseable without regexes over prose. Terminal gate: R8.
- `FR-OPS-007`: Backup produces a consistent snapshot plus WAL archive;
  restore rebuilds a node or cluster to the captured state. The procedure
  is rehearsed by a scripted disaster-recovery drill that runs in CI
  against generated data and gates release. Terminal gate: R10.
- `FR-OPS-008`: Rolling upgrade and rollback procedures are documented and
  tested within the one-minor-version mixed-version window; rollback from
  a failed upgrade restores service without data loss for acknowledged
  writes. Terminal gate: R10.
- `FR-OPS-009`: An uninstall and purge path removes every artifact Relay
  created — binaries, data directory, configuration, and archives it
  created — with a dry-run inventory before deletion. Terminal gate: R10.
- `FR-OPS-010`: `relayctl diagnose` produces a support bundle with an
  exact content inventory and redaction of secrets, credentials, and
  message bodies; redaction is verified by canary tests. Terminal gate:
  R8.
- `FR-OPS-011`: A documented capacity model relates message rate, message
  size, retention, and replication factor to disk, memory, and network
  requirements, with worked examples validated against benchmark data.
  Terminal gate: R9.
- `FR-OPS-012`: An incident-response and release-revocation procedure
  names roles, communication channels, severity levels, and the exact
  mechanics of pulling a published release and notifying operators.
  Terminal gate: R10.

### 7.8 Marketing (FR-MKT)

The FR-MKT namespace is a deliberate extension of the engineering
requirement register: launch collateral is specified, gated, and audited
like code, because the product's thesis is that claims never outrun
evidence. [./MARKETING.md](./MARKETING.md) owns the collateral itself.

- `FR-MKT-001`: The positioning and messaging document derives every
  pillar from verified guarantees in
  [./CORRECTNESS.md](./CORRECTNESS.md): each pillar cites the P-xx
  properties or NG-xx non-guarantees it rests on, and a pillar with no
  citation does not ship. Terminal gate: R9.
- `FR-MKT-002`: Every public performance claim cites a specific
  [./BENCHMARK_PLAN.md](./BENCHMARK_PLAN.md) result including hardware,
  workload, and statistical treatment; rounded or paraphrased numbers
  must remain traceable to the published result. Terminal gate: R9.
- `FR-MKT-003`: Wherever marketing copy could allow a reader to infer
  exactly-once delivery, the copy states the at-least-once model and the
  relevant non-guarantees (NG-01 and NG-09) explicitly and adjacently.
  Terminal gate: R9.
- `FR-MKT-004`: Launch collateral — site copy, README badges, and the
  comparison table — is reviewed against the claims audit before
  publication; the comparison table states only externally verifiable
  facts about other systems and links its sources. Terminal gate: R10.
- `FR-MKT-005`: A claims-audit checklist (the MKT- test family in
  [./OPERATIONS_TEST_PLAN.md](./OPERATIONS_TEST_PLAN.md)) gates every
  release announcement; an announcement without a completed audit is a
  release-process violation, not an editorial choice. Terminal gate: R10.

## 8. Failure UX

Every failure a user or operator can observe contains:

1. exactly one stable machine-readable code from the taxonomy in §5.4;
2. a retryability class (retryable with backoff, retryable after hint,
   retryable at hinted leader, not retryable);
3. whether server state may have changed, whenever the operation could have
   partially applied;
4. field-level detail for validation failures;
5. enough correlation identity (request ID) to find the server-side record.

### 8.1 Stable codes and retryability

Codes are append-only across releases: a code, once published, never changes
meaning or retryability class. Clients branch on codes and classes, never on
message text. `relay-client` retries automatically only within the declared
class rules and never retries a non-retryable failure; a send whose ack was
lost is surfaced as uncertain, not silently retried into a duplicate the
application cannot anticipate (the at-least-once model makes retry safe, and
the client says when it retried).

### 8.2 Throttle behavior

`THROTTLED` responses carry a retry-after hint derived from the tenant's
actual quota state. Throttling is admission control at the boundary: an
admitted request is served normally, and a throttled request consumes
bounded work. Sustained throttling is visible to the operator as a
per-tenant metric before customers report it.

### 8.3 Not-leader redirects

`NOT_LEADER` carries the best-known leader hint. `relay-client` follows
hints transparently with a bounded redirect budget; when the budget is
exhausted or no hint is available it surfaces `UNAVAILABLE` with backoff
guidance. During elections, clients experience a bounded window of fast
failures, never hangs: every request has a deadline, and the server never
holds a write hoping to become leader.

### 8.4 Backpressure UX

When a queue reaches its in-flight cap (120,000 standard / 20,000 FIFO),
receives fail with `INFLIGHT_LIMIT_EXCEEDED`; when the node is overloaded,
admission control sheds load with `THROTTLED` or `UNAVAILABLE` under the
bounded-backpressure guarantee (NFR-AVAIL-003). Backpressure is always an
explicit error with a class, never a silently growing latency cliff, and
every shed is counted in metrics the runbook explains.

### 8.5 Crash-recovery operator experience

After any crash, including `kill -9` and power loss, the operator restarts
`relayd` and the normal startup path performs recovery: the WAL replays to
the exact pre-crash acknowledged state, torn tail records are detected by
CRC and truncated only at the tail, and startup logs state the recovered
LSN, replay duration, and any truncation performed. Recovery requires no
flags, no fsck-style tooling, and no interpretation; if recovery cannot
complete safely — corruption beyond the tail, missing segments — `relayd`
refuses to start and names the failing artifact rather than guessing. An
fsync failure at runtime crashes the process by design; the runbook entry
for that crash explains why restarting on the same disk requires attention.

## 9. Non-Functional Requirements

Every requirement below is `planned` and carries its terminal gate.

### 9.1 Durability (NFR-DUR)

- `NFR-DUR-001`: An acknowledged send is durable: the WAL record and its
  fsync complete before the ack is emitted, and a crash at any instant
  after the ack loses no acknowledged message (P-01). Terminal gate: R2.
- `NFR-DUR-002`: Crash recovery replays the WAL to exactly the pre-crash
  acknowledged state: no acknowledged message missing, no unacknowledged
  message invented, all lease and queue metadata consistent. Terminal
  gate: R2.
- `NFR-DUR-003`: Torn and partial writes are detected by CRC on every
  record, and truncation is applied only at the log tail; a mid-log CRC
  failure is corruption and refuses startup rather than truncating live
  data. Terminal gate: R2.
- `NFR-DUR-004`: Disk-full conditions fail in-progress writes cleanly with
  no corruption of existing data, and reads continue to serve everything
  already durable. Terminal gate: R2.
- `NFR-DUR-005`: An fsync failure is fatal: the process aborts immediately
  rather than retrying, because a failed fsync leaves page-cache state
  unknowable (the fsyncgate rule, per
  [./decisions/ADR-0008-fsync-before-ack-durability-contract.md](./decisions/ADR-0008-fsync-before-ack-durability-contract.md)).
  Terminal gate: R2.
- `NFR-DUR-006`: Compaction never removes live data: every compaction run
  is checked against the live set, and reclaimed space is verified by
  post-compaction accounting. Terminal gate: R2.
- `NFR-DUR-007`: Every on-disk format (WAL segment, snapshot,
  configuration) is versioned; format migrations ship with fixtures
  generated by the old version and a stated downgrade policy. Terminal
  gate: R10.

### 9.2 Performance (NFR-PERF)

All performance targets are measured on the reference hardware — 8 vCPU /
16 GiB / local NVMe, Linux 6.x — under the workload definitions in
[./BENCHMARK_PLAN.md](./BENCHMARK_PLAN.md). Targets are release-gating
measurements, not delivery contracts (NG-05).

- `NFR-PERF-001`: A single node sustains at least 20,000 msg/s of combined
  send+receive+delete traffic at 256-byte bodies under the full durability
  contract. Terminal gate: R9.
- `NFR-PERF-002`: p99 send-to-ack latency is at most 15 ms with
  fsync-before-ack enabled at the sustained-throughput operating point.
  Terminal gate: R9.
- `NFR-PERF-003`: Long-poll wakeup completes within 10 ms of a matching
  send becoming durable, at p99 under the benchmark workload. Terminal
  gate: R9.
- `NFR-PERF-004`: Crash recovery of a 10 GiB WAL completes within 30 s on
  the reference hardware, measured from process start to ready. Terminal
  gate: R9.
- `NFR-PERF-005`: Every published number carries its hardware, workload,
  run duration, and statistical treatment (percentile provenance, run
  count, variance); a number without them may not be published anywhere,
  including the README. Terminal gate: R9.

### 9.3 Availability (NFR-AVAIL)

- `NFR-AVAIL-001`: A 3-node cluster continues to serve reads and writes
  with any one node down, with no operator action required. Terminal
  gate: R7.
- `NFR-AVAIL-002`: From a clean leader kill to the first newly acknowledged
  write is at most 5 s — established in simulation at R7 and measured on
  real hardware at R9. Terminal gate: R9.
- `NFR-AVAIL-003`: Overload produces bounded backpressure and load
  shedding with stable errors, never collapse: memory stays within
  configured bounds, admitted work completes, and recovery after load
  subsides is automatic. Terminal gate: R6.
- `NFR-AVAIL-004`: Graceful shutdown drains in-flight requests within a
  bounded window, closes connections cleanly, and leaves state such that
  restart requires only ordinary recovery. Terminal gate: R6.

### 9.4 Security (NFR-SEC)

Security claims bind to their enforcement point and adversarial evidence in
[./THREAT_MODEL.md](./THREAT_MODEL.md).

- `NFR-SEC-001`: Receipt handles are unforgeable — HMAC-SHA256 over the
  handle contents with a per-cluster key — and single-use per delivery via
  the lease epoch (P-07); forging, replaying, or transplanting a handle is
  detected and rejected. Terminal gate: R6.
- `NFR-SEC-002`: All wire input is untrusted: every parser at the network
  boundary has fuzz targets, and the fuzz corpus (including every
  crash-finding input ever discovered) gates CI. Terminal gate: R6.
- `NFR-SEC-003`: Secrets — credentials, HMAC keys, TLS private keys —
  never appear in logs, traces, error messages, metrics, or diagnostic
  bundles; redaction is verified by canary secrets planted in tests and
  hunted across every output surface. Terminal gate: R6.
- `NFR-SEC-004`: Credential comparison is constant-time; no authentication
  path's timing depends on how much of a presented secret is correct.
  Terminal gate: R6.
- `NFR-SEC-005`: The data directory is created with and verified to hold
  0700 permissions at startup; wider permissions are a fail-fast startup
  error, not a warning. Terminal gate: R2.
- `NFR-SEC-006`: Denial-of-service bounds are enforced: per-connection
  memory caps, read and write deadlines, the 1 MiB frame limit, and
  configurable connection limits, each with a deterministic rejection
  behavior. Terminal gate: R6.
- `NFR-SEC-007`: The threat model is re-reviewed at every release gate;
  each review is recorded with findings and dispositions, and an
  unresolved critical finding blocks the gate. Terminal gate: R10.
- `NFR-SEC-008`: Dependencies are exact-pinned, reviewed at a named gate,
  and carried with provenance; an unreviewed dependency bump cannot reach
  mainline. Terminal gate: R10.

### 9.5 Maintainability and verification (NFR-MAINT)

- `NFR-MAINT-001`: Every parser, reducer, and state transition has its
  failing deterministic test written before its implementation; the
  discipline is audited at every gate, and its final audit completes at
  R10. Terminal gate: R10.
- `NFR-MAINT-002`: Any simulation failure reproduces exactly from its
  seed, and the failing-seed corpus is checked in and replayed by CI
  forever; a seed that stops reproducing is itself a bug. Terminal gate:
  R3.
- `NFR-MAINT-003`: Mutation testing on `relay-core` kills at least 85% of
  generated mutants; surviving mutants are individually reviewed and
  either killed or justified in writing. Terminal gate: R4.
- `NFR-MAINT-004`: A green CI run replays the accepted evidence of every
  prior gate — no gate's evidence is ever retired, and regressions in old
  evidence block merges exactly like new failures. Terminal gate: R10.
- `NFR-MAINT-005`: Documentation discipline holds at every release:
  statuses use the fixed vocabulary, no claim outruns its named gate, and
  every reversal of a recorded decision is a new ADR. Terminal gate: R10.

## 10. Acceptance Criteria and Release Tiers

### 10.1 Gates and the acceptance rule

Implementation proceeds through the gates fixed in
[./BUILD_PLAN.md](./BUILD_PLAN.md) §5–§15:

| Gate | Evidence unlocked |
| --- | --- |
| R0 | Repository, Rust toolchain, CI, and architecture checks exist and are green. |
| R1 | Single-node in-memory core queue semantics are correct under the model checker. |
| R2 | The durable WAL storage engine survives crash, torn-write, and disk-full injection. |
| R3 | Deterministic simulation reproduces any failure from a seed and runs in CI with a checked-in corpus. |
| R4 | FIFO groups, deduplication, delay, DLQ, and redrive behave exactly to specification. |
| R5 | Topics, subscriptions, and filter policies fan out correctly. |
| R6 | A bounded, fuzzed wire API with authentication, quotas, and long polling serves real clients. |
| R7 | Raft replication survives partition and failover with no double-lease and no lost ack. |
| R8 | Metrics, tracing, the admin surface, and a runbook make Relay operable. |
| R9 | Published benchmarks, a failure-injection report, and evidence-bound marketing support stated claims and no more. |
| R10 | Packaging, deployment, upgrade, rollback, and backup/restore satisfy the 1.0 release gate. |

A requirement is accepted only at its terminal gate. Earlier gates may
implement parts of it, and partial behavior may exist on mainline behind
earlier evidence, but the requirement's status remains `planned` or `in
progress` until the terminal gate's full evidence passes. No requirement is
accepted by proximity: passing R6 accepts the requirements whose terminal
gate is R6, and nothing else.

### 10.2 Release tiers

| Tier | Produced after | Claim it supports |
| --- | --- | --- |
| Internal milestone | each of R1–R5 | Engineering evidence checkpoints. Nothing is released, installable, or supported; simulation, model-checking, and feature-semantics evidence accumulates. |
| First usable single-node release | R6 | A single `relayd` node serves real clients over the authenticated wire API with full queue, FIFO, and topic semantics and the durability contract. No replication claim, no operability claim, no published numbers. |
| Replicated beta | R7 | A 3-node cluster survives partition and failover with no double-lease and no lost ack, proven in simulation. Operability tooling is incomplete; not production-supported. |
| Operable beta | R8 | Metrics, tracing, the complete `relayctl` surface, audit logging, and the runbook exist. Suitable for adventurous production use with no performance claims. |
| Evidence-published | R9 | Benchmarks, the failure-injection report, the capacity model, and evidence-bound positioning are public. Every public number carries its provenance. |
| Relay 1.0 | R10 | The full §4.1 surface: packaging, upgrade, rollback, backup/restore drill, threat-model closure, dependency review, and the claims-audited launch. |

Each tier is cumulative: a later tier ships only while every earlier tier's
evidence remains green (NFR-MAINT-004). No tier grants claims from a later
tier, and no marketing statement may reference a tier that has not been
reached.

## 11. Requirement-to-Evidence Rule

Every requirement ID in this document — the FR-QUEUE, FR-FIFO, FR-TOPIC,
FR-API, FR-REPL, FR-ADMIN, FR-OPS, FR-MKT, NFR-DUR, NFR-PERF, NFR-AVAIL,
NFR-SEC, and NFR-MAINT namespaces — appears exactly once in the
traceability matrix of [./BUILD_PLAN.md](./BUILD_PLAN.md) §16 with the same
terminal gate stated here, and maps there to:

1. an owning crate or component boundary;
2. one or more named automated tests in the families defined by
   [./OPERATIONS_TEST_PLAN.md](./OPERATIONS_TEST_PLAN.md);
3. a named user-visible failure path from §8;
4. its terminal gate;
5. documentation that distinguishes current support from planned support.

A feature is not complete because a type, parser, or stub exists, because a
happy-path unit test passes, or because the behavior worked once in a
manual run. It is complete when the end-to-end user flow, its failure
handling, its durability behavior, its security bounds, its tests, its
observability, and its documentation all meet the terminal gate — and when
the gate's evidence replays green in CI. Claims follow the same rule in the
other direction: no document, changelog entry, or launch material may state
a behavior whose requirement has not been accepted, and any statement found
to outrun its evidence is a defect with the same priority as the missing
evidence itself.
