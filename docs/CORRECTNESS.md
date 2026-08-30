# Relay: Correctness Model, Guarantees, and Verification Design

Document status: normative. This document controls every guarantee and
non-guarantee claim made anywhere in the Relay project (precedence tier 3 in
[docs/README.md](./README.md)). No other document, and no marketing artifact,
may state a guarantee stronger than what this document binds to a named test.

Last revised: 2026-08-30.

Companion documents:

- [Build plan and gates](./BUILD_PLAN.md)
- [Product requirements](./PRODUCT_REQUIREMENTS.md)
- [Architecture](./ARCHITECTURE.md)
- [Operations and test plan](./OPERATIONS_TEST_PLAN.md)
- [Glossary](./GLOSSARY.md)
- [ADR-0005: Injected time and the log-applied clock](./decisions/ADR-0005-injected-time-and-log-applied-clock.md)
- [ADR-0007: JSONL histories and the linearizability oracle](./decisions/ADR-0007-jsonl-histories-and-linearizability-oracle.md)
- [ADR-0008: fsync-before-ack durability contract](./decisions/ADR-0008-fsync-before-ack-durability-contract.md)

## 1. How to Read This Document

### 1.1 The binding rule

A Relay guarantee exists when, and only when, the named tests listed for it in
§9 pass in CI on mainline. Until then the guarantee is a design intention, and
this document says so explicitly. The sentence "Relay guarantees X" is
shorthand for "the named tests proving X pass in CI"; if the tests do not
exist or do not pass, the sentence is false and may not be written elsewhere.

### 1.2 Status vocabulary

Every property carries one of four statuses, with the same wording used in
every Relay document:

- **accepted**: implemented on mainline, backed by its named automated gate;
- **in progress**: present on a branch, not a claim;
- **planned**: specified, not implemented;
- **deferred**: outside the named phase; forbidden as completion evidence.

A package, type, stub, or happy-path unit test is never completion. Today
nothing is built: **every property P-01 through P-10 has status `planned`**,
and every row of the §9 mapping table says so. The non-guarantees in §3 are
not statuses; they are permanent design boundaries and hold at every status.

### 1.3 Reading order

§2 states what Relay promises. §3 states what it refuses to promise. §4
defines the reference model that is the ground truth for both. §5 and §6
define how executions are recorded and checked against that model. §7 and §8
define the workloads and the fault-injecting simulation that generate the
executions. §9 binds every property to its tests. §10 states what the whole
apparatus cannot prove.

### 1.4 Promotion rules

Evidence never promotes across boundaries: an in-memory result is not a
durability claim, a single-node result is not a replication claim, and a
simulated fault is not production hardening. Each property below names its
enforcement point and its earliest gate; a property is `accepted` only at
that gate or later, only with its named tests green.

## 2. The Guarantee List

Each property is stated three ways: a quantified English statement, a precise
predicate over recorded histories (§5 defines the history format), and the
architectural enforcement point. "Acked" always means the client observed a
successful return; "log-applied time" is the deterministic clock advanced
only by `AdvanceTime` log entries (ADR-0005).

### 2.1 P-01 DURABLE-ACK

**Statement.** Every acknowledged send survives any single-process crash. For
all messages m, if a `send` (or `send-batch` entry, or `publish` copy) for m
returned `ok` to the client, then after any crash and recovery m is present
in the recovered state unless a model-permitted consumption removed it
(delete with a valid receipt, purge, retention expiry, or dead-letter move).

**Predicate.** For every history H containing a crash marker c: for every
operation s in H with `call.type ∈ {send, send_batch, publish}` and
`return_ns < invoke_ns(c)` and `result.ok` carrying message id i, the
post-recovery suffix of H is linearizable against a model state that contains
i, unless H also contains a model-permitted consuming operation for i that
linearizes before the read that failed to observe i.

**Scope and preconditions.** Applies to acked operations only (see NG-09).
Applies per process crash (SIGKILL, power-cut simulation, torn tail write)
under the fault model of §8; it is not a claim about disk firmware lying
about flush (§10). Requires the ADR-0008 contract: the ack is emitted only
after the WAL record and its fsync complete.

**Enforcement point.** `crates/relay-wal` (`Wal::append` + `Wal::sync`
ordering before the server's ack path in `crates/relay-server`); recovery via
`Wal::recover`. At R7 the enforcement point additionally includes the Raft
commit rule (majority durable append before apply).

**Failure consequence if violated.** Silent data loss: a producer that
received an ack believes the message exists, and no retry will occur. This is
the single worst failure class for a queue; any P-01 counterexample is a
release blocker at every gate from R2 onward.

### 2.2 P-02 LEASE-EXCL

**Statement.** No two consumers hold a live lease on one message. For all
messages m and all pairs of deliveries d1 ≠ d2 of m, the lease intervals of
d1 and d2 do not overlap in log-applied time.

**Predicate.** For every history H and every message id i: order the
deliveries of i by lease epoch. For consecutive deliveries with epochs e and
e+1, the model requires that delivery e+1 linearizes only after delivery e's
lease was released, consumed, or expired at log-applied time. A history in
which two receipt handles for i with distinct epochs are both accepted by
`delete` or `change-visibility` without an intervening redelivery is a
violation; so is any linearization in which two receives return i with
overlapping (grant, expiry) intervals.

**Scope and preconditions.** Single-node from R1 (in-memory), durable from
R2, cluster-wide across partitions from R7 (that extension is P-08).
Exclusivity is in log-applied time: a consumer that keeps processing after
its lease expired in log-applied time is outside the lease and may collide
with the next holder — that is the at-least-once boundary (NG-01), not a
P-02 violation.

**Enforcement point.** `crates/relay-core`: the lease table is part of
`CoreState`; every grant increments `lease_epoch` and every
`delete`/`change-visibility` validates epoch equality. There is exactly one
writer of lease state: the deterministic `apply` function.

**Failure consequence if violated.** Two consumers process one delivery
concurrently while each believes it holds exclusivity; consumer-side
side-effect deduplication assumptions break in ways idempotency alone may
not cover (e.g., "at most one worker at a time" pools).

### 2.3 P-03 EVENTUAL

**Statement.** Every message is eventually delivered or dead-lettered. In
every fair execution in which faults cease and every queue is polled by at
least one live consumer, every acked message reaches `Deleted`,
`DeadLettered`, or `Expired` (retention) within a bounded amount of
log-applied time after the last fault.

**Predicate.** This is a liveness property and is checked in simulation, not
by the linearizability oracle: for every simulation run, after the fault
schedule's last event, every message in `Delayed`, `Available`, or
`InFlight` transitions to a terminal state within bound B = (retention
period) + (maxReceiveCount × maximum visibility timeout) + the delay value,
measured in virtual time, provided the workload's consumers continue to poll.
A run in which any message is still non-terminal at the bound fails.

**Scope and preconditions.** Requires bounded fairness (§8.5): the scheduler
must eventually run every ready task, and consumers must keep receiving.
Relay cannot deliver to consumers that never call `receive`; the property
quantifies over executions with live consumers. No wall-clock latency is
implied (NG-05).

**Enforcement point.** `crates/relay-core` timer transitions driven by
`AdvanceTime` (visibility expiry, delay expiry, dead-letter move, retention),
plus the server's long-poll wakeup path at R6.

**Failure consequence if violated.** A stuck message: livelock in a group,
a lease that never expires, or a dead-letter move that never fires. The
message is not lost (P-01 still holds) but it is unreachable, which for the
consumer is indistinguishable from loss until an operator intervenes.

### 2.4 P-04 FIFO-ORDER

**Statement.** Per-group delivery order equals send order. For every FIFO
queue q and message group g: if send(m1) returned `ok` before send(m2) was
invoked (both in g, both acked), then the first delivery of m1 precedes the
first delivery of m2, and no delivery from g occurs while another message of
g is in flight.

**Predicate.** For every history H, every FIFO queue q, every group g: let
S = the sequence of acked sends into (q, g) ordered by linearization point,
and D = the sequence of first-time deliveries of (q, g) messages ordered by
linearization point. The oracle requires D to be a prefix-respecting
subsequence of S (equal order, possibly shorter), and requires that between
a delivery of message m and its delete/expiry/dead-letter, no other (q, g)
message is delivered. Redeliveries re-enter D at the blocked head, never
ahead of an undelivered earlier message.

**Scope and preconditions.** FIFO queues only; standard queues make no
ordering promise (NG-03). Order is per group; distinct groups deliver in
parallel by design. Order is preserved through DLQ move and redrive-back
(the moved message re-enters the source group behind nothing that was sent
before it and is again subject to group blocking).

**Enforcement point.** `crates/relay-core`: per-group send sequences and the
group in-flight block are `CoreState` structures; `Receive` consults only
group heads.

**Failure consequence if violated.** Consumers that rely on per-group order
(state machines keyed by group, ledger-style processing) apply events out of
order and corrupt downstream state silently.

### 2.5 P-05 DEDUP-EXACT

**Statement.** The 5-minute deduplication window holds exactly at both
boundaries. For a FIFO queue with deduplication active, a send whose
deduplication id equals that of a prior acked send is a duplicate if and
only if it linearizes strictly inside the window `[t0, t0 + 300 s)` in
log-applied time, where t0 is the prior send's linearization point. A
duplicate send returns the original message id and changes no state; a send
at exactly t0 + 300 s is a new message.

**Predicate.** For every history H, every FIFO queue q, every deduplication
id d: partition the acked sends carrying d by the model's window rule using
their linearization points. Within one window, exactly one send created a
message and every other send returned that message's id with no state
change. Across a window boundary, a new message id was issued. Any send
returning a fresh id inside a live window, or the old id at or after the
boundary, is a violation.

**Scope and preconditions.** The window is fixed at 300 s (spine-fixed
limit) and measured in log-applied time, which makes the boundary exact and
testable to the nanosecond via `AdvanceTime`. Deduplication id is the
explicit `MessageDeduplicationId` when present, else SHA-256 of the body
when content-based deduplication is enabled; explicit always overrides
content-based. Deduplication is not exactly-once delivery (NG-01): it
deduplicates producer sends, not consumer processing.

**Enforcement point.** `crates/relay-core`: the per-queue dedup map keyed by
deduplication id, expired only by `AdvanceTime` transitions.

**Failure consequence if violated.** Either duplicate messages inside the
window (producers relying on retry-safety get doubles) or silently dropped
messages after the window (a "duplicate" verdict against an expired entry
swallows a genuinely new message). Both are contract breaches producers
cannot detect.

### 2.6 P-06 DELETE-IDEM

**Statement.** Delete of the same receipt handle is idempotent. For every
receipt handle h that was valid for delivery epoch e of message m: every
`delete(h)` after a successful `delete(h)` returns `ok` and changes nothing.

**Predicate.** For every history H, every handle h: if some `delete(h)`
linearizes with result `ok`, then every later `delete(h)` also returns `ok`,
and the model state after each is identical to the state before it. A later
`delete(h')` where h' carries a different (superseded or foreign) epoch
returns the appropriate error and is not covered by this property.

**Scope and preconditions.** Idempotency is per handle (per delivery epoch),
not per message: after m is redelivered under epoch e+1, the old handle for
epoch e is superseded and rejected (FR-QUEUE-007), which is P-07 territory.

**Enforcement point.** `crates/relay-core`: the consumed-set records
(message id, epoch) pairs so a repeat delete of an already-consumed pair is
recognized as `ok` rather than "not found".

**Failure consequence if violated.** Client retry logic breaks: a delete
retried after a timeout would error or, worse, delete a different delivery,
turning safe retries into unsafe ones.

### 2.7 P-07 RECEIPT-SAFE

**Statement.** Receipt handles are unforgeable and single-use per delivery.
No party can construct a handle accepted by `delete` or
`change-visibility` other than by receiving the message; and a handle is
valid only for the single delivery epoch it was issued for.

**Predicate.** Over every history H including adversarial wire traffic: (a)
every handle accepted by the server appears verbatim in the `result.ok` of a
prior `receive` in H (unforgeability); (b) no handle is accepted after a
later delivery of the same message incremented the lease epoch
(single-use). Any accepted handle failing (a) or (b) is a violation.

**Scope and preconditions.** Cryptographic: handles are
`rh1_` + base64url(version ‖ queue_id ‖ message_id ‖ lease_epoch ‖
expiry_nanos ‖ HMAC-SHA256 tag) with a per-cluster receipt key (ADR-0006).
Unforgeability is computational (HMAC-SHA256 security), verified
adversarially at the wire boundary at R6; it is not information-theoretic.

**Enforcement point.** `crates/relay-server` receipt validation before any
command reaches `relay-core`; constant-time tag comparison (NFR-SEC-004);
epoch equality enforced again inside `relay-core` (defense in depth).

**Failure consequence if violated.** Any client (or any network observer,
under plaintext loopback config) could delete messages it never received —
a direct integrity breach of every queue on the cluster.

### 2.8 P-08 NO-SPLIT-LEASE

**Statement.** No double-lease across any network partition. P-02 holds
cluster-wide: under any partition schedule in the fault model, no two
consumers — regardless of which node they are connected to — hold a live
lease on one message at the same log-applied time.

**Predicate.** Identical to the P-02 predicate, evaluated over histories
recorded against a multi-node cluster under partition schedules: every pair
of deliveries of one message has non-overlapping lease intervals in the
single linearization order the oracle constructs across all nodes' client
histories.

**Scope and preconditions.** Requires R7: lease grants are commands in the
replicated log and take effect only when applied (FR-REPL-004); a deposed
leader that has granted a lease locally but not committed it never acked
that receive, so the grant never happened observably. Reads that could leak
stale lease state go through ReadIndex (FR-REPL-008). Crash-stop faults and
partitions only — no Byzantine nodes (NG-07).

**Enforcement point.** `crates/relay-raft` commit rule plus the rule that
`relay-core` state (including leases) mutates only via applied log entries;
no node answers `receive` from unapplied or uncommitted state.

**Failure consequence if violated.** Split-brain delivery: both sides of a
partition hand the same message to different consumers with valid-looking
handles. This is the classic distributed-queue failure Relay exists to
machine-check away; a violation invalidates the R7 gate entirely.

### 2.9 P-09 NO-LOST-ACK

**Statement.** No acknowledged write is lost across leader failover. Every
operation acked by any leader is reflected in the state served by every
subsequent leader.

**Predicate.** For every multi-node history H: the whole of H (all clients,
all nodes, spanning any number of elections) is linearizable against the
single reference model. In particular, for every acked mutating operation o
and every later read r that the model says must observe o's effect, a
linearization exists in which r observes it; a history where an acked send
vanishes after failover, or an acked delete resurrects, admits no
linearization and is a violation.

**Scope and preconditions.** Requires R7 and the FR-REPL-002 commit rule (an
entry is committed only after a majority has durably appended it: fsync on
each acceptor before its append response). Acks are issued only for
committed entries. Losing a majority of nodes' disks simultaneously is
outside the fault model (§10).

**Enforcement point.** `crates/relay-raft` (commit index advancement,
election restriction to up-to-date candidates, pre-vote) over
`crates/relay-wal` durability on every node.

**Failure consequence if violated.** Identical consumer-visible outcome to a
P-01 violation, but triggered by routine operational events (failover,
rolling restart) rather than crashes, and therefore far more frequent in
practice.

### 2.10 P-10 NO-INVENTION

**Statement.** Every delivered message was previously sent, byte-identical.
For every delivery d returning message id i with body b, there exists an
acked send (or publish-derived copy) that created i with exactly the bytes
b, and attributes are preserved unmodified.

**Predicate.** For every history H: every `receive` result's
`(id, body_sha256)` pair equals the `(id, body_sha256)` recorded by the
originating acked `send`/`send_batch`/`publish` in H (for publish, the copy
id issued for that subscription, with the published body). A delivery whose
id matches no prior send, or whose hash differs from the send's, is a
violation. The oracle checks this structurally before any linearizability
search, because a history that invents messages can never linearize and the
direct check produces a better counterexample.

**Scope and preconditions.** Holds at every gate from R1 (in-memory) and is
re-checked under crash/corruption injection from R2 (a torn write must never
surface as a mutated body — CRC detection truncates instead, per
NFR-DUR-003) and under partitions from R7. Hash comparison is SHA-256 of
the body; histories record hashes, not bodies, above the §5.4 size bound.

**Enforcement point.** `crates/relay-core` (messages are immutable records
after send), `crates/relay-wal` (CRC32C per record; corrupted records are
never surfaced as data), `crates/relay-wire` (bounded parsing cannot alias
one message's bytes into another's frame).

**Failure consequence if violated.** Consumers receive fabricated or
corrupted payloads while every integrity signal looks healthy — worse than
loss, because downstream systems act on bad data.

## 3. The Honest Non-Guarantee List

Relay's credibility rests as much on this list as on §2. Each entry states
the refusal, the reason, and what users must do instead. Marketing copy must
repeat the relevant entry wherever a stronger inference is plausible
(FR-MKT-003); no Relay document may soften these.

### 3.1 NG-01 No exactly-once delivery

Relay is at-least-once. A consumer can process a message, then fail to
delete it (crash, network drop, lease expiry mid-processing), and the
message will be redelivered. This is physics, not a design gap: the delete
and the consumer's side effect cannot be made atomic across two systems
without a coordination protocol Relay does not control on the consumer side.
Deduplication (P-05) removes duplicate *sends* inside a window; nothing
removes duplicate *processing*. **What to do instead:** make consumers
idempotent — key side effects by message id or a business key, use
conditional writes, or route side effects through a transactional outbox in
the consumer's own datastore.

### 3.2 NG-02 No cross-queue or cross-operation atomicity

No operation spans queues atomically, and no batch is atomic: `send-batch`
returns independent per-entry results, publish fanout is per-subscription
(one copy can be enqueued while another fails on a full queue), and a
redrive moves messages one at a time. Cross-resource atomicity would require
a transaction layer that multiplies the verification surface and the failure
modes; Relay refuses it to keep the model checkable. **What to do instead:**
design flows so each message is independently meaningful; use the outbox
pattern for "write my database and enqueue" atomicity; treat batch results
entry by entry.

### 3.3 NG-03 No global ordering

There is no ordering across message groups, across queues, or across a
topic's subscriptions. Standard queues do not even promise per-queue order.
Global order would serialize the entire system through one sequencer and
destroy the parallelism that makes distinct FIFO groups useful. **What to do
instead:** put everything that must be ordered into one FIFO message group;
accept that anything in different groups is concurrent.

### 3.4 NG-04 Visibility expiry is "not before", never "exactly at"

A message whose visibility timeout expires becomes available *no earlier
than* the deadline; it may return to the available set some time after. The
expiry is a log-applied-time transition processed when time advances and the
node applies it — an exact-instant promise would be unkeepable under load
or failover. **What to do instead:** treat the visibility timeout as a
minimum exclusivity period, never as a scheduler; extend leases with
`change-visibility` if processing runs long.

### 3.5 NG-05 No bounded delivery latency

Relay promises no upper bound on time from send to delivery. Long-poll
wakeup targets (NFR-PERF-003) are engineering goals measured at R9, not
contracts; under overload Relay sheds load with backpressure errors rather
than pretending to a deadline. **What to do instead:** if you need latency
bounds, measure your own deployment against the BENCHMARK_PLAN methodology
and alert on your own percentiles; design consumers to tolerate delay.

### 3.6 NG-06 No messages over 256 KiB

The body limit is 256 KiB with a stable oversize error, at every gate,
permanently. Large payloads wreck WAL segment economics, group-commit
latency, and fanout amplification. **What to do instead:** store the payload
in object storage and enqueue a reference (claim-check pattern).

### 3.7 NG-07 No Byzantine fault tolerance

The fault model is crash-stop plus network partitions. A node that lies —
corrupted RAM that flips acks, a compromised process that forges Raft
messages — is outside the model, and Raft (like all crash-fault consensus)
gives no protection against it. BFT would triple the replication cost for a
threat self-hosted single-team deployments rarely face. **What to do
instead:** run Relay nodes on trusted infrastructure you control; use the
wire-level authentication (P-07, FR-API-003) to keep untrusted parties
outside the cluster boundary entirely.

### 3.8 NG-08 No multi-region or geo replication

A Relay cluster is one Raft group in one failure domain with LAN-class
latency assumptions (election timeouts of 500–1000 ms presume it). WAN
consensus changes every timeout, every quorum-latency tradeoff, and the
entire partition test matrix. **What to do instead:** run one cluster per
region and replicate at the application layer with idempotent consumers;
treat cross-region delivery as an application concern.

### 3.9 NG-09 Unacknowledged sends may be lost

The ack is the durability boundary. A send that never returned — connection
dropped, timeout, process crash mid-request — may or may not have been
applied, and Relay promises nothing about it. Promising otherwise would
require the client and server to agree on the fate of a message neither can
observe. **What to do instead:** retry unacknowledged sends; on FIFO queues
the deduplication window (P-05) makes the retry safe within 5 minutes; on
standard queues design consumers to tolerate the duplicate (NG-01 already
requires this).

### 3.10 NG-10 FIFO throughput is not standard-queue throughput

FIFO queues serialize each group (one in-flight message per group) and pay a
dedup lookup per send; a FIFO queue with few groups will not approach
standard-queue throughput, and Relay does not promise it will. Any published
FIFO number carries its group-count and workload per NFR-PERF-005. **What to
do instead:** use many groups (parallelism is per group); use standard
queues wherever ordering is not required.

## 4. The Reference Model

This section is the complete specification implemented by
`crates/relay-model`. It is the oracle's truth: when the model and the
implementation disagree, the implementation is wrong unless an ADR amends
this section. The model is a pure function over an explicit state; it reads
no clock, no rng, and no IO (time enters only through `advance-time`,
identifiers are issued deterministically from model state).

### 4.1 Model state

```text
ModelState = {
  now_ns:   u64                       -- log-applied time (ADR-0005)
  queues:   Map<QueueName, Queue>
  topics:   Map<TopicName, Topic>
}

Queue = {
  config:      { fifo: bool, default_visibility_s, default_delay_s,
                 retention_s, max_receive_count?, dlq_target?,
                 content_dedup: bool }
  next_seq:    u64                    -- per-queue send sequence
  messages:    Map<MessageId, Msg>
  order:       per-group ordered sequences of MessageId (FIFO queues);
               a single arrival-ordered sequence (standard queues)
  leases:      Map<MessageId, { epoch: u64, expiry_ns: u64 }>
  consumed:    Set<(MessageId, epoch)> -- for delete idempotency (P-06)
  dedup:       Map<DedupId, { message_id: MessageId, sent_ns: u64 }>
  purge_until_ns: u64                 -- concurrent-purge rejection window
  redrive:     Option<{ task_id, remaining: Seq<MessageId> }>
}

Msg = {
  id, body_sha256, body_len, attributes (≤ 10, typed),
  group_id?, dedup_id?, seq: u64,
  state: Delayed | Available | InFlight | DeadLettered,
  visible_at_ns, enqueue_ns, receive_count: u32, lease_epoch: u64,
  dlq_meta?: { source_queue, receive_count_at_move, moved_ns }
}

Topic = {
  config: { name },
  subscriptions: Map<SubscriptionId, { queue: QueueName,
                                       filter?: FilterPolicy }>
}
```

Deleted and retention-expired messages are removed from `messages`
entirely; `consumed` alone remembers enough for P-06. Lifecycle states and
transitions are exactly the spine-fixed set: `Delayed → Available →
InFlight → Deleted`, with `InFlight → Available` (expiry or visibility set
to 0), `InFlight|Available → DeadLettered` (receive count exhausted), and
`* → Expired` (retention). Lease lifecycle: `Granted → Extended* →
(Released | Expired | Consumed)`.

### 4.2 Shared validation limits

All operations validate against the fixed limits before any state change:
queue/topic names match `^[A-Za-z0-9_-]{1,80}$` (FIFO queues append `.fifo`,
excluded from the 80); body ≤ 256 KiB; attributes ≤ 10; batch ≤ 10 entries;
receive max ≤ 10; visibility 0 s–12 h (default 30 s); delay 0–900 s;
long-poll wait 0–20 s; retention 60 s–14 d (default 4 d); dedup window
fixed 300 s; maxReceiveCount 1–1,000; group id ≤ 128 bytes; per-queue
in-flight cap 120,000 standard / 20,000 FIFO. A validation failure returns
the error named in the rule and changes nothing.

### 4.3 Operation rules

Every rule is precondition → postcondition → return value. Preconditions are
checked in the order written; the first failure returns its error with no
state change. Error codes here are the model-level subset of the FR-API-006
taxonomy.

**RM-01 create-queue(name, config).**
Pre: name valid; name ∉ queues; config within §4.2 limits; `dlq_target`, if
set, names an existing queue of the same type. Post: `queues[name]` = empty
queue with the given config, `next_seq = 0`. Return: `ok{}`.
Errors: `invalid_argument`, `queue_already_exists`, `dlq_not_found`.

**RM-02 delete-queue(name).**
Pre: name ∈ queues. Post: queue removed with all messages; every
subscription targeting it removed from every topic; all outstanding
receipt handles for it become foreign (RM-08 rejects them); any queue whose
`dlq_target` named it has the target cleared. Return: `ok{}`.
Errors: `queue_not_found`.

**RM-03 set-queue-attributes(name, delta).**
Pre: name ∈ queues; resulting config within limits; `fifo` is immutable.
Post: config updated; new values govern subsequent operations only (existing
leases and delays keep their deadlines). Return: `ok{}`.
Errors: `queue_not_found`, `invalid_argument`, `immutable_attribute`.

**RM-04 send(queue, body, attributes, delay?, group_id?, dedup_id?).**
Pre: queue ∈ queues; body ≤ 256 KiB (`oversize_body`); attributes ≤ 10;
delay within 0–900 s; FIFO ⇒ group_id present (`missing_group_id`);
standard ⇒ dedup_id absent (`invalid_argument`).
Dedup step (FIFO only): effective dedup id d = explicit `dedup_id`, else
SHA-256(body) if `content_dedup`, else none. If d exists in `dedup` with
`now_ns − sent_ns < 300 s`: return `ok{message_id: original}` with **no
state change** — the boundary is half-open, `[sent_ns, sent_ns + 300 s)`;
at exactly +300 s the send is new (P-05).
Post (non-duplicate): new message with id = ULID derived deterministically
from (now_ns, queue, next_seq); `seq = next_seq`, `next_seq += 1`;
`state = Delayed` with `visible_at_ns = now_ns + delay` if effective delay
> 0 (per-message delay, else queue default), otherwise `Available` with
`visible_at_ns = now_ns`; `receive_count = 0`; `lease_epoch = 0`; if d
defined, `dedup[d] = {id, now_ns}` (overwriting an expired entry).
Return: `ok{message_id}`.

**RM-05 send-batch(queue, entries[1..=10]).**
Pre: 1–10 entries (`invalid_argument` otherwise); queue ∈ queues.
Post: entries are applied in order, each under RM-04 against the state left
by its predecessors; entry failures do not affect other entries (NG-02).
Return: `ok{results: [per-entry ok{message_id} | err{code}]}`.

**RM-06 receive(queue, max, visibility_s?).**
Pre: queue ∈ queues; 1 ≤ max ≤ 10; visibility within 0 s–12 h (absent ⇒
queue default). Eligible set: messages with `state = Available` and
`visible_at_ns ≤ now_ns`, further restricted on FIFO queues to group heads
(the lowest-`seq` undelivered-or-available message of each group) of groups
with **no** InFlight message, and restricted on all queues by the in-flight
cap (a receive never takes the queue above its cap; eligibility shrinks to
fit). Selection: FIFO — group heads in ascending `seq` order up to max;
standard — **any** subset of the eligible set up to max (the model is
deliberately nondeterministic here; §6.2 handles the branching).
Post: each selected message → `InFlight`; `receive_count += 1`;
`lease_epoch += 1`; lease `{epoch, expiry_ns = now_ns + visibility}`
recorded. Return: `ok{messages: [{id, receipt, body_sha256, attributes,
receive_count}]}` where `receipt` binds (queue_id, message_id, lease_epoch,
expiry_ns) per ADR-0006. An empty eligible set returns `ok{messages: []}`
(long polling is a server concern layered at R6; the model never blocks).
Errors: `queue_not_found`, `invalid_argument`.

**RM-07 delete(receipt).**
Pre: receipt parses and its tag verifies (`invalid_receipt`); its queue ∈
queues (`queue_not_found`).
Cases, checked in order:
(a) `(message_id, epoch) ∈ consumed` → return `ok{}` with no change (P-06);
(b) message exists, `state = InFlight`, `lease_epoch = epoch` → remove the
message from `messages` and its group sequence, release the lease
(Consumed), add `(message_id, epoch)` to `consumed`; return `ok{}`;
(c) message exists but `lease_epoch ≠ epoch` → `receipt_superseded`;
(d) message exists but not InFlight (lease expired and message returned to
Available under the same epoch) → `receipt_expired`;
(e) message does not exist and (a) did not match → `invalid_receipt`
(foreign or fabricated).

**RM-08 change-visibility(receipt, visibility_s).**
Pre: receipt parses and verifies; queue exists; message exists with
`state = InFlight` and `lease_epoch = epoch` (same case analysis and errors
as RM-07 c–e, except consumed handles are `receipt_superseded` here —
change-visibility is never idempotent-ok on a deleted message); visibility
within 0 s–12 h. Post: if visibility > 0, lease `expiry_ns = now_ns +
visibility` (Extended); if visibility = 0, lease Released, message →
`Available` with `visible_at_ns = now_ns` — unless `receive_count ≥
max_receive_count` with a redrive policy configured, in which case the
message moves to the DLQ per RM-13. `lease_epoch` is unchanged (it
increments only on delivery). Return: `ok{}`.

**RM-09 purge(queue).**
Pre: queue ∈ queues; `now_ns ≥ purge_until_ns` (`purge_in_progress`
otherwise). Post: all messages removed regardless of state, including
InFlight; all leases released; `consumed` cleared; `dedup` retained (the
window outlives a purge); `purge_until_ns = now_ns + 60 s`. Outstanding
handles fail as `invalid_receipt` afterward. Return: `ok{}`.

**RM-10 start-redrive(dlq, source).**
Pre: both queues exist; source's redrive policy names `dlq`; no redrive
task active on `dlq` (`redrive_in_progress`). Post: a task is registered
over the messages currently in `dlq` (Available or Delayed). Each
subsequent applied step (any operation apply, and every `advance-time`)
moves up to 10 task messages: removed from `dlq`, re-enqueued in `source`
preserving body, attributes, and group id, with `receive_count = 0`, a
fresh `seq` in send order of the original dead-letter moves, `state =
Available`, and exemption from dedup (a redriven message is never a
duplicate of itself). At every intermediate state each message is in
exactly one queue. The task completes when `remaining` is empty.
Return: `ok{task_id, message_count}`.

**RM-11 create-topic(name)** / **RM-12-d delete-topic(name).**
Create — Pre: name valid and ∉ topics. Post: empty topic. Return: `ok{}`.
Errors: `invalid_argument`, `topic_already_exists`.
Delete — Pre: name ∈ topics. Post: topic and all its subscriptions removed;
subscribed queues and already-delivered copies are untouched
(FR-TOPIC-007). Return: `ok{}`. Errors: `topic_not_found`.

**RM-12 subscribe(topic, queue, filter?)** / **unsubscribe(sub_id).**
Subscribe — Pre: topic ∈ topics; queue ∈ queues; filter, if present, is a
valid policy (exact, anything-but, prefix, numeric range, exists — invalid
policies rejected with field-level errors, `invalid_filter_policy`). Post:
new subscription with the filter recorded verbatim at subscribe time; later
filter semantics changes never rewrite it. Return: `ok{subscription_id}`.
Unsubscribe — Pre: sub_id exists (`subscription_not_found`). Post:
subscription removed; already-enqueued copies unaffected. Return: `ok{}`.

**RM-13 publish(topic, body, attributes, group_id?, dedup_id?).**
Pre: topic ∈ topics; body and attributes within limits. Post: for each
subscription (iterated in subscription-id order) whose filter matches the
attributes (a message with no filter-referenced attribute yields
non-match), apply RM-04 to the target queue with an independent copy —
distinct message id per copy, FIFO targets enforce group and dedup rules
(FR-TOPIC-008). A per-copy failure (in-flight cap, oversize against a
stricter target) is recorded per subscription and does not affect other
copies (NG-02). Return: `ok{publish_id, deliveries: [{subscription_id,
ok{message_id} | err{code}}]}`.

**RM-14 advance-time(delta_ns).**
Pre: delta_ns > 0. Post: `now_ns += delta_ns`; then timer transitions fire
in this exact deterministic order, each pass iterating queues in name order
and messages in `seq` order:
1. **Delay expiry**: every `Delayed` message with `visible_at_ns ≤ now_ns`
   → `Available`.
2. **Lease expiry**: every lease with `expiry_ns ≤ now_ns` → Expired; its
   message: if a redrive policy is configured and `receive_count ≥
   max_receive_count` → DLQ move (message removed from source, enqueued in
   the DLQ as Available with `dlq_meta` = {source queue, receive count,
   `now_ns`}, body and attributes preserved, group id preserved); else →
   `Available` with `visible_at_ns = now_ns`.
3. **Retention expiry**: every message (any state except InFlight) with
   `enqueue_ns + retention_s ≤ now_ns` is removed (Expired). An InFlight
   message past retention is removed at lease resolution instead.
4. **Dedup expiry**: every dedup entry with `now_ns − sent_ns ≥ 300 s` is
   removed.
5. **Redrive progress**: per RM-10.
Return: `ok{}`. `advance-time` is the sole source of time in the model and
in `relay-core` (ADR-0005); no other rule reads a clock.

**RM-15 tag / untag / list operations.** Tags are a
`Map<ResourceRef, Map<TagKey, TagValue>>` side table; describe/list
operations are pure reads returning configuration plus counts computed from
model state (counts are exact in the model; the implementation labels its
counts approximate, and the oracle therefore never checks count fields —
they are excluded from `result` comparison).

## 5. History Recording

### 5.1 Format

A history is a JSONL file: one operation per line, schema below, matching
the spine §5 example:

```json
{"op":42,"client":3,"call":{"type":"receive","queue":"q1","max":1,"visibility_s":30},
 "invoke_ns":81234000,"return_ns":81239000,
 "result":{"ok":{"messages":[{"id":"01J...","receipt":"rh1_...","body_sha256":"..."}]}},"seed":"0xDEADBEEF"}
```

| Field | Type | Meaning |
| --- | --- | --- |
| `op` | u64 | Unique operation id within the history; assigned in invoke order; dense from 0. |
| `client` | u32 | Logical client (thread/session) id. One client has at most one outstanding operation at a time. |
| `call` | object | `type` plus per-operation arguments. Types: `create_queue`, `delete_queue`, `set_queue_attributes`, `send`, `send_batch`, `receive`, `delete`, `change_visibility`, `purge`, `start_redrive`, `create_topic`, `delete_topic`, `subscribe`, `unsubscribe`, `publish`, `tag`, `untag`, `advance_time`, `crash`, `recover`. Bodies appear as `body_sha256` + `body_len`, never raw bytes. |
| `invoke_ns` | u64 | Log-applied-time of invocation as observed at the recording point. |
| `return_ns` | u64 or null | Log-applied-time of return; `null` for an operation still pending at history end (crash, timeout). |
| `result` | object or null | `{"ok":{...}}` with per-type payload, or `{"err":{"code":"..."}}` with a code from the RM rules (`queue_not_found`, `queue_already_exists`, `invalid_argument`, `oversize_body`, `missing_group_id`, `invalid_receipt`, `receipt_superseded`, `receipt_expired`, `purge_in_progress`, `redrive_in_progress`, `topic_not_found`, `topic_already_exists`, `invalid_filter_policy`, `subscription_not_found`, `inflight_cap_exceeded`, `dlq_not_found`, `immutable_attribute`, `throttled`, `not_leader`) — `throttled` and `not_leader` are environment results the oracle treats as no-ops with a retry obligation on the client; `null` when pending. |
| `seed` | string | Hex master seed of the generating run; identical on every line of one history. |

`crash` and `recover` are pseudo-operations recorded by the harness (client
`0xFFFFFFFF`), marking process-kill and recovery-complete points for P-01
checking; they carry no `result`.

### 5.2 Concurrent-history semantics

Each operation occupies the real-time interval `[invoke_ns, return_ns]`.
Two operations are concurrent iff their intervals overlap; operation A
precedes B iff `return_ns(A) < invoke_ns(B)`. A pending operation
(`return_ns = null`) is concurrent with everything after its invoke, and
the checker must consider both that it took effect and that it did not —
except mutating pendings before a crash, which P-01 constrains as §2.1
states. Linearizability (§6) means: there exists a total order of all
completed operations (plus a took-effect subset of pendings), consistent
with the precedence relation, under which every result matches the RM rules
applied in that order.

### 5.3 Capture points

- **Model-checker tests (MODL-)**: histories are captured client-side in
  the test harness, at the `relay-client` call boundary — the same vantage
  a real user has. Timestamps come from the harness's virtual clock.
- **Simulation (SIM-)**: histories are captured by a wire-tap inside
  `relay-sim`'s SimNet, recording every RWP request/response pair at frame
  granularity, so protocol-level faults (duplicated frames, dropped
  responses) appear as pending or repeated operations exactly as a client
  would experience them.

Both capture points write the same JSONL schema; the oracle does not know
or care which produced a history.

### 5.4 Size bounds and truncation

One history ≤ 100,000 operations and ≤ 64 MiB serialized; message bodies
are always recorded as `body_sha256` + `body_len` (bodies over 64 bytes are
never inlined anywhere in a history). A history that hits either bound
mid-run, or whose final line is not a complete JSON document, is
**truncated**: a truncated history is a failed run with error class
`HIST-TRUNC`, never a silently-checked shorter history. The generator must
shrink its profile, not the checker its input.

## 6. The Checking Oracle

The oracle (in `crates/relay-model`, per ADR-0007) decides whether a
recorded history is linearizable against §4. It is a Wing–Gong search with
per-queue partitioning and memoization.

### 6.1 Algorithm

1. **Parse and validate** the JSONL history; reject truncation (§5.4),
   schema violations, non-dense `op` ids, or a client with two overlapping
   operations (`HIST-MALFORMED`).
2. **Structural pre-checks** that need no search: P-10 (every delivered
   `(id, body_sha256)` matches an acked send), receipt provenance (every
   handle in a call appeared in a prior receive's result — P-07's history
   side), and `seed` uniformity. A structural failure short-circuits with
   its own counterexample.
3. **Partition** operations by queue (§6.2), producing independent
   sub-histories plus a shared environment stream (`advance_time`, `crash`,
   `recover`) replicated into every partition in order.
4. **Wing–Gong search per partition**: maintain the set of linearized op
   ids and the current model state. At each step, the candidate set is
   every un-linearized operation whose invoke precedes the earliest return
   among un-linearized completed operations (the "minimal" operations —
   linearizing any other would violate real-time order). For each
   candidate: apply its call to the model state; if the model's return
   value can equal the recorded result (branching over model
   nondeterminism, §6.3), recurse with the op linearized and that state;
   otherwise try the next candidate. Pending operations may be linearized
   (took effect) or skipped permanently (never took effect); both branches
   are explored.
5. **Accept** when every completed operation is linearized in every
   partition. **Reject** when the search space is exhausted with completed
   operations remaining.
6. **On rejection**, emit the §6.5 counterexample.

### 6.2 Per-queue partitioning

Operations on distinct queues commute: the RM rules for queue A read and
write only A's state, so a history is linearizable iff each per-queue
sub-history is linearizable — with one exception, topic fanout, and one
class of cross-queue operations:

- **Publish fanout**: a `publish` is split into one virtual operation per
  subscription copy reported in its result, each assigned to its target
  queue's partition and each carrying the original `[invoke_ns, return_ns]`
  interval. This is sound because FR-TOPIC-003 defines publish as
  per-subscription and never cross-queue atomic (NG-02): each copy
  linearizes independently, exactly as the product promises. Subscription
  configuration operations (`subscribe`, `unsubscribe`, `create_topic`,
  `delete_topic`) are assigned to every partition whose queue they name
  (and a dedicated per-topic partition for the subscription-set itself),
  acting as ordering fences for the fanout membership they change.
- **Redrive and DLQ moves** couple exactly two queues; both queues'
  operations are merged into one combined partition for any history that
  contains a `start_redrive` or in which a dead-letter move can occur
  (source has a redrive policy). The partitioner computes the
  union-find of coupled queues before splitting.

### 6.3 Model nondeterminism

Standard-queue `receive` (RM-06) permits any eligible subset; the searcher
does not enumerate subsets — it checks whether the *recorded* result is a
permitted choice (each returned message eligible, count ≤ max, cap
respected) and applies exactly that choice. The only true branching is
Wing–Gong candidate order and pending-op fate.

### 6.4 Memoization and budget

- **Memoization**: the searcher caches `(linearized-set bitmap, SHA-256 of
  canonical model state)`; a revisited configuration is pruned. This is the
  standard WG optimization and is what makes contended single-queue
  histories tractable.
- **Budget**: each history has a wall-clock search budget of **60 s in
  CI**. Exceeding it is `CHECK-BUDGET` — an inconclusive result that fails
  the CI run (an unchecked history is not evidence). Nightly runs have no
  per-history limit, with escalation: any history over 30 minutes is
  reported for generator-profile shrinking, and a nightly `CHECK-BUDGET`
  after 24 h aborts the run as a failure.

### 6.5 Counterexamples

A rejected history produces: (a) the minimal counterexample — the input
shrunk by ddmin-style delta debugging (removing operations and re-checking)
until no operation can be removed without the history becoming
linearizable; (b) the generating `seed`; (c) the first non-linearizable
frontier (the op set at which every candidate failed, with the model's
expected-result alternatives). The triple is written to the failing-seed
corpus (§8.7) and printed in the CI log.

## 7. Model-Checker Workloads

Histories are produced by seeded generators in `crates/relay-model`. Every
profile is deterministic from its seed; each CI run sweeps a fixed seed
block plus the corpus, and nightly sweeps a rolling block.

| Profile | Generator behavior | Ops/history | CI histories | Nightly histories |
| --- | --- | --- | --- | --- |
| `prof-mixed` | 8 clients, 4 standard + 2 FIFO queues, all RM operations weighted toward send/receive/delete, interleaved `advance_time` | 1,000 | 50 | 2,000 |
| `prof-contended` | 16 clients hammering one standard queue: receive/delete races, duplicate-delete retries, change-visibility on contested handles | 800 | 50 | 2,000 |
| `prof-fifo-groups` | One FIFO queue, 8 groups, interleaved group sends, deliberate in-flight blocking, DLQ + redrive-back cycles | 1,200 | 40 | 1,500 |
| `prof-dedup-adversarial` | FIFO queue; duplicate dedup ids re-sent at window boundary ± 1 ns via crafted `advance_time`; explicit-vs-content-hash conflicts | 600 | 40 | 1,500 |
| `prof-lease-churn` | Short visibilities (0–2 s), change-visibility storms, visibility-zero returns, expiry racing delete | 1,000 | 50 | 2,000 |
| `prof-topics-fanout` | 2 topics, 6 subscriptions (2 filtered, 1 FIFO target), publish bursts, subscribe/unsubscribe during traffic | 900 | 30 | 1,200 |
| `prof-partition` (R7) | 3-node simulated cluster; partition schedules, leader kills, client failover mid-operation; wire-tap histories | 2,000 | 20 | 500 seeds × full sweep |

Budgets: the MODL CI job must finish in 10 minutes wall (histories are
checked in parallel; the 60 s per-history budget of §6.4 applies within
it). Nightly MODL runs own a 4-hour budget. `prof-partition` runs in the
SIM nightly lane at R7 volumes and a reduced 20-history smoke in CI.
History sizes are chosen to sit far below the §5.4 truncation bound; a
profile that produces a truncated history is a generator bug.

## 8. The Simulation Design

### 8.1 Shape

The simulation (`crates/relay-sim`) runs the entire system — one node at
R2–R6, a 3-node cluster at R7 — in **one process, one thread**, under
virtual time. Production code receives its environment only through the
injected traits fixed in the spine: `Clock`, `Net`, `Disk`, `Rng`
(SimClock, SimNet, SimDisk, SimRng). No code under simulation may reach a
real clock, socket, file, thread, or entropy source; an architecture test
(R0) denies the forbidden imports in `relay-core`, `relay-wal`,
`relay-raft`, and `relay-server`.

### 8.2 Seed → schedule derivation

A run is fully determined by one 64-bit master seed. The seed is expanded
by SplitMix64 into named substreams: `workload` (which operations clients
issue, with which payloads), `net` (message delivery order, latency draws,
drop/duplicate/reorder decisions), `disk` (write latency, torn-write
content on crash, disk-full onset), `fault` (the fault schedule), and
`tiebreak` (executor ready-queue ordering). The **fault schedule** is drawn
from the `fault` substream before the run starts: a list of timed events —
process crash (SIGKILL-equivalent at a virtual instant, including
mid-group-commit), restart, network partition (node-set bipartition with
heal time), asymmetric link loss, fsync error injection, disk-full window,
clock-skew injection into the wall clock (never the log-applied clock).
Schedules respect the fault model: crash-stop and partitions only (NG-07),
and at R7 never a simultaneous majority disk loss (§10).

### 8.3 Virtual-time executor

The executor maintains a priority queue of (virtual time, task) pairs.
Time jumps discretely to the next scheduled event; there is no real
sleeping. Every `AdvanceTime` visible to `relay-core` is a log entry
(ADR-0005), so state-machine time is deterministic even across crash and
recovery within a run.

### 8.4 Continuous invariant checkers

After every executor step the harness evaluates invariant checkers against
ground truth it maintains alongside the system (the harness sees all sends,
deliveries, and acks). Each checker maps to a property:

| Checker | Property | Evaluates |
| --- | --- | --- |
| `chk_durable_ack` | P-01 | After every recovery: the acked-send set minus model-permitted consumptions ⊆ recovered state. |
| `chk_lease_excl` | P-02 | At every step: at most one live lease per message id (single node). |
| `chk_no_split_lease` | P-08 | Same predicate evaluated cluster-wide across partitioned nodes' applied states. |
| `chk_no_lost_ack` | P-09 | After every leader change: every acked mutation is in the new leader's applied state. |
| `chk_fifo_order` | P-04 | Every FIFO delivery is the group head; per-group first-delivery order equals acked-send order. |
| `chk_dedup` | P-05 | Every dedup verdict matches the half-open 300 s window in log-applied time. |
| `chk_delete_idem` | P-06 | Every repeat delete of a consumed (id, epoch) returned ok with no state delta. |
| `chk_receipt_epoch` | P-07 (state half) | No handle accepted whose epoch is not the message's current epoch. Wire-level unforgeability is proven in WIRE-/FUZZ-, not here. |
| `chk_no_invention` | P-10 | Every delivered (id, body hash) matches a prior acked send. |
| `chk_liveness` | P-03 | §8.5 watchdog. |

A checker firing aborts the run at that step with the seed, the virtual
time, and the violating state delta.

### 8.5 Liveness under bounded fairness (P-03)

Safety checkers cannot see "never happens", so P-03 gets a watchdog: after
the fault schedule's final event, the workload switches to drain mode
(consumers poll every queue; producers stop). The watchdog computes the
§2.3 bound B per message from queue configuration and fails the run if any
message is non-terminal at `last_fault_time + B` in virtual time. Bounded
fairness is enforced by construction: the executor never starves a ready
task (the `tiebreak` stream orders equal-time tasks; every ready task runs
before time advances), and drain-mode consumers are always ready.

### 8.6 The reproducibility contract

Same seed ⇒ byte-identical trace. Every run emits a canonical trace (the
ordered event log of the executor, hashed with SHA-256). Re-running any
seed on the same commit must reproduce the identical hash. Divergence is
itself a first-class bug (`DET-DIVERGE`, severity equal to a property
violation): it means nondeterminism has leaked into the system (a real
clock read, a HashMap iteration order, a thread), and until it is fixed no
simulation result on that commit is trustworthy. CI re-runs a fixed sample
of 5 seeds twice per run and compares hashes; nightly re-runs 100.

### 8.7 Failing-seed corpus

Layout (checked into the repository, replayed by every CI run per
NFR-MAINT-002):

```text
crates/relay-sim/corpus/
  failing-seeds/
    P-01/0x4f21c09a99d3be77.toml
    P-08/0x11aa00bc45ef0102.toml
    DET-DIVERGE/0x77e1a4b0c3d25f19.toml
  minimized-histories/
    P-04/0x2b8de401ffa06c33.jsonl
```

Each seed file records: master seed, profile, commit that first failed,
property or bug class, fix commit once fixed, and the §6.5 minimal history
when one exists. **Replay policy**: the corpus is append-only; every CI run
replays every corpus seed with all checkers armed; a fixed seed stays in
the corpus forever as a regression test; removal or profile-invalidation
of a corpus entry requires an ADR.

## 9. Property-to-Test Mapping

This table is the core of the document: it is the exhaustive binding
between every §2 property and the named tests that will prove it. A
property's status may change only when its rows change, and vice versa.
Test mechanics, fixtures, runtime budgets, and flake policy live in
[OPERATIONS_TEST_PLAN.md](./OPERATIONS_TEST_PLAN.md); family prefixes are
the spine-fixed set. Today, every row is `planned` because nothing is
built.

| Property | Named test(s) | Family | Earliest gate | Status |
| --- | --- | --- | --- | --- |
| P-01 DURABLE-ACK | `crsh_ack_survives_sigkill_mid_group_commit` | CRSH- | R2 | planned |
| P-01 DURABLE-ACK | `crsh_ack_survives_torn_tail_write` | CRSH- | R2 | planned |
| P-01 DURABLE-ACK | `sim_crash_restart_acked_set_preserved` | SIM- | R3 | planned |
| P-02 LEASE-EXCL | `core_lease_epoch_single_holder` | CORE- | R1 | planned |
| P-02 LEASE-EXCL | `modl_lease_exclusivity_contended` | MODL- | R1 | planned |
| P-02 LEASE-EXCL | `sim_lease_churn_no_dual_holder` | SIM- | R3 | planned |
| P-03 EVENTUAL | `sim_liveness_delay_and_visibility_progress` | SIM- | R3 | planned |
| P-03 EVENTUAL | `sim_liveness_all_delivered_or_dlq_after_faults_cease` | SIM- | R4 | planned |
| P-04 FIFO-ORDER | `core_fifo_group_blocks_until_delete_or_expiry` | CORE- | R4 | planned |
| P-04 FIFO-ORDER | `modl_fifo_group_order_equals_send_order` | MODL- | R4 | planned |
| P-04 FIFO-ORDER | `sim_fifo_order_preserved_across_crash_recovery` | SIM- | R4 | planned |
| P-05 DEDUP-EXACT | `core_dedup_window_boundary_exact` | CORE- | R4 | planned |
| P-05 DEDUP-EXACT | `core_dedup_explicit_id_overrides_content_hash` | CORE- | R4 | planned |
| P-05 DEDUP-EXACT | `modl_dedup_adversarial_resend_storm` | MODL- | R4 | planned |
| P-06 DELETE-IDEM | `core_delete_idempotent_repeat_handle` | CORE- | R1 | planned |
| P-06 DELETE-IDEM | `modl_delete_idempotency_concurrent_retries` | MODL- | R1 | planned |
| P-07 RECEIPT-SAFE | `wire_receipt_forgery_rejected` | WIRE- | R6 | planned |
| P-07 RECEIPT-SAFE | `wire_receipt_replay_after_redelivery_rejected` | WIRE- | R6 | planned |
| P-07 RECEIPT-SAFE | `fuzz_receipt_parser_never_accepts_invalid_tag` | FUZZ- | R6 | planned |
| P-08 NO-SPLIT-LEASE | `raft_lease_grant_linearized_through_log` | RAFT- | R7 | planned |
| P-08 NO-SPLIT-LEASE | `sim_partition_no_double_lease_seed_sweep` | SIM- | R7 | planned |
| P-09 NO-LOST-ACK | `raft_commit_requires_majority_durable_append` | RAFT- | R7 | planned |
| P-09 NO-LOST-ACK | `sim_failover_no_lost_ack` | SIM- | R7 | planned |
| P-09 NO-LOST-ACK | `modl_acked_send_visible_after_leader_change` | MODL- | R7 | planned |
| P-10 NO-INVENTION | `modl_no_invention_body_hash_audit` | MODL- | R1 | planned |
| P-10 NO-INVENTION | `crsh_torn_write_never_surfaces_mutated_body` | CRSH- | R2 | planned |
| P-10 NO-INVENTION | `sim_delivered_bytes_match_sent_bytes` | SIM- | R3 | planned |

Reading rules for the table:

1. A property is `accepted` only when **every** row it owns is green in CI
   at or after its own earliest gate. Earlier rows passing (e.g. the R1
   MODL row of P-02) never promote the property past a later row's gate.
2. Rows are additive over time: gates may add tests to a property (they
   must update this table in the same change) but may never remove one
   without an ADR.
3. The `Earliest gate` column follows the promotion rules of §1.4:
   in-memory rows sit at R1, durability rows at R2, simulation rows at
   R3+, wire rows at R6, replication rows at R7.
4. `SOAK-` and `BENCH-` families never appear here: they mitigate §10
   residuals and measure performance; they are not proof of any P-xx.

## 10. What This Apparatus Cannot Prove

The verification design above is strong and it is still not a proof of
correctness of the deployed system. These residuals are permanent; each is
mitigated, never closed, and no Relay claim may paper over them.

### 10.1 The model–implementation gap

The oracle proves that recorded histories are consistent with §4 — it
checks executions, not all executions. The generators and the simulation
explore a large scheduled space, but the real concurrency, the real
allocator, and the real kernel can produce interleavings no profile drew.
Likewise, `crates/relay-model` could itself mis-encode §4 and agree with a
matching bug in the implementation. **Mitigation:** mutation testing on
`relay-core` (MUT-, ≥ 85% mutants killed per NFR-MAINT-003) to show the
tests can see change; independent review of the RM rules against
PRODUCT_REQUIREMENTS at each gate; SOAK- runs on real hardware with the
same invariant checkers armed against production builds. None of this
upgrades "checked" to "proven", and no document may say "proven".

### 10.2 Hardware faults outside the injected model

SimDisk injects the faults the ADR-0008 contract defends against — crash,
torn tail write, disk-full, fsync error — but a disk that acknowledges an
fsync and silently loses the data, bit rot below the CRC's detection
probability, or RAM corruption altering state between check and use are
outside the model. So is simultaneous durable-storage loss on a majority
of nodes at R7. **Mitigation:** CRC32C on every WAL record and snapshot
chunk detects (not prevents) most corruption on read; the backup and
restore drill (FR-OPS-007) bounds the damage window; the capacity and
operations documentation tells operators to run Relay on storage with
honest flush semantics. Relay does not claim to survive lying hardware.

### 10.3 Byzantine behavior

NG-07 is a design boundary and also a verification boundary: no test
family exercises a node that forges Raft messages or an insider with the
receipt-handle HMAC key, because the system is not designed to withstand
them. **Mitigation:** the wire boundary is verified adversarially — FUZZ-
corpus gating on the RWP parser, WIRE- authentication and forgery tests,
canary-based secret-leak tests (NFR-SEC-003) — so the *untrusted-client*
surface is hardened even though the *inter-node* surface assumes honesty.
THREAT_MODEL.md owns the exact trust boundary; claims stop at it.

### 10.4 Real-network pathologies beyond the fault model

SimNet delivers, delays, drops, duplicates, and partitions. It does not
model middlebox connection resets under load, kernel buffer exhaustion,
NIC offload corruption, asymmetric MTU blackholes, or pathological latency
distributions of real datacenter fabrics. A partition schedule is a clean
abstraction of a messy phenomenon. **Mitigation:** SOAK- runs a real
3-node cluster under `tc`/`iptables`-driven network abuse for extended
periods with invariant checkers sampling live state; the live smoke suite
runs the client conformance workload against every release candidate on
real networks; both report violations into the same corpus discipline as
§8.7. These runs raise confidence and catch regressions; they are
explicitly not exhaustive and are never cited as proof of P-08 or P-09 —
the SIM- rows in §9 remain the binding evidence, with SOAK- as the bridge
between the model and the world.

### 10.5 The claim boundary, restated

What Relay may say when all §9 rows are green: "these ten properties hold
in every execution our checkers have examined, the checkers are
deterministic and reproducible from seeds, and every failure ever found is
replayed forever." What Relay may never say: "verified" without naming the
apparatus, "proven correct", "exactly-once", or any §3 negation.
MARKETING.md inherits this boundary through the FR-MKT claims audit and
may never strengthen it.
