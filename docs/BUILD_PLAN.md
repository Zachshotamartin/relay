# Relay: Exhaustive Build Plan

Document status: normative implementation plan. Nothing described in this plan is
implemented. Every gate below is `planned`; the only `accepted` artifacts in the
repository are the documentation set and ADR-0001 through ADR-0011.

Last revised: 2026-08-30.

Companion specifications:

- [Product requirements](./PRODUCT_REQUIREMENTS.md)
- [Architecture](./ARCHITECTURE.md)
- [Correctness properties and non-guarantees](./CORRECTNESS.md)
- [Installation, testing, operations, and release](./OPERATIONS_TEST_PLAN.md)
- [Threat model](./THREAT_MODEL.md)
- [Benchmark plan](./BENCHMARK_PLAN.md)
- [Marketing and claims policy](./MARKETING.md)
- [Glossary](./GLOSSARY.md)
- [Open questions](./OPEN_QUESTIONS.md)
- [Accepted decisions](./decisions/)

Relay is a self-hosted message queue and pub/sub service whose delivery
guarantees are machine-checked by deterministic simulation and model checking
rather than asserted in documentation. The verification apparatus — the pure
state machine, the reference model, the linearizability checker, the
deterministic simulator, and the crash-injection harness — is the product
thesis, not test scaffolding. Even so, the WAL engine, the simulator, the Raft
implementation, and the model checker are internal systems that support a queue
a user can run and trust. They do not advance ahead of a user-visible slice
unless that slice depends on them, and no internal subsystem's existence is
ever presented as product capability.

## 1. How to Read and Enforce This Plan

### 1.1 Status vocabulary

Every deliverable has one of four statuses:

- **accepted**: implemented on mainline, backed by its named automated gate;
- **in progress**: present on a branch, not a claim;
- **planned**: specified, not implemented;
- **deferred**: outside the named phase; forbidden as completion evidence.

A package, type, stub, or happy-path unit test is never completion. A gate is
accepted only when its semantics, failure behavior, security behavior,
documentation, and acceptance evidence pass together under the named automated
gate. At the start of this project **nothing is built; every gate is
`planned`**, and the ADRs are `accepted` (they are decisions, not code).

### 1.2 Gates

The gate labels are dependency gates, not marketing versions. The section
numbers of this document are fixed to the gates so that cross-references from
every companion specification remain stable:

| BP § | Gate | Evidence unlocked | Effort (focused days) |
| --- | --- | --- | --- |
| 5 | R0 | Repository, Rust toolchain, CI, and architecture checks exist and are green. | 3–5 |
| 6 | R1 | Single-node in-memory core queue semantics are correct under the model checker. | 10–15 |
| 7 | R2 | The durable WAL storage engine survives crash, torn-write, and disk-full injection. | 12–18 |
| 8 | R3 | Deterministic simulation reproduces any failure from a seed and runs in CI with a checked-in corpus. | 10–14 |
| 9 | R4 | FIFO groups, deduplication, delay, DLQ, and redrive behave exactly to specification. | 8–12 |
| 10 | R5 | Topics, subscriptions, and filter policies fan out correctly. | 5–8 |
| 11 | R6 | A bounded, fuzzed wire API with authentication, quotas, and long polling serves real clients. | 12–18 |
| 12 | R7 | Raft replication survives partition and failover with no double-lease and no lost ack. | 20–30 |
| 13 | R8 | Metrics, tracing, the admin surface, and a runbook make Relay operable. | 8–12 |
| 14 | R9 | Published benchmarks, a failure-injection report, and evidence-bound marketing support stated claims and no more. | 6–10 |
| 15 | R10 | Packaging, deployment, upgrade, rollback, and backup/restore satisfy the 1.0 release gate. | 8–12 |

No gate may borrow evidence from a later gate, and no later gate may claim an
earlier gate's evidence without re-running it. The first supported release is
1.0 at R10; nothing before R10 is installable product, and nothing before R9 is
a public performance or reliability claim.

### 1.3 Sequencing rules

1. Write the failing deterministic test before implementation for every parser,
   validator, reducer, state transition, format migration, and error category.
2. Complete the thinnest user-visible vertical slice before broadening an
   internal subsystem. A storage engine feature with no queue semantics on top
   of it does not merge.
3. Prefer determinism over live infrastructure. Never use a real cluster, real
   disk fault, or real network partition to prove behavior that deterministic
   simulation can prove; live infrastructure appears only where the property
   under test is itself about live infrastructure.
4. Keep exactly one production state machine. `relay-core` drives single-node
   and replicated modes identically; the replicated mode feeds the same
   `apply` function from the Raft log. The reference model in `relay-model` is
   a deliberately independent oracle, never a shipping code path, and never a
   second source of production semantics.
5. Treat every external byte as untrusted. Wire frames, WAL bytes read back
   from disk, configuration files, snapshot files, history files, and CLI
   input all pass through a bounded parser at the owning boundary before any
   allocation proportional to claimed length.
6. Make no unearned claims. An in-memory pass is never a durability claim; a
   single-node pass is never a replication claim; a simulated fault is never
   production hardening; a benchmark on one machine is never a general
   performance claim. Every claim names the gate and evidence behind it.
7. Land on-disk and wire format changes with old-version fixtures and a stated
   downgrade policy before any compatibility code is removed (NFR-DUR-007).
8. A reversed decision is a new ADR that supersedes the old one. Accepted ADRs
   are never edited to match code.
9. Release claims are never stronger than the measured platform, workload, and
   fault model. The non-guarantee list in [CORRECTNESS.md](./CORRECTNESS.md)
   (NG-01 through NG-10) travels with every guarantee statement.

## 2. Current Baseline: What Is and Is Not Built

### 2.1 What exists

Nothing is built. The repository at the start of R0 contains only:

- this documentation set (`docs/`), consisting of the companion specifications
  listed in the header;
- the accepted decision records ADR-0001 through ADR-0011 under
  [./decisions/](./decisions/), which fix the language and toolchain, the
  hand-rolled segmented WAL, the in-house Raft implementation, the RWP/1
  binary protocol, injected time, ULID identifiers and HMAC receipt handles,
  JSONL histories with a Wing–Gong linearizability oracle, the
  fsync-before-ack durability contract, single static binary deployment, the
  observability stack, and the supported platforms.

There is no Cargo workspace, no crate, no CI pipeline, no test, no fixture, no
benchmark, and no binary. The ADRs are `accepted` because they are decisions;
every implementation item in this plan is `planned`.

### 2.2 Current honest product claim

There is no product claim. The truthful public statement until R1 is accepted
is:

> Relay is a documented design for a verification-first message queue. No
> binary exists, no test exists, no benchmark exists, and no delivery
> guarantee has been demonstrated.

No README, repository description, demo, release tag, or portfolio bullet may
imply otherwise. The docs/README.md precedence order governs any conflict
between documents; implemented code and passing tests control claims about
what works today, and today that set is empty.

### 2.3 Branch and document hygiene

- The default branch holds the documentation set and, after R0, the accepted
  toolchain and CI baseline. Every gate lands through a pull request whose CI
  run replays the evidence commands of every previously accepted gate
  (NFR-MAINT-004).
- A changeset that alters behavior updates the affected documentation —
  statuses, limits, formats, and traceability rows — in the same pull request.
  Documentation drift is a gate failure, not a cleanup task.
- Work that does not pass its gate stays on its branch and is described as
  `in progress` wherever it is described at all. No branch content is cited as
  evidence for any claim.
- Golden files, fixtures, and checked-in corpora change only with a reviewed
  semantic justification; a blanket regeneration is rejected.
- Any deviation from an accepted ADR is a new superseding ADR in
  [./decisions/](./decisions/) before the deviating code merges.

## 3. Target Architecture

### 3.1 Product center

The primary path is deliberately short, and every hop on it is either pure or
injected:

```text
client request (RWP/1 frame)
    -> bounded frame parser and authenticator      (relay-wire, relay-server)
    -> command validation into a typed Command     (relay-core types)
    -> durability and ordering                     (relay-wal; relay-raft when clustered)
    -> pure state transition: apply(state, entry)  (relay-core)
    -> outputs: replies, lease events              (relay-core Output)
    -> reply frame to the client                   (relay-wire)
```

The same `apply` function runs in production, in the deterministic simulator,
under the model checker, and during WAL recovery. Time enters the state
machine only as `AdvanceTime` log entries (ADR-0005), so lease expiry,
delay promotion, and retention are deterministic functions of the log, never
of the host clock. The verification stack — `relay-sim`, `relay-model`, and
the crash-injection harness inside `relay-wal` tests — surrounds this path; it
does not sit on it.

### 3.2 Trust and process boundaries

| Boundary | Trusted responsibility | Untrusted input |
| --- | --- | --- |
| RWP/1 frame parser | Enforce magic, length, CRC32C, and opcode bounds before allocation. | every byte from any network peer. |
| Authenticator | Verify per-tenant HMAC and constant-time credential comparison. | claimed tenant identifiers and signatures. |
| Command validator | Produce one typed, bounds-checked `Command` or one stable error. | field values inside an authenticated frame. |
| Core state machine | Apply one log entry as a total, pure function. | nothing — it accepts only validated `LogEntry` values. |
| WAL reader | CRC-verify and bounds-check every record read back from disk. | disk contents, torn writes, truncated segments. |
| Snapshot reader | Verify per-chunk CRC and the footer SHA-256 before use. | snapshot files, including operator-supplied restores. |
| Raft transport | Frame, verify, and bound peer messages. | bytes from cluster peers. |
| Config loader | Parse TOML, environment, and flags with fixed precedence; fail fast. | `/etc/relay/relay.toml`, environment, argv. |
| Admin CLI (`relayctl`) | Render server replies; never interpret them as commands. | server responses, operator input. |
| Simulator | Deliver injected faults exactly as scheduled by the seed. | nothing at runtime; seeds and scenarios are test inputs. |

Message bodies, attribute values, queue names, and tags are customer data.
They cross boundaries as opaque bounded bytes; no boundary logs them, parses
them for meaning, or lets them influence control flow beyond documented
matching (filter policies at R5 match attributes, never bodies).

### 3.3 Repository layout

The workspace root is `/Users/zacharymartin/Desktop/portfolio_projects/relay/`.

```text
crates/
  relay-core/    # pure deterministic state machine — no IO, no clock, no rng
  relay-wal/     # segmented write-ahead-log storage engine
  relay-raft/    # in-house Raft (ADR-0003)
  relay-sim/     # SimClock, SimNet, SimDisk, SimRng, virtual-time executor
  relay-model/   # reference model, JSONL history format, linearizability checker
  relay-wire/    # RWP/1 codec + fuzz targets
  relay-server/  # relayd binary: composition root, ports 7414/7415/7416
  relay-client/  # client library, used by tests and relayctl
  relay-cli/     # relayctl
  relay-bench/   # benchmark harness
tools/
  arch-check/    # dependency-graph and source-purity checks run in CI
fixtures/
  histories/     # JSONL operation histories, known-good and known-bad
  wal/           # old-version segment and snapshot fixtures (from R2)
  wire/          # RWP/1 frame corpora, valid and adversarial (from R6)
  seeds/         # failing-seed corpus for the simulator (from R3)
ci/
  gates.toml     # gate -> exact evidence commands, replayed on every CI run
docs/            # this documentation set
```

Crate responsibilities and forbidden dependencies:

| Crate | Responsibility | Forbidden dependencies |
| --- | --- | --- |
| `relay-core` | Types, limits, validation, and the pure `apply` state machine. Defines the `Clock`/`Rng`/`Disk`/`Net` environment traits but contains no implementation and no call site for them. | `tokio`, `rand`, any IO crate, `std::time::{SystemTime, Instant}` reads, threads, atomics for logic. |
| `relay-wal` | Segment format, append, fsync, recovery, compaction — all IO through the injected `Disk` trait. | direct `std::fs`, `tokio::fs`, wall-clock reads, `relay-raft`, `relay-wire`. |
| `relay-raft` | Election, log replication, snapshots, membership — all IO through injected `Net`, `Disk`, `Clock`, `Rng`. | direct sockets, direct filesystem, wall-clock reads, `relay-wire`. |
| `relay-sim` | Deterministic single-threaded implementations of the four environment traits plus the virtual-time executor. | `tokio` at runtime, OS randomness, real sleeps. |
| `relay-model` | Independent reference semantics, JSONL history encode/decode, Wing–Gong checker, bounded state-space explorer. | `relay-wal`, `relay-raft`, `relay-server` — the oracle must not share production plumbing. |
| `relay-wire` | RWP/1 frame codec, opcode bodies, bounded decoding, fuzz targets. | general-purpose serde on the wire path (ADR-0004), `tokio`. |
| `relay-server` | `relayd`: config, listener, tenant auth, request pipeline, composition of core+wal+raft with production environment impls. | `relay-sim` (test-only), test fixtures. |
| `relay-client` | Typed client over RWP/1 with leader-hint retry and backoff. | `relay-core` internals beyond public reply types. |
| `relay-cli` | `relayctl` over `relay-client`; human and JSON output. | direct wire encoding — it goes through `relay-client`. |
| `relay-bench` | Load generation, latency capture, statistical treatment. | production crates' test-only features. |

`tools/arch-check` enforces this table mechanically: it reads `cargo metadata`,
asserts each crate's dependency set is a subset of its allowlist, and scans
`relay-core`, `relay-wal`, and `relay-raft` sources for forbidden tokens
(`SystemTime::now`, `Instant::now`, `thread::sleep`, `rand::`, `std::fs::`,
`tokio::`). A violation is a CI failure, not a review comment.

### 3.4 Core interfaces to establish and preserve

The following signatures are normative. Names change only through a
superseding ADR and a migration. The first block is fixed across every
companion document:

```rust
// relay-core: pure, deterministic. No clock, rng, IO, or thread access.
pub fn apply(state: &CoreState, entry: &LogEntry) -> Applied; // returns NEW state (immutable style)
pub struct Applied { pub state: CoreState, pub outputs: Vec<Output>, }

pub enum Command {
    CreateQueue(QueueConfig), DeleteQueue(QueueId), SetQueueAttributes(QueueId, QueueConfigDelta),
    Send(SendCommand), SendBatch(Vec<SendCommand>),
    Receive(ReceiveCommand), Delete(DeleteCommand), ChangeVisibility(ChangeVisibilityCommand),
    Purge(QueueId), StartRedrive(RedriveCommand),
    CreateTopic(TopicConfig), DeleteTopic(TopicId), Subscribe(SubscribeCommand),
    Unsubscribe(SubscriptionId), Publish(PublishCommand),
    Tag(ResourceRef, Vec<TagPair>), Untag(ResourceRef, Vec<TagKey>),
    AdvanceTime(Nanos), // sole source of time inside the state machine (ADR-0005)
}

// Injected environment (production: tokio-backed; simulation: virtual-time single thread)
pub trait Clock: Send + Sync { fn monotonic(&self) -> Nanos; fn wall(&self) -> WallClock; }
pub trait Rng: Send { fn fill_bytes(&mut self, dst: &mut [u8]); }
pub trait Disk: Send + Sync {
    fn create(&self, path: &DiskPath) -> Result<FileHandle, DiskError>;
    fn open(&self, path: &DiskPath) -> Result<FileHandle, DiskError>;
    fn append(&self, f: &FileHandle, data: &[u8]) -> Result<u64, DiskError>;
    fn read_at(&self, f: &FileHandle, offset: u64, len: u32) -> Result<Bytes, DiskError>;
    fn fsync(&self, f: &FileHandle) -> Result<(), DiskError>;
    fn fsync_dir(&self, path: &DiskPath) -> Result<(), DiskError>;
    fn rename(&self, from: &DiskPath, to: &DiskPath) -> Result<(), DiskError>;
    fn delete(&self, path: &DiskPath) -> Result<(), DiskError>;
    fn list(&self, dir: &DiskPath) -> Result<Vec<DiskPath>, DiskError>;
}
pub trait Net: Send + Sync {
    fn listen(&self, addr: NodeAddr) -> Result<Listener, NetError>;
    fn connect(&self, addr: NodeAddr) -> BoxFuture<'static, Result<Conn, NetError>>;
}

// relay-wal
impl Wal {
    pub fn recover(disk: Arc<dyn Disk>, dir: DiskPath, opts: WalOptions) -> Result<(Wal, RecoveredState), WalError>;
    pub fn append(&mut self, records: &[Record]) -> Result<Lsn, WalError>; // buffered
    pub fn sync(&mut self) -> Result<Lsn, WalError>;                       // durable up to returned Lsn
    pub fn compact(&mut self, upto: Lsn, live: &LiveSet) -> Result<CompactionReport, WalError>;
}
```

The supporting types below extend that contract. Identifiers are newtypes so a
`QueueId` can never be passed where a `MessageId` is expected:

```rust
// ---- scalars and identifiers ----
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)] pub struct Nanos(pub u64); // virtual monotonic ns
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)] pub struct Lsn(pub u64);
#[derive(Clone, Copy, PartialEq, Eq, Hash)] pub struct RequestId(pub u64);
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)] pub struct QueueId(pub [u8; 16]);        // ULID
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)] pub struct TopicId(pub [u8; 16]);        // ULID
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)] pub struct MessageId(pub [u8; 16]);      // ULID
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)] pub struct SubscriptionId(pub [u8; 16]); // ULID
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)] pub struct RedriveTaskId(pub [u8; 16]);  // ULID

pub struct QueueName(String);   // validated ^[A-Za-z0-9_-]{1,80}$; ".fifo" suffix excluded from the 80
pub struct GroupId(String);     // <= 128 bytes, validated UTF-8
pub struct DedupId(String);     // <= 128 bytes, validated UTF-8
pub struct AttributeName(String); // ^[A-Za-z0-9_.-]{1,256}$; the "relay." prefix is reserved and rejected
pub struct TagKey(String);      // ^[A-Za-z0-9_.:/=+@-]{1,128}$
pub struct TagPair { pub key: TagKey, pub value: String } // value <= 256 bytes
pub enum ResourceRef { Queue(QueueId), Topic(TopicId) }

// ---- fixed limits (single source of truth: relay_core::limits) ----
pub const MAX_BODY_BYTES: u32 = 256 * 1024;            // FR-QUEUE-013; body + attributes combined
pub const MAX_BATCH_ENTRIES: usize = 10;               // FR-QUEUE-003
pub const MAX_RECEIVE_MESSAGES: u8 = 10;               // FR-QUEUE-004
pub const MAX_ATTRIBUTES: usize = 10;                  // FR-QUEUE-012
pub const VISIBILITY_MIN: Nanos = Nanos(0);
pub const VISIBILITY_MAX: Nanos = Nanos(12 * 3_600 * 1_000_000_000);   // 12 h
pub const VISIBILITY_DEFAULT: Nanos = Nanos(30 * 1_000_000_000);       // 30 s
pub const DELAY_MAX: Nanos = Nanos(900 * 1_000_000_000);               // 900 s
pub const RETENTION_MIN: Nanos = Nanos(60 * 1_000_000_000);            // 60 s
pub const RETENTION_MAX: Nanos = Nanos(14 * 86_400 * 1_000_000_000);   // 14 d
pub const RETENTION_DEFAULT: Nanos = Nanos(4 * 86_400 * 1_000_000_000); // 4 d
pub const DEDUP_WINDOW: Nanos = Nanos(300 * 1_000_000_000);            // fixed 300 s
pub const MAX_RECEIVE_COUNT_MIN: u32 = 1;
pub const MAX_RECEIVE_COUNT_MAX: u32 = 1_000;
pub const IN_FLIGHT_CAP_STANDARD: u32 = 120_000;       // FR-QUEUE-016
pub const IN_FLIGHT_CAP_FIFO: u32 = 20_000;            // FR-QUEUE-016
pub const PURGE_COOLDOWN: Nanos = Nanos(60 * 1_000_000_000);           // FR-QUEUE-015

// ---- configuration ----
pub enum QueueKind { Standard, Fifo }

pub struct QueueConfig {
    pub name: QueueName,
    pub kind: QueueKind,                 // Fifo requires the ".fifo" name suffix
    pub visibility_timeout: Nanos,       // VISIBILITY_MIN..=VISIBILITY_MAX, default 30 s
    pub default_delay: Nanos,            // 0..=DELAY_MAX
    pub retention: Nanos,                // RETENTION_MIN..=RETENTION_MAX, default 4 d
    pub max_in_flight: u32,              // <= kind cap; defaults to the kind cap
    pub redrive: Option<RedrivePolicy>,  // FR-QUEUE-017 (R4)
    pub content_based_dedup: bool,       // Fifo only (R4)
    pub tags: Vec<TagPair>,
}

pub struct RedrivePolicy {
    pub dead_letter_queue: QueueId,      // must exist, same kind as source
    pub max_receive_count: u32,          // 1..=1_000
}

pub struct QueueConfigDelta {
    pub visibility_timeout: Option<Nanos>,
    pub default_delay: Option<Nanos>,
    pub retention: Option<Nanos>,
    pub max_in_flight: Option<u32>,
    pub redrive: Option<Option<RedrivePolicy>>, // Some(None) clears the policy
    pub content_based_dedup: Option<bool>,
}

pub struct TopicConfig {                 // R5
    pub name: QueueName,                 // same grammar as queue names, no ".fifo"
    pub tags: Vec<TagPair>,
}

// ---- command payloads ----
pub struct SendCommand {
    pub queue: QueueId,
    pub body: Bytes,                        // body + encoded attributes <= MAX_BODY_BYTES
    pub attributes: Vec<MessageAttribute>,  // <= MAX_ATTRIBUTES, names unique
    pub delay: Option<Nanos>,               // None -> queue default_delay; Some in 0..=DELAY_MAX
    pub group_id: Option<GroupId>,          // required iff the queue is Fifo (R4)
    pub dedup_id: Option<DedupId>,          // Fifo only (R4)
}

pub struct MessageAttribute { pub name: AttributeName, pub value: AttributeValue }

pub enum AttributeValue {
    String(String),   // valid UTF-8
    Number(String),   // decimal string, optional sign and point, <= 38 significant digits
    Binary(Bytes),
}

pub struct ReceiveCommand {
    pub queue: QueueId,
    pub max_messages: u8,                   // 1..=MAX_RECEIVE_MESSAGES
    pub visibility_timeout: Option<Nanos>,  // None -> queue visibility_timeout
    // WaitTimeSeconds is a wire-layer concern (FR-QUEUE-009, R6); the core never waits.
}

pub struct Receipt {
    pub queue: QueueId,
    pub message: MessageId,
    pub lease_epoch: u64,                   // increments on every delivery of this message
    pub expires_at: Nanos,                  // informational; the state machine is authoritative
}
// The wire layer (R6) wraps Receipt as rh1_<base64url(...)> with an HMAC-SHA256
// tag per ADR-0006. Inside relay-core a Receipt is structural; forgery
// resistance is a wire-layer property, never a core property.

pub struct DeleteCommand { pub queue: QueueId, pub receipt: Receipt }

pub struct ChangeVisibilityCommand {
    pub queue: QueueId,
    pub receipt: Receipt,
    pub visibility_timeout: Nanos,          // 0 returns the message immediately
}

pub struct RedriveCommand {                 // R4
    pub source: QueueId,                    // must be configured as a DLQ
    pub destination: Option<QueueId>,       // None -> original source queue per message
    pub max_messages_per_advance: u32,      // bounded work per AdvanceTime tick
}

pub struct PublishCommand {                 // R5
    pub topic: TopicId,
    pub body: Bytes,
    pub attributes: Vec<MessageAttribute>,
    pub group_id: Option<GroupId>,          // honored by Fifo subscribers (R5)
    pub dedup_id: Option<DedupId>,
}

pub struct SubscribeCommand {               // R5
    pub topic: TopicId,
    pub queue: QueueId,
    pub filter: Option<FilterPolicy>,       // validated at subscribe time (FR-TOPIC-005)
}

pub struct FilterPolicy { pub clauses: Vec<FilterClause> } // AND of clauses; <= 16 clauses

pub struct FilterClause { pub attribute: AttributeName, pub predicate: FilterPredicate }

pub enum FilterPredicate {                  // FR-TOPIC-004
    Exact(Vec<AttributeValue>),             // OR within a clause; <= 16 values
    AnythingBut(Vec<AttributeValue>),
    Prefix(String),
    NumericRange { min: Option<f64>, min_inclusive: bool, max: Option<f64>, max_inclusive: bool },
    Exists(bool),
}

// ---- log entries and outputs ----
pub struct LogEntry {
    pub lsn: Lsn,
    pub request: Option<RequestId>,         // None for internally scheduled AdvanceTime
    pub command: Command,
}

pub enum Output {
    Reply { request: RequestId, reply: Result<Reply, CoreError> },
    LeaseExpired { queue: QueueId, message: MessageId, receive_count: u32 },
    MessageDeadLettered { source: QueueId, dlq: QueueId, message: MessageId },   // R4
    MessageExpired { queue: QueueId, message: MessageId },                        // R4 retention
    RedriveProgressed { task: RedriveTaskId, moved: u64, remaining: u64 },        // R4
}

pub enum Reply {
    QueueCreated(QueueId),
    QueueDeleted,
    QueueAttributesSet,
    Sent { message: MessageId },
    SentBatch(Vec<Result<MessageId, CoreError>>),   // positional, one per entry
    Received(Vec<Delivery>),                        // possibly empty; the core never waits
    Deleted,                                        // idempotent (FR-QUEUE-006)
    VisibilityChanged { expires_at: Option<Nanos> },// None when returned immediately
    Purged { removed: u64 },
    RedriveStarted(RedriveTaskId),                  // R4
    TopicCreated(TopicId),                          // R5
    TopicDeleted,                                   // R5
    Subscribed(SubscriptionId),                     // R5
    Unsubscribed,                                   // R5
    Published { matched_subscriptions: u32 },       // R5
    Tagged,
    Untagged,
    TimeAdvanced { now: Nanos },
}

pub struct Delivery {
    pub message: MessageId,
    pub body: Bytes,
    pub attributes: Vec<MessageAttribute>,
    pub sent_at: Nanos,
    pub receive_count: u32,     // attempt ordinal: prior unconsumed deliveries + 1
    pub receipt: Receipt,
}

// ---- state (persistent data structures; apply never mutates its input) ----
pub struct CoreState {
    pub now: Nanos,                                     // advanced only by AdvanceTime
    pub queues: im::OrdMap<QueueId, QueueState>,
    pub queue_names: im::OrdMap<QueueName, QueueId>,    // uniqueness index
    pub topics: im::OrdMap<TopicId, TopicState>,         // R5
    pub subscriptions: im::OrdMap<SubscriptionId, SubscriptionState>, // R5
    pub redrive_tasks: im::OrdMap<RedriveTaskId, RedriveTaskState>,   // R4
    pub ulid_seq: UlidSeq,   // deterministic ULID generation from (now, per-entry counter)
}

pub struct QueueState {
    pub id: QueueId,
    pub config: QueueConfig,
    pub messages: im::OrdMap<MessageId, StoredMessage>,
    pub delayed: im::OrdSet<(Nanos, MessageId)>,        // (ready_at, id)
    pub available: im::OrdSet<(Nanos, MessageId)>,      // (available_at, id) — selection order
    pub in_flight: im::OrdMap<MessageId, Lease>,
    pub last_purge_at: Option<Nanos>,                   // FR-QUEUE-015 cooldown
    pub dedup_window: im::OrdMap<DedupId, (MessageId, Nanos)>, // R4, Fifo only
    pub groups: im::OrdMap<GroupId, GroupState>,        // R4, Fifo only
}

pub struct StoredMessage {
    pub id: MessageId,
    pub body: Bytes,
    pub attributes: Vec<MessageAttribute>,
    pub sent_at: Nanos,
    pub expires_at: Nanos,          // sent_at + retention (enforced at R4)
    pub receive_count: u32,         // completed-without-delete deliveries (see §3.5)
    pub lease_epoch: u64,           // last granted epoch; 0 before first delivery
    pub group: Option<GroupId>,     // R4
    pub state: MessageState,
}

pub enum MessageState { Delayed { ready_at: Nanos }, Available { available_at: Nanos }, InFlight }
// Deleted, DeadLettered, and Expired are terminal: the message leaves `messages`
// entirely; DeadLettered re-enters as a new StoredMessage in the DLQ (R4).

pub struct Lease {
    pub epoch: u64,                 // equals StoredMessage.lease_epoch while live
    pub granted_at: Nanos,
    pub expires_at: Nanos,
}

// ---- errors ----
pub enum CoreError {
    InvalidInput { field: &'static str, reason: ValidationReason },
    QueueNotFound(QueueId),
    QueueNameTaken(QueueName),
    TopicNotFound(TopicId),                 // R5
    SubscriptionNotFound(SubscriptionId),   // R5
    PayloadTooLarge { limit: u32, actual: u64 },
    ReceiptRejected(ReceiptRejection),
    PurgeInProgress { queue: QueueId, retry_at: Nanos },
    InFlightCapExceeded { queue: QueueId, cap: u32 },
    RedriveAlreadyActive { queue: QueueId },        // R4
    NotADeadLetterQueue { queue: QueueId },         // R4
    TimeRegression { now: Nanos, attempted: Nanos },
}

pub enum ValidationReason {
    Empty, TooLong, TooShort, BadCharacter, OutOfRange,
    DuplicateAttributeName, ReservedPrefix, BadNumberFormat, BadUtf8,
    FifoFieldOnStandardQueue, MissingGroupId, FifoNotYetSupported, // Fifo arrives at R4
    BatchEmpty, BatchTooLarge,
    FilterTooComplex, UnknownFilterOperator,        // R5
}

pub enum ReceiptRejection {
    ForeignQueue,           // receipt.queue != command.queue
    NotInFlight,            // message exists but holds no live lease (expired or returned)
    EpochSuperseded { current: u64, presented: u64 },
}
```

Every `CoreError` maps to exactly one wire error category in §4.2, and that
mapping is a compile-checked exhaustive `match` in `relay-wire` (R6), not a
convention. `CoreState` uses persistent (structurally shared) maps so that
`apply` can return a new state cheaply; the model checker (R1) and the
simulator (R3) fork states millions of times, and copy-on-write sharing is
what makes exhaustive exploration affordable.

### 3.5 The message lifecycle state machine

State names are fixed: `Delayed`, `Available`, `InFlight`, `Deleted`,
`DeadLettered`, `Expired`. `Deleted`, `DeadLettered`, and `Expired` are
terminal. Every transition is driven by exactly one log entry:

| # | From | To | Triggering entry | Guard | Gate |
| --- | --- | --- | --- | --- | --- |
| T1 | (none) | Delayed | `Send` / `SendBatch` entry / `Publish` fanout | effective delay > 0; all validation passed | R1 (Publish R5) |
| T2 | (none) | Available | `Send` / `SendBatch` entry / `Publish` fanout | effective delay == 0 | R1 (Publish R5) |
| T3 | Delayed | Available | `AdvanceTime` | `now >= ready_at`; promoted in `(ready_at, message_id)` order | R1 |
| T4 | Available | InFlight | `Receive` | selected by §6.4 order; in-flight cap not exceeded; (Fifo: group unblocked, R4) | R1 |
| T5 | InFlight | Deleted | `Delete` | receipt passes the §6.4 validation matrix; lease live; epoch equal | R1 |
| T6 | InFlight | Available | `AdvanceTime` | `now >= lease.expires_at`; increments `receive_count` (FR-QUEUE-005) | R1 |
| T7 | InFlight | Available | `ChangeVisibility` with timeout 0 | receipt valid; increments `receive_count` | R1 |
| T8 | InFlight | InFlight | `ChangeVisibility` with timeout > 0 | receipt valid; lease `expires_at := now + timeout` | R1 |
| T9 | InFlight \| Available | DeadLettered | `AdvanceTime` (at return) / `Receive` (at would-be grant) | redrive policy set and `receive_count >= max_receive_count` | R4 |
| T10 | Delayed \| Available \| InFlight | Deleted | `Purge` | purge cooldown passed; removes in-flight messages too (FR-QUEUE-015) | R1 |
| T11 | Delayed \| Available \| InFlight | Expired | `AdvanceTime` | `now >= expires_at` (retention, FR-QUEUE-014) | R4 |
| T12 | DeadLettered (in DLQ, Available) | Available (in source) | `StartRedrive` + `AdvanceTime` progress | active redrive task; bounded batch per tick | R4 |

`receive_count` semantics are fixed here and used identically by R1 and R4: it
counts deliveries that ended without a delete. It increments at T6 and T7 (the
delivery failed to consume the message), never at grant; a `Delivery` reports
the attempt ordinal `receive_count + 1`. The R4 dead-letter guard therefore
fires on the return path, after the increment, which is what makes the
`maxReceiveCount` boundary exact (FR-QUEUE-017).

The state machine is total: any entry that matches no legal transition
produces an error `Output` and an unchanged state, never a panic and never a
silent drop. R1 proves T1–T8 and T10 under the model checker; R4 proves
T9, T11, and T12; R5 proves the fanout instantiations of T1/T2. No later gate
may alter an earlier gate's transitions without a superseding ADR.

### 3.6 The lease lifecycle

A lease is the exclusive right to delete one message during one delivery:

```text
Granted ──(ChangeVisibility > 0)──▶ Extended ──┐   (Extended may repeat)
   │                                           │
   ├──(Delete, epoch equal)──────▶ Consumed    │   terminal
   ├──(ChangeVisibility == 0)────▶ Released ◀──┘   terminal for this lease
   └──(AdvanceTime ≥ expiry)─────▶ Expired         terminal for this lease
```

Epoch rules, binding for every gate:

1. `StoredMessage.lease_epoch` starts at 0 and increments by exactly 1 at each
   grant (T4). The `Receipt` issued by that grant embeds the new epoch.
2. A `Delete` or `ChangeVisibility` is valid only while the message is
   `InFlight` **and** the presented epoch equals the stored epoch. `Extended`
   does not change the epoch; a lease and its receipt survive any number of
   extensions.
3. Once a lease reaches `Released` or `Expired`, the message is `Available`
   with its epoch unchanged; the old receipt now fails with
   `ReceiptRejected(NotInFlight)`. After the next grant the same receipt fails
   with `ReceiptRejected(EpochSuperseded)`. Both rejections satisfy
   FR-QUEUE-007; which one occurs depends only on log order, so the outcome is
   deterministic and model-checkable.
4. `Consumed` removes the message. A later `Delete` naming a message id absent
   from the queue succeeds idempotently (FR-QUEUE-006); the core cannot and
   need not distinguish "already deleted" from "never existed" — rejecting
   fabricated receipts is the wire layer's HMAC obligation (NFR-SEC-001, R6).
5. Exactly one live lease can exist per message because grants happen only
   from `Available` and returns only from `InFlight`; this is correctness
   property P-02 (LEASE-EXCL) and is exhaustively checked at R1 and again
   under partitions at R7 (P-08).

## 4. Cross-Phase Engineering Rules

### 4.1 Test-first workflow

Each ticket follows this merge order:

1. add a test or fixture that fails for the intended reason, in the test
   family named by the ticket (spine families: `CORE-`, `STOR-`, `CRSH-`,
   `SIM-`, `MODL-`, `FIFO-`, `TOPC-`, `WIRE-`, `FUZZ-`, `RAFT-`, `ADMN-`,
   `OPSX-`, `MIGR-`, `SOAK-`, `BENCH-`, `MUT-`, `MKT-`);
2. add or extend the typed boundary and its error variants;
3. implement the smallest behavior that makes the failing test pass;
4. add property-based coverage over the same behavior where the input space is
   nontrivial (seeded, shrinking, deterministic replay of failures);
5. add the adversarial and interruption cases named by the gate's §X.7;
6. run the crate tests, `tools/arch-check`, the current gate's suite, and the
   replay of every accepted earlier gate from `ci/gates.toml`;
7. update statuses, limits, and traceability rows in the affected documents in
   the same changeset.

Deterministic suites are zero-flake: a flake is a bug with a seed attached,
and the seed goes into `fixtures/seeds/` before the fix merges
(NFR-MAINT-002). Tests never sleep on the wall clock; time is `SimClock` or
explicit `AdvanceTime` entries. Live-infrastructure suites (real disks at R9
soak, real clusters at R10 drills) have named quarantine rules in
[OPERATIONS_TEST_PLAN.md](./OPERATIONS_TEST_PLAN.md) and never gate a pull
request.

### 4.2 Error taxonomy

Every failure anywhere in Relay maps to exactly one category below before it
crosses a process or wire boundary. The wire code is the stable u16 carried in
RWP/1 error replies from R6 onward; the category names are used in logs,
`relayctl` output, and documentation from R1 onward:

| Category | Wire code | Retryable | Meaning and rules |
| --- | --- | --- | --- |
| `InvalidInput` | 1001 | no | A field failed validation. The reply names the field and a `ValidationReason`; it never echoes body bytes. |
| `NotFound` | 1002 | no | The named queue, topic, subscription, or task does not exist. |
| `Conflict` | 1003 | no | The operation conflicts with current state: name taken, purge cooldown active, redrive already active. |
| `PreconditionFailed` | 1004 | no | A receipt was rejected: foreign, not in flight, or epoch-superseded (FR-QUEUE-007). Re-receive to obtain a fresh receipt. |
| `Throttled` | 1005 | yes, after backoff | A quota, rate limit, or the per-queue in-flight cap (FR-QUEUE-016) refused work. The reply carries a retry-after hint. |
| `Unauthorized` | 1006 | no | Authentication failed: unknown tenant or bad HMAC (R6). Constant-time comparison; no detail leaks. |
| `Forbidden` | 1007 | no | Authenticated but denied by ACL; deny precedence (R6). |
| `PayloadTooLarge` | 1008 | no | Body plus attributes exceed 256 KiB (FR-QUEUE-013), or a frame exceeds 1 MiB (R6). |
| `Internal` | 1009 | no | An invariant failed. Logged with full context server-side, opaque correlation id client-side. Every occurrence is a bug. |
| `Unavailable` | 1010 | yes, with backoff | The node is shutting down, recovering, or load-shedding (NFR-AVAIL-003). |
| `NotLeader` | 1011 | yes, at the hinted leader | Replicated mode only (R7): the reply carries a leader hint (FR-REPL-007). |
| `Timeout` | 1012 | uncertain — retry only idempotent operations | A deadline elapsed with the outcome unknown. Sends may have landed (NG-09); receives are safe to retry. |

Rules: codes are append-only — a code is never renumbered or reused; the
`CoreError` → category mapping is exhaustive and compile-checked; user-facing
messages are complete sentences that never contain message bodies, attribute
values, credentials, or receipt key material; server logs carry structured
detail under the redaction rules of §4.4.

### 4.3 Determinism and clock discipline

- Per ADR-0005, `relay-core` contains no wall-clock read, no OS randomness,
  and no thread. `CoreState.now` advances only when an `AdvanceTime` entry is
  applied, so every lease expiry, delay promotion, retention sweep, dedup
  window boundary, and purge cooldown is a deterministic function of the log.
- In production, `relay-server` samples its injected `Clock` and appends
  `AdvanceTime` entries on a bounded cadence; in simulation and model
  checking, the harness appends them explicitly. The same log bytes always
  produce the same `CoreState`, byte for byte, on every platform — this is
  the replay property that R2 recovery, R3 simulation, and R7 Raft all
  inherit for free.
- Randomness is a seeded PRNG behind the `Rng` trait. Production seeds from
  the OS once at startup and logs the fact (not the seed) at debug level;
  simulation seeds from the scenario. `relay-core` never consumes randomness:
  ULIDs derive from `(CoreState.now, per-entry counter)` via `UlidSeq`, so id
  generation replays exactly.
- Election timeouts (R7) draw from the injected `Rng`; heartbeat and election
  constants in spine terms (100 ms heartbeat, 500–1000 ms randomized election
  timeout) are simulated-time values, which is why partition schedules
  reproduce from a seed.
- Any code that would read `SystemTime::now`, `Instant::now`, or
  `rand::thread_rng` inside `relay-core`, `relay-wal`, or `relay-raft` fails
  `tools/arch-check` before it fails review.

### 4.4 Data and privacy defaults

- Message bodies, attribute values, group ids, dedup ids, and tag values are
  customer data. They are never logged, never traced, never included in error
  messages, never sampled into metrics labels, and never written into support
  bundles. This holds at every gate, not only after observability lands at R8.
- Redaction is tested, not asserted: from R1 onward the fixtures include
  canary byte strings (`RELAY_CANARY_` prefixed) placed into bodies,
  attributes, and tags, and every log, error, panic message, and diagnostic
  artifact produced by any test run is scanned for canaries. A canary hit
  fails CI (NFR-SEC-003 begins here; its terminal adversarial evidence is R6).
- Structured logs and traces may carry: queue and topic ids and names, message
  ids, counts, sizes, durations, error categories, and LSNs. Names are
  operator-chosen; documentation warns that queue names appear in logs.
- Credentials (tenant HMAC keys, R6) live only in the configured secret
  source, are compared in constant time, are zeroized on drop where the
  platform allows, and have no `Debug` representation.
- The data directory is created `0700` and verified at startup (NFR-SEC-005,
  R2). Backups and support bundles inherit the same body-exclusion rules.

### 4.5 Performance budgets

Budgets are gates to measure, never claims to publish before R9
(NFR-PERF-005). Two numbers exist per budget so a slow CI runner cannot
silently redefine the product: the target from the NFR register, and a looser
CI regression ceiling that trips before the target erodes. Budgets are
enforced as CI regression checks from R2 onward — R1 is in-memory and its
speed proves nothing a claim could use:

| Budget | Register target | First tracked | Published |
| --- | --- | --- | --- |
| Sustained send+receive+delete, 256 B bodies, single node | ≥ 20,000 msg/s (NFR-PERF-001) | R2 (regression only) | R9 |
| p99 send-to-ack with fsync-before-ack | ≤ 15 ms (NFR-PERF-002) | R2 (regression only) | R9 |
| Long-poll wakeup after a matching send | ≤ 10 ms (NFR-PERF-003) | R6 (regression only) | R9 |
| Crash recovery of a 10 GiB WAL | ≤ 30 s (NFR-PERF-004) | R2 (scaled-down fixture) | R9 |
| Clean leader kill to first new acked write | ≤ 5 s (NFR-AVAIL-002) | R7 (simulated time) | R9 (measured) |

Verification-apparatus budgets are budgets too, because a checker nobody can
afford to run is not a gate: the R1 exhaustive model-check suite completes
within 5 minutes on the CI runner; each Wing–Gong history check carries the
per-history wall-clock budget from ADR-0007; the R3 CI simulation batch runs
a fixed seed count within 10 minutes, with the larger sweep nightly. Reference
hardware for all published numbers: 8 vCPU / 16 GiB / local NVMe, Linux 6.x.

### 4.6 Dependency review gates

Relay builds its own state machine, WAL, Raft, simulator, model checker, wire
codec, and benchmark harness in this repository — those are the product
thesis, and adopting a framework for any of them requires a superseding ADR
(ADR-0002, ADR-0003, ADR-0004). It may use narrow, auditable dependencies:

- permitted classes: hashing and MACs (`sha2`, `hmac`, `crc32c`), byte
  handling (`bytes`), persistent collections (`im`), property testing
  (`proptest`), fuzzing (`cargo-fuzz`/`libfuzzer` targets in `relay-wire`),
  `serde_json` only inside `relay-model` (JSONL histories) and config/tooling,
  `tokio` only in `relay-server`, `relay-client`, `relay-cli`, `relay-bench`;
- excluded without a superseding ADR: general-purpose databases or embedded
  stores as the queue store, external Raft or consensus crates, wire
  serialization frameworks on the RWP/1 path, async runtimes below
  `relay-server`, and any crate that spawns background threads inside a
  deterministic component.

Mechanics, in force from R0:

1. `cargo-deny` runs in CI with an explicit license allowlist, the RustSec
   advisory feed, a duplicate-version check, and a ban list that encodes the
   exclusions above.
2. Every dependency is exact-pinned (`=x.y.z`) in workspace `Cargo.toml`; the
   committed `Cargo.lock` is authoritative and CI builds with `--locked`.
   Lockfile drift fails the build (NFR-SEC-008 begins at R0, terminal R10).
3. A new dependency merges only with a recorded review in its pull request:
   license, maintenance cadence, transitive closure size, `unsafe` usage,
   `build.rs` behavior, and whether a narrower hand-written alternative was
   considered.
4. Dependency upgrades are their own pull requests, never riders on feature
   work, so gate evidence re-runs attribute regressions correctly.

## 5. R0 — Repository, Toolchain, CI, and Architecture Checks

**Status:** planned. Nothing in this gate exists.

**Effort range:** 3–5 focused days.

### 5.1 Why this gate exists

Every later gate's evidence is a CI run. If the toolchain is unpinned, the
lints are advisory, the dependency policy is unenforced, or the crate
boundaries are conventions instead of checks, then every later "green" is
untrustworthy. R0 builds the enforcement machinery first so that the very
first line of `relay-core` is already forbidden from reading a clock, and so
that the evidence-replay discipline (NFR-MAINT-004) exists before there is any
evidence to replay. R0 also prevents the specific dishonesty this project is
structured against: a repository full of named crates that looks like a
message queue. R0's crates compile to empty, documented shells, and the README
says exactly that.

### 5.2 Prerequisites

- ADR-0001 (Rust, edition 2024, MSRV 1.85, deny-warnings, clippy pedantic
  baseline, cargo-deny) is accepted.
- ADR-0011 (tier-1 Linux x86_64/aarch64, tier-2 macOS aarch64 dev-only) is
  accepted; it fixes the CI matrix.
- The GitHub repository exists under the Zachshotamartin account with the
  documentation set on the default branch, and `gh` is authenticated.
- The docs/README.md precedence order and status vocabulary are in place, so
  the status-discipline check has something normative to enforce.

### 5.3 Owned files, interfaces, and state

R0 creates:

- workspace `Cargo.toml` naming the ten crates of §3.3 plus
  `tools/arch-check`; committed `Cargo.lock`;
- `rust-toolchain.toml` pinning the stable toolchain; an MSRV field
  (`rust-version = "1.85"`) in every crate manifest;
- `rustfmt.toml`, `clippy.toml`, and workspace lint tables
  (`[workspace.lints]`) encoding deny-warnings and the pedantic baseline with
  each allow individually justified in a comment;
- `deny.toml` with the license allowlist, advisory feed, duplicate check, and
  ban list from §4.6;
- `.github/workflows/ci.yml` with the job graph of §5.4;
- `tools/arch-check/` as a small Rust binary (it is itself tested);
- `ci/gates.toml`, the machine-readable gate registry;
- crate skeletons: each crate contains `src/lib.rs` with
  `#![forbid(unsafe_code)]` (exceptions later require an ADR), a crate-level
  doc comment naming its §3.3 responsibility and forbidden dependencies, and
  no other items;
- `fixtures/` and `docs/` layout directories, each with a README stating that
  directory's contract; a contract statement is documentation, and no code is
  claimed to exist behind it.

The gate registry format is fixed here because every later gate appends to it:

```toml
# ci/gates.toml — replayed in full by every CI run (NFR-MAINT-004)
schema = 1

[gate.R0]
status  = "planned"   # flips to "accepted" in the pull request that closes R0
section = "BUILD_PLAN.md §5"
commands = [
  "cargo fmt --all -- --check",
  "cargo clippy --workspace --all-targets --locked -- -D warnings",
  "cargo test --workspace --locked",
  "cargo deny check",
  "cargo run -p arch-check --locked",
]
```

The arch-check configuration is data, not code, so boundary changes are
reviewable diffs:

```toml
# tools/arch-check/arch.toml
[crate.relay-core]
allowed-deps = ["im"]
forbidden-tokens = [
  "SystemTime::now", "Instant::now", "thread::sleep",
  "rand::", "std::fs::", "tokio::", "std::net::",
]

[crate.relay-wal]
allowed-deps = ["relay-core", "bytes", "crc32c"]
forbidden-tokens = ["std::fs::", "SystemTime::now", "Instant::now", "tokio::"]

[crate.relay-model]
allowed-deps = ["relay-core", "serde", "serde_json", "sha2"]
forbidden-deps = ["relay-wal", "relay-raft", "relay-server"]
```

### 5.4 Algorithms and state behavior

The CI pipeline is an ordered job graph; later jobs do not run if an earlier
job fails, so evidence is never produced on top of a formatting or lint
violation:

1. `fmt`: `cargo fmt --all -- --check` on Linux x86_64.
2. `lint`: `cargo clippy --workspace --all-targets --locked -- -D warnings`
   on the pinned toolchain.
3. `msrv`: `cargo check --workspace --locked` on 1.85 to prevent silent MSRV
   drift.
4. `deny`: `cargo deny check` (licenses, advisories, bans, duplicates).
5. `arch`: `cargo run -p arch-check --locked`.
6. `test`: `cargo test --workspace --locked` on the matrix from ADR-0011 —
   Linux x86_64 and aarch64 required; macOS aarch64 advisory (dev-only tier).
7. `gates`: parse `ci/gates.toml`, run every command of every gate whose
   status is `accepted`, and fail on any non-zero exit. At R0 close this
   replays R0 itself; at R7 it replays seven gates. Runtime growth is managed
   by each gate's §4.5 CI budget, never by skipping.

The arch-check algorithm:

1. run `cargo metadata --locked` and build the crate dependency graph;
2. for each crate with an `arch.toml` entry, assert its direct dependency set
   is a subset of `allowed-deps` (workspace-internal and external alike) and
   disjoint from `forbidden-deps`;
3. scan the crate's `src/` for `forbidden-tokens` outside of comments and
   `#[cfg(test)]` modules, reporting file and line for each hit;
4. assert every workspace dependency version requirement is exact (`=`);
5. assert `ci/gates.toml` parses, every gate section from §1.2 is present
   exactly once, and no gate is marked `accepted` without its commands;
6. exit non-zero listing every violation; the check is total and never
   silently skips an unreadable file — an unreadable file is itself a
   violation.

The status-discipline check is part of arch-check step 5's documentation
pass: it scans `docs/` for the four status words applied to deliverables,
fails on any claim word ("supports", "guarantees", "provides") applied to a
`planned` item without the word "planned" in the same sentence or table row,
and validates every relative link in `docs/` resolves to a file.

### 5.5 Implementation tickets and sequence

1. **R0.01 — Create the workspace and pin the toolchain.** Workspace
   `Cargo.toml`, ten crate skeletons plus `arch-check`, `rust-toolchain.toml`,
   `rust-version` fields, committed `Cargo.lock`. Done when `cargo build
   --workspace --locked` succeeds on a clean checkout with only the pinned
   toolchain installed.
2. **R0.02 — Establish the lint baseline.** `rustfmt.toml`, workspace lint
   tables with deny-warnings and clippy pedantic, each allow justified inline.
   Done when `fmt` and `lint` jobs pass and a seeded violation of each kind
   fails.
3. **R0.03 — Configure cargo-deny.** License allowlist, advisory feed, ban
   list encoding §4.6 exclusions, duplicate detection. Done when `cargo deny
   check` passes and a seeded GPL dependency and a seeded banned crate each
   fail in a throwaway branch recorded in the PR.
4. **R0.04 — Write arch-check: dependency graph.** `cargo metadata` parsing,
   allowlist/denylist enforcement, exact-pin enforcement, with unit tests over
   fixture metadata JSON. Done when the tests pass and adding `rand` to
   `relay-core` fails CI.
5. **R0.05 — Write arch-check: source purity scan.** Token scan with comment
   and `#[cfg(test)]` exclusion, tested against fixture sources containing
   each forbidden token in code, comments, and test modules. Done when only
   the code hits fail.
6. **R0.06 — Write arch-check: gates and docs pass.** `ci/gates.toml` schema
   validation, status-word scan, relative-link validation. Done when a seeded
   broken link, a seeded unearned claim, and a seeded malformed gate entry
   each fail with a file-and-line message.
7. **R0.07 — Author ci/gates.toml.** All eleven gate sections, R0 populated,
   R1–R10 `planned` with empty command lists. Done when the `gates` job
   replays R0's commands.
8. **R0.08 — Build the CI workflow.** The seven-job graph of §5.4 on the
   ADR-0011 matrix, with actions pinned by full commit SHA and no cache used
   by the `gates` job. Done when a pull request shows all jobs green and a
   seeded failure in each job blocks merge.
9. **R0.09 — Protect the branch.** Require the CI checks, require review,
   forbid force-push on the default branch, verified via `gh api` read-back
   recorded in the PR. Done when an unreviewed push is rejected.
10. **R0.10 — Establish test conventions.** Document (in
    [OPERATIONS_TEST_PLAN.md](./OPERATIONS_TEST_PLAN.md) cross-reference) and
    enforce the family-prefix naming for test functions (`core_`, `modl_`,
    lowercased from spine §7 families) so CI can report per-family counts;
    wire the canary scan of §4.4 into the `test` job. Done when a test named
    outside the convention in a `tests/` tree is flagged and a seeded canary
    in captured test output fails CI.
11. **R0.11 — Write the truthful README.** Repository README states the §2.2
    claim verbatim, links the documentation set, shows contributor setup
    (`rustup`, `cargo build --workspace --locked`, `cargo test`), and shows no
    install instructions. Done when the status-discipline check passes over
    it.
12. **R0.12 — Close the gate.** Flip `gate.R0.status` to `accepted` in the
    same PR that records the evidence checklist of §5.9. Done when CI replays
    R0 green on the merge commit.

### 5.6 Test-driven evidence matrix

| Test | First failing condition | Required passing assertion |
| --- | --- | --- |
| toolchain pin | A contributor toolchain differs from `rust-toolchain.toml`. | Clean-checkout build uses the pinned version; CI logs record it. |
| msrv check | A dependency or syntax raises the effective MSRV above 1.85. | `cargo check --locked` passes on 1.85 for the whole workspace. |
| fmt gate | Any file diverges from `rustfmt.toml`. | `cargo fmt --all -- --check` exits zero. |
| clippy gate | Any warning at the pedantic-with-justified-allows baseline. | `cargo clippy --all-targets -- -D warnings` exits zero. |
| deny: license | A dependency outside the allowlist enters the graph. | Seeded GPL test-dependency fails `cargo deny check` in the recorded probe branch. |
| deny: bans | A banned crate class (external raft, embedded DB, serde on wire path) enters the graph. | Seeded banned crate fails; ban list matches §4.6 exclusions. |
| lockfile authority | `Cargo.lock` drifts from manifests or CI resolves freshly. | All CI commands run `--locked`; a drifted lockfile fails the first job that builds. |
| arch: dependency graph | `relay-core` gains any dep beyond its allowlist. | Injected `rand` dependency fails arch-check with crate and dep named. |
| arch: purity scan | `SystemTime::now` (or any forbidden token) appears in `relay-core` code. | Injected call fails with file and line; the same token in a comment or `#[cfg(test)]` passes. |
| arch: exact pins | A `^`/`~`/bare version requirement appears in any manifest. | Injected `bytes = "1"` fails; `bytes = "=1.9.0"` passes. |
| gates replay | An accepted gate's command is removed, altered, or failing. | The `gates` job runs every accepted gate's exact command list and fails on any non-zero exit. |
| docs status discipline | A `planned` deliverable is described with an unqualified claim, or a relative doc link dangles. | Seeded unearned claim and seeded broken link each fail with file and line. |
| canary scan | Captured test output contains a `RELAY_CANARY_` string. | Seeded canary emission fails the `test` job. |
| branch protection | A direct push or force-push lands on the default branch. | `gh api` read-back shows required checks, required review, and force-push disabled. |

### 5.7 Failure and security cases

- GitHub Actions are pinned by full commit SHA, never by mutable tag; the
  `gates` job runs without caches so replayed evidence cannot be satisfied
  from a poisoned cache.
- CI holds no secrets at R0 — there is nothing to deploy and nothing to
  publish. Any workflow step that requests a secret is a review failure.
- A yanked or advisory-flagged dependency turns the `deny` job red; the
  response is a dedicated upgrade PR, never `--allow` riders on feature work.
- If the pinned toolchain is unavailable on a runner, the job fails rather
  than falling back to "latest stable"; a toolchain bump is its own PR.
- arch-check failing open is itself tested: fixture cases include an
  unreadable file, a malformed `arch.toml`, and an empty crate list, each of
  which must exit non-zero.
- `#![forbid(unsafe_code)]` is workspace-wide at R0; the first crate that
  needs `unsafe` (plausibly `relay-wal` io_uring paths under ADR-0011's
  optional backend) requires a superseding ADR and a scoped
  `#![deny(unsafe_code)]` relaxation with per-block justification.

### 5.8 Migration, documentation, and installation work

There is no migration: no user, no data, no released artifact. Documentation
work is the truthful README (R0.11), the directory-contract stubs of §5.3,
and a `CONTRIBUTING` section documenting the merge order of §4.1 and the
zero-flake policy. Installation work is deliberately absent: R0 publishes no
binary, no crate to crates.io, no container image, and no install one-liner.
The only supported invocation after R0 is `cargo test --workspace --locked`
on a contributor machine.

### 5.9 Acceptance evidence

R0 is accepted only when all of the following are recorded in the closing
pull request:

- [ ] a clean-checkout build and test log on the pinned toolchain and on MSRV
      1.85, for Linux x86_64 and aarch64;
- [ ] all seven CI jobs green on the merge commit, with the `gates` job
      replaying R0's own command list;
- [ ] recorded probe branches showing each seeded violation class failing:
      fmt, clippy, license, ban, lockfile drift, arch dependency, arch purity,
      inexact pin, broken doc link, unearned claim, canary emission;
- [ ] branch-protection read-back from `gh api`;
- [ ] the README carrying the §2.2 honest claim verbatim;
- [ ] `ci/gates.toml` with `gate.R0.status = "accepted"` and every other gate
      `planned`.

### 5.10 Explicit deferrals

R0 takes no credit for any queue semantics. It defers all of: `relay-core`
behavior (R1), durability (R2), simulation (R3), FIFO/delay/DLQ (R4), topics
(R5), the wire protocol, authentication, and fuzzing (R6), replication (R7),
observability (R8), benchmarks (R9), and packaging or installation (R10). The
crate skeletons are enforcement targets, not features; describing them as a
"queue implementation" anywhere is a status-discipline failure that R0's own
check must catch.

### 5.11 Requirements traced

R0 terminally owns no requirement. It begins:

- **NFR-MAINT-004** (CI green replays the accepted evidence of every prior
  gate; terminal R10) — `ci/gates.toml` and the `gates` job are its
  mechanism, live from this gate forward;
- **NFR-SEC-008** (exact-pinned, reviewed dependencies with provenance;
  terminal R10) — pinning, `cargo-deny`, and the review checklist start here;
- **NFR-MAINT-001** (failing test first for every parser, reducer, and
  transition; terminal R10, audited each gate) — the workflow of §4.1 is
  binding from the first R1 ticket;
- **NFR-MAINT-005** (documentation status discipline; terminal R10) — the
  status and link checks enforce it mechanically;
- **NFR-SEC-003** (secrets and customer data never in logs or diagnostics;
  terminal R6) — the canary scan machinery lands here so R1 fixtures can use
  it on day one.

Per §16, none of these IDs lists R0 as terminal owner; R0 appears in their
rows as the beginning gate only.

## 6. R1 — Single-Node In-Memory Core Semantics Under the Model Checker

**Status:** planned.

**Effort range:** 10–15 focused days.

### 6.1 Why this gate exists

Everything Relay will ever claim reduces to the correctness of
`apply(state, entry)`. R2 wraps it in durability, R3 schedules it under
faults, R7 replicates its log — none of those layers can repair a wrong
transition, and every one of them can hide one behind IO noise. R1 therefore
proves the core semantics while they are still in memory, where the model
checker can enumerate interleavings exhaustively and the property tester can
shrink counterexamples to minimal command sequences. R1 also builds the
oracle itself: the independent reference model, the JSONL history format, and
the Wing–Gong linearizability checker of ADR-0007 that every later gate cites
as its judge. A defect in the oracle is as damaging as a defect in the core,
so R1's evidence includes checker self-tests against deliberately broken
cores.

### 6.2 Prerequisites

- R0 is accepted; arch-check already forbids `relay-core` from clocks,
  randomness, and IO, so R1 cannot accidentally earn its determinism.
- ADR-0005 (injected time, `AdvanceTime` entries), ADR-0006 (ULID ids,
  receipt-handle construction), and ADR-0007 (JSONL histories, Wing–Gong
  checker with per-queue partitioning and a wall-clock budget) are accepted.
- The limits of §3.4 and the lifecycle of §3.5/§3.6 are fixed in this
  document and mirrored in [PRODUCT_REQUIREMENTS.md](./PRODUCT_REQUIREMENTS.md);
  R1 implements them and may not adjust them without a documentation change
  in the same PR.

### 6.3 Owned files, interfaces, and state

R1 fills `relay-core` and `relay-model`:

```text
crates/relay-core/src/
  lib.rs          # public surface: apply, Command, Output, CoreState, errors
  ids.rs          # newtypes, ULID encoding, UlidSeq
  limits.rs       # the constants of §3.4, single source of truth
  config.rs       # QueueConfig, QueueConfigDelta, validation
  command.rs      # command payload types and bounded validation
  state.rs        # CoreState, QueueState, StoredMessage, Lease
  apply.rs        # the total apply function; dispatch only
  queue_admin.rs  # CreateQueue, DeleteQueue, SetQueueAttributes, Tag, Untag
  send.rs         # Send, SendBatch
  receive.rs      # Receive: selection, lease grant, cap enforcement
  lease.rs        # Delete, ChangeVisibility, receipt validation matrix
  time.rs         # AdvanceTime: delay promotion, expiry sweep, cooldowns
  purge.rs        # Purge
  error.rs        # CoreError, ValidationReason, ReceiptRejection
crates/relay-model/src/
  lib.rs
  model.rs        # independent reference semantics (maps + vectors, no im)
  history.rs      # JSONL record encode/decode with bounded parsing
  wg.rs           # Wing–Gong linearizability checker, memoized, budgeted
  explore.rs      # bounded exhaustive state-space explorer
  invariants.rs   # I-1..I-6 predicates over CoreState
fixtures/histories/
  good/           # linearizable histories the checker must accept
  bad/            # non-linearizable histories the checker must reject
```

Beyond the §3.4 types, R1 fixes these interfaces:

```rust
impl CoreState {
    pub fn empty() -> CoreState;                 // now = Nanos(0), no queues
    pub fn state_hash(&self) -> [u8; 32];        // SHA-256 of the canonical encoding
}
// Canonical encoding: fields in declaration order, maps iterated in key order,
// lengths as u64 LE. Two states are semantically equal iff hashes are equal;
// the explorer, R2 recovery checks, and R7 snapshot verification all reuse it.

// relay-model — the explorer
pub struct ExploreConfig {
    pub queues: u8,                    // 1..=2 in CI configurations
    pub clients: u8,                   // logical concurrent clients, 2..=3
    pub max_entries: u8,               // exhaustive log length bound, 8..=12 in CI
    pub time_steps: &'static [Nanos],  // AdvanceTime deltas the explorer may inject
    pub command_vocabulary: Vec<CommandTemplate>,
}
pub struct ExploreReport {
    pub states_visited: u64,
    pub max_depth_reached: u8,
    pub violations: Vec<Violation>,    // empty is the only passing report
    pub config_hash: [u8; 32],
}
pub fn explore(cfg: &ExploreConfig) -> ExploreReport;

pub enum Invariant {
    LeaseExclusive,        // I-1: <= 1 live lease per message           (P-02)
    NoInvention,           // I-2: every visible body was sent, byte-identical (P-10)
    EpochMonotone,         // I-3: lease_epoch never decreases; +1 per grant
    CapRespected,          // I-4: |in_flight| <= max_in_flight
    SetsPartition,         // I-5: delayed/available/in_flight partition `messages`
    CountersCoherent,      // I-6: receive_count changes only at T6/T7, by +1
}

// relay-model — the checker (ADR-0007)
pub struct WgBudget { pub wall_clock: Duration, pub max_memo_entries: usize }
pub enum WgVerdict { Linearizable, NotLinearizable(WgWitness), BudgetExhausted }
pub fn check_history(history: &[HistoryRecord], budget: WgBudget) -> WgVerdict;
// Histories are partitioned per queue before checking; BudgetExhausted is a
// CI failure for fixture histories (budgets are sized to the fixtures), and
// a recorded inconclusive — never a pass — anywhere else.
```

The JSONL history record is the spine-fixed shape, one operation per line:

```json
{"op":42,"client":3,"call":{"type":"receive","queue":"q1","max":1,"visibility_s":30},
 "invoke_ns":81234000,"return_ns":81239000,
 "result":{"ok":{"messages":[{"id":"01J...","receipt":"rh1_...","body_sha256":"..."}]}},"seed":"0xDEADBEEF"}
```

The reference model in `model.rs` re-implements §3.5 in the plainest possible
style — vectors, explicit loops, no structural sharing, no shared code with
`relay-core` beyond the public command/reply types. Divergence between model
and core on any input is a bug in one of them by definition, and the test
harness reports both traces.

### 6.4 Algorithms and state behavior

#### Validation order

Validation is fail-fast in a fixed order so error selection is deterministic
and wire-stable. For `Send`: (1) queue exists, else `QueueNotFound`;
(2) queue kind — any `group_id` or `dedup_id` on a Standard queue is
`InvalidInput(FifoFieldOnStandardQueue)` (Fifo queues themselves cannot exist
before R4); (3) body-plus-attributes size, where the accounted size is body
bytes + Σ(name bytes + value payload bytes + 3 bytes type tag/length
overhead) — over 256 KiB is `PayloadTooLarge` (FR-QUEUE-013); (4) attribute
count ≤ 10, names valid, names unique, `relay.` prefix rejected, `Number`
values parse as bounded decimals, `String` values are valid UTF-8
(FR-QUEUE-012); (5) delay within `0..=DELAY_MAX`. `CreateQueue` validates:
name grammar (`.fifo` suffix present iff kind is Fifo; kind Fifo is rejected
with `InvalidInput(FifoNotYetSupported)` until R4 — the error variant exists
now so the wire code never changes); name uniqueness (`QueueNameTaken`);
every bound of §3.4; redrive target existence and kind (structurally
validated now, behaviorally owned by R4).

#### Message id assignment

Each accepted send derives a ULID from `(CoreState.now, UlidSeq counter)`;
the counter increments per id and resets when `now` advances. Ids are
strictly increasing within a log, identical across replays, and carry no
host entropy — determinism is a property the model checker requires, not an
optimization.

#### Receive selection order

1. Compute `free = config.max_in_flight − |in_flight|`.
2. If `free == 0` and `available` is nonempty, reply
   `InFlightCapExceeded` — the stable backpressure error of FR-QUEUE-016.
3. Otherwise let `n = min(max_messages, |available|, free)`. If `n == 0`,
   reply `Received([])`; the core never waits (long polling is R6, at the
   wire).
4. Select the first `n` elements of `available` in ascending
   `(available_at, message_id)` order. This order is fixed for determinism
   and is **not** a user-facing ordering guarantee (NG-03); documentation
   states both facts together.
5. For each selected message: remove from `available`; increment
   `lease_epoch`; insert a `Lease { epoch, granted_at: now, expires_at: now +
   effective_visibility }` where `effective_visibility` is the command
   override or the queue default; set state `InFlight`; emit a `Delivery`
   with `receive_count + 1` as the attempt ordinal and a `Receipt` embedding
   the new epoch.

Partial fulfillment against the cap (step 3 taking `free < max_messages`) is
deliberate: the cap sheds load without starving small receives, and the
partial-batch case is a named model-checker scenario.

#### Visibility bookkeeping and the AdvanceTime sweep

`AdvanceTime(t)` applies as:

1. If `t < state.now`, reply `TimeRegression` and change nothing; equal time
   is a no-op that still replies `TimeAdvanced` (idempotent ticks simplify
   the server's cadence logic).
2. Set `now = t`.
3. Promote every delayed message with `ready_at <= now` to `Available` in
   `(ready_at, message_id)` order, setting `available_at = ready_at` (not
   `now`), so promotion order is independent of tick coarseness.
4. Expire every lease with `expires_at <= now` in `(expires_at, message_id)`
   order: remove the lease, increment `receive_count` (FR-QUEUE-005), set
   state `Available` with `available_at = lease.expires_at`, and emit
   `Output::LeaseExpired`. The epoch does not change on return — rule 3 of
   §3.6 governs subsequent receipt rejections. (At R4 this step gains the
   dead-letter check T9 after the increment; at R4 a retention sweep T11 and
   redrive progress T12 append here. The step order is fixed now so R4 does
   not reorder R1 semantics.)

Expiry is "not before" (NG-04): a lease whose `expires_at` falls between two
ticks returns at the later tick, and nothing in the core promises otherwise.

#### The delete-after-expiry race

The race is resolved by log order, deterministically. If a `Delete` entry
precedes the `AdvanceTime` that crosses the lease's `expires_at`, the lease
is still live, epochs match, and the delete consumes the message. If the
`AdvanceTime` comes first, the message returned to `Available` and the same
`Delete` is rejected `NotInFlight` (FR-QUEUE-007's "expired" case); after a
subsequent regrant it is rejected `EpochSuperseded`. All three outcomes are
enumerated by the explorer, and the wire layer (R6) documents that a rejected
delete after expiry means the message will be redelivered — at-least-once,
exactly as NG-01 states.

#### Delete and the receipt validation matrix

`Delete` evaluates, in order:

| Condition | Result |
| --- | --- |
| `command.queue` does not exist | `QueueNotFound` |
| `receipt.queue != command.queue` | `ReceiptRejected(ForeignQueue)` |
| `receipt.message` absent from `messages` | `Deleted` — idempotent success (FR-QUEUE-006); already-deleted and never-existed are indistinguishable in-core, and anti-forgery is R6's HMAC duty |
| message present, state not `InFlight` | `ReceiptRejected(NotInFlight)` |
| `receipt.lease_epoch != stored lease_epoch` | `ReceiptRejected(EpochSuperseded)` |
| all pass | remove message from `messages` and `in_flight`; `Deleted`; lease `Consumed` |

`ChangeVisibility` runs the same matrix, then: timeout out of
`0..=VISIBILITY_MAX` is `InvalidInput(OutOfRange)`; timeout 0 executes T7
(return now: increment `receive_count`, `available_at = now`, drop lease,
reply `VisibilityChanged { expires_at: None }`); timeout > 0 executes T8
(`expires_at := now + timeout`, replacing — not adding to — the remaining
time, reply with the new absolute expiry). Extending never changes the epoch.

#### Purge during in-flight

`Purge` evaluates: queue exists; if `last_purge_at` is set and
`now < last_purge_at + PURGE_COOLDOWN`, reply
`PurgeInProgress { retry_at }` (FR-QUEUE-015's concurrent-purge rejection —
in a serialized log, "concurrent" means "within the cooldown window").
Otherwise remove every message in `delayed`, `available`, **and** `in_flight`
(the cap frees instantly), drop all leases, set `last_purge_at = now`, and
reply `Purged { removed }`. Outstanding receipts from before the purge now
hit the absent-message row of the matrix and succeed idempotently as deletes;
this is documented behavior, named in the evidence matrix, and consistent
with FR-QUEUE-006.

#### Batch partial failure

`SendBatch` first validates the batch shape: empty is
`InvalidInput(BatchEmpty)`; more than 10 entries is
`InvalidInput(BatchTooLarge)`; a nonexistent queue in any entry fails that
entry (not the batch) with `QueueNotFound`. Then each entry is validated and
applied independently in order, producing a positional
`Vec<Result<MessageId, CoreError>>` (FR-QUEUE-003). A failing entry has no
effect on state; a succeeding entry is applied even when its neighbors fail.
Batch atomicity is explicitly not offered (NG-02), and ids assigned to
successful entries are contiguous in `UlidSeq` order.

#### DeleteQueue and SetQueueAttributes

`DeleteQueue` removes the queue, all messages, and the name-index entry;
outstanding receipts fail with `QueueNotFound` thereafter. (Subscription
cleanup joins at R5; handle-invalidation-and-storage claims complete at R6/
FR-ADMIN-005.) `SetQueueAttributes` validates the delta against the same
bounds as `CreateQueue`; a lowered `max_in_flight` does not evict existing
leases — the cap binds at the next grant; a changed `visibility_timeout`
affects only future grants; changed `default_delay` affects only future
sends. Each of those non-retroactivity rules is a named test.

#### Exhaustive exploration and history checking

The explorer performs breadth-first search over logs up to
`max_entries`: at each frontier state it applies every instantiation of the
command vocabulary (bounded: fixed small bodies, one attribute, the
configured `time_steps`), canonicalizes via `state_hash`, deduplicates
visited states, and evaluates every invariant of `invariants.rs` on every
state and every output. Symmetry reduction canonicalizes client ids.
Violations report the full minimal entry sequence. Separately, the
property-test driver runs randomized concurrent clients against the real
core (interleaved at command granularity), records JSONL histories, replays
them against the reference model, and feeds them to `check_history`.
Checker self-test: six mutant cores (double-grant, epoch-skip, invented
message, lost delete, count-drift, order-swap oracle mutants) must each
produce a `NotLinearizable` verdict or an invariant violation; a mutant that
survives fails the gate.

### 6.5 Implementation tickets and sequence

1. **R1.01 — Ids, limits, and validation primitives.** `ids.rs`, `limits.rs`,
   name/attribute/number grammar parsers with failing `CORE-` tests first,
   including boundary and adversarial inputs (80/81-char names, `relay.`
   prefix, 10/11 attributes, invalid UTF-8, 39-digit numbers). Done when the
   validation tables of §6.4 are fully covered and property tests over
   generated valid inputs round-trip.
2. **R1.02 — CoreState, canonical encoding, state_hash.** `state.rs` with the
   persistent maps, `empty()`, canonical encoding, and hash; tests prove
   hash equality iff semantic equality across map-insertion orders. Done when
   two differently-constructed equal states hash identically and any
   single-field difference changes the hash.
3. **R1.03 — apply skeleton and AdvanceTime.** Total dispatch in `apply.rs`;
   `time.rs` with monotonicity, idempotent equal-time ticks, and the sweep
   ordering of §6.4 over hand-built states. Done when `TimeRegression`,
   promotion order, and expiry order tests pass.
4. **R1.04 — Queue administration.** `queue_admin.rs`: CreateQueue (FR-QUEUE-001),
   DeleteQueue, SetQueueAttributes with non-retroactivity, Tag/Untag. Done
   when name uniqueness, Fifo rejection-until-R4, delta validation, and
   non-retroactivity tests pass.
5. **R1.05 — Send and SendBatch.** `send.rs`: size accounting, delay handling
   into `Delayed`/`Available`, deterministic ULIDs, positional batch results
   (FR-QUEUE-003, 012, 013). Done when the exact 256 KiB boundary (pass at
   limit, fail at limit+1) and batch partial-failure tests pass.
6. **R1.06 — Receive.** `receive.rs`: selection order, lease grant, epoch
   increment, cap enforcement with partial fulfillment (FR-QUEUE-004, 016).
   Done when determinism (same state, same command ⇒ same deliveries),
   cap-partial, and cap-exhausted tests pass.
7. **R1.07 — Delete and the receipt matrix.** `lease.rs` delete path
   implementing the §6.4 matrix rows in order (FR-QUEUE-006, 007). Done when
   every matrix row has a test and double-delete idempotency holds under
   property testing.
8. **R1.08 — ChangeVisibility.** Zero-return, extension, shortening,
   absolute-replacement semantics, epoch stability across extension
   (FR-QUEUE-008). Done when T7/T8 tests and the extend-then-expire sequence
   pass.
9. **R1.09 — Visibility expiry integration.** The full T6 path with
   `receive_count` increment, `LeaseExpired` outputs, and the
   delete-after-expiry race resolved by log order. Done when all three race
   outcomes of §6.4 are asserted by explicit log-order tests.
10. **R1.10 — Purge.** `purge.rs`: cooldown, in-flight removal, cap release,
    post-purge receipt behavior (FR-QUEUE-015). Done when purge-during-
    in-flight and cooldown-rejection tests pass.
11. **R1.11 — Reference model and history format.** `relay-model::model` and
    `history.rs` with bounded JSONL parsing (malformed line, oversized line,
    unknown field, truncated file are all typed errors). Done when
    model-vs-core differential property tests run green over randomized
    command sequences and both fixture history directories round-trip.
12. **R1.12 — Invariants and the explorer.** `invariants.rs` and
    `explore.rs` with memoized BFS, symmetry reduction, and the
    `ExploreReport`. Done when the CI configuration (2 clients, 1 queue,
    depth 10) completes within the §4.5 budget with zero violations and a
    seeded core bug (grant from `InFlight`) is caught with a minimal trace.
13. **R1.13 — Wing–Gong checker.** `wg.rs` per ADR-0007: per-queue
    partitioning, memoization, budget handling, witness extraction. Done when
    every `fixtures/histories/good` file verifies, every `bad` file is
    rejected with a witness, and `BudgetExhausted` is proven reachable and
    treated as non-passing.
14. **R1.14 — Mutant self-test and gate closure.** The six oracle mutants of
    §6.4; wire `CORE-`/`MODL-` suites and the explorer into `ci/gates.toml
    [gate.R1]`; update CORRECTNESS.md's mapping rows for P-02, P-06, P-10 and
    the traceability matrix of §16. Done when all mutants are killed, the
    `gates` job replays R0+R1, and the §6.9 checklist is recorded.

### 6.6 Test-driven evidence matrix

| Test | First failing condition | Required passing assertion |
| --- | --- | --- |
| CORE-NAME-01 | Queue name grammar accepts 81 chars, empty, or bad characters, or rejects a legal 80-char name. | Exact `^[A-Za-z0-9_-]{1,80}$` behavior at both boundaries; `.fifo` handling matches §6.4. |
| CORE-SIZE-01 | A body+attributes payload of exactly 256 KiB is rejected, or 256 KiB + 1 is accepted. | Boundary-exact `PayloadTooLarge` with the documented size accounting (FR-QUEUE-013). |
| CORE-ATTR-01 | 10 attributes rejected, 11 accepted, duplicate names accepted, or `relay.` prefix accepted. | FR-QUEUE-012 limits and grammar enforced with field-naming errors. |
| CORE-BATCH-01 | One invalid entry poisons its neighbors, or results lose positional correspondence. | Independent per-entry results; state reflects exactly the successful entries (FR-QUEUE-003). |
| CORE-RECV-01 | Two applies of the same Receive on the same state yield different deliveries. | Selection is a pure function of state: ascending `(available_at, message_id)`. |
| CORE-RECV-02 | A receive at the in-flight cap grants past the cap, or an under-cap receive returns the backpressure error. | Partial fulfillment to `free`, `InFlightCapExceeded` only at `free == 0` with backlog (FR-QUEUE-016). |
| CORE-VIS-01 | Expiry fails to return the message, returns it before `expires_at`, or fails to increment `receive_count`. | T6 exactly: not-before return, +1 count, `LeaseExpired` output, epoch unchanged (FR-QUEUE-005). |
| CORE-VIS-02 | ChangeVisibility(0) leaves the message in flight, or an extension adds to instead of replacing remaining time. | T7 immediate return with count increment; T8 absolute `now + timeout` (FR-QUEUE-008). |
| CORE-RACE-01 | The delete/expiry race resolves differently from log order, or non-deterministically. | Delete-before-tick consumes; tick-before-delete rejects `NotInFlight`; post-regrant rejects `EpochSuperseded`. |
| CORE-DEL-01 | A second delete with the same receipt fails, or a foreign-queue receipt succeeds. | Full §6.4 matrix: idempotent success on absent, ordered rejections otherwise (FR-QUEUE-006, 007). |
| CORE-PURGE-01 | Purge leaves in-flight messages, or a purge inside the cooldown window succeeds. | All states removed including `InFlight`; `PurgeInProgress` with `retry_at` inside 60 s (FR-QUEUE-015). |
| CORE-ADMIN-01 | SetQueueAttributes retroactively rewrites live leases or delays. | Non-retroactivity: new visibility/delay/cap bind only at next grant/send. |
| CORE-TIME-01 | AdvanceTime accepts regression, or equal-time ticks change state. | `TimeRegression` on decrease; equal-time idempotence; sweep order fixed. |
| MODL-DIFF-01 | Reference model and relay-core disagree on any generated command sequence. | 10,000 seeded random sequences per CI run produce identical replies and equivalent states, with shrunk counterexamples on failure. |
| MODL-EXPL-01 | The exhaustive explorer finds any invariant violation, or fails to finish in budget. | Zero violations at the CI configuration; `ExploreReport` archived as gate evidence with `states_visited` and `config_hash`. |
| MODL-EXPL-02 | A seeded double-grant core mutation survives exploration. | Every §6.4 mutant is caught with a minimal violating trace (P-02, P-06, P-10 witnesses). |
| MODL-WG-01 | A `fixtures/histories/bad` file passes, or a `good` file fails, or budget exhaustion counts as a pass. | Wing–Gong verdicts match fixture labels; `BudgetExhausted` is non-passing (ADR-0007). |
| MODL-HASH-01 | Replaying an identical log yields a different `state_hash`. | Byte-identical state hash across replays and across map-construction orders — the determinism anchor R2/R3/R7 inherit. |

### 6.7 Failure and security cases

- `apply` is total: property tests drive every command variant against every
  reachable-shape state (including empty, purged, and at-cap queues) and
  assert no panic, no unwrap, and no state change on any error reply.
- Receipt forgery is out of scope for the core and in scope for the plan: a
  structurally valid `Receipt` with a fabricated `MessageId` succeeds
  idempotently as a delete of an absent message. This is stated in the code,
  in this section, and in [THREAT_MODEL.md](./THREAT_MODEL.md); NFR-SEC-001's
  unforgeability claim binds at R6's HMAC boundary and may not be cited
  earlier.
- History files are untrusted input to `relay-model`: oversized lines,
  malformed JSON, unknown `type` values, and truncated final lines produce
  typed errors, never panics — the checker will later consume histories
  produced by crashed simulation runs (R3).
- Fixture bodies include `RELAY_CANARY_` strings; the R0 canary scan verifies
  no test log, error message, explorer report, or checker witness emits body
  bytes. Witnesses reference messages by id and `body_sha256` only.
- Adversarial validation inputs are first-class `CORE-` cases: NUL and
  control bytes in names, combining-character UTF-8 in attribute strings,
  `Number` values with exponent overflow, zero-length binary attributes, and
  a 10 MiB body (rejected by size before any attribute parsing).
- The explorer bounds its own memory (`max_memo_entries`); exhaustion is a
  reported failure with the frontier size, never an OOM kill that CI
  misreads as infrastructure flake.

### 6.8 Migration, documentation, and installation work

R1 is in-memory; there is no data format to migrate. The JSONL history format
carries a per-file header line `{"relay_history":1}` from its first version
so R3's corpus and any future format revision have a versioned migration path
(MIGR- families begin at R2 for disk formats). Documentation work:
CORRECTNESS.md's property-to-test mapping gains concrete rows (P-02 →
MODL-EXPL-01/02, P-06 → CORE-DEL-01 + MODL-DIFF-01, P-10 → MODL-EXPL-01 with
the `NoInvention` invariant); PRODUCT_REQUIREMENTS acceptance references for
the eleven R1-terminal requirements point at the matrix above; the README
status moves from "no test exists" to naming the R1 evidence, and no further.
Installation work: none. There is still no binary — `relay-server` remains an
empty shell, and R1 publishes nothing.

### 6.9 Acceptance evidence

R1 is accepted only when all of the following are recorded in the closing
pull request:

- [ ] every `CORE-` and `MODL-` suite of §6.6 green on the ADR-0011 tier-1
      matrix, within the §4.5 model-check budget;
- [ ] the archived `ExploreReport` (states visited, depth, config hash, zero
      violations) attached as a build artifact and referenced by path;
- [ ] all six oracle mutants killed, with their minimal traces archived;
- [ ] `fixtures/histories/good` and `bad` verdicts exactly matching labels;
- [ ] the differential suite (MODL-DIFF-01) green at 10,000 sequences with
      the run seed recorded for replay;
- [ ] `state_hash` replay determinism proven on both x86_64 and aarch64 with
      identical hashes;
- [ ] `ci/gates.toml` gains `[gate.R1]` with its exact commands, and the
      `gates` job replays R0 and R1 together;
- [ ] CORRECTNESS.md, PRODUCT_REQUIREMENTS.md, and §16 traceability rows
      updated in the same changeset;
- [ ] the canary scan green across all new suites.

### 6.10 Explicit deferrals

R1 defers **all** durability claims — there is no WAL, no fsync, no crash
recovery, and an R1 "sent" message survives nothing (R2 owns NFR-DUR-001
through 006). It defers FIFO queues, message groups, deduplication, per-queue
and per-message delay defaults beyond the mechanics used by T1/T3, retention
enforcement, DLQ, and redrive (R4); topics, subscriptions, and filter
evaluation (R5); every wire concern — RWP/1, authentication, receipt-handle
HMAC, quotas, long polling, and therefore FR-QUEUE-009 (R6); every
replication claim (R7); observability (R8); every performance number (R9);
and packaging (R10). The model checker at R1 checks the single-node core
only; no R1 result may be cited as evidence about partitions, crashes, or
concurrency beyond command-interleaving — those are R3 and R7 claims with
their own gates.

### 6.11 Requirements traced

R1 is the terminal owning gate for: **FR-QUEUE-001** (CreateQueue),
**FR-QUEUE-003** (SendMessageBatch), **FR-QUEUE-004** (ReceiveMessage
leases), **FR-QUEUE-005** (visibility expiry), **FR-QUEUE-006** (idempotent
delete), **FR-QUEUE-007** (receipt rejection and single-use epochs),
**FR-QUEUE-008** (ChangeMessageVisibility), **FR-QUEUE-012** (typed
attributes), **FR-QUEUE-013** (256 KiB body limit), **FR-QUEUE-015**
(PurgeQueue), and **FR-QUEUE-016** (in-flight cap). "Terminal" means the
in-core semantics complete here; later gates carry these behaviors across
their own boundaries (R2 makes them durable, R6 exposes them on the wire)
under their own requirement IDs without reopening these.

R1 begins, without owning terminally: FR-QUEUE-002 (send semantics exist;
the durability half completes at R2 under ADR-0008), FR-QUEUE-010/011 (the
`Delayed` state and promotion mechanics exist; the delay-parameter surface
completes at R4), FR-QUEUE-014 (retention configuration validates;
enforcement is R4's T11), NFR-SEC-001 (epoch single-use is in place; the
unforgeability half is R6), NFR-MAINT-002 (seeded, replayable failures are
the norm here; the simulation corpus is R3's), and NFR-MAINT-003 (mutation
thinking starts with the oracle mutants; the ≥ 85 % mutant-kill bar on
`relay-core` completes at R4). Per §16, each of these IDs lists its own
terminal gate; R1 appears in their rows as a beginning gate only.

## 7. R2 — Durable WAL Storage Engine Under Crash, Torn-Write, and Disk-Full Injection

**Status:** planned.

**Effort range:** 12–18 focused days, including the crash-injection matrix and the
recovery-equivalence property harness.

### 7.1 Why this gate exists

R1 proved queue semantics against the model checker with everything in memory; an
R1 crash loses every message and violates nothing, because R1 made no durability
claim. R2 is where Relay earns the single sentence that justifies the product:
an acknowledged send survives any single-process crash. Per
[ADR-0002](./decisions/ADR-0002-hand-rolled-segmented-wal.md) the storage engine
is a first-party segmented write-ahead log, because a durability guarantee that
is machine-checked requires an on-disk format Relay controls byte for byte and a
recovery path Relay can drive deterministically through injected faults. Per
[ADR-0008](./decisions/ADR-0008-fsync-before-ack-durability-contract.md) the ack
boundary is exact: ack ⇔ record appended and fsync complete, group commit is
adaptive with a 2 ms cap, and fsync failure aborts the process rather than
retrying. R2 also prevents a dangerous shortcut: declaring durability because
"we call fsync somewhere" instead of because the CRSH- suite kills the process
mid-write on an injected disk and proves the recovered state byte-equal to the
acknowledged prefix.

### 7.2 Prerequisites

- R0 and R1 are accepted; the workspace, CI, architecture checks, and the pure
  `relay-core` state machine with its model-checker gate are green on mainline.
- The `LogEntry` binary encoding produced by R1 is frozen with byte fixtures;
  R2 stores those bytes as WAL record payloads and never re-encodes them.
- ADR-0002 and ADR-0008 are accepted. The `Disk` trait defined in
  [ARCHITECTURE.md](./ARCHITECTURE.md) §interfaces is the only filesystem access
  path in `relay-wal`; the R0 architecture check that rejects direct `std::fs`
  use outside the production `Disk` implementation is extended to `relay-wal`.
- The on-disk record, segment-header, and snapshot layouts in this section are
  reviewed against [CORRECTNESS.md](./CORRECTNESS.md) P-01 before the first
  segment is written; on-disk formats are never introduced unversioned.

### 7.3 Owned files, interfaces, and state

Create `crates/relay-wal` modules:

- `src/record.rs`: WAL record framing, CRC32C, bounds, encode/decode;
- `src/segment.rs`: segment header codec, segment naming, sealing;
- `src/writer.rs`: buffered append, group commit, rotation, directory fsync;
- `src/reader.rs`: bounded sequential segment scanning;
- `src/recovery.rs`: recovery scan, corruption classification, torn-tail repair;
- `src/snapshot.rs`: `RSNAP1` snapshot write, verify, and load;
- `src/live_set.rs`: live-set representation handed in by the caller;
- `src/compaction.rs`: snapshot-then-reclaim compaction and its report;
- `src/wal.rs`: the `Wal` type composing the above;
- `src/error.rs`: `WalError` taxonomy with stable variants;
- `src/fault.rs`: `FaultDisk`, a deterministic injecting `Disk` wrapper used by
  the CRSH- suites (fail-at-op, torn-tail-on-crash, byte budget for disk-full);
- `tests/fixtures/`: checked-in byte fixtures for every versioned layout.

The public surface uses the workspace interfaces verbatim:

```rust
impl Wal {
    pub fn recover(disk: Arc<dyn Disk>, dir: DiskPath, opts: WalOptions) -> Result<(Wal, RecoveredState), WalError>;
    pub fn append(&mut self, records: &[Record]) -> Result<Lsn, WalError>; // buffered
    pub fn sync(&mut self) -> Result<Lsn, WalError>;                       // durable up to returned Lsn
    pub fn compact(&mut self, upto: Lsn, live: &LiveSet) -> Result<CompactionReport, WalError>;
}
```

R2 defines the supporting types:

```rust
pub struct WalOptions {
    pub segment_target_bytes: u64,   // default 64 MiB; bounds 8 MiB..=1 GiB
    pub group_commit_cap: Nanos,     // hard cap 2_000_000 ns (ADR-0008); not configurable upward
    pub max_record_bytes: u32,       // default 1 MiB; every len checked before allocation
    pub preallocate_segments: bool,  // best-effort fallocate; portable fallback required (ADR-0011)
}

pub struct Record {
    pub record_type: RecordType,
    pub flags: u8,        // must be zero in format version 1
    pub lsn: Lsn,
    pub payload: Bytes,   // LogEntry bytes for Entry records
}

#[repr(u8)]
pub enum RecordType { Entry = 1, SnapshotNote = 2, CompactionNote = 3 }

pub struct RecoveredState {
    pub durable_lsn: Lsn,                 // highest LSN proven durable
    pub next_lsn: Lsn,                    // durable_lsn + 1
    pub snapshot: Option<LoadedSnapshot>, // newest valid snapshot, if any
    pub replayable: RecordCursor,         // records after the snapshot, in LSN order
    pub tail_repair: Option<TornTailReport>,
}

pub struct TornTailReport {
    pub segment: SegmentSeq,
    pub valid_through_offset: u64,
    pub discarded_bytes: u64,
    pub cause: TailCause, // ShortHeader | ShortPayload | BadCrc | OversizeLen | BadLsn
}

pub struct CompactionReport {
    pub snapshot_lsn: Lsn,
    pub segments_deleted: Vec<SegmentSeq>,
    pub bytes_reclaimed: u64,
    pub live_bytes_retained: u64,
}
```

The WAL record layout is fixed for format version 1:

`[len u32 LE][crc32c u32][type u8][flags u8][reserved u16][lsn u64][payload len bytes]`

`len` counts `type` through the end of `payload`; `crc32c` covers the same range
(`type..payload`), so a corrupted length field fails either the bounds check or
the CRC, never both silently. `reserved` must be zero and is verified on read.

Segment files are named `wal-<seq:016x>.seg`, target 64 MiB, and begin with a
4 KiB header:

| Offset | Size | Field | Rule |
| --- | --- | --- | --- |
| 0 | 8 | magic `RWALSEG1` | exact bytes or the segment is rejected |
| 8 | 2 | format version u16 LE | version 1; higher versions refuse to open |
| 10 | 6 | reserved | must be zero |
| 16 | 8 | segment seq u64 LE | must match the filename seq |
| 24 | 8 | base LSN u64 LE | LSN of the first record in this segment |
| 32 | 8 | created wall time u64 LE | nanoseconds; diagnostic only, never ordering |
| 40 | 4 | header CRC32C | covers bytes 0..40 |
| 44 | 4052 | zero padding | must be zero through byte 4095 |

Snapshot files are named `snap-<lsn:016x>.rsnap`: magic `RSNAP1`, format version
u16 LE, then a sequence of chunks `[chunk len u32 LE][chunk crc32c u32][chunk
bytes]` (chunk cap 1 MiB), terminated by a zero-length chunk, then a footer:
chunk count u32 LE, total payload length u64 LE, full-state SHA-256 (32 bytes),
footer CRC32C. A snapshot missing its terminator or failing any chunk CRC, the
footer CRC, or the full-state hash is invalid in its entirety; snapshots are
never partially loaded.

Durable state owned by R2 is exactly the WAL directory: `wal-*.seg` segments,
`snap-*.rsnap` snapshots, and transient `*.tmp` files that recovery deletes.
There is no index file and no manifest; the directory listing plus the formats
above are the complete source of truth, so recovery can never disagree with a
stale sidecar.

### 7.4 Algorithms and state behavior

**Append and group commit (ADR-0008).** `append` assigns contiguous LSNs,
encodes records into the writer buffer, and returns without touching the disk.
`sync` drives the durability boundary:

1. If a sync batch is already flushing, the caller's target LSN joins the next
   batch; batching is by joining, never by reordering.
2. The committer waits for an adaptive window: one quarter of the EWMA of recent
   fsync latencies, clamped to `[0, group_commit_cap]` where the cap is 2 ms.
   Under an idle queue the window collapses to zero; the window never exceeds
   2 ms regardless of measured latency.
3. All buffered bytes are written to the active segment through
   `Disk::append` with a short-write loop; a record is never split across
   segments (rotation happens on record boundaries, step 6).
4. `Disk::fsync` runs on the active segment. On success the durable LSN
   advances to the batch's highest LSN and every waiter at or below it is
   released with that LSN; the caller may now acknowledge (FR-QUEUE-002,
   NFR-DUR-001).
5. On `fsync` failure the process logs the error with segment, offset, and
   errno context and calls `std::process::abort()`. There is no retry, no
   error return, and no degraded mode: after a failed fsync the kernel page
   cache state is unknowable, so continuing would risk acknowledging
   non-durable data (NFR-DUR-005, the fsyncgate rule). `fsync_dir` failures
   abort identically.
6. Rotation: when the active segment would exceed `segment_target_bytes` at a
   record boundary, the writer creates `wal-<seq+1:016x>.seg`, writes and
   fsyncs its 4 KiB header, fsyncs the WAL directory so the new name is
   durable, and only then directs appends to it. The previous segment receives
   a final fsync and is thereafter immutable (sealed by position, not by a
   marker byte).

Directory fsync points are exactly: after segment creation, after snapshot
rename, and after segment or snapshot deletion. Each is load-bearing: a missing
directory fsync can resurrect a deleted segment or lose a created one across a
power failure, and the CRSH- matrix crashes on both sides of each point.

**Recovery scan.** `recover` rebuilds durable state from the directory alone:

1. Verify the WAL directory exists, is a real directory (not a symlink), is
   owned by the current user, and has mode `0700`; otherwise fail startup with
   `WalError::InsecureDataDir` before reading any byte (NFR-SEC-005).
2. List the directory. Delete every `*.tmp` file (incomplete snapshot or
   compaction output; deletion is safe because a `.tmp` name is by definition
   not yet durable state), then fsync the directory.
3. Load the newest snapshot whose chunks, footer, and full-state SHA-256 all
   verify. An invalid newest snapshot is quarantined by rename to
   `snap-<lsn>.rsnap.bad` and the next-newest is tried; having no valid
   snapshot is normal and means full log replay.
4. Sort segments by seq. Verify each header: magic, version, zero reserved and
   padding, seq/filename agreement, header CRC. Verify base-LSN continuity:
   each segment's base LSN must equal the previous segment's last LSN + 1.
5. Scan records segment by segment. For each record: read the 20-byte fixed
   header; check `len ≤ max_record_bytes` before any allocation; read exactly
   `len` bytes; verify CRC32C over `type..payload`; verify `flags`/`reserved`
   are zero; verify the LSN is exactly the expected next LSN.
6. Classify every defect by position. A defect is *tail-positioned* iff it
   occurs in the highest-seq segment and no valid record exists after it in
   that segment. Every other defect is *mid-log*.
   - Short header at tail, short payload at tail, bad CRC at tail, oversize
     `len` at tail, wrong LSN at tail: torn write. Record a `TornTailReport`,
     truncate the segment to the last valid record boundary via copy-truncate
     (write the valid prefix length, `Disk` truncation is modeled as
     rewrite-and-rename in the portable path), fsync segment and directory,
     and continue startup (NFR-DUR-003). The truncated bytes were never
     acknowledged — ADR-0008 guarantees no ack precedes fsync, and fsync
     covers whole records only — so truncation cannot lose an acked message.
   - Bad CRC mid-log, short read mid-log, oversize `len` mid-log: fatal
     `WalError::CorruptSegment { seq, offset }`. Bytes before the durable
     frontier were fsynced; their corruption means media or operator damage,
     and truncating through committed history would silently drop acked
     messages. Relay refuses to start and names the segment and offset.
   - Duplicate LSN (record LSN ≤ previous): fatal `WalError::DuplicateLsn`
     wherever it appears — a duplicate can only be produced by a format bug or
     tampering, never by a torn write, because LSNs are assigned monotonically
     before encoding.
   - LSN gap (record LSN > expected next): fatal `WalError::LsnGap` mid-log;
     at the tail it is subsumed by the wrong-LSN torn-write case only when the
     gapped record also fails CRC; a CRC-valid gapped tail record is fatal,
     because a valid checksum proves the bytes were written intact and the gap
     therefore reflects lost committed state.
7. Rebuild in-memory state: durable LSN = last valid record's LSN; hand the
   caller the snapshot (if any) plus a cursor over all records with LSN greater
   than the snapshot LSN, in LSN order. `relay-server` replays those payloads
   through `relay_core::apply` to reach the exact pre-crash acknowledged state
   (NFR-DUR-002).
8. Emit a structured recovery report: segments scanned, records replayed,
   snapshot used, tail repair (if any), elapsed time.

**Compaction.** Relay compacts by copying live state forward into a snapshot and
reclaiming whole segments; it never edits a segment in place:

1. The caller supplies `upto` (an applied, durable LSN) and the `LiveSet` — the
   full core state at `upto` as produced by `relay-core`, serialized
   deterministically. Computing liveness from state, not from log scanning,
   makes the live-set computation exactly as trustworthy as the model-checked
   state machine.
2. Write the live set to `snap-<upto:016x>.rsnap.tmp`: header, chunks with
   per-chunk CRC, terminator, footer with full-state SHA-256. Fsync the file.
3. Re-open and re-verify the temp file end to end (chunks, footer, hash). Only
   a verified file may be installed.
4. Atomically rename to `snap-<upto:016x>.rsnap`; fsync the directory. The
   snapshot is now durable.
5. Only after step 4 completes may any segment be deleted (the
   never-delete-before-durable rule, NFR-DUR-006). Delete every segment whose
   records all have LSN ≤ `upto` — such segments are fully superseded by the
   snapshot — oldest first, fsyncing the directory after the batch. The
   segment containing `upto` is retained if it also contains records above
   `upto`.
6. Delete snapshots older than the one just installed, retaining the newest
   two (the new one and its predecessor) so a latent media fault in the new
   snapshot never strands recovery.
7. Return the `CompactionReport`. A crash at any step is safe: before step 4
   recovery deletes the `.tmp`; after step 4 recovery prefers the new
   snapshot; between deletions in step 5 recovery simply sees extra
   still-valid segments whose records are ignored below the snapshot LSN.
8. Space reclaim is verified, not assumed: the STOR- suite measures directory
   bytes before and after and asserts the reported reclaim within one segment
   of the measurement.

**Disk-full behavior (NFR-DUR-004).** `Disk::append`, segment creation, and
snapshot writes that fail with an out-of-space error return
`WalError::DiskFull` to the caller; they never abort. The write buffer is
rolled back to the last durable boundary, the assigned-but-unwritten LSNs are
released for reuse (nothing was acknowledged, so no client observes the
reuse), and the WAL remains open for reads and for `sync` of already-written
bytes. `relay-server` maps `DiskFull` to the stable backpressure error from
the R1 error taxonomy; receives, deletes of in-flight messages, and reads
continue. Disk-full during compaction abandons the `.tmp` file, deletes it,
and reports the bytes that would be required; the old segments are untouched.
Disk-full is the one storage error that is expected to be transient, and the
CRSH- suite proves that filling, failing, freeing, and retrying leaves zero
corruption.

### 7.5 Implementation tickets and sequence

1. **R2.01 — Freeze on-disk byte formats.** Check in byte fixtures for the WAL
   record, the 4 KiB segment header, and the `RSNAP1` snapshot, including a
   fixture for every rejection: bad magic, higher version, nonzero reserved,
   nonzero padding, bad header CRC. Done when decoding each fixture yields the
   exact documented value or the exact documented `WalError` variant.
2. **R2.02 — Implement the record codec.** Encode/decode with CRC32C over
   `type..payload`, length checked against `max_record_bytes` before
   allocation, zero-copy payload views. Done when property tests round-trip
   arbitrary records and every truncation offset of a valid record decodes to
   a classified error, never a panic.
3. **R2.03 — Implement the segment writer and rotation.** Header write, record
   boundaries, rotation with header fsync and directory fsync, sealed-segment
   immutability assertion. Done when a 300 MiB append stream produces
   correctly chained segments whose headers and base LSNs verify.
4. **R2.04 — Implement group commit and `sync`.** Batch joining, adaptive
   window with the 2 ms cap, EWMA update, waiter release at the exact LSN.
   Done when a deterministic-clock test proves the window never exceeds 2 ms
   and a concurrent-caller test proves no waiter is released below its LSN.
5. **R2.05 — Implement the fsync-failure abort.** Route every `fsync` and
   `fsync_dir` result through one enforcement point that logs and aborts.
   Done when CRSH-FSYNC injection at every fsync site produces process abort
   with the documented log line and a clean subsequent recovery.
6. **R2.06 — Implement the sequential reader.** Bounded reads through
   `Disk::read_at`, no allocation before length validation, cursor semantics
   for replay. Done when the reader replays R2.03's segments byte-identically
   and rejects every fixture from R2.01.
7. **R2.07 — Implement recovery classification.** The full scan of section
   7.4 with every corruption case: torn tail (short header, short payload, bad
   CRC, oversize len, wrong LSN), mid-log CRC, duplicate LSN, gap, invalid
   header, base-LSN discontinuity. Done when a generated corruption matrix —
   every case at every position class — recovers or refuses exactly as
   documented.
8. **R2.08 — Build `FaultDisk` and the crash matrix.** A deterministic `Disk`
   wrapper that fails the Nth operation, tears the final append to a
   configured byte count on simulated crash, and enforces a byte budget for
   disk-full; plus a subprocess harness that SIGKILLs a writer child at
   injected points. Done when both harnesses drive R2.07's cases end to end
   and a real-SIGKILL smoke run passes on tier-1 Linux.
9. **R2.09 — Implement `RSNAP1` snapshots.** Chunked write, terminator,
   footer, full verify-on-load, quarantine-and-fall-back on invalid newest.
   Done when a snapshot with any single corrupted byte is rejected whole and
   recovery falls back to the predecessor or full replay.
10. **R2.10 — Implement compaction.** Live-set serialization, temp write,
    re-verify, atomic rename, directory fsync, never-delete-before-durable
    ordering, retained-predecessor rule, measured reclaim. Done when a crash
    injected between every pair of compaction steps recovers to a state equal
    to the acknowledged prefix and no live record is ever unreachable.
11. **R2.11 — Implement disk-full handling.** Budgeted `FaultDisk` runs for
    append, rotation, and compaction paths; buffer rollback; LSN release;
    stable `DiskFull` mapping; fill-free-retry cycles. Done when a
    fill/free/retry loop of 1,000 iterations ends with zero recovery
    anomalies and reads succeed while full.
12. **R2.12 — Wire the recovery-equivalence gate.** A harness that runs a
    scripted workload against `relay-core` + `relay-wal` under `FaultDisk`,
    records the acknowledged prefix, SIGKILLs, recovers, and asserts
    recovered state == `relay-core` state rebuilt by replaying exactly the
    acknowledged prefix. Done when the harness runs in CI as the named R2
    gate with a fixed seed set and zero flakes.

### 7.6 Test-driven evidence matrix

| Test | First failing condition | Required passing assertion |
| --- | --- | --- |
| STOR-REC-001 record codec bounds | A length field is trusted before validation or a truncated record panics. | Every prefix of a valid record decodes to a classified `WalError`; `len` is checked against `max_record_bytes` before allocation. |
| STOR-SEG-002 segment header fixtures | Bad magic, future version, nonzero reserved/padding, or bad header CRC is accepted. | Each rejection fixture yields its exact documented error; the valid fixture yields the exact documented fields. |
| STOR-ROT-003 rotation chain | A record spans segments or a rotated segment lacks header/directory fsync. | Rotation occurs only on record boundaries; base-LSN continuity holds across every generated segment chain. |
| STOR-GRP-004 group commit window | The commit window exceeds 2 ms or a waiter is released below its LSN. | Deterministic-clock measurement bounds every window at ≤ 2 ms; released waiters' LSNs ≤ durable LSN always. |
| CRSH-KILL-001 SIGKILL mid-append | A child killed during append recovers to a state containing an unacked record or missing an acked one. | At every injected kill point, recovered state == state rebuilt from the acknowledged prefix (NFR-DUR-001, NFR-DUR-002). |
| CRSH-TORN-002 torn-write truncation | A torn tail is replayed as data, or truncation removes a record below the durable LSN. | Every torn-tail shape (short header, short payload, bad CRC, oversize len, wrong LSN) truncates to the last valid boundary; the `TornTailReport` matches the injection (NFR-DUR-003). |
| CRSH-MIDLOG-003 mid-log corruption refusal | A CRC failure before the durable frontier is truncated or skipped. | Mid-log corruption, duplicate LSN, and LSN gap each refuse startup with the exact segment and offset named. |
| CRSH-FSYNC-004 fsync error injection | A failed fsync is retried or returned as a recoverable error. | Injection at every fsync and fsync_dir site aborts the process; the next recovery is clean (NFR-DUR-005). |
| CRSH-FULL-005 disk full | Out-of-space corrupts state, aborts the process, or blocks reads. | Fill/free/retry cycles produce stable `DiskFull` errors, continued reads, and zero recovery anomalies (NFR-DUR-004). |
| CRSH-EQIV-006 recovery equivalence | Any seed in the fixed set recovers to a state ≠ replay of the acknowledged prefix. | The scripted-workload harness proves recovered state == acked-prefix replay for every seed and kill point in CI. |
| STOR-CMP-007 compaction liveness | A record needed to rebuild live state is deleted, or deletion precedes snapshot durability. | Crash injection between every compaction step never loses live data; segment deletion is observed only after snapshot rename + directory fsync (NFR-DUR-006). |
| STOR-SNAP-008 snapshot integrity | A snapshot with one corrupted byte loads, or loads partially. | Any chunk-CRC, footer, or full-state-SHA-256 failure rejects the whole file and falls back to predecessor or full replay. |
| STOR-PERM-009 data directory permissions | A world-readable, group-writable, symlinked, or foreign-owned WAL directory is used. | Startup fails with `InsecureDataDir` before any byte is read unless the directory is a real `0700` directory owned by the process user (NFR-SEC-005). |
| STOR-RECL-010 measured reclaim | Reported reclaim disagrees with measured directory shrinkage by more than one segment. | `CompactionReport.bytes_reclaimed` matches the before/after directory measurement within one segment. |

### 7.7 Failure and security cases

- Every length field in every format is validated before allocation; arithmetic
  on offsets uses checked operations, and the scanner never seeks past the
  verified file size.
- The scanner never searches forward for a later magic value after a defect:
  payloads are untrusted and may contain `RWALSEG1` or valid-looking record
  headers, so resynchronization would let an attacker or a fault fabricate
  history. Classification is by position only.
- Truncation is permitted only at the tail of the highest-seq segment and only
  below records that cannot have been acknowledged. Committed history is never
  repaired by skipping bytes.
- A `.tmp` file is never read as state and is always deleted at recovery; a
  crash can therefore never install a half-written snapshot.
- The WAL directory must be `0700`, a real directory, and owned by the process
  user; failure is a startup error, not a warning, because the WAL will later
  hold message bodies and the receipt-key material introduced at R6.
- `WalError` messages name segments, offsets, and sizes but never include
  payload bytes, so log output cannot leak message content (feeds NFR-SEC-003,
  enforced with canaries at R6).
- fsync failure aborts even when the error looks retryable (`EINTR` is retried
  by the `Disk` implementation below the contract line; everything surfacing
  as an fsync error above it aborts).
- Disk-full during rotation must not strand a headerless segment file: the new
  segment is usable only after its header fsync, and recovery deletes a
  zero-record segment whose header never became durable.

### 7.8 Migration, documentation, and installation work

R2 creates on-disk format version 1 for segments and snapshots and therefore
ships the format discipline that NFR-DUR-007 (terminal at R10) will audit:

- every layout carries a format version; version-2 readers are refused with a
  stable error naming the found and supported versions;
- a fixture WAL directory generated by the R2 build is checked in so every
  future gate proves it still recovers version-1 data;
- [ARCHITECTURE.md](./ARCHITECTURE.md) gains the byte-level record, segment,
  and snapshot tables from section 7.3 verbatim;
- [OPERATIONS_TEST_PLAN.md](./OPERATIONS_TEST_PLAN.md) gains the STOR- and
  CRSH- family matrices with harness, fixture, runtime budget, and the
  zero-flake policy (deterministic suites: a flake is a bug);
- developer documentation states the dev data directory default
  `./relay-data`, the `0700` requirement, and that deleting the WAL directory
  deletes all messages.

There is no released-user migration because nothing has shipped. Backup and
restore procedures are R10 work (FR-OPS-007); R2 documents only that a
consistent copy requires the process stopped.

### 7.9 Acceptance evidence

R2 is accepted only when:

- the STOR- and CRSH- suites in section 7.6 are green in CI on tier-1 Linux
  with the fixed seed set, zero flakes;
- the recovery-equivalence harness (CRSH-EQIV-006) proves recovered state ==
  acknowledged-prefix replay at every injected kill point;
- fsync-failure injection at every site aborts the process (NFR-DUR-005
  evidence recorded with log excerpts in the gate pull request);
- fill/free/retry disk-full cycling completes with zero anomalies;
- compaction crash injection never loses live data and measured reclaim
  matches the report;
- the version-1 fixture WAL directory recovers on mainline;
- the R0 architecture check proves `relay-wal` performs no filesystem access
  outside the `Disk` trait and `relay-core` remains free of IO, clock, and rng;
- docs label the result exactly: single-process durability under injected
  faults, nothing more.

### 7.10 Explicit deferrals

R2 earns no replication claim, no availability claim, and no
production-hardening claim: surviving `FaultDisk` and SIGKILL injection is
evidence about the storage engine, not about kernels, firmware, or real power
loss, and the documentation must say so. R2 defers Raft log storage concerns
(R7 layers its entries through this same WAL), long polling, wire exposure,
encryption at rest (OPEN_QUESTIONS.md), io_uring optimization (ADR-0011
optional path), backup tooling (R10), format migration tooling (R10), and any
performance number — group commit exists for correctness of the ack boundary
here; its latency is measured only by [BENCHMARK_PLAN.md](./BENCHMARK_PLAN.md)
at R9.

### 7.11 Requirements traced

R2 is the terminal owning gate for `FR-QUEUE-002`, `NFR-DUR-001`,
`NFR-DUR-002`, `NFR-DUR-003`, `NFR-DUR-004`, `NFR-DUR-005`, `NFR-DUR-006`,
and `NFR-SEC-005`; the section-16 matrix records their completion here. R2
begins `NFR-DUR-007` (versioned formats; terminal R10), `NFR-PERF-004`
(recovery time; measured at R9), and `FR-OPS-007` (backup semantics; terminal
R10), and advances the every-gate audit of `NFR-MAINT-001`.

## 8. R3 — Deterministic Simulation Reproduces Any Failure From a Seed

**Status:** planned.

**Effort range:** 10–14 focused days, including the corpus runner and the
divergence alarm.

### 8.1 Why this gate exists

Every hard bug Relay will ever have — a lease granted twice during a partition,
an ack lost across a failover, a message resurrected by a torn write — lives in
an interleaving that ordinary tests hit once a month and can never hit twice.
R3 builds the instrument that makes such bugs cheap: a single-threaded
virtual-time simulation in which every source of nondeterminism — time, network,
disk, randomness — is owned by the simulator and derived from one seed, so any
failure ever observed is a seed, any seed replays byte-identically, and a
failing seed checked into the corpus becomes a permanent regression test. R7's
Raft evidence and R9's failure-injection report are impossible without this
gate; building it before FIFO (R4) and replication (R7) means every subsequent
feature is born inside the harness instead of being retrofitted into it. The
product thesis — guarantees machine-checked, not asserted — is operationally
this section.

### 8.2 Prerequisites

- R1 and R2 are accepted: `relay-core` is pure and model-checked, and
  `relay-wal` speaks only through the `Disk` trait, so both can be hosted
  unmodified inside the simulator.
- [ADR-0005](./decisions/ADR-0005-injected-time-and-log-applied-clock.md) is
  accepted: no wall-clock reads inside `relay-core`; all state-machine time
  flows through `AdvanceTime` entries.
- The R0 architecture check already denies `std::time`, `std::thread`,
  `rand::thread_rng`, and direct `std::fs`/`std::net` in `relay-core` and
  `relay-wal`; R3 extends the deny list to every crate compiled into a
  simulation binary.
- The `Clock`, `Rng`, `Disk`, and `Net` traits from
  [ARCHITECTURE.md](./ARCHITECTURE.md) are frozen; the simulator implements
  all four.

### 8.3 Owned files, interfaces, and state

Create `crates/relay-sim` modules:

- `src/executor.rs`: the single-threaded virtual-time executor and event queue;
- `src/clock.rs`: `SimClock` implementing `Clock`;
- `src/rng.rs`: `SimRng` implementing `Rng` with domain-separated sub-streams;
- `src/disk.rs`: `SimDisk` implementing `Disk` with fail, fill, and tear;
- `src/net.rs`: `SimNet` implementing `Net` with drop, delay, duplicate,
  reorder, asymmetric partition, and slow node;
- `src/schedule.rs`: fault-schedule derivation from the seed;
- `src/workload.rs`: seeded workload generators;
- `src/invariants.rs`: per-tick checkers for P-01 through P-10;
- `src/trace.rs`: the canonical event trace, its serialization, and its hash;
- `src/corpus.rs`: corpus file format, loader, and replay runner;
- `src/minimize.rs`: seed-schedule minimization;
- `src/bin/sim-run.rs`, `src/bin/sim-corpus.rs`, `src/bin/sim-min.rs`:
  developer and CI entry points.

The public surface:

```rust
pub struct SimConfig {
    pub seed: u64,
    pub nodes: u8,                 // 1 at R3/R4; 3 or 5 once R7 exists
    pub tick_budget: u64,          // virtual-event budget, not wall time
    pub faults: FaultProfile,      // intensity knobs; realized schedule comes from the seed
    pub workload: WorkloadProfile, // generator selection and op mix
}

pub struct SimOutcome {
    pub events_executed: u64,
    pub trace_sha256: [u8; 32],
    pub violation: Option<Violation>,
}

pub struct Violation {
    pub property: PropertyId,      // P01..=P10
    pub tick: u64,
    pub event_index: u64,
    pub detail: String,            // never includes payload bytes
}

impl Sim {
    pub fn new(config: SimConfig) -> Sim;
    pub fn run(self) -> SimOutcome;                         // to budget or first violation
    pub fn run_traced(self, sink: &mut dyn TraceSink) -> SimOutcome;
    pub fn run_against(self, expected: &Trace) -> SimOutcome; // divergence alarm mode
}

pub enum DiskFault { FailOp { op: DiskOp, error: DiskErrorKind },
                     Fill { remaining_bytes: u64 },
                     TearOnCrash { keep_bytes: u64 } }

pub enum NetFault { Drop, Delay(Nanos), Duplicate, Reorder,
                    PartitionAsymmetric { from: NodeId, to: NodeId },
                    SlowNode { node: NodeId, factor: u32 } }
```

The executor owns the only event queue in a simulation process. An event is
`(virtual_time: Nanos, tie_break: u64, target: ComponentId, payload:
SimEvent)`; `tie_break` is a monotonically assigned sequence number, so
ordering is a total order and never depends on hash-map iteration, allocator
behavior, or wall time. Components (a hosted `relay-core` + `relay-wal` node,
a simulated client, a fault injector) may interact with the world only by
returning new events to the executor; the architecture check denies spawning
threads or reading OS time inside any simulation build.

`SimRng` is a counter-based PRNG (ChaCha12) keyed by
`SHA-256(seed ‖ domain-tag)`, with fixed domain tags `"schedule"`, `"workload"`,
`"net"`, `"disk"`, `"tiebreak-audit"`; consuming randomness in one subsystem
therefore never perturbs another, which keeps minimization stable.

### 8.4 Algorithms and state behavior

**Run loop.** `Sim::run` executes:

1. Derive the fault schedule and workload streams from the seed: expand
   `SimRng("schedule")` against the `FaultProfile` into a concrete ordered
   list of `(virtual_time, DiskFault | NetFault | CrashRestart)` injections,
   and seed each workload generator from `SimRng("workload")`. The realized
   schedule is a pure function of `(seed, config)`.
2. Enqueue the initial events: node startup, client arrivals, and the first
   scheduled fault.
3. Pop the least event by `(virtual_time, tie_break)`. Advance `SimClock` to
   its time; the clock never moves backward and never advances except here.
4. Dispatch to the owning component. The component runs to completion on the
   single thread (a blocking wait is modeled as returning a continuation
   event), possibly enqueueing new events with times ≥ now.
5. Append to the trace: `(tick, virtual_time, tie_break, component, event
   kind, payload digest)` — digests, not payloads, so traces are compact and
   leak-free but still collision-detectable.
6. Run every invariant checker (section 8.4 "Invariants") against the global
   state.
7. On a violation: stop immediately, emit the `Violation`, write a corpus
   candidate file containing the seed, full config, and expected failure, and
   exit nonzero.
8. Otherwise loop from step 3 until the tick budget is exhausted; then run
   the terminal liveness checks and return the outcome with the trace SHA-256.

**Invariant checkers.** Each checker is a pure function of the simulation's
global state (all nodes' core states, WAL contents, in-flight client
knowledge) and runs every tick; the properties are the CORRECTNESS.md set,
cited by ID:

- P-01 DURABLE-ACK: every send the client model has seen acknowledged exists
  in the durable state of the owning node's `SimDisk` bytes (recovered on
  demand by a shadow recovery pass at crash boundaries).
- P-02 LEASE-EXCL: no message has two live leases.
- P-03 EVENTUAL: liveness, checked terminally under the fairness condition
  (faults stop before the final drain window); every sent message is by then
  delivered, dead-lettered, or expired.
- P-04 FIFO-ORDER: per-group delivered order equals acknowledged send order
  (active once R4 lands; the checker ships at R3 against R1 groups-free
  queues as a vacuous pass with a coverage counter proving it executed).
- P-05 DEDUP-EXACT, P-06 DELETE-IDEM: checked against the client model's
  op/result journal.
- P-07 RECEIPT-SAFE: structural check that no receipt handle is accepted
  twice across a lease epoch (full adversarial evidence is R6's).
- P-08 NO-SPLIT-LEASE and P-09 NO-LOST-ACK: partition-aware variants of P-02
  and P-01; they ship at R3, run vacuously green on one node, and become
  load-bearing at R7 without modification.
- P-10 NO-INVENTION: every delivered body's digest equals a previously sent
  body's digest, byte-identical.

A checker that cannot run (missing feature) must record itself as
`vacuous`, and the CI job fails if a property is vacuous at a gate that claims
it — this prevents silently green checkers.

**Reproducibility meta-test and divergence alarm.** SIM-DET-001 runs every
corpus seed twice and asserts the two traces are byte-identical (equal
SHA-256 and equal length). `run_against` powers the stronger alarm: the second
run holds a cursor into the first run's trace and compares every appended
entry; the first mismatch aborts the simulation immediately with both entries
printed — tick, component, event kind, both digests — so a nondeterminism bug
is caught at its first divergent event, not diagnosed backward from a
divergent final hash. Any divergence anywhere is a build-stopping bug
(NFR-MAINT-002): the run aborts nonzero, and CI treats it exactly like a
property violation.

**Failing-seed corpus.** A corpus entry is a TOML file at
`sim-corpus/<category>/<seed-hex>.toml`, where `<category>` is one of
`lease`, `durability`, `fifo`, `dedup`, `net`, `disk`, `liveness`, `raft`
(reserved), and `<seed-hex>` is the 16-digit lowercase hex seed:

```toml
# sim-corpus/durability/00003f9a12c4be77.toml
seed = "0x00003f9a12c4be77"
minimized = true
discovered = "2026-09-14"
fixed-by = "a1b2c3d"                 # commit that made this pass
[config]
nodes = 1
tick-budget = 200000
faults = "disk-heavy"
workload = "mixed-small"
[expected]
outcome = "pass"                     # regression: must not violate
was = "violation:P-01"               # what it produced before the fix
```

While a bug is open its entry carries `outcome = "violation:P-01"` (the run
must still reproduce the violation, proving the seed remains meaningful);
the fixing pull request flips it to `pass` and records `fixed-by`. The CI job
`sim-corpus` replays every entry, twice each (feeding SIM-DET-001), and fails
on any expectation mismatch. Corpus entries are never deleted; superseded
configs are migrated by a checked-in config-version table.

**Seed minimization.** `sim-min` shrinks a failing case while preserving its
violation:

1. Fix the seed. Binary-search the smallest failing tick budget.
2. Delta-debug the realized fault schedule: materialize it, then repeatedly
   remove half the injections; keep any half that still fails; recurse. The
   minimized run uses the explicit schedule (recorded in the corpus file
   under `[config.explicit-faults]`) instead of re-derivation.
3. Delta-debug the workload the same way over generated operations.
4. Reduce node count if the violation survives (relevant from R7).
5. Re-run the final candidate three times; all three must produce the
   identical violation at the identical event index, or minimization is
   rejected and the original is kept.
6. Write the minimized entry beside the original with `minimized = true`.

**Workload generators.** All generators draw only from `SimRng("workload")`:
`mixed-small` (uniform send/receive/delete, 256-byte bodies), `fifo-groups`
(group-skewed sends with interleaved receives; primary at R4), `delay-heavy`
(delayed sends racing visibility and retention), `churn` (create, purge, and
delete queues under traffic), and `crash-restart` (node kill/recover cycles
against `mixed-small`). Each generator maintains the client-side journal the
checkers consume, and each records an operation-mix histogram so coverage is
observable rather than assumed.

### 8.5 Implementation tickets and sequence

1. **R3.01 — Build the virtual-time executor.** Event queue with total
   ordering, `SimClock`, run-to-budget loop, continuation-style blocking.
   Done when a scripted event set executes in the documented order under
   1,000 shuffled insertion orders.
2. **R3.02 — Build `SimRng`.** ChaCha12 sub-streams under domain-separated
   keys, stable across platforms. Done when cross-platform fixtures (x86_64
   and aarch64 CI runners) produce identical streams.
3. **R3.03 — Build `SimDisk`.** Full `Disk` trait, per-op failure, byte-budget
   fill, tear-on-crash keeping a configured prefix of the final append, and
   crash/restart that discards non-fsynced state. Done when the entire R2
   CRSH- corruption matrix reproduces under `SimDisk` with identical
   classifications to `FaultDisk`.
4. **R3.04 — Build `SimNet`.** Drop, delay, duplicate, reorder, asymmetric
   partition (A→B severed while B→A flows), and slow node (all its events
   dilated by a factor). Done when each fault is provably applied — a
   per-fault counter and a scripted assertion per behavior — on a two-node
   echo workload.
5. **R3.05 — Derive fault schedules from seeds.** `FaultProfile` expansion to
   a realized injection list, plus explicit-schedule mode for minimization.
   Done when schedule derivation is a pure function (same seed+config twice
   ⇒ identical schedule) and every profile knob observably changes realized
   intensity.
6. **R3.06 — Host a Relay node in the simulator.** `relay-core` + `relay-wal`
   over `SimClock`/`SimDisk`/`SimRng`, with `AdvanceTime` entries driven by
   virtual time per ADR-0005. Done when the `mixed-small` workload completes
   a fault-free run whose final state matches the R1 reference model.
7. **R3.07 — Implement workload generators and client journals.** All five
   generators with histograms and journals. Done when each generator's
   histogram meets its documented mix within tolerance across 100 seeds.
8. **R3.08 — Implement the invariant checkers.** P-01 through P-10 as pure
   per-tick functions with vacuous-run accounting. Done when, for each
   non-vacuous property, a deliberately seeded bug (one-line mutation behind
   a test-only feature flag) is caught by exactly the intended checker.
9. **R3.09 — Implement traces, the meta-test, and the divergence alarm.**
   Trace sink, SHA-256, `run_against` first-divergence abort. Done when an
   injected nondeterminism (a hash-map iteration order deliberately leaked
   into scheduling under a test flag) is caught at its first divergent event
   with both entries printed.
10. **R3.10 — Implement the corpus and CI wiring.** TOML format, loader with
    schema validation, `sim-corpus` runner (each entry twice), `sim-min`
    minimizer with the triple-confirmation rule, and the CI job with a fixed
    additional per-commit fresh-seed budget (200 fresh seeds per CI run,
    seed = commit-derived, so every commit explores new schedules). Done when
    CI fails on a planted expectation mismatch and on a planted vacuous
    property, and the whole job fits its documented runtime budget.

### 8.6 Test-driven evidence matrix

| Test | First failing condition | Required passing assertion |
| --- | --- | --- |
| SIM-DET-001 reproducibility meta-test | Any corpus seed run twice yields differing traces. | Same seed and config twice ⇒ byte-identical event trace (equal length and SHA-256) for every corpus entry and fresh CI seed (NFR-MAINT-002). |
| SIM-DET-002 divergence alarm | Nondeterminism is reported only as a final-hash mismatch, or not at all. | `run_against` aborts at the first divergent event, printing tick, component, kind, and both digests; the planted hash-order leak is caught this way. |
| SIM-EXEC-003 total event order | Event execution order depends on insertion order, hashing, or allocator. | 1,000 shuffled insertions of one scripted event set execute in one documented order. |
| SIM-DISK-004 CRSH parity | A corruption case classifies differently under `SimDisk` than under R2's `FaultDisk`. | The full R2 corruption matrix reproduces under `SimDisk` with identical `WalError`/repair outcomes. |
| SIM-NET-005 fault application | A configured net fault is silently unapplied. | Per-fault counters plus scripted behavioral assertions prove drop, delay, duplicate, reorder, asymmetric partition, and slow node each occur and act as specified. |
| SIM-INV-006 checker sensitivity | A seeded one-line bug for some property survives its checker. | Each non-vacuous property's planted bug is caught by exactly the intended checker at a reported tick. |
| SIM-INV-007 vacuous-run guard | A checker that never truly ran counts as green. | CI fails when any property claimed at the current gate reports vacuous. |
| SIM-DUR-008 crash-restart durability | A `crash-restart` workload seed loses an acknowledged send. | P-01 holds across all corpus and fresh seeds; every violation would emit a corpus candidate. |
| SIM-LIVE-009 terminal liveness | A message is neither delivered, dead-lettered, nor expired after the fault-free drain window. | P-03 passes terminally under the fairness condition for every seed. |
| SIM-CORP-010 corpus replay | A corpus entry's expectation mismatches, or an entry silently fails to load. | Every `sim-corpus/**/*.toml` validates against the schema and reproduces its expected outcome, twice. |
| SIM-MIN-011 minimization soundness | A minimized case fails to reproduce the original violation identically. | Triple-confirmation holds: three runs of the minimized entry violate the same property at the same event index. |
| SIM-CLK-012 nondeterminism deny-list | A simulation-linked crate reaches OS time, threads, or ambient randomness. | The architecture check proves no `std::time::Instant`/`SystemTime`, `std::thread::spawn`, `thread_rng`, `std::fs`, or `std::net` use in any crate compiled into a sim binary. |

### 8.7 Failure and security cases

- The divergence alarm is itself fail-closed: if the recorded trace ends early
  or the sink errors, the run aborts as divergent rather than passing by
  omission.
- Trace files and `Violation.detail` carry digests and metadata only, never
  message bodies, so failing seeds and traces can be attached to public issues
  without leaking payload content.
- Corpus TOML is untrusted input to CI: the loader schema-validates every
  field, bounds `tick-budget` and node count, and rejects unknown keys, so a
  malformed or malicious corpus entry fails the job instead of executing an
  unbounded run.
- A corpus category directory that is empty or missing fails the runner;
  silence is never evidence.
- Simulated results are labeled simulated everywhere they surface: the runner's
  output, CI job names, and documentation all say "deterministic simulation",
  and no SIM- result may be cited for a live-cluster or production claim
  (section 8.10).
- The fresh-seed budget derives seeds from the commit hash, never from wall
  time, so a red CI run is re-runnable bit-for-bit.
- `SimDisk` crash-restart discards exactly the non-fsynced suffix; it never
  models a disk that keeps unfsynced data, because an optimistic disk model
  would make P-01 vacuously easy and the parity suite (SIM-DISK-004) exists to
  prevent that drift.

### 8.8 Migration, documentation, and installation work

No on-disk user format is introduced; the corpus and trace formats are
repository-internal but versioned anyway (`format = 1` in each TOML; a config
version table governs migration of old entries). Documentation work:

- [OPERATIONS_TEST_PLAN.md](./OPERATIONS_TEST_PLAN.md) gains the SIM- family
  matrix, the corpus lifecycle (candidate → open violation → fixed regression),
  the fresh-seed budget, runtime budgets, and the zero-flake rule stated
  sharply: in a deterministic suite a flake is by definition a bug, and the
  divergence alarm is the tool that finds it.
- [CORRECTNESS.md](./CORRECTNESS.md) §mapping is updated so P-01 through P-10
  each name their SIM- checker and its vacuous/active status per gate.
- `docs/` gains a developer guide: how to run a seed, read a trace, minimize a
  failure, and check in a corpus entry, with the exact `cargo run -p relay-sim
  --bin sim-run -- --seed 0x…` invocations.
- CONTRIBUTING gains the rule that any bug found by simulation must land with
  its minimized corpus entry in the fixing pull request.

### 8.9 Acceptance evidence

R3 is accepted only when:

- SIM-DET-001 and SIM-DET-002 are green: byte-identical replays and
  first-event divergence detection, demonstrated against a planted
  nondeterminism;
- the full R2 corruption matrix reproduces under `SimDisk` (SIM-DISK-004);
- every `SimNet` fault is behaviorally verified (SIM-NET-005);
- P-01 through P-10 checkers exist, planted bugs are caught per property, and
  the vacuous-run guard fails a planted vacuous claim;
- the corpus runner replays every entry twice in CI within its runtime budget
  and the fresh-seed budget executes per commit;
- `sim-min` triple-confirms a real minimized case (at least one genuine
  R1/R2-era bug or a deliberately retained planted one is in the corpus at
  acceptance, so the corpus is demonstrably non-empty);
- the architecture deny-list check covers every sim-linked crate;
- documentation labels every simulation result as simulated.

### 8.10 Explicit deferrals

Simulation results never claim live-cluster behavior: R3 evidence is about
Relay's logic under a modeled OS, and kernel, filesystem, NIC, and timing
behavior of real deployments remain unverified until the live-cluster and
soak suites of R7–R9 — documentation and marketing must keep this line, per
the promotion rules in [docs/README.md](./README.md). R3 also defers:
multi-node Raft simulation content (the executor, `SimNet`, P-08, and P-09
are built now but earn nothing until R7 drives them), FIFO/dedup/DLQ
workloads as load-bearing evidence (R4), wire-protocol fuzzing (R6, FUZZ-),
performance measurement inside the simulator (virtual time makes latency
numbers meaningless; R9 measures on hardware), and any coverage-guided
fuzzing integration (OPEN_QUESTIONS.md, fail-closed: not claimed until
built).

### 8.11 Requirements traced

R3 is the terminal owning gate for `NFR-MAINT-002`; the section-16 matrix
records its completion here. R3 begins and materially advances `NFR-DUR-001`
and `NFR-DUR-002` evidence breadth (crash-restart seeds), `FR-FIFO` and
`FR-QUEUE` simulation coverage consumed at R4, `FR-REPL-001` through
`FR-REPL-008` tooling consumed at R7, and `NFR-MAINT-001` (checkers and
harnesses land test-first like everything else). It completes nothing else.

## 9. R4 — FIFO Groups, Deduplication, Delay, DLQ, and Redrive to Specification

**Status:** planned.

**Effort range:** 8–12 focused days, including the mutation-testing gate on
`relay-core`.

### 9.1 Why this gate exists

R1 proved the standard-queue lifecycle; R4 completes the message-semantics
surface users actually depend on for correctness: strict per-group ordering,
exact five-minute deduplication, deferred availability, automatic
dead-lettering, and operator-driven redrive. These features interact in ways
that destroy naive implementations — a delayed message must not jump its
group, a dead-letter move must not reorder a group, a dedup window boundary
is an off-by-one factory — so every algorithm here lands inside `relay-core`
where the model checker (MODL-, from R1) and the simulator (SIM-, from R3)
can grind it. R4 is also where `NFR-MAINT-003` lands: with the semantic core
now feature-complete for queues, mutation testing proves the test suite
actually constrains the code — at least 85% of generated mutants on
`relay-core` must be killed — before the wire (R6) and replication (R7) start
depending on it.

### 9.2 Prerequisites

- R1, R2, and R3 are accepted: the core lifecycle is model-checked, the WAL
  gate is green, and the simulator hosts a node with checkers P-04, P-05, and
  P-06 currently vacuous and waiting.
- ADR-0005 is accepted: all time below — delay readiness, dedup windows,
  retention, visibility — advances only via `AdvanceTime` log entries, so
  every behavior in this section is deterministic and simulable.
- The R1 error taxonomy has stable codes reserved for: FIFO validation
  errors, dedup results, missing/invalid DLQ configuration, and concurrent
  redrive rejection.
- The reference model in `crates/relay-model` is ready to be extended in the
  same pull requests as the implementation; a semantics change that lands
  without its model counterpart fails the R1 conformance job.

### 9.3 Owned files, interfaces, and state

Extend `crates/relay-core` with modules (all pure; no IO, clock, or rng, per
the frozen architecture check):

- `src/fifo/group_index.rs`: per-queue, per-group ordered index and blocking;
- `src/fifo/dedup.rs`: the deduplication window ring;
- `src/delay.rs`: the delay wheel driven by `AdvanceTime`;
- `src/retention.rs`: the retention expiry index and sweep;
- `src/dlq.rs`: redrive-policy validation and the dead-letter move;
- `src/redrive.rs`: redrive-task state machine;
- `crates/relay-model/src/fifo.rs`, `dedup.rs`, `dlq.rs`: reference-model
  counterparts;
- `crates/relay-core/mutants.toml` and `.github/workflows/mutation.yml`:
  cargo-mutants configuration and the MUT- gate.

Core types, in the workspace's immutable style (every function returns new
state; nothing mutates in place):

```rust
pub struct GroupIndex {
    // (QueueId, GroupId) → ordered pending MessageIds and the blocking lease, if any
    groups: OrdMap<GroupKey, GroupQueue>,
}
pub struct GroupQueue { pending: Vector<MessageId>, in_flight: Option<LeaseEpochRef> }

pub fn group_enqueue(ix: &GroupIndex, key: &GroupKey, id: MessageId) -> GroupIndex;
pub fn group_next_deliverable(ix: &GroupIndex, max_groups: usize) -> Vec<GroupKey>;
pub fn group_block(ix: &GroupIndex, key: &GroupKey, lease: LeaseEpochRef) -> GroupIndex;
pub fn group_unblock(ix: &GroupIndex, key: &GroupKey) -> GroupIndex;

pub struct DedupState {
    ring: Vector<DedupEntry>,               // ordered by expiry (accepted_at + 300 s)
    by_key: OrdMap<DedupKey, DedupEntry>,   // DedupKey = explicit id | sha256(body)
}
pub struct DedupEntry { key: DedupKey, original_id: MessageId, accepted_at: Nanos }

pub enum DedupOutcome { Fresh, Duplicate { original_id: MessageId } }
pub fn dedup_check(d: &DedupState, key: &DedupKey, now: Nanos) -> DedupOutcome;
pub fn dedup_admit(d: &DedupState, key: DedupKey, id: MessageId, now: Nanos) -> DedupState;
pub fn dedup_expire(d: &DedupState, now: Nanos) -> DedupState; // drops entries with accepted_at + WINDOW <= now

pub const DEDUP_WINDOW: Nanos = Nanos::from_secs(300); // fixed, not configurable (spine §6)

pub struct RedriveTask {
    pub task_id: TaskId,
    pub source_dlq: QueueId,
    pub destination: QueueId,
    pub total_at_start: u64,
    pub moved: u64,
    pub status: RedriveStatus, // Running | Completed | Failed(RedriveFailure)
}
```

Owned state extensions inside `CoreState`: the group index, dedup ring, delay
wheel, retention index, per-queue redrive policy
(`{ dlq: QueueId, max_receive_count: u32 /* 1..=1000 */ }`), dead-letter
provenance per message (`{ source_queue, receive_count_at_move, moved_at }`,
FR-QUEUE-018), and at most one `RedriveTask` per DLQ. All of it is reached
exclusively through `relay_core::apply`, so R2 durability and R3 simulation
cover it with zero new plumbing.

### 9.4 Algorithms and state behavior

**FIFO admission (FR-FIFO-001).** `CreateQueue` with a name ending `.fifo`
creates a FIFO queue; the suffix is required for FIFO and forbidden for
standard queues, and is excluded from the 80-character name budget. On a FIFO
queue, `Send` without `MessageGroupId` is a validation error; `GroupId` is
limited to 128 bytes. Per-message `DelaySeconds` on a FIFO queue is rejected
with a stable error — only the queue-level default delay applies, uniformly,
because a per-message delay could reorder a group against its acknowledged
send order (this is the edge case that forces the rule; the error message
states it). Standard queues reject `MessageGroupId` and dedup parameters.

**Per-group ordering and blocking (FR-FIFO-002, -003, -004).**

1. An acknowledged FIFO send appends the message ID to its group's `pending`
   vector; acknowledged send order is definitionally the vector order (P-04).
2. `Receive` collects deliverable groups: a group is deliverable iff its
   `in_flight` slot is empty and its head message is `Available` (not
   `Delayed`). Groups are scanned in order of oldest available head; distinct
   groups fill the receive independently, up to the requested max of 10
   (FR-FIFO-003).
3. From one deliverable group, a receive may take multiple consecutive head
   messages in pending order; all delivered messages of that receive from
   that group share one lease epoch and the group's `in_flight` slot records
   it.
4. While `in_flight` is occupied, the group yields nothing — later messages
   of that group are invisible to every consumer regardless of demand
   (FR-FIFO-004). No starvation of other groups results: blocking is
   per-group state, and step 2 skips blocked groups.
5. The group unblocks when every message of the blocking delivery is deleted,
   or the lease expires, or `ChangeMessageVisibility(0)` returns the delivery.
   Expiry and return put undeleted messages back at the head of `pending` in
   their original order, and their receive counts increment (FR-QUEUE-005
   interaction).
6. A group whose `pending` becomes empty and whose `in_flight` is empty is
   removed from the index; group identity has no lifecycle of its own.

**Dedup window ring (FR-FIFO-005, -006, -007; P-05).** The dedup key is the
explicit `MessageDeduplicationId` when supplied; otherwise, on a queue with
content-based deduplication enabled, `SHA-256(body)`; a FIFO send with
neither is a validation error. Semantics of `dedup_check` at applied time
`now` against an entry accepted at `t`:

1. The window is half-open: `[t, t + 300 s)`. A matching send strictly inside
   the window is suppressed and returns the original message ID with a
   duplicate marker — success, not an error, so retrying producers converge
   (FR-FIFO-007).
2. Both boundaries are exact and tested from both directions: a duplicate
   applied at `t + 299.999 s` is suppressed; a send applied at exactly
   `t + 300.000 s` is accepted as a new message, and its acceptance arms a
   fresh window at its own applied time. Symmetrically at the opening edge: a
   send at exactly `t` with the same key is the entry itself; the first
   subsequent duplicate at any `now > t, now < t + 300 s` is suppressed.
3. Expiry is driven by `AdvanceTime`: `dedup_expire` drops entries whose
   `accepted_at + 300 s ≤ now` from the ring head (ring order equals expiry
   order, so expiry is O(expired)). Because both `dedup_check` and expiry use
   the same applied clock, a boundary can never be time-of-check racy —
   determinism here is ADR-0005 doing its job.
4. A suppressed duplicate performs no state change other than nothing: no
   group append, no delay entry, no retention entry, no receive-count effect.
5. Dedup applies to sends only. DLQ moves and redrive moves are not sends and
   never consult or arm the window (a redriven message must not be suppressed
   because its body was recently sent elsewhere).

**Delay wheel (FR-QUEUE-010, -011).**

1. A standard-queue send with `DelaySeconds d ∈ 1..=900`, or any send on a
   queue whose default delay is `d` and which does not override it (a
   per-message value, including explicit 0, overrides the queue default —
   FR-QUEUE-011), enters state `Delayed` with `ready_at = applied_time + d`.
2. The wheel is a two-level hierarchical structure keyed by `ready_at`
   (1-second outer buckets, exact `Nanos` ordering within); `AdvanceTime(t)`
   pops every entry with `ready_at ≤ t` in `(ready_at, message ULID)` order
   and transitions each `Delayed → Available`, appending FIFO messages to
   their group index at that moment.
3. Availability is "not before" (`NG-04`): entries become available at the
   first `AdvanceTime` at or after `ready_at`, never before it.
4. A delayed message is invisible to `Receive`, is purged by `PurgeQueue`,
   counts against retention from its send time (not its ready time), and can
   expire while still delayed (`Delayed → Expired` removes it from the
   wheel).

**Retention expiry sweep (FR-QUEUE-014).**

1. Every stored message carries `expires_at = accepted_at + retention`
   (retention 60 s–14 d, default 4 d, per queue); dead-lettered messages
   restart the clock from their move time under the DLQ's retention.
2. The retention index orders messages by `expires_at`; each `AdvanceTime`
   pops due entries and transitions them to `Expired`, removing them from
   the available set, delay wheel, group index, and dedup provenance —
   whatever state they were in, including `InFlight` (spine lifecycle:
   `* → Expired`).
3. Expiring an in-flight message invalidates its receipt handle: a
   subsequent `Delete` or `ChangeMessageVisibility` with that handle returns
   the stable invalid-handle error (FR-QUEUE-007 interaction); the
   already-idempotent-delete case is distinguished by the message having
   been deleted, not expired.
4. Expiring a FIFO group's blocking in-flight message unblocks the group;
   expiring a group head advances the head. Order among survivors is
   untouched.
5. `SetQueueAttributes` lowering retention re-times existing messages against
   the new value at the next `AdvanceTime`; raising it extends them. The
   sweep is incremental and bounded per applied entry (at most a documented
   batch per `AdvanceTime`, with the remainder carried to the next entry) so
   one entry cannot stall the state machine.

**Dead-letter move (FR-QUEUE-017, -018; FR-FIFO-008).** The redrive policy
`{ dlq, maxReceiveCount ∈ 1..=1000 }` is validated at configuration time:
the DLQ must exist, a FIFO source requires a FIFO DLQ and a standard source a
standard DLQ, and a queue cannot be its own DLQ. The move algorithm:

1. Trigger: a message returns to eligibility — visibility expiry,
   `ChangeMessageVisibility(0)`, or lease release — with
   `receive_count ≥ maxReceiveCount`. The check runs at return time, not at
   next receive, so poisoned messages leave the queue without needing another
   consumer.
2. Construct the dead-lettered record preserving body and attributes
   byte-identically and recording provenance: source queue ID, final receive
   count, and move time from the applied clock (FR-QUEUE-018). The message
   keeps its ULID; identity is stable across the move.
3. Remove the message from every source index — available set, group index,
   retention index — in the same `apply` step; the move is atomic within one
   log entry (it is one state transition, so a crash either applied it or
   did not; R2 guarantees nothing in between).
4. Append to the DLQ: standard DLQ at the tail; FIFO DLQ under the message's
   original `MessageGroupId`. When one return event dead-letters several
   messages of one group (a multi-message delivery expiring), they are
   appended in their original relative send order, so per-group order is
   preserved through the move (FR-FIFO-008). The DLQ's dedup window is not
   consulted (section "Dedup", rule 5).
5. Set the message's DLQ retention clock and reset nothing else;
   `receive_count` is preserved as provenance and a fresh in-DLQ receive
   count starts at zero.
6. Edge: the configured DLQ was deleted after validation. The move fails
   closed — the message stays in the source queue as `Available`, an
   `Output::DeadLetterMoveFailed` is emitted for observability, and the
   receive count no longer increments the trigger (the check is
   `≥`, so it simply re-fires when a DLQ exists again). No message is
   dropped because an operator broke configuration.

**Redrive task (FR-QUEUE-019).**

1. `StartRedrive { source_dlq, destination }` validates: source has messages,
   destination exists and is type-compatible (FIFO↔FIFO, standard↔standard),
   destination is configured as a legal redrive target (its own DLQ chain
   does not point back at the source), and no task is currently `Running`
   for this DLQ — a concurrent start returns the stable
   concurrent-redrive-rejected error.
2. The accepted command creates the `RedriveTask` with
   `total_at_start` = current DLQ depth and `moved = 0`. The task is core
   state: it is in the log, durable via R2, replicated later via R7, and
   visible to the simulator.
3. The task advances by `RedriveStep` log entries, each moving up to 10
   oldest messages: receive count resets to 0, body, attributes, ULID, and
   (for FIFO) group are preserved, DLQ provenance fields are retained on the
   record for audit, and FIFO messages enter the destination group index in
   their DLQ order — which is their original per-group send order by
   FR-FIFO-008 — so redrive-back also preserves group order.
4. Progress is `moved / total_at_start`, exposed through the describe
   surface; messages that arrive in the DLQ after the task started are not
   part of `total_at_start` and are not moved (the task drains a fixed
   prefix, so it terminates).
5. Failure mid-task: a crash between steps recovers the task in `Running`
   state and the next step continues from the durable `moved` count —
   each step moves distinct concrete message IDs, so recovery can neither
   skip nor duplicate a message (the R3 `crash-restart` workload gains a
   redrive variant to prove it). Destination deletion mid-task transitions
   the task to `Failed(DestinationDeleted)` with `moved` preserved; already
   moved messages stay moved (partial redrive is not rolled back, and the
   describe output says so). A `Failed` or `Completed` task frees the DLQ
   for a new `StartRedrive`.

**Mutation testing (NFR-MAINT-003).** cargo-mutants runs over `relay-core`
with a checked-in `mutants.toml`: timeouts per mutant, exclusions limited to
derived trait impls and error-display formatting (each exclusion individually
justified in the file), and the kill threshold ≥ 85%. The gate fails below
threshold and publishes the surviving-mutant list as a CI artifact; every
surviving mutant is either killed by a new test in the same pull request or
documented in the gate review as semantically unreachable. The MUT- job is
required for R4 acceptance and runs nightly thereafter.

### 9.5 Implementation tickets and sequence

1. **R4.01 — Enforce FIFO admission rules.** `.fifo` suffix logic, required
   `MessageGroupId`, 128-byte group bound, per-message-delay rejection on
   FIFO, standard-queue rejection of FIFO parameters, all with stable error
   codes and model-side mirrors. Done when the R1 conformance job passes
   with the new command validations active.
2. **R4.02 — Implement the group index.** Pure enqueue, deliverable-scan,
   block, unblock, head-return ordering, and empty-group removal. Done when
   property tests over arbitrary interleavings uphold vector-order delivery
   and the model checker explores block/unblock against expiry.
3. **R4.03 — Wire group-blocking receives.** Multi-group fills, single-lease
   multi-message group deliveries, blocked-group invisibility, and unblock on
   delete/expiry/visibility-zero. Done when MODL- histories over concurrent
   FIFO receivers verify P-04 and no blocked-group delivery exists in any
   explored schedule.
4. **R4.04 — Implement the dedup ring.** Half-open window, both exact
   boundaries in both directions, explicit-ID override of content hashing,
   duplicate returns original ID, expiry via `AdvanceTime`, no side effects
   on suppression. Done when boundary tests at `t + 300 s ± 1 ns` pass and
   the reference model agrees on every generated dedup history.
5. **R4.05 — Implement the delay wheel.** Delayed state, queue-default vs
   per-message precedence (explicit 0 overrides), not-before pop order,
   purge/retention interactions. Done when SIM- `delay-heavy` seeds and CORE
   property tests uphold NG-04 and FIFO order with uniform default delay.
6. **R4.06 — Implement the retention sweep.** Expiry index, `* → Expired`
   from every state, in-flight handle invalidation, group unblock/advance on
   expiry, bounded incremental sweep, retention reconfiguration. Done when a
   generated matrix of (state × expiry timing) transitions exactly as the
   lifecycle diagram specifies.
7. **R4.07 — Implement redrive-policy validation and the DLQ move.**
   Policy validation, return-time trigger, atomic single-entry move,
   provenance record, deleted-DLQ fail-closed path. Done when every trigger
   path (expiry, visibility-zero, release) moves at exactly
   `receive_count ≥ maxReceiveCount` and never below it.
8. **R4.08 — Preserve FIFO order through dead-lettering.** Group-preserving
   DLQ append, multi-message same-group move ordering, and dedup
   non-consultation on moves. Done when a MODL- scenario dead-letters an
   interleaved multi-group workload and per-group order verifies end to end
   in the DLQ (FR-FIFO-008).
9. **R4.09 — Implement the redrive task.** Start validation, concurrent-start
   rejection, `RedriveStep` batching, fixed-prefix termination, progress,
   crash recovery mid-task, destination-deleted failure. Done when the R3
   `crash-restart` redrive workload completes across injected crashes with
   `moved` exact and zero duplicated or lost messages.
10. **R4.10 — Extend model, simulator, and corpus.** Reference-model FIFO/
    dedup/DLQ semantics, activation of P-04/P-05/P-06 checkers (vacuous no
    longer permitted for them), `fifo-groups` and `delay-heavy` workloads
    promoted to load-bearing, and corpus categories `fifo` and `dedup`
    seeded with at least the boundary and reorder cases found during
    development. Done when the vacuous-run guard passes with the three
    properties active and the corpus replays green.
11. **R4.11 — Land the mutation-testing gate.** `mutants.toml`, CI job,
    threshold enforcement at ≥ 85% killed on `relay-core`, surviving-mutant
    artifact, and the kill-or-justify review rule in CONTRIBUTING. Done when
    the job is required, green at or above threshold, and a planted
    trivially-survivable mutant (behind a test branch) demonstrably fails
    the job.

### 9.6 Test-driven evidence matrix

| Test | First failing condition | Required passing assertion |
| --- | --- | --- |
| CORE-FIFO-001 admission rules | A FIFO send without a group is accepted, a standard send with one is accepted, or per-message delay reaches a FIFO queue. | Every admission rule in section 9.4 rejects with its exact stable error code, mirrored by the reference model. |
| FIFO-ORD-002 per-group delivery order | Any explored schedule delivers a group's messages out of acknowledged send order. | MODL- exploration over concurrent FIFO receivers finds vector-order delivery in every history (P-04, FR-FIFO-002). |
| FIFO-BLK-003 group blocking | A blocked group yields a delivery, or a blocked group starves an unrelated group. | No delivery from an in-flight group in any schedule; distinct groups fill receives in parallel (FR-FIFO-003, -004). |
| FIFO-RET-004 head return order | Expiry or visibility-zero returns messages behind newer pending messages. | Returned messages reoccupy the head of `pending` in original order with receive counts incremented. |
| CORE-DED-005 dedup boundary, suppress side | A duplicate at `t + 299.999 s` is accepted as new. | The duplicate is suppressed and returns the original message ID with the duplicate marker (FR-FIFO-007, P-05). |
| CORE-DED-006 dedup boundary, accept side | A send at exactly `t + 300.000 s` is suppressed. | The send is accepted as a new message and arms a fresh window at its own applied time. |
| CORE-DED-007 dedup key precedence | Content hashing runs despite an explicit `MessageDeduplicationId`, or suppression mutates state. | Explicit ID overrides SHA-256(body); suppression performs no group, delay, retention, or count change (FR-FIFO-005, -006). |
| CORE-DLY-008 delay precedence and not-before | A per-message delay (including explicit 0) loses to the queue default, or a message is available before `ready_at`. | Precedence follows FR-QUEUE-010/-011; availability begins at the first `AdvanceTime` ≥ `ready_at` (NG-04). |
| CORE-RET-009 expiry from every state | Some (state, expiry) pair diverges from the lifecycle diagram. | The full state × timing matrix transitions per `* → Expired`, including in-flight handle invalidation and group unblock (FR-QUEUE-014). |
| CORE-DLQ-010 move trigger exactness | A message moves below `maxReceiveCount`, fails to move at it, or loses body/attribute bytes or provenance. | Moves occur exactly at `receive_count ≥ maxReceiveCount` on return; body and attributes are byte-identical and provenance is complete (FR-QUEUE-017, -018). |
| FIFO-DLQ-011 order through dead-letter | A multi-message group move reorders the group, or a DLQ move consults the dedup window. | Per-group order is preserved into the DLQ and through redrive-back; moves never touch dedup state (FR-FIFO-008). |
| CORE-RDR-012 redrive progress and recovery | A crash mid-task duplicates or loses a message, progress misreports, or two tasks run on one DLQ. | Crash-injected redrive completes with exact `moved`, fixed-prefix termination, concurrent start rejected, destination-deletion yields `Failed` with partial moves preserved (FR-QUEUE-019). |
| SIM-FIFO-013 simulated FIFO under faults | A `fifo-groups` or `delay-heavy` seed violates P-04, P-05, or P-06, or those checkers report vacuous. | All three checkers are active and green across corpus and fresh seeds; violations would produce corpus candidates. |
| MUT-CORE-014 mutation kill rate | Fewer than 85% of generated `relay-core` mutants are killed, or an exclusion lacks written justification. | cargo-mutants reports ≥ 85% killed; every survivor is killed in-PR or justified in the gate review; the planted survivable mutant fails the job (NFR-MAINT-003). |

### 9.7 Failure and security cases

- Group IDs are bounded at 128 bytes and treated as opaque bytes; they are
  never interpolated into paths, log messages beyond a length-limited hex
  digest, or error text, so a hostile group ID cannot smuggle content into
  operator surfaces.
- The dedup ring and by-key map are bounded by construction (entries expire
  in 300 s), but a burst of unique keys is still memory: the per-queue
  in-flight and depth caps from FR-QUEUE-016 bound admission upstream, and
  the dedup structures are counted in the per-queue memory accounting that
  R6's quota work will enforce.
- Content-based dedup hashes exactly the body bytes — attributes are
  excluded by specification, and the tests pin this so two sends differing
  only in attributes deduplicate (the documented, intentional behavior).
- A dead-letter cycle (A→B and B→A redrive policies) is legal configuration;
  the move algorithm cannot loop unboundedly because each move requires a
  fresh exhaustion of `maxReceiveCount` receives, and the redrive-task
  validation rejects a destination whose DLQ chain points back at the source
  DLQ.
- The deleted-DLQ path fails closed with the message retained; no
  configuration mistake can drop a message silently.
- `RedriveStep` batching means a hostile or buggy client cannot start a task
  that produces one unbounded log entry; every entry moves at most 10
  messages.
- Retention lowering is an operator-authorized data-loss operation; the
  `SetQueueAttributes` response names the number of messages that the next
  sweep will expire, so the loss is announced, not discovered.
- All new state lives behind `relay_core::apply`; there is no auxiliary
  store, timer thread, or background job to diverge from the log — the
  bounded incremental retention sweep exists precisely to keep it that way.

### 9.8 Migration, documentation, and installation work

No new on-disk format is introduced: every R4 structure serializes inside
the existing snapshot state (its state-schema version increments, with an
old-snapshot fixture proving version-1 snapshots still load and re-derive
the new empty indexes). Documentation work:

- [PRODUCT_REQUIREMENTS.md](./PRODUCT_REQUIREMENTS.md) acceptance sections
  for FR-QUEUE-010/-011/-014/-017/-018/-019 and FR-FIFO-001..008 are marked
  against their landed tests;
- [CORRECTNESS.md](./CORRECTNESS.md) flips P-04, P-05, and P-06 from planned
  to gate-backed, cites FIFO-ORD-002, CORE-DED-005/-006, and CORE-DLQ-010 in
  its mapping, and re-states NG-01 and NG-03 next to the FIFO section so
  per-group ordering is never promoted to global ordering;
- [OPERATIONS_TEST_PLAN.md](./OPERATIONS_TEST_PLAN.md) gains the FIFO- and
  MUT- family matrices and the nightly mutation schedule;
- user-facing queue documentation gains the FIFO, delay, DLQ, and redrive
  semantics with the exact boundary rules (half-open dedup window, not-before
  delay, return-time DLQ trigger) stated as normative.

No installation work: R4 is core semantics, reachable only through the test
harnesses until R6 exposes the wire.

### 9.9 Acceptance evidence

R4 is accepted only when:

- every matrix row in section 9.6 is green in CI, including the MODL-
  conformance job with the extended reference model;
- the dedup boundary tests pass at `t + 300 s ± 1 ns` in both directions;
- the crash-injected redrive workload completes with exact progress and zero
  duplicated or lost messages across the R3 seed set;
- P-04, P-05, and P-06 are active (non-vacuous) in the simulator and green
  across the corpus plus the fresh-seed budget;
- the `fifo` and `dedup` corpus categories are non-empty;
- MUT-CORE-014 reports ≥ 85% mutants killed on `relay-core` with every
  survivor killed or justified;
- the R2 recovery-equivalence gate and the R1 model gate both still pass
  with the new state — prior evidence replays green (NFR-MAINT-004
  discipline);
- documentation states the boundary semantics exactly and repeats NG-01 and
  NG-03 beside every FIFO claim.

### 9.10 Explicit deferrals

R4 earns queue-semantics evidence only. It takes no credit for wire-visible
behavior — FR-QUEUE-009 long polling, the receive/redrive API surface,
authentication, and quotas are R6 — and no credit for replicated FIFO
behavior: per-group ordering under leader failover is proven at R7, and
until then every FIFO claim is single-node. Topic fanout into FIFO queues
(FR-TOPIC-008) is R5. Dedup-window configurability, delay above 900 s, and
priority within groups are not planned features; if wanted they enter
[OPEN_QUESTIONS.md](./OPEN_QUESTIONS.md) with fail-closed defaults. The
mutation gate covers `relay-core` only; extending MUT- to `relay-wal` and
`relay-raft` is R10's audit scope, not an R4 claim.

### 9.11 Requirements traced

R4 is the terminal owning gate for `FR-QUEUE-010`, `FR-QUEUE-011`,
`FR-QUEUE-014`, `FR-QUEUE-017`, `FR-QUEUE-018`, `FR-QUEUE-019`,
`FR-FIFO-001` through `FR-FIFO-008`, and `NFR-MAINT-003`; the section-16
matrix records their completion here. R4 advances `FR-TOPIC-008` (FIFO
fanout semantics it will reuse at R5), `FR-REPL-004`-adjacent lease
semantics consumed at R7, `NFR-MAINT-001` (every algorithm above lands
test-first), and `NFR-MAINT-002` (new corpus categories). It completes
nothing outside its terminal list.

## 10. R5 — Topics, Subscriptions, and Filter Policies Fan Out Correctly

**Status:** planned.

**Effort range:** 5–8 focused days.

### 10.1 Why R5 exists

R4 finishes the single-queue delivery machine. R5 adds the pub/sub layer on top
of it without inventing a second delivery machine: a topic is a routing table,
a subscription is a queue binding with an optional filter recorded at subscribe
time, and a publish is a validation step followed by zero or more ordinary sends
through the identical reducer path that `Command::Send` uses. This phase exists
as its own gate because fanout is where queue systems quietly break their own
guarantees — by promising cross-queue atomicity they cannot deliver (NG-02
forbids it), by evaluating filters against mutated subscription state, or by
losing FIFO group and dedup semantics the moment a message arrives via a topic
instead of a direct send (FR-TOPIC-008 forbids that).

R5 is deliberately small. Everything it adds lives in `crates/relay-core` and
its reference model, is exercised through the R3 simulation harness, and is
checked by the R1 model checker extended with topic operations. No wire surface,
no storage format change, and no replication behavior is introduced here.

### 10.2 Prerequisites

- R4 is accepted: FIFO groups, content and explicit deduplication, delay,
  retention, DLQ, and redrive pass their MODL-FIFO and SIM suites, and the
  mutation-testing floor (NFR-MAINT-003) holds on `relay-core`.
- R3 is accepted: any new failing simulation seed can be checked into the corpus
  and replayed byte-identically in CI (NFR-MAINT-002).
- R1's reference model and JSONL history format (ADR-0007) accept new operation
  types without changing existing record encodings.
- The `Command` enum already reserves `CreateTopic`, `DeleteTopic`, `Subscribe`,
  `Unsubscribe`, and `Publish` variants (BUILD_PLAN §3, spine-fixed signatures);
  R5 implements their reducers rather than adding new command surface.

### 10.3 Owned files, interfaces, and state

R5 creates, inside `crates/relay-core`:

- `src/topic/mod.rs`: topic and subscription state, exported invariant checks;
- `src/topic/registry.rs`: `CreateTopic`, `DeleteTopic` reducers;
- `src/topic/subscription.rs`: `Subscribe`, `Unsubscribe` reducers;
- `src/topic/publish.rs`: the publish pipeline of section 10.4;
- `src/filter/ast.rs`: the filter-policy AST and its size accounting;
- `src/filter/parse.rs`: the bounded filter-policy parser (also the R6 fuzz
  target `filter_policy`, exported behind a `pub` parse function now so the
  fuzz target added in R6 links against exactly this code);
- `src/filter/eval.rs`: the pure evaluator, attributes in, boolean out;
- `crates/relay-model/src/topic.rs`: reference-model mirror of all of the above.

State added to `CoreState` (immutable-update style; every reducer returns a new
`CoreState` inside `Applied`, never mutates the input, matching the spine §5
`apply` signature):

```rust
pub struct TopicId(pub Ulid);
pub struct SubscriptionId(pub Ulid);

pub struct TopicConfig {
    pub name: TopicName, // ^[A-Za-z0-9_-]{1,80}$, spine §6
    pub tags: Vec<TagPair>,
}

pub struct TopicState {
    pub id: TopicId,
    pub config: TopicConfig,
    pub created_at: Nanos,               // log-applied clock, ADR-0005
    pub subscriptions: BTreeSet<SubscriptionId>,
}

pub struct SubscriptionState {
    pub id: SubscriptionId,
    pub topic: TopicId,
    pub queue: QueueId,
    pub filter: Option<FilterPolicy>,    // recorded at subscribe time, FR-TOPIC-002
    pub raw_filter_json: Option<Bytes>,  // byte-exact original, for Describe
    pub created_at: Nanos,
}

pub struct PublishCommand {
    pub topic: TopicId,
    pub body: Bytes,                     // ≤ 256 KiB, FR-QUEUE-013 limits reused
    pub attributes: Vec<MessageAttribute>, // ≤ 10, typed, FR-QUEUE-012 reused
    pub message_group_id: Option<GroupId>,        // required by FIFO subscriptions
    pub message_deduplication_id: Option<DedupId>,// per-destination-queue window
}
```

The filter-policy AST is depth-1 by construction — a map from attribute key to a
clause list, with no nested policy type representable:

```rust
pub struct FilterPolicy {
    // 1..=5 keys (FR-TOPIC-004 validation limit); keys are ANDed.
    pub keys: BTreeMap<AttrKey, ClauseList>,
    pub total_values: u16, // ≤ 150 across the whole policy, counted at parse
}

pub struct ClauseList(pub Vec<FilterClause>); // clauses within a key are ORed

pub enum FilterClause {
    Exact(AttrLiteral),                      // string or number literal
    AnythingBut(Vec<AttrLiteral>),           // 1..=20 literals
    Prefix(String),                          // non-empty, ≤ 256 bytes
    NumericRange {
        low: Option<(NumBound, f64)>,        // NumBound::Inclusive | Exclusive
        high: Option<(NumBound, f64)>,       // at least one of low/high present
    },
    Exists(bool),
}

pub fn parse_filter_policy(json: &[u8]) -> Result<FilterPolicy, FilterPolicyError>;
pub fn eval_filter(policy: &FilterPolicy, attrs: &[MessageAttribute]) -> bool;
```

`FilterPolicyError` carries a field path (`keys["tier"].clauses[2]`) so that
FR-TOPIC-005's field-level rejection at subscribe time is a stable, testable
string, not free prose. `Output` gains three variants consumed by later gates:
`TopicCreated`, `SubscriptionRecorded`, and `PublishResult { publish_id: Ulid,
matched: u32, delivered: u32, failures: Vec<(SubscriptionId, DeliveryError)> }`.

### 10.4 Algorithms and state behavior

**Publish pipeline.** `Command::Publish` applies as one atomic reducer step in
the single-threaded state machine; "concurrent" always means "adjacent in the
log", which is what makes every edge case below decidable:

1. Resolve `topic`. If absent (never created, or removed by an earlier
   `DeleteTopic` log entry), return `Err(NoSuchTopic)` with no state change.
2. Validate the message exactly as `Send` does: body ≤ 256 KiB with the stable
   oversize error, ≤ 10 typed attributes, group ID ≤ 128 bytes when present.
   A message rejected here is rejected for all subscriptions; validation runs
   once, before any fanout work.
3. Snapshot the subscription set: the `BTreeSet<SubscriptionId>` held by the
   topic at this log position. An `Unsubscribe` earlier in the log has already
   removed its entry; an `Unsubscribe` later in the log has not. No other
   definition of "concurrent with unsubscribe" exists in this system.
4. For each subscription in ID order (deterministic iteration — required so a
   seed replays identically under R3): evaluate `eval_filter` against the
   message attributes. `filter: None` matches everything. Evaluation is pure;
   it cannot observe or modify state.
5. For each matching subscription, construct a `SendCommand` for the target
   queue carrying the identical body and attributes, plus group and dedup
   fields per the FIFO rules below, and pass it through the same internal send
   reducer that `Command::Send` uses — the same limits, the same in-flight and
   backpressure checks, the same delay/retention stamping, the same dedup
   window. There is no parallel delivery code path; a bug fixed in `Send` is
   fixed for fanout by construction.
6. Record each per-subscription result independently. A failure for one
   subscription (for example `GroupIdRequired` on a FIFO destination) is
   recorded in `PublishResult.failures` and does not roll back other copies:
   FR-TOPIC-003 is per-subscription and never cross-queue atomic, and NG-02 is
   the published non-guarantee that makes this honest.
7. Return `PublishResult` with `publish_id` (a fresh ULID from the log-applied
   clock), `matched`, `delivered`, and the failure list. Zero matches is a
   success with `matched == 0`, not an error.

**Filter evaluation.** For a policy and a message attribute set:

1. For each policy key, look up the attribute by exact key match. Type rules:
   `Exact` string literals compare byte-equal against String attributes;
   `Exact` number literals and `NumericRange` compare against Number attributes
   parsed as finite f64 (the parse already succeeded at send validation);
   Binary attributes match only `Exists(true)`. A clause applied to an
   attribute of the wrong type evaluates false, never errors.
2. If the attribute is absent: `Exists(false)` evaluates true; every other
   clause — including `AnythingBut` — evaluates false. "Anything but X" still
   requires the key to exist.
3. Within a key, clauses are ORed; across keys, results are ANDed. Empty clause
   lists are unrepresentable (parse rejects them), so no vacuous-truth case
   exists.
4. `NumericRange` bounds are checked as `low ≤/< v` and `v ≤/< high` with
   inclusive/exclusive per `NumBound`; a range with `low > high` was rejected
   at parse time.

**Filter-policy parsing and validation limits.** The policy travels as a UTF-8
JSON document (this is the one deliberate JSON surface in Relay; ADR-0004's
"no general-purpose serde on the wire" holds because R6 carries it as an opaque
length-prefixed byte field and this dedicated bounded parser — a named fuzz
target — is the only consumer). Parse steps:

1. Reject input over 8,192 bytes before any parsing.
2. Parse as a JSON object; any other top-level value is a field-level error.
3. Enforce 1–5 keys. Each key must be a valid attribute key (≤ 256 bytes).
4. Each value must be an array of clauses (depth 1: a clause is a literal, or a
   single-key object from the closed set `anything-but`, `prefix`, `numeric`,
   `exists`). Any nested object, nested array beyond the clause forms, or
   unknown clause key is rejected with its field path.
5. Count every literal across the policy; reject when the total exceeds 150.
6. Reject NaN, infinities, numeric strings where numbers are required, empty
   prefixes, empty `anything-but` lists, and `numeric` arrays that do not
   decode to a valid one- or two-bound range.
7. Return the AST plus the original bytes; `Subscribe` stores both, so the
   policy a user reads back is byte-identical to the one they wrote, while
   evaluation uses only the validated AST.

**Fanout into FIFO queues (FR-TOPIC-008).** When a matching subscription's
destination is a FIFO queue:

1. The `SendCommand` group ID is `PublishCommand.message_group_id`. If absent,
   this subscription's delivery fails with `GroupIdRequired` (recorded per
   step 6 above); FR-FIFO-001 is not waived for fanout.
2. The dedup ID is the explicit `message_deduplication_id` if present
   (FR-FIFO-006); otherwise, if the destination queue enables content-based
   deduplication, SHA-256 of the body (FR-FIFO-005); otherwise the delivery
   fails with `DedupIdRequired`.
3. The dedup window is per destination queue, exactly as for direct sends
   (FR-FIFO-007): one publish fanned into two FIFO queues deduplicates
   independently in each, and a duplicate within one queue's 300 s window
   returns that queue's original message ID as a successful delivery in the
   per-subscription results.
4. Group ordering is defined by reducer application order: because fanout calls
   the ordinary send reducer inside a single atomic apply, two publishes to the
   same topic enqueue into every common FIFO destination in log order, so
   per-group order in each destination equals publish acknowledgement order
   (P-04 extended to the topic path, proven by MODL-FIFO histories that mix
   direct sends and publishes into one group).

**Subscribe, unsubscribe, and deletion edge cases.**

1. `Subscribe` validates: topic exists, queue exists, filter (when present)
   parses under the limits above. Failures are field-level errors and change
   nothing. A duplicate binding (same topic, same queue, same raw filter bytes)
   returns the existing `SubscriptionId` idempotently; the same topic/queue
   pair with a different filter is a distinct subscription.
2. `Unsubscribe` removes the subscription; copies already enqueued by earlier
   publishes are untouched (FR-TOPIC-006). Unsubscribing an unknown ID is the
   stable `NoSuchSubscription` error, and a repeated unsubscribe of the same ID
   therefore fails cleanly rather than silently succeeding.
3. `DeleteTopic` removes the topic and all its subscriptions in one apply;
   subscribed queues and their messages are untouched (FR-TOPIC-007). A
   `Publish` entry after the delete gets `NoSuchTopic`. There is no "topic
   deleted mid-publish" interleaving: publish is a single atomic apply, so the
   fanout either sees the topic whole or not at all.
4. `DeleteQueue` (reducer semantics owned here; its wire surface is R6, per
   FR-ADMIN-005) removes every subscription that targets the queue in the same
   apply that removes the queue. Consequently a dangling subscription — one
   whose queue does not exist — is a state-machine invariant violation, not a
   runtime case: `topic::invariants::check` asserts every subscription's queue
   exists, the model checker enforces it across all explored interleavings, and
   `debug_assert!` guards it in every reducer. The publish pipeline does not
   contain defensive skip-a-missing-queue code, because that code would mask
   the invariant breach the checker exists to catch.

### 10.5 Implementation tickets and sequence

1. **R5.01 — Filter-policy AST and bounded parser.** Implement `ast.rs` and
   `parse.rs` with the limits of section 10.4, field-path errors, and byte
   preservation. Done when every limit (6th key, 151st value, depth-2 nesting,
   9 KiB input, NaN, empty prefix, empty anything-but, inverted range) has a
   failing-first test asserting the exact field path, and round-tripping stored
   raw bytes is byte-identical.
2. **R5.02 — Filter evaluator.** Implement `eval.rs` with the type rules,
   absent-key rules, and OR/AND composition. Done when a table-driven suite
   covers every clause × attribute-type × presence cell, and a property test
   confirms `eval_filter` is pure (identical inputs, identical output, no state
   access is even representable in its signature).
3. **R5.03 — Topic and subscription reducers.** Implement `CreateTopic`,
   `DeleteTopic`, `Subscribe`, `Unsubscribe` with idempotent duplicate
   subscribe, field-level subscribe rejection, and delete-removes-subscriptions.
   Done when reducer tests cover every error and idempotency case and the
   `check` invariant runs in all of them.
4. **R5.04 — Publish pipeline.** Implement `publish.rs` per the seven numbered
   steps, reusing the internal send reducer, with deterministic subscription
   iteration and per-subscription results. Done when a spy on the send reducer
   proves publish enqueues through it (no second code path exists to
   instrument), and zero-match, all-match, and mixed-failure publishes return
   the specified `PublishResult`.
5. **R5.05 — FIFO fanout semantics.** Implement group/dedup mapping per
   FR-TOPIC-008 including `GroupIdRequired`, `DedupIdRequired`, per-queue dedup
   windows, and duplicate-returns-original-ID. Done when mixed direct-send and
   publish traffic into one group passes the extended FIFO-order property test.
6. **R5.06 — Reference model and history extension.** Mirror topics in
   `relay-model`, extend the JSONL history schema with `create_topic`,
   `subscribe`, `unsubscribe`, `delete_topic`, and `publish` call records, and
   teach the Wing–Gong checker that a publish linearizes as its set of
   per-queue enqueues at one point. Done when a hand-written non-linearizable
   fanout history is rejected and all legal generated histories pass.
7. **R5.07 — Simulation scenarios and corpus.** Add SIM scenarios driving
   publish/subscribe/unsubscribe/delete races through `relay-sim`, including
   the log-adjacent unsubscribe and delete-topic cases, and check failing seeds
   into the R3 corpus. Done when the scenarios run in CI within the R3 runtime
   budget and every past failing seed replays.
8. **R5.08 — Mutation and acceptance sweep.** Run mutation testing over
   `src/topic/` and `src/filter/`, kill survivors to the ≥ 85% floor
   (NFR-MAINT-003), and re-run all R1–R4 accepted evidence. Done when the
   mutation report and the green prior-gate replay are attached to the R5 pull
   request.

### 10.6 Test-driven evidence matrix

| Test | First failing condition | Required passing assertion |
| --- | --- | --- |
| TOPC-PARSE-LIMITS | Parser accepts a 6th key, a 151st value, or depth-2 nesting. | Every limit violation returns `FilterPolicyError` with the exact offending field path; nothing over 8 KiB is parsed at all. |
| TOPC-PARSE-ROUNDTRIP | Stored policy bytes differ from submitted bytes. | `raw_filter_json` read back after subscribe is byte-identical to the subscribe input. |
| TOPC-EVAL-TABLE | Any clause × type × presence cell disagrees with section 10.4. | Full table passes, including `AnythingBut` on an absent key = false and `Exists(false)` on an absent key = true. |
| TOPC-EVAL-NUMERIC | Boundary value matches an exclusive bound or misses an inclusive one. | `NumericRange` inclusive/exclusive boundaries hold at exact f64 values on both ends. |
| TOPC-PUB-REUSE | Fanout enqueues without passing the instrumented send reducer. | Reducer spy counts one internal send per matching subscription; counts reconcile with `PublishResult.delivered`. |
| TOPC-PUB-PARTIAL | One failing FIFO destination rolls back or blocks other copies. | Mixed publish delivers to all valid destinations and records exactly the failing subscriptions in `failures`. |
| TOPC-PUB-UNSUB-RACE | A subscription removed earlier in the log still receives a copy, or one added later is missed. | For every log interleaving generated by the model checker, delivery set equals the subscription set at the publish entry's log position. |
| TOPC-DEL-TOPIC | Publish after `DeleteTopic` delivers, or delete strands subscription state. | Post-delete publish returns `NoSuchTopic`; no `SubscriptionState` referencing the topic survives the delete apply. |
| TOPC-DEL-QUEUE | Deleting a subscribed queue leaves a dangling subscription. | Invariant `check` passes after every explored `DeleteQueue`; model checker explores publish adjacent to queue delete with no invariant breach. |
| TOPC-FIFO-GROUP | A topic-delivered FIFO message skips group blocking or ordering. | MODL-FIFO history mixing direct sends and publishes into one group satisfies P-04; in-flight group blocking holds for fanned-in messages. |
| TOPC-FIFO-DEDUP | Dedup window is shared across destination queues, or duplicate is an error. | Same dedup ID fans into two queues independently; an in-window duplicate returns the destination queue's original message ID as success. |
| SIM-TOPC-SEEDS | A previously failing seed no longer reproduces its failure deterministically. | Every corpus seed replays byte-identically (NFR-MAINT-002) within the CI budget. |
| MUT-TOPC | Mutation score on `src/topic/` + `src/filter/` below 85%. | ≥ 85% mutants killed; surviving mutants individually justified in the report. |

### 10.7 Failure and security cases

- A filter policy is untrusted input even though R5 has no wire surface yet:
  the parser must be total (no panic on any byte sequence), allocation-bounded
  by the 8 KiB pre-check, and free of recursion (depth-1 AST means iterative
  parsing; a recursive descent over attacker-nested JSON is rejected at step 4
  before recursing). R6 fuzzes exactly this function.
- Filter evaluation must be O(keys × clauses) with no backtracking; a policy at
  the limits (5 keys, 150 values) evaluates in microseconds, so a hostile
  subscriber cannot turn publish into a CPU amplifier.
- Fanout amplification is bounded by subscription count, and each copy passes
  the destination queue's own in-flight and backpressure checks; a publish can
  never bypass a limit a direct send would hit.
- Per-subscription failures must not leak other tenants' queue names into
  `PublishResult` once R6 adds multi-tenancy: the failure entry carries the
  subscription ID, which the caller already knows, never the destination queue
  name. This shape is fixed now so R6 does not need to redact it.
- `publish_id` and message IDs come from the log-applied clock (ADR-0005);
  no reducer added in R5 reads wall time, randomness, or IO, preserving the
  determinism the R3 corpus depends on.

### 10.8 Migration, documentation, and installation work

R5 changes no on-disk format: topics, subscriptions, and publishes are new
`Command` variants in the existing WAL record encoding, and WAL format
versioning (NFR-DUR-007) already covers unknown-command rejection by older
binaries — the documented downgrade policy is that a WAL containing topic
commands does not open under a pre-R5 binary, which cannot exist outside
development because nothing has shipped. The JSONL history schema version
increments with additive record types; existing R1–R4 golden histories remain
valid without rewriting.

Documentation work: PRODUCT_REQUIREMENTS.md FR-TOPIC sections gain their
acceptance-evidence links; CORRECTNESS.md's P-04 mapping is extended with the
mixed direct/fanout FIFO histories; docs/README.md status tables move R5 rows
from planned to accepted only when section 10.9 completes. No installation work
exists — there is still no binary surface to install.

### 10.9 Acceptance evidence

R5 is accepted only when:

- every FR-TOPIC-001 through FR-TOPIC-008 behavior has its named failing-first
  test, and the full matrix of section 10.6 is green in CI;
- the model checker explores topic operations interleaved with queue operations
  and reports no invariant breach and no non-linearizable history;
- mixed direct-send and publish FIFO histories satisfy P-04 under MODL-FIFO;
- the mutation floor holds on all R5-owned modules;
- all R1–R4 accepted evidence replays green after the R5 merge (the
  NFR-MAINT-004 discipline, audited every gate);
- the simulation corpus contains at least the unsubscribe-race and
  delete-race seeds, replaying deterministically.

### 10.10 Explicit deferrals

R5 defers: the wire representation of every topic operation, subscribe-time
authentication and per-topic ACLs, and publish quotas (all R6); topic metrics,
audit logging of subscription changes, and `relayctl` topic commands (R8);
replicated fanout behavior under partition (R7); any push-style delivery to
non-queue endpoints, message transformation on fanout, and filter policies over
message bodies rather than attributes (not planned for 1.0; OPEN_QUESTIONS.md
records body-filtering with a fail-closed default of attribute-only).

### 10.11 Requirements traced

R5 is the terminal owning gate for `FR-TOPIC-001` through `FR-TOPIC-008`. It
advances, without completing, `FR-ADMIN-005` (DeleteQueue's
subscription-removal reducer semantics; the wire-visible terminal behavior
completes in R6), `NFR-MAINT-002` and `NFR-MAINT-003` (which remain under
continuous audit), and the P-04 mapping in CORRECTNESS.md. The section 16
matrix lists each FR-TOPIC ID exactly once with R5 as terminal owner.

## 11. R6 — Bounded, Fuzzed Wire API With Authentication, Quotas, and Long Polling

**Status:** planned.

**Effort range:** 12–18 focused days.

### 11.1 Why R6 exists

Through R5, Relay is a verified library: a deterministic state machine, a
durable log, a simulator, and a model checker, with no way for a real client to
send a byte. R6 turns it into a server without surrendering the properties the
earlier gates proved. The threat is specific: the wire is where untrusted input
enters, where unbounded allocation, forged receipts, timing side channels,
slowloris connections, and head-of-line blocking live. ADR-0004 chose a custom
framed binary protocol precisely so that every parser in the input path is
small enough to fuzz exhaustively and every length is checked before every
allocation (FR-API-002). R6 also owns the operational contract of a single
process: configuration precedence and fail-fast validation (FR-OPS-002),
bounded backpressure instead of collapse (NFR-AVAIL-003), and a graceful drain
on shutdown (NFR-AVAIL-004). Nothing in R6 is replicated; `relayd` here is a
correct, hostile-input-hardened single node.

### 11.2 Prerequisites

- R2 is accepted: `SendMessage` acks only after the ADR-0008 durability
  contract, so wiring ack-to-fsync here is composition, not new invention.
- R4 and R5 are accepted: every command the wire exposes has verified reducer
  semantics; R6 maps bytes to commands and never adds semantics of its own.
- ADR-0004 (RWP/1, no SQS compatibility) and ADR-0006 (ULID IDs, HMAC receipt
  handles) are accepted and fix the formats this section makes concrete.
- The threat model names the wire-facing assets and attacks: parser memory,
  credential comparison timing, receipt forgery, quota bypass, connection
  exhaustion, and secret leakage into logs (NFR-SEC-003 canaries defined).
- cargo-fuzz runs locally and in CI on tier-1 Linux targets (ADR-0011).

### 11.3 Owned files, interfaces, and state

R6 creates four crates:

- `crates/relay-wire`: frame codec, per-opcode body codecs, error taxonomy,
  and the fuzz targets under `crates/relay-wire/fuzz/fuzz_targets/{frame_parser,
  body_parsers,filter_policy}.rs` with corpora under
  `crates/relay-wire/fuzz/corpus/<target>/` (checked in; CI replays them);
- `crates/relay-server`: the `relayd` binary — listener, connection actor,
  session auth, ACL and quota enforcement, long-poll scheduler, dispatch into
  `relay-core` + `relay-wal`, config loading, drain;
- `crates/relay-client`: typed client used by every integration test and by
  `relayctl`; owns redirect-following and retry policy surfaces (retry bodies
  land fully in R7's session work);
- `crates/relay-cli`: `relayctl` skeleton — argv parsing, config/endpoint
  resolution, and the `queue`/`topic` subcommand tree wired to
  `relay-client`; complete admin coverage is R8 (FR-ADMIN-006).

Core wire interfaces:

```rust
// relay-wire
pub struct FrameHeader {
    pub len: u32,        // total frame length, ≤ 1 MiB (FR-API-010)
    pub crc32c: u32,     // over opcode..body
    pub opcode: u16,
    pub flags: u16,      // bit 0 RESPONSE, bit 1 ERROR, bits 2..16 must be zero
    pub request_id: u64, // client-assigned, echoed on response (FR-API-007)
}

pub enum DecodeError {
    BadMagic, FrameTooShort, FrameTooLong { len: u32 },
    CrcMismatch { expected: u32, actual: u32 },
    UnknownOpcode(u16), ReservedFlagSet(u16),
    FieldOverrun { opcode: u16, field: &'static str },
    FieldLimit { opcode: u16, field: &'static str, max: u32, got: u32 },
    TrailingBytes { opcode: u16, extra: u32 },
}

pub fn decode_header(buf: &[u8; 24]) -> Result<FrameHeader, DecodeError>;
pub fn decode_body(h: &FrameHeader, body: &[u8]) -> Result<Request, DecodeError>;
pub fn encode_response(r: &Response, request_id: u64, out: &mut BytesMut);

// relay-server
pub struct ConnState {
    pub session: Option<AuthedSession>, // None until HelloAck
    pub inflight: BoundedMap<u64, InflightReq>, // ≤ max_inflight_per_conn
    pub recv_deadline: Nanos,
    pub mem_budget: MemBudget,          // per-connection cap (NFR-SEC-006)
}

pub struct AuthedSession {
    pub tenant: TenantId,
    pub session_id: u64,
    pub session_key: SessionKey,        // HMAC(tenant_key, cn ‖ sn), zeroized on drop
    pub negotiated_version: u16,
    pub quota: TokenBucketSet,
}
```

Configuration schema (`/etc/relay/relay.toml`, spine §0 defaults):

```toml
[node]
data_dir = "/var/lib/relay"     # env RELAY_NODE_DATA_DIR, flag --data-dir
api_addr = "0.0.0.0:7414"
metrics_addr = "127.0.0.1:7415"
raft_addr = "0.0.0.0:7416"      # validated but unused until R7

[tls]
mode = "required"               # "required" | "loopback-plaintext"
cert_path = "/etc/relay/tls/server.crt"
key_path = "/etc/relay/tls/server.key"

[limits]
max_frame_bytes = 1048576
max_inflight_per_conn = 128
max_connections = 4096
conn_mem_budget_bytes = 4194304
header_read_deadline_ms = 5000
idle_timeout_s = 300
drain_deadline_s = 30

[quota.default]
send_per_s = 2000
receive_per_s = 2000
admin_per_s = 50
burst_multiplier = 4
```

Precedence is fixed: command-line flags override environment variables
(`RELAY_<SECTION>_<KEY>`, uppercase, underscores) override the TOML file
override built-in defaults (FR-OPS-002). Startup validation parses everything,
collects every error (unknown key, out-of-range value, unreadable key file,
non-loopback plaintext), prints the complete list with source attribution
("--data-dir from flag", "tls.mode from /etc/relay/relay.toml line 12"), and
exits nonzero before binding any socket.

### 11.4 Algorithms and state behavior

**RWP/1 frame layout (spine §6, normative):**

```text
[magic "RWP1" 4B][len u32 LE (max 1 MiB)][crc32c u32][opcode u16][flags u16][request_id u64][body]
```

`len` counts the entire frame including magic. Header is 24 bytes. `crc32c`
covers `opcode` through the last body byte. Responses set flags bit 0 and reuse
the request opcode; the Error frame is its own opcode and sets bits 0 and 1.

**Opcode number table (normative):**

| Opcode | Name | Opcode | Name |
| --- | --- | --- | --- |
| 0x0001 | Hello | 0x0021 | DeleteQueue |
| 0x0002 | HelloAck | 0x0022 | SetQueueAttributes |
| 0x0010 | SendMessage | 0x0023 | Describe |
| 0x0011 | SendBatch | 0x0024 | List |
| 0x0012 | Receive | 0x0025 | Tag |
| 0x0013 | Delete | 0x0026 | Untag |
| 0x0014 | ChangeVisibility | 0x0030 | CreateTopic |
| 0x0015 | Purge | 0x0031 | DeleteTopic |
| 0x0020 | CreateQueue | 0x0032 | Subscribe |
| 0x0040 | StartRedrive | 0x0033 | Unsubscribe |
| 0xFFFF | Error | 0x0034 | Publish |

Unlisted opcodes are `UnknownOpcode` and close nothing; the connection answers
with an Error frame and stays usable (a scanner cannot cheaply kill sessions).

**Common field encodings.** All integers little-endian. `str16` = u16 length +
UTF-8 bytes; `bytes32` = u32 length + raw bytes; `id16` = 16 raw bytes
(ULID/queue/topic/subscription ID); `mac32` = 32-byte HMAC-SHA256 tag; `attrs`
= u8 count (≤ 10), then per attribute: key `str16` (≤ 256 B), type u8
(0 = String, 1 = Number, 2 = Binary), value `bytes32` (message body plus all
attribute values ≤ 256 KiB combined). Every authenticated request body begins
with `mac32` computed as
`HMAC-SHA256(session_key, session_id u64 ‖ request_id u64 ‖ opcode u16 ‖ body-after-mac)`.

**Per-opcode body layouts (request unless marked resp):**

| Opcode | Field | Type | Constraint |
| --- | --- | --- | --- |
| Hello | min_version | u16 | supported window; RWP/1 ⇒ 1 |
| Hello | max_version | u16 | ≥ min_version |
| Hello | tenant_id | str16 | ≤ 64 B |
| Hello | client_nonce | 16 B | random, from client |
| HelloAck (resp) | chosen_version | u16 | in client window (FR-API-009) |
| HelloAck (resp) | server_nonce | 16 B | random |
| HelloAck (resp) | session_id | u64 | unique per connection lifetime |
| HelloAck (resp) | ack_mac | mac32 | proves server knows tenant key |
| SendMessage | mac | mac32 | per-request MAC |
| SendMessage | queue | str16 | name rules, spine §6 |
| SendMessage | delay_s | u32 | 0–900; 0xFFFFFFFF = queue default |
| SendMessage | group_id | str16 | ≤ 128 B; empty = absent |
| SendMessage | dedup_id | str16 | ≤ 128 B; empty = absent |
| SendMessage | attrs | attrs | ≤ 10 |
| SendMessage | body | bytes32 | ≤ 256 KiB |
| SendMessage (resp) | message_id | id16 | ULID |
| SendMessage (resp) | acked_lsn | u64 | durable LSN backing the ack |
| SendBatch | mac | mac32 | |
| SendBatch | queue | str16 | one queue per batch |
| SendBatch | count | u8 | 1–10 (FR-QUEUE-003) |
| SendBatch | entries[count] | per-entry | delay_s, group_id, dedup_id, attrs, body as above |
| SendBatch (resp) | results[count] | u8 status + (id16 \| u16 error) | independent per entry |
| Receive | mac | mac32 | |
| Receive | queue | str16 | |
| Receive | max_messages | u8 | 1–10 (FR-QUEUE-004) |
| Receive | visibility_s | u32 | 0–43,200; 0xFFFFFFFF = queue default |
| Receive | wait_time_s | u8 | 0–20 (FR-QUEUE-009) |
| Receive (resp) | count | u8 | 0–10 |
| Receive (resp) | messages[count] | id16 + receipt str16 + receive_count u32 + attrs + body bytes32 | receipt = `rh1_…` per ADR-0006 |
| Delete | mac | mac32 | |
| Delete | queue | str16 | |
| Delete | receipt | str16 | ≤ 256 B before decode |
| ChangeVisibility | mac | mac32 | |
| ChangeVisibility | queue | str16 | |
| ChangeVisibility | receipt | str16 | ≤ 256 B |
| ChangeVisibility | visibility_s | u32 | 0 returns message now (FR-QUEUE-008) |
| Purge | mac | mac32 | |
| Purge | queue | str16 | concurrent purge → `PurgeInProgress` |
| CreateQueue | mac | mac32 | |
| CreateQueue | name | str16 | `.fifo` suffix ⇒ FIFO |
| CreateQueue | attr_count | u8 | ≤ 16 |
| CreateQueue | attrs[n] | key str16 + value str16 | closed key set; unknown key rejected |
| DeleteQueue | mac | mac32 | |
| DeleteQueue | name | str16 | terminal (FR-ADMIN-005) |
| SetQueueAttributes | mac | mac32 | |
| SetQueueAttributes | name | str16 | |
| SetQueueAttributes | attrs[n] | u8 count + key/value str16 pairs | validated before apply (FR-ADMIN-004) |
| Describe | mac | mac32 | |
| Describe | kind | u8 | 0 = queue, 1 = topic |
| Describe | name | str16 | |
| Describe (resp) | config + counts | per-kind layout | counts carry staleness_ms u32 label (FR-ADMIN-001) |
| List | mac | mac32 | |
| List | kind | u8 | 0 = queues, 1 = topics |
| List | prefix | str16 | may be empty |
| List | cursor | str16 | opaque; empty = start (FR-ADMIN-002) |
| List | page_size | u16 | 1–1000 |
| List (resp) | count u16 + names[count] str16 + next_cursor str16 | | empty cursor = end |
| Tag / Untag | mac | mac32 | |
| Tag / Untag | resource | u8 kind + str16 name | |
| Tag | pairs[n] | u8 count + key/value str16 | ≤ 50 tags per resource (FR-ADMIN-003) |
| Untag | keys[n] | u8 count + key str16 | |
| CreateTopic | mac | mac32 | |
| CreateTopic | name | str16 | topic name rules |
| DeleteTopic | mac | mac32 | |
| DeleteTopic | name | str16 | |
| Subscribe | mac | mac32 | |
| Subscribe | topic | str16 | |
| Subscribe | queue | str16 | |
| Subscribe | filter_policy | bytes32 | ≤ 8 KiB; empty = no filter; parsed by §10.4 parser |
| Subscribe (resp) | subscription_id | id16 | |
| Unsubscribe | mac | mac32 | |
| Unsubscribe | subscription_id | id16 | |
| Publish | mac | mac32 | |
| Publish | topic | str16 | |
| Publish | group_id | str16 | for FIFO destinations (FR-TOPIC-008) |
| Publish | dedup_id | str16 | |
| Publish | attrs | attrs | filter evaluation input |
| Publish | body | bytes32 | ≤ 256 KiB |
| Publish (resp) | publish_id id16 + matched u32 + delivered u32 + fail_count u16 + failures[n] (id16 + u16 code) | | per §10.4 |
| StartRedrive | mac | mac32 | |
| StartRedrive | dlq | str16 | |
| StartRedrive | max_rate_per_s | u32 | 0 = unthrottled |
| StartRedrive (resp) | task_id | id16 | progress via Describe (FR-QUEUE-019) |
| Error (resp) | code | u16 | error taxonomy (FR-API-006) |
| Error (resp) | retryable | u8 | 0/1 |
| Error (resp) | leader_hint | str16 | empty until R7 populates it |
| Error (resp) | detail | str16 | ≤ 512 B, no secrets, no internal paths |

**Parser algorithm (numbered, bound-before-allocate):**

1. Read exactly 24 header bytes under the header read deadline. Fewer bytes by
   the deadline is a slowloris verdict: close the connection, count the metric.
2. Compare magic to `"RWP1"` with a plain (non-secret) comparison; mismatch
   closes the connection immediately — a non-RWP peer gets no parser surface.
3. Read `len`. Reject `len < 24` (`FrameTooShort`) and `len > max_frame_bytes`
   (`FrameTooLong`) before allocating anything. Only then reserve `len − 24`
   bytes from the connection's `MemBudget`; budget exhaustion is `Overloaded`,
   not an allocation attempt.
4. Read the body bytes under the read deadline. Verify `crc32c` over
   `opcode..body`; mismatch is `CrcMismatch` and closes the connection (framing
   is now untrustworthy; resynchronization is not attempted).
5. Reject reserved flag bits and unknown opcodes with Error frames (connection
   survives). Reject any request other than `Hello` on an unauthenticated
   connection with `AuthRequired`.
6. Dispatch to the per-opcode body parser. Each parser walks a cursor: for a
   fixed field, check remaining ≥ size; for a length-prefixed field, read the
   prefix, check it against both the remaining byte count and the field's
   documented limit (`FieldLimit` carries opcode, field name, max, got) before
   slicing or allocating. Counts (`attrs`, batch entries, list pages) are
   checked against their maxima before loop entry.
7. After the last field, require the cursor to sit exactly at the body end;
   `TrailingBytes` otherwise. No parser ever reads beyond `len`.
8. The decoded request is handed to auth (below). Body parsers construct only
   borrowed views plus one bounded copy into command types; peak parse memory
   per request is ≤ 2× frame size and is charged to `MemBudget`.

**Authentication handshake and per-request MAC.**

1. `Hello` carries the tenant ID and a 16-byte client nonce. The server looks
   up the tenant's static key from the credential store (file-based in R6,
   0600, loaded at startup, never logged; NFR-SEC-003).
2. The server generates a 16-byte nonce, computes
   `session_key = HMAC-SHA256(tenant_key, client_nonce ‖ server_nonce)`,
   assigns `session_id`, and answers `HelloAck` whose `ack_mac` is
   `HMAC-SHA256(session_key, "relay-ack" ‖ session_id)` — the client verifies
   it before sending anything, so a fake server learns nothing usable.
3. Every subsequent request's `mac32` is recomputed server-side and compared
   with a constant-time equality (`subtle::ConstantTimeEq`; NFR-SEC-004).
   Failure is `AuthFailed` with no distinction between unknown tenant, wrong
   key, and bit-flipped MAC, and the connection closes after one failure.
4. An unknown tenant in `Hello` is answered after computing an HMAC over a
   server-local dummy key so the timing profile matches the known-tenant path.
5. Version negotiation happens in the same exchange: the server picks the
   highest mutually supported version or answers `VersionUnsupported` before
   any state change (FR-API-009).

**Receipt handles (ADR-0006, terminal here for NFR-SEC-001).** Receive mints
`rh1_` + base64url(version u8 ‖ queue_id 16 B ‖ message_id 16 B ‖ lease_epoch
u64 ‖ expiry_nanos u64 ‖ HMAC-SHA256 tag 32 B) keyed by the per-cluster receipt
key. Delete/ChangeVisibility verify the tag constant-time, then hand the
decoded epoch to the core reducer, which enforces single-use epoch equality
(FR-QUEUE-007). A forged, truncated, foreign-queue, or stale-epoch handle fails
verification before any reducer runs.

**ACL evaluation (FR-API-004).** Tenant grants are `(resource_pattern, ops,
effect)` where a pattern is an exact name or a single trailing-`*` prefix.
Evaluation collects all matching grants; any matching deny wins over any allow
(deny > allow, no exceptions); no matching grant is a deny (default deny). The
decision is made after authentication and before quota spend, so a denied
caller cannot consume quota or learn resource existence: `AccessDenied` is
byte-identical whether the queue exists or not.

**Quotas (FR-API-005).** Each session holds token buckets per operation class
(send, receive, admin) with configured rate and burst. A request that cannot
take a token gets `Throttled` with `retryable = 1` and a `retry_after_ms`
detail; the request performs no reducer work. Buckets refill from the server
monotonic clock, never the state-machine clock — throttling is an edge
concern, not replicated state.

**Long polling without head-of-line blocking (FR-QUEUE-009, FR-API-007).**

1. A connection is a full-duplex pipeline: the read loop decodes frames and
   admits up to `max_inflight_per_conn` requests; the write loop sends
   responses in completion order, not arrival order, correlated by
   `request_id`. A parked long poll is just an in-flight entry.
2. `Receive` with `wait_time_s > 0` that finds no message registers a waiter
   `(conn, request_id, max_messages, visibility, deadline)` on the queue's
   waiter list and returns nothing yet. Other requests on the same connection
   continue to be read, executed, and answered — the evidence test drives a
   send and a describe past a parked 20 s poll and requires both to complete
   in milliseconds.
3. When the reducer makes a message available (send, visibility expiry, delay
   maturity, redrive), it wakes waiters FIFO by park time; each woken waiter
   re-runs a normal Receive through the reducer, so lease grants stay inside
   verified core semantics. A woken waiter that loses the race (another
   consumer took the message) re-parks with its remaining deadline.
4. At the deadline the waiter completes with an empty (successful) response.
   Waiter registration counts against the connection's in-flight budget; a
   connection cannot park unbounded polls.

**Backpressure and shed (NFR-AVAIL-003).** Three nested budgets: per-connection
in-flight, per-connection memory, and a global in-flight request budget. When
the global budget is exhausted, new requests are shed with `Overloaded`
(`retryable = 1`) after header parse and before body allocation; when
`max_connections` is reached, accepts are refused at the listener. Shedding is
load-proportional and never queues unboundedly; the evidence test drives 4×
capacity and requires bounded memory, bounded p99 for admitted requests, and
zero process death.

**Connection lifecycle and graceful shutdown (NFR-AVAIL-004).** TLS 1.3 via
rustls; plaintext only when `tls.mode = "loopback-plaintext"` and the bind
address is loopback, validated at startup (FR-API-008). Idle connections close
at `idle_timeout_s`. On SIGTERM: stop accepting; complete in-flight requests;
complete parked long polls immediately with empty success; flush and fsync the
WAL; close connections; exit 0 within `drain_deadline_s`, else force-close
remaining connections and exit with a distinct code. No ack is ever sent for
work whose fsync did not complete — drain preserves ADR-0008 to the last frame.

### 11.5 Implementation tickets and sequence

1. **R6.01 — Frame codec.** Implement header encode/decode, CRC coverage, and
   the limits of the parser algorithm steps 1–5. Done when every `DecodeError`
   variant has a failing-first test constructing the exact malformed bytes, and
   a round-trip property test holds for arbitrary valid frames.
2. **R6.02 — Body codecs.** Implement every per-opcode layout in section 11.4
   with cursor-based bound checks and `TrailingBytes` enforcement. Done when a
   generated encode→decode round trip covers every opcode and every field
   limit has an at-limit and over-limit test naming the field.
3. **R6.03 — Error taxonomy.** Define the closed u16 error-code table (one code
   per failure class, `retryable` flag, detail rules), map every reducer error
   and every `DecodeError` onto it, and freeze it in docs. Done when an
   exhaustiveness test fails to compile if a reducer error lacks a mapping
   (FR-API-006).
4. **R6.04 — Fuzz targets and CI gate.** Add the three cargo-fuzz targets
   (frame parser, body parsers via arbitrary opcode + bytes, filter-policy
   parser), seed corpora from the codec tests, and gate CI: replay the full
   checked-in corpus every run, plus a bounded new-exploration budget nightly;
   any crash becomes a checked-in regression input before the fix merges. Done
   when CI demonstrably fails on a seeded planted panic (NFR-SEC-002).
5. **R6.05 — Listener and connection actor.** Implement TLS, the full-duplex
   read/write loops, per-connection budgets, header deadline, idle timeout.
   Done when the slowloris test (byte-per-second header) is closed at the
   deadline and a memory-cap test bounds a hostile connection's allocation.
6. **R6.06 — Auth handshake and per-request MAC.** Implement Hello/HelloAck,
   session-key derivation, constant-time verification, uniform-timing unknown
   tenant, and version negotiation. Done when a statistical timing test over
   valid/invalid MACs and known/unknown tenants shows no measurable separation
   at the test's power, and one MAC failure closes the connection.
7. **R6.07 — Receipt-handle mint and verify.** Implement ADR-0006 construction,
   per-cluster key with epoch field for rotation, constant-time tag check.
   Done when forged/truncated/foreign/stale-epoch handles all fail before the
   reducer and the R1 single-use tests pass end-to-end over the wire.
8. **R6.08 — ACL store and evaluation.** Implement grant loading, deny > allow,
   default deny, and existence-hiding `AccessDenied`. Done when the matrix of
   (grant sets × ops × resources) passes and denied requests spend no quota.
9. **R6.09 — Token-bucket quotas.** Implement per-session buckets, `Throttled`
   with retry-after, config-driven rates. Done when a rate test measures the
   configured rate ± burst and throttled requests provably skip the reducer.
10. **R6.10 — Dispatch and durability wiring.** Map every decoded request to a
    core command, wire Send acks to `Wal::sync` completion (ADR-0008), and
    return `acked_lsn`. Done when a crash test between append and sync shows
    no ack was emitted for the lost record (composition check against R2).
11. **R6.11 — Long-poll scheduler.** Implement waiter lists, FIFO wake,
    re-park on race loss, deadline completion, in-flight accounting. Done when
    the head-of-line test (parked 20 s poll; concurrent send and describe on
    the same connection answered < 50 ms) and the wake-on-send test pass.
12. **R6.12 — Backpressure, shed, and drain.** Implement the three budgets,
    `Overloaded` shed, SIGTERM drain per section 11.4. Done when the 4×
    overload test and the drain test (in-flight send acked, parked poll
    completed empty, exit 0 inside deadline) pass.
13. **R6.13 — Config loader.** Implement TOML + env + flag precedence, source
    attribution, collect-all-errors fail-fast startup validation, and the
    loopback-plaintext check. Done when a table of conflicting sources
    resolves per the fixed precedence and a config with three errors reports
    all three with sources and exits before binding (FR-OPS-002).
14. **R6.14 — Client, CLI skeleton, and secret canaries.** Implement
    `relay-client` typed calls for every opcode, the `relayctl` skeleton, and
    the NFR-SEC-003 canary suite: plant canary secrets in tenant keys, TLS
    keys, and receipt keys, then grep every log line, error detail, trace, and
    `relayctl` output produced by the full integration suite for them. Done
    when the canary sweep is a CI job that fails on any hit and the client
    exercises every opcode against a live `relayd` in the integration tests.

### 11.6 Test-driven evidence matrix

| Test | First failing condition | Required passing assertion |
| --- | --- | --- |
| WIRE-FRAME-BOUNDS | A frame with len > 1 MiB or a field over its limit causes an allocation before rejection. | Allocation-tracking harness shows zero body allocation for oversized frames; every `FieldLimit` fires before slicing (FR-API-002, FR-API-010). |
| WIRE-FRAME-CRC | A bit-flipped body is dispatched. | CRC mismatch closes the connection with no reducer call and no partial decode escaping the codec. |
| FUZZ-CORPUS-GATE | CI passes with a corpus input that panics, over-allocates, or hangs a parser. | Full corpus replay is a required CI job; a planted panic input turns CI red; fuzz targets run under an allocation and time limit (NFR-SEC-002). |
| WIRE-AUTH-TIMING | MAC or tenant lookup timing distinguishes valid from invalid. | Constant-time comparison verified by code audit plus statistical timing test with no separation at test power (NFR-SEC-004). |
| WIRE-RECEIPT-FORGE | A forged, truncated, foreign, or stale-epoch receipt reaches the reducer. | All forged classes rejected at verification; valid handle is single-use per delivery epoch end-to-end (NFR-SEC-001, FR-QUEUE-007). |
| WIRE-ACL-MATRIX | An allow overrides a deny, or a denied caller learns queue existence or spends quota. | Deny > allow across the full grant matrix; `AccessDenied` byte-identical for existing and missing resources; quota untouched (FR-API-004). |
| WIRE-QUOTA-RATE | Sustained throughput exceeds configured rate + burst, or throttled requests reach the reducer. | Measured admission matches token-bucket math; `Throttled` is stable and retryable (FR-API-005). |
| WIRE-LONGPOLL-HOL | A parked long poll delays any other request on the same connection. | With a 20 s poll parked, send and describe on the same connection complete < 50 ms; responses correlate by request_id out of order (FR-API-007, FR-QUEUE-009). |
| WIRE-LONGPOLL-WAKE | A send does not wake a matching parked poll promptly, or a woken loser fails instead of re-parking. | Wake-to-response on send is bounded in the integration harness; race-losing waiters re-park and later succeed or return empty at deadline. |
| WIRE-SLOWLORIS | A byte-per-second header holds a connection past the deadline or unbounded connections exhaust memory. | Header deadline closes the connection; per-connection memory cap and max_connections hold under the connection-flood test (NFR-SEC-006). |
| WIRE-OVERLOAD-SHED | 4× offered load collapses the process or queues unboundedly. | Shed with `Overloaded` after header parse; bounded memory; admitted-request p99 bounded; zero crashes (NFR-AVAIL-003). |
| WIRE-DRAIN | SIGTERM loses an acked write, hangs a parked poll, or exceeds the drain deadline. | In-flight sends complete with fsync-backed acks, parked polls return empty success, process exits 0 within drain_deadline_s (NFR-AVAIL-004). |
| WIRE-CONFIG-PRECEDENCE | Flag/env/file precedence resolves wrong, or startup reports only the first config error. | Fixed precedence holds across the conflict table; all planted errors reported with source attribution before any socket binds (FR-OPS-002). |
| WIRE-VERSION-NEG | An unsupported version changes state or gets a non-stable error. | `VersionUnsupported` before any state change; mutually supported window negotiates the highest version (FR-API-009). |
| WIRE-SECRET-CANARY | A planted key canary appears in any log, trace, error detail, or CLI output. | Canary sweep over the full integration run finds zero hits (NFR-SEC-003). |
| WIRE-ADMIN-SURFACE | Describe/List/Tag/SetQueueAttributes/DeleteQueue deviates from FR-ADMIN-001..005. | Counts labeled with staleness; cursor pagination is stable under interleaved creates; DeleteQueue invalidates handles, unsubscribes, and frees storage. |

### 11.7 Failure and security cases

- Every byte before MAC verification is treated as attacker-controlled: parsers
  are panic-free, allocation-bounded, recursion-free, and fuzzed; the only
  pre-auth opcode is `Hello`.
- One MAC failure closes the connection; there is no retry-inside-session for
  authentication, which removes the online-oracle surface for key guessing.
- Tenant keys, session keys, and the receipt key are held in zeroizing wrappers,
  never formatted via `Debug`/`Display`, and excluded from `relayctl diagnose`
  scope by construction (the R8 bundle redacts by allowlist, not blocklist).
- Error `detail` strings are built from a closed set of templates over
  validated fields; reducer internals, file paths, and peer addresses of other
  tenants never appear (FR-API-006 keeps codes stable, this keeps details safe).
- The plaintext-loopback escape hatch refuses non-loopback binds at startup;
  there is no runtime downgrade from TLS to plaintext.
- Quota accounting happens after ACL denial checks but a `Throttled` response
  must not become an existence oracle either: throttling is per-tenant and
  op-class, never per-resource.
- If the write side of a connection stalls (client stops reading), the write
  buffer hits its budget and the connection closes; a slow reader cannot pin
  response memory (the write-side slowloris).
- A crash between WAL append and fsync completion must leave the client
  without an ack; the R2 crash harness re-runs over the wire path in R6.10's
  composition test — durability claims are never re-proven by weaker tests.

### 11.8 Migration, documentation, and installation work

R6 introduces on-disk credential and ACL files (`/etc/relay/tenants.d/`, 0600,
validated at startup alongside NFR-SEC-005's 0700 data-dir check from R2) and
the configuration schema, all version-tagged from first release; there is no
prior installed base to migrate. Documentation deliverables: the RWP/1
normative wire reference (frame, opcode table, every body layout, error-code
table) as `docs/wire/RWP1.md` generated from the same tables the codec tests
consume, so document and code cannot drift; configuration reference with
precedence rules and every key's range; a "running relayd" page covering TLS
setup, tenant provisioning, and drain behavior. `relayctl` ships as a skeleton
with `queue`/`topic` verbs documented as such — the complete administrative
surface remains an R8 claim and is labeled planned. Packaging, service units,
and upgrade tooling remain R10 work.

### 11.9 Acceptance evidence

R6 is accepted only when:

- the full section 11.6 matrix is green in CI, including the fuzz corpus gate
  as a required job with the corpora checked in;
- a live `relayd` on tier-1 Linux serves the complete integration suite —
  every opcode, via `relay-client`, over TLS — with the R1/R4/R5 semantic
  suites re-run over the wire and passing;
- the overload, slowloris, drain, and timing evidence runs are attached to the
  gate pull request with their harness commands;
- the secret-canary sweep passes over the entire integration run's output;
- 24 hours of SOAK traffic against a single node shows no memory growth trend,
  no descriptor leak, and no quota drift;
- all prior accepted gate evidence replays green (NFR-MAINT-004).

### 11.10 Explicit deferrals

R6 defers: replication, leader hints carrying real leader identities, and
client redirect-following behavior under failover (R7 — the `leader_hint`
field exists but is always empty on a single node); metrics, tracing, health
endpoints, audit logging, `relayctl` completeness, and the diagnose bundle
(R8); published performance numbers — R6's overload tests bound behavior but
publish nothing (R9); packaging, key-rotation tooling beyond the epoch field,
mixed-version protocol windows, and the HTTP/JSON gateway question recorded in
OPEN_QUESTIONS.md with a fail-closed default of RWP/1-only (R10 and beyond).

### 11.11 Requirements traced

R6 is the terminal owning gate for `FR-QUEUE-009`, `FR-API-001` through
`FR-API-010`, `FR-ADMIN-001` through `FR-ADMIN-005`, `FR-OPS-002`,
`NFR-AVAIL-003`, `NFR-AVAIL-004`, and `NFR-SEC-001`, `NFR-SEC-002`,
`NFR-SEC-003`, `NFR-SEC-004`, `NFR-SEC-006`. It advances `FR-ADMIN-006` and
`FR-OPS-010` (R8), `FR-REPL-007`'s client side (R7), `NFR-PERF-001..003`
measurement surfaces (R9), and `NFR-SEC-007`'s ongoing threat-model review.
The section 16 matrix lists each terminal ID exactly once against R6.

## 12. R7 — Raft Replication Survives Partition and Failover With No Double-Lease and No Lost Ack

**Status:** planned.

**Effort range:** 20–30 focused days. This is the longest gate in the plan;
ADR-0003 accepted that cost when it rejected external Raft libraries.

### 12.1 Why R7 exists

Everything before R7 is provably correct on one node. R7 makes Relay a 3-node
cluster whose two headline properties — P-08 NO-SPLIT-LEASE and P-09
NO-LOST-ACK — hold across partitions, failovers, crashes, and clock jumps, and
are demonstrated by the machinery this project exists to showcase: every Raft
behavior is driven deterministically in `relay-sim` first, every failure
reproduces from a seed, and every failover history is checked for
linearizability against the reference model. The spine rule is absolute here:
determinism over live infrastructure. A live 3-node smoke test exists in this
gate, but it proves only that processes start, connect, and elect — it is
never admitted as correctness evidence, because a test that cannot reproduce
its own failure cannot prove absence of one.

The design leverage is that R1–R5 made `relay-core` a pure deterministic state
machine applied from a log. Raft's job is exactly to replicate a log; the state
machine does not change. What R7 must get right is the log itself: election
safety, durable-majority commit, snapshot transfer, membership change, and the
routing of client acks — and the insight that lease operations are log entries,
which is what turns Raft's single-log linearization into P-08.

### 12.2 Prerequisites

- R2 is accepted: the WAL provides the durable append and fsync primitives
  Raft's persistence rules require, plus torn-tail recovery.
- R3 is accepted: `relay-sim` provides SimNet (partitions, asymmetry, delay,
  reorder, duplication, loss), SimClock (jumps, skew), SimDisk (crash points,
  torn writes), and the seed-corpus discipline (NFR-MAINT-002).
- R6 is accepted: the wire carries requests, the `Error` frame has its
  `leader_hint` field, and `relay-client` exists to grow redirect handling.
- ADR-0003 (in-house Raft) and ADR-0005 (log-applied time) are accepted; the
  Raft parameters are spine-fixed: pre-vote on, heartbeat 100 ms, election
  timeout 500–1000 ms randomized in simulated time, ReadIndex reads,
  single-server membership change, 1 MiB snapshot chunks.

### 12.3 Owned files, interfaces, and state

R7 creates `crates/relay-raft` as a sans-IO deterministic core — pure state
plus returned effects, so the identical code runs under the tokio driver in
`relay-server` and the virtual-time driver in `relay-sim`:

- `src/node.rs`: `RaftNode`, roles, `tick`/`step`/`on_durable`/`propose`;
- `src/election.rs`: pre-vote and vote handling, randomized timeouts;
- `src/replicate.rs`: AppendEntries emission, conflict backup, commit rule;
- `src/log.rs`: in-memory log window over WAL-backed entries;
- `src/persist.rs`: hard-state and entry persistence contracts against
  `relay-wal` record types `RaftHardState` and `RaftEntry`;
- `src/snapshot.rs`: chunked install with resume, per spine §6 `RSNAP1`;
- `src/membership.rs`: single-server configuration change;
- `src/readindex.rs`: linearizable read protocol;
- `src/session.rs`: client sessions and retried-command deduplication;
- `crates/relay-server/src/raft_driver.rs` and
  `crates/relay-sim/src/raft_harness.rs`: the two effect executors.

```rust
pub type Term = u64;
pub type LogIndex = u64;
pub struct NodeId(pub u64);

pub struct HardState {
    pub current_term: Term,
    pub voted_for: Option<NodeId>,
    // commit_index is volatile per the Raft paper; persisted opportunistically
    // as a recovery hint, never trusted over the recomputed value.
}

pub enum Role { Follower, PreCandidate, Candidate, Leader }

pub enum RaftMsg {
    PreVote { term: Term, last_log: (Term, LogIndex) },
    PreVoteResp { term: Term, granted: bool },
    RequestVote { term: Term, last_log: (Term, LogIndex) },
    VoteResp { term: Term, granted: bool },
    AppendEntries { term: Term, prev: (Term, LogIndex),
                    entries: Vec<RaftLogEntry>, leader_commit: LogIndex },
    AppendResp { term: Term, success: bool, match_index: LogIndex,
                 conflict: Option<(Term, LogIndex)> }, // fast backup
    InstallSnapshotChunk { term: Term, snap_id: SnapId, offset: u64,
                          data: Bytes, crc32c: u32, last: bool },
    SnapshotChunkAck { term: Term, snap_id: SnapId, next_offset: u64 },
    TimeoutNow { term: Term }, // leadership transfer, used by R8's admin surface
}

pub enum Effect {
    Send { to: NodeId, msg: RaftMsg },
    PersistHardState(HardState),          // must be durable before dependent Sends
    AppendEntries(Vec<RaftLogEntry>),     // WAL append; durability via on_durable
    TruncateFrom(LogIndex),               // conflict resolution, tail only
    ApplyCommitted { upto: LogIndex },    // drive relay-core apply()
    ResetElectionTimer { deadline: Nanos },
    BeginSnapshot { upto: LogIndex },
    AbortSnapshotInstall { snap_id: SnapId },
    RespondToClient { session: SessionId, seq: ClientSeq, result: Output },
    ReportLeader { leader: Option<NodeId> }, // feeds R6 leader_hint
}

impl RaftNode {
    pub fn tick(&mut self, now: Nanos) -> Vec<Effect>;
    pub fn step(&mut self, from: NodeId, msg: RaftMsg, now: Nanos) -> Vec<Effect>;
    pub fn on_durable(&mut self, upto: Lsn) -> Vec<Effect>; // WAL fsync completion
    pub fn propose(&mut self, origin: ClientOrigin, cmd: Command)
        -> Result<LogIndex, NotLeader>; // NotLeader carries the leader hint
    pub fn read_index(&mut self, req: ReadTicket) -> Vec<Effect>;
}

pub struct RaftLogEntry {
    pub term: Term,
    pub index: LogIndex,
    pub payload: EntryPayload, // Command | Noop | ConfigChange(Membership)
    pub origin: Option<(SessionId, ClientSeq)>, // client dedup identity
}
```

The effect contract is the safety keystone and is enforced by a harness
assertion, not convention: any `Send` that acknowledges an append or grants a
vote is emitted only from `on_durable` (or after a `PersistHardState` the
driver has confirmed), never directly from `step`. The sim driver fails any
schedule in which a message depending on un-fsynced state is transmitted.

Client sessions live inside `CoreState` (they must survive failover, so they
are replicated state, not server state):

```rust
pub struct SessionId(pub Ulid);
pub struct ClientSeq(pub u64);
pub struct SessionState {
    pub last_applied_seq: ClientSeq,
    pub cached: BTreeMap<ClientSeq, Output>, // bounded window of recent results
    pub expires_at: Nanos,                    // via AdvanceTime entries
}
```

### 12.4 Algorithms and state behavior

**Persistence and WAL integration.** `HardState` is a WAL record fsynced
before any vote or vote-response leaves the node; log entries are WAL records
whose durability flows back through `on_durable(upto_lsn)`, which maps LSNs to
log indices. Recovery order: WAL recovers per R2 (torn tail truncated), then
`relay-raft` rebuilds `HardState` from the last hard-state record, the log
window from entry records, and re-derives everything volatile. A node that
crashed mid-append recovers to exactly its last durable prefix — Raft's
correctness only needs that prefix property, which R2 already proved.

**Pre-vote and election (FR-REPL-001).**

1. A follower whose election timer (randomized uniformly in [500 ms, 1000 ms]
   simulated time, re-randomized every reset) expires becomes PreCandidate: it
   solicits `PreVote` at `current_term + 1` without incrementing its term, so
   a partitioned node cannot inflate terms and depose a healthy leader on heal.
2. A node grants a pre-vote only if it has not heard from a live leader within
   its minimum election timeout and the candidate's `last_log` is at least as
   up-to-date (term, then index) as its own.
3. On pre-vote majority, the node increments its term, votes for itself,
   persists `HardState`, and sends `RequestVote`. Vote granting persists
   `voted_for` before the response is sent (one vote per term, durable).
4. On vote majority the leader appends a `Noop` entry for its new term (this
   is what makes the commit rule below able to commit prior-term entries) and
   begins heartbeats every 100 ms.
5. Any message carrying a higher term steps the node down to Follower and
   persists the new term before further processing.

**AppendEntries and conflict backup.**

1. The leader sends `prev = (term, index)` of the entry preceding the batch.
   A follower whose log lacks a matching entry at `prev` rejects, returning
   `conflict = (term_of_conflicting_entry_or_last, first_index_of_that_term)`
   so the leader backs up a term at a time, not an entry at a time.
2. On match, the follower truncates any conflicting tail (`TruncateFrom` —
   only ever a tail truncate, which the R2 WAL supports as the same operation
   as torn-tail recovery), appends the new entries, and acknowledges only from
   `on_durable`.
3. The follower advances its volatile commit index to
   `min(leader_commit, last_new_index)` and emits `ApplyCommitted`.

**Commit rule (FR-REPL-002).** The leader tracks `match_index` per follower,
advanced only by `AppendResp` messages that follow the follower's fsync. An
index N is committed when a majority (including the leader's own durable
append) has N durable and `log[N].term == current_term`. Prior-term entries
commit only transitively via the new leader's Noop. "Majority durable" is
literal: an ack sent before fsync is a bug the sim harness detects by crashing
the follower immediately after send and asserting the leader's commit index
never counted the lost entry.

**Apply pipeline.** A single applier consumes `ApplyCommitted` effects in
order, calls `relay_core::apply` per entry, and routes `Applied.outputs`: if
the entry's `origin` session is connected to this node, `RespondToClient`
completes the pending wire request; otherwise the output is discarded (the
client will retry through its session, and dedup answers from cache). The
apply index never exceeds the commit index; snapshots record the apply index
they cover.

**ReadIndex (FR-REPL-008).**

1. On a linearizable read, the leader records `read_index = commit_index` and
   its current term.
2. It confirms leadership with a heartbeat round acknowledged by a majority in
   the same term (a leader one heartbeat behind a partition cannot serve a
   stale read).
3. When `apply_index ≥ read_index`, the read executes against `CoreState` and
   returns. Followers forward ReadIndex tickets to the leader; they never
   serve linearizable reads locally in R7.

**Snapshot install with resume (FR-REPL-005).**

1. When a follower's needed entries are compacted away, the leader streams the
   current snapshot (`snap-<lsn:016x>.rsnap`, `RSNAP1`, per-chunk CRC, footer
   with full-state SHA-256) in 1 MiB chunks, each carrying `snap_id`, offset,
   and CRC.
2. The follower writes chunks to a temp file, verifying each CRC, and persists
   an install marker `(snap_id, bytes_received)` so a crash mid-install
   resumes: on recovery it re-derives `next_offset` from the marker and the
   temp file's verified length, and its `SnapshotChunkAck` asks the leader to
   continue from there. A `snap_id` mismatch (leader compacted again) aborts
   the temp state and restarts from offset 0.
3. On the last chunk, the follower verifies the footer SHA-256 over the whole
   file; only then does it atomically rename the snapshot into place, adopt
   its state and apply index, and discard covered log entries. A torn install
   can therefore never be adopted: adoption is gated on the full-state hash.
4. The evidence scenario SIM-RAFT-SNAPTORN crashes the follower at every chunk
   boundary and mid-chunk via SimDisk torn writes, and asserts recovery either
   resumes or restarts, never adopts a partial state, and eventually converges.

**Single-server membership change (FR-REPL-006).** Only one configuration
change may be in flight; a `ConfigChange` entry is rejected while another is
uncommitted. A change adds or removes exactly one server; the new configuration
takes effect at each node when the entry is appended (not committed), per the
Raft dissertation's single-server rule. Safety argument, recorded here because
the sim tests it directly: any majority of the old configuration and any
majority of the new configuration intersect when they differ by one server, so
two leaders cannot be elected in disjoint majorities during the transition —
which is exactly the property SIM-RAFT-MEMBER partitions try and fail to break.
A removed server is fenced by pre-vote: it cannot inflate terms after removal.
New servers join as non-voting learners until caught up within one snapshot +
bounded log tail, then the voting change entry is proposed.

**Leader hints and client redirects (FR-REPL-007).** `propose` on a non-leader
returns `NotLeader { hint: Option<NodeId> }` from the last `ReportLeader`; R6's
`Error` frame carries the hint's advertised address in `leader_hint`.
`relay-client` follows at most 3 redirects with backoff, then falls back to
iterating its seed list; a stale hint (deposed leader) terminates because the
deposed node itself answers `NotLeader` with its newer hint or none.

**The lease-linearization argument (FR-REPL-003, FR-REPL-004; P-08, P-09).**
This is the core of the gate and is stated as the invariant the simulation
attacks:

1. Every lease-affecting operation — grant (Receive), extend
   (ChangeVisibility), consume (Delete), and expiry (via `AdvanceTime`
   entries, ADR-0005) — is a replicated log entry applied by the deterministic
   state machine. There is no node-local lease state and no node-local clock
   in the lease path.
2. Therefore two conflicting leases would require two conflicting `Receive`
   entries both applied at the same message with overlapping validity — which
   the single-node R1 model checker already proved impossible for one log, and
   Raft's Log Matching plus Leader Completeness properties reduce the
   multi-node case to the single-log case: at most one leader per term can
   commit, and commitment requires a majority.
3. A partitioned old leader cannot grant a lease: granting requires committing
   a `Receive` entry, committing requires a majority, and the majority is on
   the other side of the partition electing (via pre-vote) a new leader. The
   old leader can accept the wire request but can never ack it; the client
   times out or follows the new hint. It equally cannot expire leases early,
   because expiry advances only by committed `AdvanceTime` entries.
4. No lost ack is the dual: an ack (send ack or delete ack) is emitted only by
   `RespondToClient`, which is emitted only from apply, which is at or below
   commit, which is majority-durable. A leader killed one instant after the
   client saw the ack has already placed the entry on a durable majority, and
   Leader Completeness guarantees every future leader's log contains it.
5. Retried acks are deduplicated by session: a client that never saw its ack
   retries with the same `(SessionId, ClientSeq)`; apply consults
   `SessionState`, returns the cached `Output` for an already-applied seq, and
   never applies the command twice. This closes the last P-09 hole — a retry
   crossing a failover neither loses the operation nor doubles it.

**Simulation-first verification (spine rule).** Every algorithm above ships
with its `relay-sim` scenarios before the live driver is written; the sim
harness explores seeds over partitions (symmetric, asymmetric, partial),
message loss/duplication/reorder, crash-restart at every persistence boundary,
and SimClock jumps (which may only distort timers and delivery timing — never
lease correctness, by construction of point 1 above). All histories from every
scenario are exported in the ADR-0007 JSONL format and checked by the
Wing–Gong linearizability checker against the reference model. The 3-node live
smoke test (`tests/live/three_node_smoke.rs`) starts real processes on
loopback, elects, sends, fails over once, and asserts liveness; its comment
header states, normatively, that it proves process wiring only and may never
be cited as evidence for any FR-REPL requirement.

### 12.5 Implementation tickets and sequence

1. **R7.01 — Sans-IO node skeleton.** Implement `RaftNode`, roles, `tick`,
   `step`, `on_durable`, and the effect contract with the
   durable-before-dependent-send harness assertion. Done when a two-node
   scripted exchange elects a leader under the sim driver and the harness
   fails a deliberately reordered persist/send schedule.
2. **R7.02 — Persistence in the WAL.** Implement `RaftHardState` and
   `RaftEntry` records, LSN-to-index mapping, recovery, and tail truncation
   reuse. Done when crash-restart at every persistence boundary recovers the
   exact durable prefix under SimDisk fault schedules.
3. **R7.03 — Pre-vote and election.** Implement the five election steps with
   spine-fixed timers. Done when SIM scenarios show: no election with a live
   leader, convergent election after leader crash, and a rejoining partitioned
   node that never deposes a healthy leader (the pre-vote property).
4. **R7.04 — Replication and conflict backup.** Implement AppendEntries both
   sides, term-granular backup, tail truncation. Done when divergent-log
   scenarios (generated by crashing leaders mid-replication across seeds)
   always converge to the Log Matching invariant, checked after every step.
5. **R7.05 — Durable-majority commit.** Implement `match_index` on post-fsync
   acks and the current-term commit rule. Done when the crash-after-send
   scenario proves commit never counts a lost append, and prior-term entries
   commit only via the new leader's Noop.
6. **R7.06 — Apply pipeline and output routing.** Wire `ApplyCommitted` into
   `relay_core::apply`, route outputs to sessions, enforce apply ≤ commit.
   Done when a full queue workload (R1 suite) runs replicated under sim with
   identical semantics to single-node, verified by the model checker.
7. **R7.07 — Client sessions and ack dedup.** Implement `SessionState` in
   `CoreState`, bounded result cache, session expiry via `AdvanceTime`, and
   retry semantics in `relay-client`. Done when a retried Delete crossing a
   failover applies once and returns the cached result byte-identically.
8. **R7.08 — ReadIndex.** Implement the three-step read protocol and follower
   forwarding. Done when a partitioned stale leader can never answer a read
   with pre-partition state (SIM scenario asserts every ReadIndex response is
   linearizable in the checked history).
9. **R7.09 — Snapshots with resumable install.** Implement creation at the
   apply index, chunked streaming, install markers, resume, and hash-gated
   adoption. Done when SIM-RAFT-SNAPTORN passes across its full crash-point
   sweep and a lagging follower converges through snapshot + tail.
10. **R7.10 — Membership change.** Implement single-server change, learners,
    one-in-flight enforcement, and removed-server fencing. Done when the
    membership partition sweep finds no double-leader and a 3→4→3 cycle under
    faults converges with no availability gap beyond election time.
11. **R7.11 — Leader hints and client redirects.** Populate `ReportLeader`
    into R6's `leader_hint`, implement bounded redirect-following in
    `relay-client`. Done when a client-visible failover completes within
    redirect bounds in sim, and stale hints terminate.
12. **R7.12 — Adversarial scenario corpus.** Build the named scenarios of
    section 12.6 (partition during lease grant, leader kill after ack,
    asymmetric partition, clock jump, torn snapshot) plus randomized seed
    sweeps; check every failing seed into the corpus. Done when a nightly
    10⁴-seed sweep runs clean and the corpus replays in CI's budget.
13. **R7.13 — Linearizability at scale.** Export JSONL histories from every
    R7 scenario, extend the checker's search strategy (per-queue partitioning
    with memoized Wing–Gong, ADR-0007) to failover-length histories within the
    wall-clock budget. Done when a planted double-lease and a planted lost
    ack are both caught by the checker (validation of the oracle itself).
14. **R7.14 — Live smoke test and gate assembly.** Implement the tokio effect
    driver, the 3-node loopback smoke test with its wiring-only disclaimer,
    and the NFR-AVAIL-001 sim evidence (full workload with one node down).
    Done when the smoke test is green, quarantine-ruled as a live suite, and
    the gate PR assembles every evidence artifact of section 12.9.

### 12.6 Test-driven evidence matrix

| Test | First failing condition | Required passing assertion |
| --- | --- | --- |
| SIM-RAFT-PARTGRANT | During a partition formed between a Receive request and its commit, the old leader's lease and the new leader's lease on the same message both become visible. | Across the full seed sweep, at most one live lease per message exists in every checked history; the old leader never acks the grant (P-08, FR-REPL-004). |
| SIM-RAFT-KILLACK | The leader is killed immediately after a client-visible send ack and the entry is absent from the new leader's log. | Every acked entry survives failover and is delivered or dead-lettered; checker finds no lost ack in any seed (P-09, FR-REPL-003). |
| SIM-RAFT-ASYM | An asymmetric partition (old leader can send, cannot receive) lets the old leader commit, serve a ReadIndex read, or expire a lease. | The isolated leader commits nothing, serves no linearizable read, and steps down on heal without deposing the new leader (pre-vote). |
| SIM-RAFT-CLOCK | A SimClock jump (±1 h, either node subset) causes early lease expiry, spurious commit, or split brain. | Clock jumps affect only timer firing rates; all lease and commit behavior is unchanged because time advances only via committed AdvanceTime entries (ADR-0005). |
| SIM-RAFT-SNAPTORN | A crash mid-snapshot-install (chunk boundary or torn mid-chunk) adopts partial state or wedges the follower. | Every crash point resumes from the install marker or restarts cleanly; adoption occurs only after footer SHA-256 verification; the follower converges (FR-REPL-005). |
| RAFT-ELECT-SAFETY | Any explored seed produces two leaders in one term, or a vote is granted twice in a term. | Election Safety holds across the sweep; voted_for is durable before every vote response (FR-REPL-001). |
| RAFT-COMMIT-DURABLE | Commit index counts a follower whose ack preceded its fsync (follower crashed after send, before sync). | The harness durable-before-send assertion plus the crash schedule prove match_index only advances on durable appends (FR-REPL-002). |
| RAFT-LOGMATCH-PROP | After any step in any seed, two logs disagree at the same (term, index). | Log Matching invariant checked after every applied step across the randomized sweep; conflict backup converges divergent logs. |
| RAFT-READINDEX | A read returns state older than a previously acknowledged write, in any interleaving. | Every ReadIndex response is linearizable in the Wing–Gong-checked history; stale leaders fail the heartbeat-round confirmation (FR-REPL-008). |
| RAFT-MEMBER | A membership partition sweep elects two leaders during a 3→4 or 4→3 transition, or two changes run concurrently. | Single-server overlap argument holds empirically: no disjoint majorities; second in-flight change is rejected; removed node is fenced (FR-REPL-006). |
| RAFT-HINT-REDIRECT | A client retries forever on a deposed leader or exceeds the redirect bound on stale hints. | NotLeader carries the hint; relay-client reaches the new leader within 3 redirects or falls back to the seed list; stale-hint loops terminate (FR-REPL-007). |
| SIM-RAFT-DEDUP | A retried ack (Delete) crossing failover applies twice or returns a divergent result. | Session dedup returns the cached Output byte-identically; apply count for the (session, seq) pair is exactly one across all seeds. |
| SIM-RAFT-AVAIL | With one node of three down (each node, in turn), a write or linearizable read fails after election settles. | Full R1 workload passes with any single node down; unavailability is bounded to election time in simulated clock (NFR-AVAIL-001). |
| MODL-RAFT-ORACLE | The checker passes a history with a planted double-lease or planted lost ack. | Oracle validation: both planted violations are detected; only then are clean sweeps meaningful (ADR-0007). |
| RAFT-LIVE-SMOKE | Three real processes fail to elect, serve, and survive one failover on loopback. | Smoke test green under live-suite quarantine rules; its report is labeled process-wiring evidence only, never correctness evidence. |

### 12.7 Failure and security cases

- The Raft port (7416) is cluster-internal: peers authenticate mutually with
  per-cluster keys over TLS, and RaftMsg parsing uses the same
  bounds-before-allocation discipline as RWP/1 — a hostile peer packet cannot
  allocate unboundedly or panic a node. Peer messages from unknown NodeIds are
  dropped and counted.
- A removed member's credentials are invalidated with the committed
  ConfigChange; fencing is both cryptographic (rejected connection) and
  protocol-level (pre-vote).
- fsync failure during Raft persistence follows NFR-DUR-005 unchanged: the
  process aborts. A node that cannot persist must not vote, ack, or lead.
- Disk-full on a follower fails appends cleanly (R2 semantics); the leader
  routes around it and the follower rejoins via snapshot after operator
  remediation — the sim disk-full scenario asserts no corruption and no
  false ack.
- Snapshot temp files and install markers live inside the 0700 data dir; a
  crash leaves at most one temp install, cleaned or resumed at recovery,
  never double-counted storage.
- `TimeoutNow` (leadership transfer) is accepted only from the current leader
  in the current term, preventing a peer from forcing elections.
- The session result cache is bounded per session and sessions expire via
  AdvanceTime; a client cannot grow replicated state unboundedly by opening
  sessions — session creation is quota-charged at the R6 edge.
- All sim scenarios run single-threaded in virtual time; any nondeterminism
  (a hash-map iteration, a time read, a real RNG) that breaks seed replay is
  itself a released-blocking bug under NFR-MAINT-002.

### 12.8 Migration, documentation, and installation work

R7 adds WAL record types (`RaftHardState`, `RaftEntry`) and the `RSNAP1`
snapshot file under the existing format-version regime (NFR-DUR-007): a
single-node R6 data directory opens under R7 as a one-node cluster whose
migration fixture is checked in; the documented downgrade policy is that a
data directory containing Raft records does not open under an R6 binary.
Documentation deliverables: a cluster-formation guide (3-node bootstrap,
peer addresses, per-cluster keys), the replication section of
CORRECTNESS.md updating the P-08/P-09 mappings to the SIM-RAFT and MODL-RAFT
evidence with the lease-linearization argument of section 12.4 reproduced
normatively, and the FR-REPL acceptance links in PRODUCT_REQUIREMENTS.md.
Multi-node packaging, rolling upgrade across versions (FR-REPL-009), and
backup of clustered deployments remain R10; cluster admin verbs in `relayctl`
(member list, health, leadership transfer) remain R8 (FR-ADMIN-007).

### 12.9 Acceptance evidence

R7 is accepted only when:

- the full section 12.6 matrix is green, with the named adversarial scenarios
  and the randomized seed sweeps running in CI (bounded) and nightly (10⁴
  seeds), and the failing-seed corpus replaying deterministically;
- the oracle-validation test (planted double-lease, planted lost ack) proves
  the checker can catch what the sweeps claim is absent;
- every R7 evidence artifact is simulation-or-checker derived; the gate pull
  request contains no claim resting on the live smoke test beyond "processes
  wire together";
- the R1–R6 accepted evidence replays green, including the R1 semantic suites
  re-run against a replicated 3-node sim cluster;
- NFR-AVAIL-001 sim evidence (each node down in turn, full workload) is
  attached with seeds;
- CORRECTNESS.md's P-08 and P-09 rows point at the named passing tests and
  the non-guarantees NG-07 (crash-stop only) and NG-08 (no multi-region) are
  restated beside them.

### 12.10 Explicit deferrals

R7 defers: measured (wall-clock) failover time — NFR-AVAIL-002 is simulated
here and measured in R9; mixed-version clusters and rolling upgrade
(FR-REPL-009, R10); cluster administration UX, leadership-transfer tooling,
health/readiness reflecting Raft state, and audit logging (R8); witness or
learner-only read replicas, clusters larger than 5 voters, geo-replication
(NG-08), and Byzantine tolerance (NG-07) — the latter two permanently, as
published non-guarantees. Performance tuning of the replication path (batching
beyond correctness needs, pipelining depth) is R9 work gated on benchmarks.

### 12.11 Requirements traced

R7 is the terminal owning gate for `FR-REPL-001` through `FR-REPL-008` and
`NFR-AVAIL-001`. It advances `FR-REPL-009` (R10), `NFR-AVAIL-002` (measured
at R9), `FR-ADMIN-007` (R8), and the continuing `NFR-MAINT-002` corpus
discipline. The section 16 matrix lists each terminal ID exactly once against
R7, and CORRECTNESS.md's property mapping for P-08 and P-09 names the
SIM-RAFT and MODL-RAFT tests of section 12.6 as their proving evidence.

## 13. R8 — Metrics, Tracing, Admin Surface, and Runbook Make Relay Operable

**Status:** planned.

**Effort range:** 8–12 focused days, including the runbook and the redacted
diagnostics bundle with its canary suite.

### 13.1 Why this gate exists

Through R7 Relay is correct but blind: an operator cannot see queue depth,
cannot tell which node leads, cannot transfer leadership before maintenance,
and cannot hand a support engineer anything better than raw log files. R8
exists so that every operational question named in the runbook has a metric,
span, log field, health endpoint, or `relayctl` command that answers it, and
so that every administrative mutation leaves an audit record. R8 also closes
the admin CLI: after this gate, `relayctl` covers every administrative
operation the server exposes, in both human and JSON output, and any server
operation without a `relayctl` verb is a build failure in the coverage check.

### 13.2 Prerequisites

- R6 is accepted: the RWP/1 wire API, authentication, ACLs, quotas,
  configuration loading, and the error taxonomy exist and are fuzz-gated.
- R7 is accepted: Raft election, replication, snapshots, membership change,
  and ReadIndex are green under the simulation corpus.
- ADR-0010 (Prometheus + OTLP traces + JSON logs) is accepted and names the
  observability stack this gate implements.
- The metrics/health port 7415 is reserved in configuration and documented in
  ./ARCHITECTURE.md; nothing else binds it.
- The OPERATIONS_TEST_PLAN ADMN- and OPSX- family matrices are written, so
  every ticket below lands against a named test row.

### 13.3 Owned files, interfaces, and state

R8 owns:

- `crates/relay-server/src/obs/metrics.rs` — the metric registry, inventory,
  and cardinality guard;
- `crates/relay-server/src/obs/tracing.rs` — OTLP span construction over the
  request lifecycle;
- `crates/relay-server/src/obs/logging.rs` — the JSON log encoder and field
  conventions;
- `crates/relay-server/src/obs/health.rs` — `/healthz` and `/readyz` on port
  7415 with Raft-aware readiness;
- `crates/relay-server/src/admin/audit.rs` — the append-only audit log;
- `crates/relay-cli/src/**` — the complete `relayctl` command tree, output
  formatting, and the `diagnose` bundle builder;
- `docs/RUNBOOK.md` — the operator runbook;
- the CI coverage check that maps every admin-capable RWP/1 opcode to at
  least one `relayctl` verb.

Readiness is a pure value computed from replicated state plus local lag:

```rust
pub enum ReadyState {
    Ready { role: RaftRole, leader: Option<NodeId> },
    NotReady { reason: ReadyReason },
}

pub enum ReadyReason {
    NotInRaftConfig,
    NoKnownLeader,
    ApplyLagExceeded { commit_index: u64, applied_index: u64, max_lag: u64 },
    WalNotWritable,
    ShuttingDown,
}

pub fn readiness(raft: &RaftStatus, wal: &WalStatus, cfg: &ReadyConfig) -> ReadyState;
```

Every audit record is an immutable JSONL line with a tamper-evident chain:

```rust
pub struct AuditRecord {
    pub seq: u64,                  // strictly increasing per node
    pub ts_wall: WallClock,        // labeled approximate; ordering authority is seq
    pub actor: TenantKeyId,        // key ID only; never the key
    pub source: SourceAddr,
    pub op: AuditOp,               // one variant per administrative mutation
    pub resource: ResourceRef,
    pub params_redacted: serde_json::Value,
    pub result: AuditResult,       // Ok | Denied { code } | Failed { code }
    pub request_id: u64,
    pub node: NodeId,
    pub raft_term: u64,
    pub prev_sha256: [u8; 32],     // chain over the previous encoded record
}
```

The metric inventory is fixed. Every series Relay emits appears in this table;
emitting an unlisted metric name fails the OPSX inventory test.

| Metric | Type | Labels | Meaning |
| --- | --- | --- | --- |
| `relay_send_total` | counter | `queue`, `result` | Send commands applied, by outcome code class. |
| `relay_receive_total` | counter | `queue`, `result` | Receive commands applied, by outcome code class. |
| `relay_delete_total` | counter | `queue`, `result` | Delete commands applied, by outcome code class. |
| `relay_publish_total` | counter | `topic`, `result` | Publish commands applied, by outcome code class. |
| `relay_fanout_copies_total` | counter | `topic` | Per-subscription copies produced by fanout. |
| `relay_queue_depth` | gauge | `queue`, `state` | Messages per lifecycle state: `available`, `delayed`, `deadlettered`. |
| `relay_inflight` | gauge | `queue` | Live leases (InFlight messages). |
| `relay_long_poll_waiters` | gauge | `queue` | Parked long-poll receives. |
| `relay_redrive_active` | gauge | `queue` | 1 while a redrive task runs on the queue. |
| `relay_dedup_hits_total` | counter | `queue` | FIFO sends resolved by the dedup window. |
| `relay_wal_fsync_seconds` | histogram | none | fsync latency; buckets 0.0005 to 1.0, factor 2. |
| `relay_wal_appended_bytes_total` | counter | none | Bytes appended to the WAL. |
| `relay_wal_segments` | gauge | none | Live WAL segment files. |
| `relay_wal_durable_lsn` | gauge | none | Highest fsynced LSN. |
| `relay_raft_term` | gauge | none | Current Raft term on this node. |
| `relay_leader` | gauge | none | 1 if this node is leader, else 0. |
| `relay_raft_commit_index` | gauge | none | Raft commit index. |
| `relay_raft_applied_index` | gauge | none | Highest log index applied to relay-core. |
| `relay_raft_snapshot_total` | counter | `direction` | Snapshots taken (`create`) or installed (`install`). |
| `relay_request_duration_seconds` | histogram | `opcode` | End-to-end request latency; opcode set is the closed RWP/1 table. |
| `relay_connections` | gauge | none | Open client connections. |
| `relay_auth_failures_total` | counter | `reason` | Authentication failures by closed reason set. |
| `relay_throttled_total` | counter | none | Requests rejected by quota or rate limit (no tenant label; tenants are unbounded). |
| `relay_errors_total` | counter | `code` | Responses by stable error-taxonomy code. |
| `relay_queue_series_suppressed_total` | counter | none | Per-queue series withheld by the cardinality guard. |

The cardinality budget is named, not implied: at most **40 series per queue**
across all per-queue metrics, and a default guard of **2,000 queues** beyond
which per-queue labels are suppressed. Above the guard, per-queue series stop
being registered, aggregate node-level series continue, and
`relay_queue_series_suppressed_total` counts what was withheld; the guard
threshold is configurable and its crossing is logged once per boundary
transition. Worst-case default budget is therefore 80,000 per-queue series
plus a fixed node-level set, and the OPSX budget test computes the actual
count against these numbers.

### 13.4 Algorithms and state behavior

**Span tree.** Every request produces exactly one root span with a fixed
child structure; span names and attributes are closed sets asserted by test:

```text
relay.request            (root; attrs: opcode, request_id, tenant_key_id, error_code)
├── relay.decode         (frame parse and validation)
├── relay.auth           (HMAC verification)
├── relay.authorize      (ACL evaluation)
├── relay.apply          (command handling)
│   └── relay.raft.propose
│       ├── relay.raft.append   (per-peer replication, attr: peer)
│       └── relay.wal.fsync     (group-commit durability barrier)
├── relay.longpoll.wait  (present only for parked receives; attr: waited_ms)
└── relay.encode         (response framing)
```

Spans never carry message bodies, attribute values, credentials, or receipt
handles; the redaction canary suite injects unique markers through every path
and asserts zero occurrences in exported spans.

**JSON log conventions.** One JSON object per line with the stable field set
`ts`, `level`, `event`, `node_id`, and, where applicable, `request_id`,
`tenant_key_id`, `queue`, `topic`, `opcode`, `error_code`, `duration_ms`,
`raft_term`, `msg`. `event` values are a closed, documented enumeration; free
text lives only in `msg`. The same redaction canaries that gate spans gate
logs (NFR-SEC-003 is closed at R6; R8 extends its canary corpus to the new
surfaces without weakening it).

**Health and readiness.** `/healthz` answers process liveness only: the
process is running and the WAL directory is writable. `/readyz` computes
`readiness()` in this order and returns the first failure as a 503 with a
JSON body naming the `ReadyReason`: (1) shutting down; (2) not a voter or
learner in the current Raft configuration; (3) no known leader; (4) applied
index lags commit index beyond the configured bound; (5) WAL not writable.
Otherwise 200 with `role` and `leader` fields. A follower that satisfies all
checks is ready — readiness means "safe to route to", and followers serve
reads via leader-verified ReadIndex forwarding, so load balancers must not
require leadership for readiness (FR-OPS-003).

**Leadership transfer.** `relayctl cluster transfer-leadership --to <node>`
issues the R7 transfer operation: the leader brings the target up to date,
sends TimeoutNow, and steps down only after the target's match index equals
the leader's last index. The command blocks with progress output until a new
leader is observed or a deadline expires; on deadline it reports the cluster
state without claiming success.

**relayctl command tree.** The complete tree; every leaf supports
`--output human` (default) and `--output json`, plus global `--endpoint`,
`--profile`, and `--timeout` flags:

```text
relayctl queue        create | delete | describe | list | set-attributes | purge | tag | untag
relayctl message      send | send-batch | receive | delete | change-visibility
relayctl dlq          redrive-start | redrive-status
relayctl topic        create | delete | describe | list
relayctl subscription create | delete | list
relayctl cluster      members | health | transfer-leadership | add-member | remove-member
relayctl audit        tail | export
relayctl diagnose
relayctl config       validate
relayctl version
```

JSON output is the machine contract: stable field names, one JSON document on
stdout, diagnostics on stderr, and exit codes drawn from a documented closed
set. The CI coverage check parses the RWP/1 opcode table and fails if any
admin-capable opcode lacks a mapped verb (FR-ADMIN-006).

**Audit log.** Every administrative mutation — queue/topic create, delete,
set-attributes, purge, tag/untag, subscribe/unsubscribe, redrive start,
membership change, leadership transfer — appends one `AuditRecord` before the
success response is sent. The chain field makes truncation and tampering
detectable by `relayctl audit export --verify`, which recomputes the SHA-256
chain and reports the first broken link. Audit writes share the WAL fsync
discipline: an audit append that cannot be made durable fails the mutation
with a stable error rather than succeeding unaudited (FR-ADMIN-008).

**Diagnose bundle.** `relayctl diagnose` produces one tar.gz containing, in
fixed file names: build/version info; the effective configuration with every
secret-classified field replaced by `<redacted>`; the last 10,000 log lines
after redaction; a metrics snapshot; Raft status (term, role, indices,
membership); the queue/topic inventory as names, configurations, and
approximate counts; disk usage of the data directory; open file-descriptor
count; the last 200 audit records; and both health endpoint bodies. The
bundle never contains message bodies, message attributes, credentials,
receipt keys, TLS private keys, or tenant HMAC keys; the exclusion list is
enforced by canary tests that plant unique secret markers and scan the
produced archive (FR-OPS-010).

**Runbook.** `docs/RUNBOOK.md` ships with this gate and contains, as
numbered procedures: single-node start/stop/restart; cluster bootstrap;
adding and removing a member; planned leadership transfer; responding to
`readyz` failure by reason; disk-pressure response and compaction
verification; interpreting `relay_wal_fsync_seconds` regressions; DLQ triage
and redrive; quota-throttle diagnosis; certificate rotation; collecting a
diagnose bundle for support; and the escalation pointer to the incident
procedure that R10 completes. Every procedure names the exact commands,
metrics, and expected outputs; a docs test asserts each referenced command
and metric exists.

### 13.5 Implementation tickets and sequence

1. **R8.01 — Metric registry and inventory.** Implement the registry, all
   inventory metrics, and the OPSX inventory test that fails on any emitted
   series absent from section 13.3 or any table entry never emitted by the
   exercised test workload. Done when the inventory test passes both
   directions.
2. **R8.02 — Cardinality guard.** Implement the 40-series-per-queue
   accounting and the 2,000-queue suppression guard with its counter and
   boundary log. Done when the OPSX budget test creates 2,001 queues and
   observes suppression, the counter, exactly one boundary log line, and
   intact node-level series.
3. **R8.03 — OTLP request spans.** Implement the fixed span tree over every
   opcode, including the long-poll and Raft children. Done when a span-shape
   test asserts the exact tree per opcode against an in-process OTLP
   collector and the redaction canaries pass.
4. **R8.04 — JSON log conventions.** Implement the encoder, the closed
   `event` enumeration, and the field table. Done when a log-schema test
   validates every emitted line against the convention and rejects an
   unknown field or event value.
5. **R8.05 — Health and readiness endpoints.** Implement `/healthz` and
   `/readyz` with the ordered `ReadyReason` evaluation. Done when OPSX tests
   drive each reason via simulation (removed from config, leaderless
   partition, apply lag, read-only disk, shutdown) and observe the matching
   503 body, and a healthy follower returns 200.
6. **R8.06 — relayctl command tree.** Implement every verb in section 13.4
   with human and JSON output and the closed exit-code set. Done when the
   opcode-coverage check passes and ADMN golden tests cover both output modes
   for every leaf.
7. **R8.07 — Cluster administration.** Implement `cluster members`,
   `cluster health`, and `transfer-leadership` end to end, including the
   blocking progress behavior and deadline handling. Done when a three-node
   integration test transfers leadership with zero failed acknowledged writes
   during the transfer.
8. **R8.08 — Audit log.** Implement `AuditRecord`, the durable append-before-
   ack rule, chain verification, and `relayctl audit tail|export`. Done when
   ADMN tests show one record per mutation, a detected truncation, and a
   failed mutation when the audit device is full.
9. **R8.09 — Diagnose bundle.** Implement the bundle builder with the fixed
   content list and redaction. Done when the canary suite plants secrets in
   config, logs, and state, and the archive scan finds zero markers while all
   listed files are present and parseable.
10. **R8.10 — Operator runbook.** Write `docs/RUNBOOK.md` per section 13.4
    and the docs test that resolves every referenced command and metric name.
    Done when the docs test passes and two runbook procedures (leadership
    transfer, DLQ redrive) are executed verbatim against a test cluster in
    CI.

### 13.6 Test-driven evidence matrix

| Test | First failing condition | Required passing assertion |
| --- | --- | --- |
| OPSX metric inventory | Relay emits a series not in the inventory, or an inventory row never appears. | Bidirectional match between emitted series and section 13.3 under the exercised workload. |
| OPSX cardinality budget | Per-queue series exceed 40, or queue 2,001 registers per-queue labels. | Budget arithmetic holds; suppression counter and single boundary log observed. |
| OPSX span shape | A request produces a missing, extra, or misnamed span or attribute. | Exact fixed span tree per opcode at the in-process OTLP collector. |
| OPSX log schema | A log line has an unknown field, unknown event, or non-JSON content. | Every line validates against the closed field and event sets. |
| OPSX readiness matrix | A `ReadyReason` state returns 200, or a ready follower returns 503. | Each simulated reason yields its named 503 body; healthy leader and follower return 200. |
| ADMN coverage check | An admin-capable RWP/1 opcode has no `relayctl` verb. | Opcode table maps onto the command tree with zero gaps. |
| ADMN output goldens | A leaf's JSON output changes shape or its exit code leaves the closed set. | Golden human and JSON outputs match for every leaf, including error paths. |
| ADMN leadership transfer | Transfer loses an acknowledged write or reports success without a new leader. | Three-node transfer completes with zero lost acks and truthful deadline reporting. |
| ADMN audit chain | A mutation lacks a record, or a truncated log verifies clean. | One chained record per mutation; `audit export --verify` pinpoints the first broken link. |
| OPSX diagnose redaction | A planted canary appears anywhere in the bundle. | Zero canary occurrences; all fixed-name files present and parseable. |
| OPSX runbook resolution | A runbook procedure references a nonexistent command or metric. | Every referenced name resolves; the two CI-executed procedures pass verbatim. |

### 13.7 Failure and security cases

- Metrics and health share port 7415 and must never require authentication
  weaker than the deployment intends: the port binds loopback by default and
  requires explicit configuration to expose, mirroring the R6 plaintext rule.
- A full disk under the audit log fails the mutation, not the audit: no
  administrative change ever completes unrecorded.
- The diagnose bundle is the highest-risk exfiltration surface; its exclusion
  list is enforced by scanning, not by construction alone, and any new bundle
  file requires a new canary before merge.
- Readiness must not flap during elections: a bounded grace window (one
  election timeout) suppresses `NoKnownLeader` transitions shorter than the
  window, and the test matrix covers the flap case.
- `relayctl` inherits R6 authentication; there is no local bypass socket. An
  operator without a valid tenant key cannot administer a running cluster.
- Leadership transfer to a lagging or unreachable target must abort at the
  deadline with the cluster unchanged; the failure case is tested under a
  partitioned target.
- Cardinality suppression must fail open for correctness and closed for
  cost: message flow is never blocked by the metrics guard.

### 13.8 Migration, documentation, and installation work

No on-disk format changes in R8 except the new audit log file, which carries
its own versioned header (`RAUD1`, format version u16) and is covered by a
MIGR fixture from this gate forward. Documentation work is first-class here:
`docs/RUNBOOK.md` ships, ./ARCHITECTURE.md gains the observability section,
and ./OPERATIONS_TEST_PLAN.md's ADMN-/OPSX- matrices flip their rows from
planned to accepted as tickets land. Installation is unchanged; the metrics
port default and its loopback binding are documented in the configuration
reference.

### 13.9 Acceptance evidence

R8 is accepted only when:

- the OPSX inventory, budget, span, log, readiness, diagnose, and runbook
  tests are green in CI from a clean commit;
- the ADMN coverage check proves `relayctl` maps every admin-capable opcode,
  and golden tests cover human and JSON output for every leaf;
- a three-node leadership transfer completes in CI with zero lost
  acknowledged writes;
- the audit chain verifies across a mutation-heavy integration run and
  detects an injected truncation;
- the diagnose canary suite reports zero secret occurrences;
- `docs/RUNBOOK.md` exists, resolves, and its two CI-executed procedures
  pass;
- every prior accepted gate remains green.

### 13.10 Explicit deferrals

R8 defers: measured performance claims about the observability overhead
(R9 benchmarks run with metrics enabled and report the cost); backup,
restore, and the DR drill (R10, FR-OPS-007); uninstall and purge
(R10, FR-OPS-009); the incident-response procedure beyond the runbook's
escalation pointer (R10, FR-OPS-012); the capacity model (R9, FR-OPS-011);
and any alerting-rule or dashboard deliverable, which is recorded in
OPEN_QUESTIONS.md with a fail-closed default of shipping none.

### 13.11 Requirements traced

R8 is the terminal owning gate for FR-ADMIN-006, FR-ADMIN-007, FR-ADMIN-008,
FR-OPS-003, FR-OPS-004, FR-OPS-005, FR-OPS-006, and FR-OPS-010. It extends,
without owning, NFR-SEC-003 (new redaction surfaces keep the R6 canary
guarantee) and FR-API-006 (error codes surface unchanged through `relayctl`).
It begins FR-OPS-011 by emitting the measurements the R9 capacity model
consumes.

## 14. R9 — Published Benchmarks, Failure-Injection Report, and Evidence-Bound Marketing

**Status:** planned.

**Effort range:** 6–10 focused days on fixed reference hardware, including
report writing and the claims audit.

### 14.1 Why this gate exists

Every number Relay publishes must be reproduced, not remembered. R9 runs the
benchmark harness against ./BENCHMARK_PLAN.md on the named reference
hardware, publishes the failure-injection report that shows what the
simulation and crash corpora actually demonstrated, derives the capacity
model from measured constants, and passes every piece of marketing copy
through the claims audit so that no public sentence outruns
./CORRECTNESS.md, ./THREAT_MODEL.md, or a BENCH result. The product thesis —
guarantees that are machine-checked, not asserted — dies the first time a
README claim lacks a citation; R9 is the gate that makes that structurally
impossible.

### 14.2 Prerequisites

- R8 is accepted: benchmarks run with metrics, tracing, and logging enabled
  at production defaults, and the harness reads `relay_*` metrics as a
  cross-check on its own measurements.
- ./BENCHMARK_PLAN.md is accepted and fixes workloads, hardware
  (8 vCPU / 16 GiB / local NVMe, Linux 6.x), warmup, duration, repetition
  count, and statistical treatment for every BENCH ID.
- ./MARKETING.md is accepted and fixes the messaging pillars, the launch
  plan by gate, and the MKT- claims-audit checklist shape.
- The R3 simulation corpus and R2 crash corpus are green, because the
  failure-injection report cites their seeds and scenarios directly.

### 14.3 Owned files, interfaces, and state

R9 owns:

- `crates/relay-bench/src/**` — the workload drivers, latency recorder, and
  report generator;
- `bench/results/<date>-<hardware-id>/` — committed raw results, one
  directory per accepted run;
- `docs/reports/FAILURE_INJECTION.md` — the failure-injection report;
- `docs/reports/CAPACITY_MODEL.md` — the capacity model (FR-OPS-011);
- `docs/reports/BENCHMARKS.md` — the published numbers with full context;
- the MKT- claims-audit fixtures under `docs/marketing/claims/`.

The harness produces a self-describing report; no number is published
without its provenance struct:

```rust
pub struct BenchRunSpec {
    pub bench_id: BenchId,            // from ./BENCHMARK_PLAN.md
    pub workload: WorkloadId,         // send | send-receive-delete | longpoll | recovery | failover
    pub body_bytes: u32,
    pub connections: u32,
    pub target_rate: Option<u32>,     // None = open throttle
    pub warmup: Duration,
    pub measured: Duration,
    pub repetitions: u32,
}

pub struct BenchReport {
    pub spec: BenchRunSpec,
    pub relayd_version: String,       // exact commit, dirty forbidden
    pub hardware: HardwareFingerprint,// cpu model, cores, memory, disk model, kernel
    pub config_sha256: [u8; 32],      // effective relay.toml
    pub throughput: ThroughputSummary,
    pub latency: LatencySummary,      // p50/p90/p99/p999 with HDR histogram export
    pub metric_crosscheck: CrossCheck,// server counters vs harness counters, tolerance
    pub statistical_treatment: StatSummary, // per-repetition results, median-of-runs rule
}
```

### 14.4 Algorithms and state behavior

**Benchmark execution.** Each BENCH ID runs exactly as ./BENCHMARK_PLAN.md
specifies: fresh data directory, production-default configuration with the
diff recorded, warmup excluded from measurement, fixed repetition count, and
the plan's statistical rule (median of repetitions for throughput; pooled
HDR histogram for percentiles). The harness cross-checks its own operation
counts against `relay_send_total`, `relay_receive_total`, and
`relay_delete_total`; a divergence beyond tolerance invalidates the run. The
gate targets, restated from the register: NFR-PERF-001 (≥ 20,000 msg/s
sustained send+receive+delete at 256-byte bodies, single node), NFR-PERF-002
(p99 send-to-ack ≤ 15 ms with fsync-before-ack), NFR-PERF-003 (long-poll
wakeup ≤ 10 ms), NFR-PERF-004 (crash recovery ≤ 30 s for a 10 GiB WAL), and
NFR-AVAIL-002 (clean leader kill to first new acknowledged write ≤ 5 s,
measured on a real three-node cluster, not simulated time).

**Measured failover.** The failover run kills the leader process with
SIGKILL at a recorded instant while a closed-loop writer runs; the harness
records the timestamp of the last pre-kill acknowledged write, the first
post-kill acknowledged write, verifies through the MODL history checker that
no acknowledged write was lost, and reports the gap distribution over 20
repetitions. The published NFR-AVAIL-002 number is the p99 of that
distribution.

**Failure-injection report.** `docs/reports/FAILURE_INJECTION.md` is a
generated-then-edited document with one section per scenario class, each
citing exact seeds and corpus files: SIM partition scenarios (leader
isolation, symmetric split, partial partition, message reordering and
duplication), SIM clock-skew scenarios, CRSH torn-write and truncation
scenarios, CRSH disk-full scenarios, and the fsync-failure abort. Each
section states the scenario mechanics, the property it exercises
(P-01, P-02, P-08, P-09, P-10 by ID), the observed behavior, and the link to
the checked-in corpus seed that reproduces it. A report section without a
reproducible seed is a build failure of the report generator.

**Capacity model.** `docs/reports/CAPACITY_MODEL.md` derives disk, memory,
and throughput planning formulas from measured constants: bytes-per-message
WAL amplification (measured), segment and snapshot overhead (measured),
retention-driven disk formula
`disk ≈ rate × (body + overhead) × retention + snapshot + slack`, in-flight
memory cost per lease, and the long-poll waiter cost. Every constant cites
the BENCH run that produced it; the model states its validity bounds (the
reference hardware and the tested ranges) and refuses extrapolation claims
beyond them (FR-OPS-011).

**Claims audit.** Every public claim — README, docs/MARKETING.md-derived
copy, comparison table rows, badges — is enumerated in
`docs/marketing/claims/claims.jsonl`, one claim per line with fields
`claim_id`, `text`, `evidence` (a list of P-xx, NG-xx, or BENCH-xx IDs), and
`surface`. The MKT audit test fails when: a claim has an empty evidence
list; a cited ID does not exist; a performance claim lacks a BENCH ID; or a
delivery-guarantee claim appears on a surface that omits the applicable
non-guarantees (NG-01 wherever exactly-once could be inferred, per
FR-MKT-003). A claim that cannot be evidenced is removed from the surface,
not weakened into vagueness (FR-MKT-001, FR-MKT-002).

### 14.5 Implementation tickets and sequence

1. **R9.01 — Harness completion.** Finish `relay-bench` workload drivers,
   the HDR latency recorder, the metric cross-check, and the `BenchReport`
   generator. Done when a smoke run on developer hardware produces a
   complete, schema-valid report with cross-check within tolerance.
2. **R9.02 — Throughput and latency runs.** Execute BENCH throughput and
   send-to-ack latency IDs on reference hardware per plan. Done when
   NFR-PERF-001 and NFR-PERF-002 targets are met or the miss is recorded as
   a blocking finding, and raw results are committed under `bench/results/`.
3. **R9.03 — Long-poll wakeup run.** Execute the wakeup BENCH ID. Done when
   the p99 wakeup ≤ 10 ms result (NFR-PERF-003) is committed with the goal-
   not-contract framing (NG-05) attached in the report text.
4. **R9.04 — Recovery benchmark.** Build a 10 GiB WAL by scripted load,
   SIGKILL, and measure recovery to ready. Done when the ≤ 30 s NFR-PERF-004
   result is committed across 5 repetitions with variance reported.
5. **R9.05 — Measured failover.** Run the three-node SIGKILL failover
   protocol of section 14.4 twenty times. Done when the p99 gap ≤ 5 s
   (NFR-AVAIL-002) and the MODL checker confirms zero lost acknowledged
   writes in every repetition.
6. **R9.06 — Failure-injection report.** Generate and edit
   `docs/reports/FAILURE_INJECTION.md` with every scenario class, seed link,
   and property citation. Done when the generator verifies every cited seed
   replays to the described outcome in CI.
7. **R9.07 — Capacity model.** Write `docs/reports/CAPACITY_MODEL.md` from
   measured constants with validity bounds. Done when a docs test resolves
   every constant to a committed BENCH result.
8. **R9.08 — Claims audit and marketing deliverables.** Produce the
   ./MARKETING.md-specified deliverables for this gate, enumerate
   `claims.jsonl`, and wire the MKT audit test into CI. Done when the audit
   passes with zero unevidenced claims and NG-01 appears on every surface
   where exactly-once could be inferred.

### 14.6 Test-driven evidence matrix

| Test | First failing condition | Required passing assertion |
| --- | --- | --- |
| BENCH report schema | A published number lacks version, hardware, config hash, or statistical treatment. | Every committed `BenchReport` is schema-valid and its commit is clean (NFR-PERF-005). |
| BENCH cross-check | Harness counts diverge from server counters beyond tolerance. | Cross-check within tolerance for every accepted run. |
| BENCH throughput | Sustained mixed workload < 20,000 msg/s at 256-byte bodies. | NFR-PERF-001 met on reference hardware, median of repetitions. |
| BENCH latency | p99 send-to-ack > 15 ms with fsync-before-ack. | NFR-PERF-002 met from pooled HDR histograms. |
| BENCH wakeup | p99 long-poll wakeup > 10 ms. | NFR-PERF-003 met; NG-05 framing present in the report. |
| BENCH recovery | 10 GiB WAL recovery > 30 s in any repetition beyond the stated variance rule. | NFR-PERF-004 met across 5 repetitions with variance reported. |
| BENCH failover | Failover gap p99 > 5 s, or MODL finds a lost acknowledged write. | NFR-AVAIL-002 met over 20 kills with zero lost acks (P-09). |
| SOAK endurance | The 24-hour mixed soak leaks memory, degrades throughput monotonically, or trips any invariant. | Soak completes with bounded memory, stable throughput, and zero invariant violations. |
| MKT claims audit | Any public claim lacks resolvable P-xx/NG-xx/BENCH-xx evidence. | Every claim in every surface resolves; unevidenced claims are absent, not softened. |
| MKT non-guarantee placement | A surface implies exactly-once without NG-01. | NG-01 (and applicable NG-02..NG-10) present on every flagged surface. |
| OPSX report seeds | A failure-injection section cites a seed that does not replay to the described outcome. | Every cited seed replays deterministically in CI to the reported behavior. |

### 14.7 Failure and security cases

- A missed performance target is a finding, never a footnote: the gate
  blocks until the target is met or the requirement is renegotiated through
  a new ADR that also rewrites every affected claim.
- Benchmark runs from a dirty tree, non-reference hardware, or modified
  configuration are rejected by the report validator; there is no "roughly
  comparable machine" escape hatch.
- The failover test must use SIGKILL, not graceful shutdown; a graceful-only
  result would overstate availability.
- Marketing copy is untrusted input to the audit: the audit parses surfaces
  from committed files, not from a hand-maintained list, so an added README
  badge cannot bypass it.
- The soak run executes with redaction canaries planted, extending
  NFR-SEC-003 evidence across a 24-hour window.
- Published reports contain hardware fingerprints but never hostnames,
  usernames, or absolute developer paths; the report validator scans for
  them.

### 14.8 Migration, documentation, and installation work

No on-disk format changes. Documentation is the deliverable:
`docs/reports/BENCHMARKS.md`, `docs/reports/FAILURE_INJECTION.md`, and
`docs/reports/CAPACITY_MODEL.md` ship, and ./MARKETING.md's launch-plan rows
for this gate flip to accepted with links. The README gains its performance
section only through the claims audit. Installation guidance gains the
capacity-planning pointer; no installer changes occur until R10.

### 14.9 Acceptance evidence

R9 is accepted only when:

- every BENCH ID named for this gate has a committed, schema-valid report
  from reference hardware at a clean commit;
- NFR-PERF-001 through NFR-PERF-004 targets are met and NFR-PERF-005's
  provenance rule holds for every published number;
- the measured failover run meets NFR-AVAIL-002 with MODL-verified zero
  lost acknowledged writes;
- the failure-injection report's every seed replays in CI;
- the capacity model resolves every constant to a BENCH result;
- the MKT claims audit passes over every public surface;
- the 24-hour SOAK run is green;
- every prior accepted gate remains green.

### 14.10 Explicit deferrals

R9 defers: launch collateral and the release-announcement audit
(R10, FR-MKT-004 and FR-MKT-005 — R9 audits surfaces that exist, R10 audits
the release event); multi-node throughput scaling claims (no requirement
exists; recorded in OPEN_QUESTIONS.md, fail-closed as unclaimed); any
comparison-table benchmark of third-party systems (MARKETING.md comparison
rules allow only cited third-party published numbers, never our unaudited
reruns); and packaging of the harness for end users.

### 14.11 Requirements traced

R9 is the terminal owning gate for FR-MKT-001, FR-MKT-002, FR-MKT-003,
FR-OPS-011, NFR-PERF-001, NFR-PERF-002, NFR-PERF-003, NFR-PERF-004,
NFR-PERF-005, and NFR-AVAIL-002. It consumes but does not own the R3
simulation corpus (NFR-MAINT-002) and the R7 availability evidence
(NFR-AVAIL-001), and it begins FR-MKT-004 and FR-MKT-005 by establishing the
claims registry that R10's release audit closes.

## 15. R10 — Packaging, Deployment, Upgrade, Rollback, and Backup/Restore for 1.0

**Status:** planned.

**Effort range:** 8–12 focused days, including the DR drill, the
mixed-version upgrade rehearsal, and the final audits.

### 15.1 Why this gate exists

A queue that is correct but cannot be installed, upgraded, rolled back,
backed up, restored, or revoked is not a 1.0. R10 turns the accepted
engineering of R0–R9 into a release an operator can trust across its whole
lifecycle: a reproducible build with provenance, a static binary and a
container image whose contents are enumerated, an upgrade that works with
mixed versions running, a downgrade policy that is stated rather than
implied, a backup that has been restored on schedule by script rather than
hope, an uninstall that actually removes data, and an incident procedure
that can pull a bad release back. R10 is also where the final audits close:
threat model, dependency provenance, documentation status, and the release
claims audit over launch collateral.

### 15.2 Prerequisites

- R9 is accepted: every published number and claim is evidence-bound, so the
  release has something truthful to say.
- ADR-0009 (single static binary) and ADR-0011 (supported platforms:
  tier-1 Linux x86_64/aarch64; tier-2 macOS aarch64 dev-only; Windows
  unsupported at 1.0) are accepted and control the artifact matrix.
- ADR-0001's toolchain pins (edition 2024, MSRV 1.85, cargo-deny) are green
  on mainline.
- The MIGR- family matrix in ./OPERATIONS_TEST_PLAN.md is written, including
  the old-version fixture policy this gate populates.
- The R8 audit log and diagnose bundle exist, because the incident procedure
  depends on both.

### 15.3 Owned files, interfaces, and state

R10 owns:

- `.github/workflows/release.yml` — the reproducible release pipeline;
- `dist/systemd/relayd.service` — the shipped systemd unit;
- `dist/container/Containerfile` — the container image definition;
- `scripts/install.sh`, `scripts/uninstall.sh` — install and uninstall/purge;
- `scripts/dr-drill.sh` — the scripted disaster-recovery drill;
- `crates/relay-server/src/build_info.rs` — embedded version and provenance;
- `docs/UPGRADE.md`, `docs/BACKUP_RESTORE.md`, `docs/INCIDENT_RESPONSE.md`,
  `docs/INSTALL.md` — the lifecycle documentation set;
- `tests/migration/fixtures/` — MIGR old-version data-directory fixtures;
- the release manifest generator and the 1.0 definition-of-done checker.

Every release artifact is described by a manifest generated from a clean,
tagged commit:

```rust
pub struct ReleaseManifest {
    pub version: SemVer,                    // v1.0.0; tags are signed
    pub git_commit: [u8; 20],
    pub rustc_version: String,              // pinned toolchain, exact
    pub cargo_lock_sha256: [u8; 32],
    pub artifacts: Vec<ArtifactDigest>,     // name, target triple, sha256, size
    pub provenance: SlsaProvenanceRef,      // in-toto attestation per artifact
    pub sbom_sha256: [u8; 32],              // CycloneDX SBOM digest
    pub claims_audit_commit: [u8; 20],      // the commit whose MKT audit passed
    pub upgrade_window: UpgradeWindow,      // { from: ">=0.9, <1.0", mixed_minor_span: 1 }
    pub downgrade_policy: DowngradePolicyRef, // NFR-DUR-007 statement, by format version
}
```

The embedded build info answers `relayd --version` and
`relayctl version` with version, commit, target triple, rustc version, and
build timestamp; a development build without generated info prints a
deterministic `0.0.0-dev` fallback and refuses release-mode packaging.

### 15.4 Algorithms and state behavior

**Reproducible release pipeline.** The pipeline builds from a signed tag on
a pinned runner image with a pinned toolchain and vendored, hash-locked
dependencies; builds twice and compares artifact digests to verify
reproducibility; produces the static `relayd`/`relayctl` binaries for
Linux x86_64 and aarch64 (musl, fully static, verified by `ldd` refusal and
a scratch-container execution test); generates SLSA-style in-toto provenance
attestations binding each artifact digest to the workflow, tag, and commit;
generates the CycloneDX SBOM from the lockfile; signs artifacts, manifest,
and attestations; and publishes nothing automatically — publication is a
manual step gated by the section 17 checklist (FR-OPS-001).

**Container image and install paths.** The container image contains exactly:
the two static binaries, the default `relay.toml`, CA certificates, and a
non-root user; base is a distroless static image; the image digest appears
in the release manifest. Native install paths: binaries to
`/usr/local/bin/relayd` and `/usr/local/bin/relayctl`, configuration to
`/etc/relay/relay.toml`, data to `/var/lib/relay` (mode 0700, owner
`relay`, verified at startup per NFR-SEC-005), and the systemd unit to
`/etc/systemd/system/relayd.service`. The unit runs as the dedicated `relay`
user with `NoNewPrivileges=yes`, `ProtectSystem=strict`,
`ReadWritePaths=/var/lib/relay`, `Restart=on-failure`, and
`RestartSec=2` — restart-on-failure is load-bearing because ADR-0008 makes
fsync failure a deliberate process abort.

**Mixed-version rolling upgrade (FR-REPL-009).** The supported window is one
minor version: every node in a cluster must be within one minor of every
other. The procedure, per node, followers first, leader last after a
`relayctl cluster transfer-leadership`: drain via readiness withdrawal, stop,
replace binary, start, verify `readyz` and applied-index catch-up, proceed.
Wire and Raft messages carry the protocol feature level; a node refuses to
join a cluster outside the window with a stable error naming both versions.
The MIGR suite runs the previous released version's binary (checked in as a
fixture artifact for test use) against the new version in one cluster,
drives the full operation surface during the mixed window, and asserts zero
lost acknowledged writes and zero double-leases via the MODL checker.

**Format versioning and downgrade policy (NFR-DUR-007).** WAL segments,
snapshots, and the audit log carry format version fields (spine formats
`RWALSEG1`, `RSNAP1`, `RAUD1`). The 1.0 policy: a binary reads formats up to
its own version and one behind; it writes only its own version after a
cluster-wide feature-level bump that the operator triggers explicitly with
`relayctl cluster finalize-upgrade`; before finalization, downgrade to the
previous minor is supported by stopping the node and restarting the old
binary; after finalization, downgrade requires restore from backup, and both
facts are printed by the finalize command before it asks for confirmation.
MIGR fixtures include complete old-version data directories (WAL + snapshot
+ audit) that every release must recover byte-for-byte to the fixture's
recorded logical state.

**Backup and restore (FR-OPS-007).** Backup is a consistent snapshot plus
the WAL archive from the snapshot LSN: `relayctl cluster backup --to <dir>`
triggers a snapshot, hard-links or copies the snapshot file and subsequent
sealed segments, and writes a backup manifest with per-file digests and the
covered LSN range. Restore is `relayd --restore-from <dir>` into an empty
data directory: verify digests, install snapshot, replay archived WAL,
report the recovered LSN and state hash. The scripted DR drill
(`scripts/dr-drill.sh`) runs end to end in CI weekly and before any release:
load a cluster with a known workload, back up, destroy the data directory,
restore, and compare the restored state hash and MODL history against the
pre-destruction record. The drill's last green run date is a named field in
the release checklist; a drill older than 30 days blocks release.

**Uninstall and purge (FR-OPS-009).** `scripts/uninstall.sh` removes
binaries and the systemd unit but preserves `/etc/relay` and
`/var/lib/relay`, printing exactly what was kept; `uninstall.sh --purge`
additionally removes both after an interactive confirmation that names the
paths, and refuses to run against a data directory whose marker file is
absent (it never removes an unowned directory). The container path documents
volume removal equivalently.

**Incident response and release revocation (FR-OPS-012).**
`docs/INCIDENT_RESPONSE.md` defines: severity levels with response targets;
the on-call information flow (diagnose bundle first, audit export second);
the security-report intake channel and disclosure policy; and the revocation
procedure — mark the release yanked in the manifest index, publish a signed
revocation notice naming affected versions and the upgrade/downgrade path,
and add a regression test reproducing the defect before the fixed release
ships. Revocation is rehearsed once against a pre-1.0 artifact so the
procedure's first execution is not during an incident.

**Final audits.** Threat-model re-review (NFR-SEC-007): walk every
THREAT_MODEL.md entry against the shipped surface, record deltas, and file
each unmitigated finding as release-blocking or explicitly accepted with
rationale. Dependency provenance (NFR-SEC-008): `cargo-deny` green with
exact pins, the SBOM published, and every dependency change since R0
traceable to a reviewed commit. Documentation status audit (NFR-MAINT-005):
a scripted pass over every doc asserting statuses match gate reality and no
claim outruns its gate. Release claims audit (FR-MKT-004, FR-MKT-005): the
R9 claims machinery re-runs over launch collateral — site copy, README
badges, the comparison table, and the announcement text — and the
announcement cannot ship until the audit's checklist is signed in the
release pull request.

### 15.5 Implementation tickets and sequence

1. **R10.01 — Build info and version embedding.** Implement
   `build_info.rs`, generated at build time, with the dev fallback and the
   release-mode refusal. Done when version output tests pass for dev and
   release builds and the fallback never appears in a packaged artifact.
2. **R10.02 — Reproducible pipeline.** Implement `release.yml` with pinned
   runner, double-build digest comparison, and signing. Done when two
   pipeline runs from the same tag produce identical artifact digests in CI.
3. **R10.03 — Provenance and SBOM.** Generate in-toto SLSA-style
   attestations and the CycloneDX SBOM; implement
   `relayctl version --verify-provenance` guidance in docs using standard
   verifiers. Done when attestation verification passes against a built
   artifact and fails against a tampered byte.
4. **R10.04 — Static binary and container image.** Enforce musl static
   linking, the scratch-execution test, and the enumerated distroless image.
   Done when the image contains exactly the listed files and both binaries
   run in a scratch container.
5. **R10.05 — Install paths and systemd unit.** Ship `install.sh`, the unit
   file with the hardening directives, and the 0700 data-directory startup
   check integration. Done when an install-boot-serve-uninstall cycle passes
   in a clean VM test and startup refuses a world-readable data directory.
6. **R10.06 — Mixed-version upgrade suite.** Check in the prior-version
   fixture binary, implement the feature-level handshake and window
   refusal, and write the MIGR mixed-cluster test. Done when the mixed
   window drives the full operation surface with zero lost acks and zero
   double-leases, and an out-of-window node is refused with the named error.
7. **R10.07 — Format versioning and downgrade policy.** Implement the
   read-back-one rule, `cluster finalize-upgrade` with its confirmation
   text, and the MIGR old-version data-directory fixtures. Done when every
   fixture recovers to its recorded logical state and pre-finalization
   downgrade passes with the old binary.
8. **R10.08 — Backup and restore.** Implement `cluster backup`,
   `--restore-from`, the backup manifest, and digest verification. Done when
   restore of a live-workload backup reproduces the recorded state hash and
   a corrupted backup file is detected before any state is installed.
9. **R10.09 — Scripted DR drill.** Implement `dr-drill.sh` end to end with
   the MODL comparison and the dated green-run record consumed by the
   release checklist. Done when the drill runs green in CI on schedule and
   its date gate blocks a stale release in a negative test.
10. **R10.10 — Uninstall and purge.** Implement `uninstall.sh` with the
    preserve/purge split, marker-file ownership check, and confirmation
    text. Done when VM tests verify preserved data after plain uninstall,
    complete removal after purge, and refusal on an unowned directory.
11. **R10.11 — Incident response and revocation.** Write
    `docs/INCIDENT_RESPONSE.md`, implement the yank marking in the manifest
    index, and rehearse revocation against a pre-1.0 artifact. Done when the
    rehearsal produces a signed revocation notice and the yanked artifact is
    reported by the documented verification flow.
12. **R10.12 — Final audits and 1.0 definition of done.** Execute the
    threat-model re-review, dependency-provenance audit, documentation
    status audit, and release claims audit; implement the checker that
    mechanically evaluates section 17. Done when the checker passes from a
    clean tagged commit and every audit's record is linked in the release
    pull request.

### 15.6 Test-driven evidence matrix

| Test | First failing condition | Required passing assertion |
| --- | --- | --- |
| MIGR mixed-version cluster | The mixed window loses an ack, double-leases, or refuses in-window traffic. | Full operation surface green across the window; MODL confirms P-09 and P-08; out-of-window join refused with the named error. |
| MIGR old-version fixtures | A prior-version data directory fails to recover to its recorded logical state. | Every fixture (WAL + snapshot + audit) recovers byte-exactly to its recorded state hash. |
| MIGR downgrade rehearsal | Pre-finalization downgrade corrupts state or post-finalization downgrade is misreported as supported. | Old binary serves correctly before finalization; finalize prints the point-of-no-return text and flips write formats cluster-wide. |
| OPSX reproducible build | Two builds of one tag differ in any artifact digest. | Double-build digest equality for every artifact in the manifest. |
| OPSX provenance verify | Attestation verifies against a tampered artifact or fails against a genuine one. | Verification passes for genuine artifacts and fails for a single flipped byte. |
| OPSX static execution | A binary links dynamically or fails in a scratch container. | Both binaries execute in scratch; the container image contains exactly the enumerated files. |
| OPSX install lifecycle | Install, boot, serve, uninstall, or purge deviates from the documented behavior in a clean VM. | Full cycle passes; plain uninstall preserves data; purge removes exactly the named paths; unowned directory refused. |
| OPSX DR drill | Restore diverges from the pre-destruction state or the drill date gate fails to block. | Drill green with matching state hash and MODL history; stale-drill negative test blocks release. |
| OPSX backup corruption | A corrupted backup installs partially before detection. | Digest verification rejects the backup before any state is written. |
| OPSX revocation rehearsal | The yanked artifact still verifies as current through the documented flow. | Revocation notice signed; documented verification reports the yank. |
| MKT release claims audit | Launch collateral contains a claim without resolvable evidence, or the announcement ships unaudited. | Every collateral claim resolves to P-xx/NG-xx/BENCH-xx; the signed checklist is present in the release pull request. |
| OPSX docs status audit | Any document claims a status its gate evidence does not support. | The scripted status pass reports zero unearned claims across the documentation set. |

### 15.7 Failure and security cases

- A reproducibility failure is a release blocker, not a warning: an
  unreproducible artifact cannot carry a truthful attestation.
- The signing key ceremony is documented and the key never enters CI logs or
  the repository; canary scans cover pipeline output.
- The prior-version fixture binary is a test input, never a distribution
  channel; it is digest-pinned and quarantined from release artifacts.
- Restore into a non-empty data directory is refused before any write; the
  operator must move or purge explicitly.
- `finalize-upgrade` is the single most dangerous command in the product; it
  requires typed confirmation of the cluster name and prints the downgrade
  consequence before acting, and the confirmation text is golden-tested.
- Purge refusal on a missing marker file protects against a mistyped path
  deleting unrelated data; the refusal is tested against `/var/lib`.
- The threat-model re-review explicitly revisits the diagnose bundle, backup
  files (which contain message bodies and must be stated as
  operator-secured), and the audit log's tamper-evidence limits.
- A revoked release's artifacts remain downloadable but marked, because
  breaking existing clusters' verification mid-incident is worse than a
  marked artifact; the notice states this.

### 15.8 Migration, documentation, and installation work

R10 is the migration and installation gate; the work in sections 15.4–15.5
is the content. Documentation shipped: `docs/INSTALL.md`, `docs/UPGRADE.md`,
`docs/BACKUP_RESTORE.md`, `docs/INCIDENT_RESPONSE.md`, and updates to
`docs/RUNBOOK.md` linking all four. docs/README.md's precedence and status
blocks are re-verified by the documentation status audit, and the FR-MKT
namespace note required by the spine register is confirmed present.

### 15.9 Acceptance evidence

R10 — and with it Relay 1.0 — is accepted only when:

- every MIGR, OPSX, and MKT row in section 15.6 is green from a clean,
  signed, tagged commit;
- the section 17 release-candidate readiness checklist passes its
  mechanical checker and its human-signed items are recorded in the release
  pull request;
- all prior gates R0–R9 replay green in the same CI run (NFR-MAINT-004);
- the DR drill's green run is dated within 30 days of the tag;
- the threat-model re-review, dependency audit, documentation status audit,
  and release claims audit records are linked and contain no open blocking
  finding;
- the 1.0 definition of done holds: a fresh tier-1 machine can install a
  verified artifact, check its provenance, boot under systemd, serve the
  full queue/topic surface, be backed up and restored, be upgraded and
  rolled back within the window, produce a diagnose bundle, and be
  uninstalled with data preserved and then purged — each step from the
  shipped documentation alone.

### 15.10 Explicit deferrals

R10 defers, with OPEN_QUESTIONS.md entries and fail-closed defaults:
distribution-channel packaging (deb/rpm/Homebrew — default: tarball and
container only); Windows support (excluded by ADR-0011); multi-region
replication (NG-08); an HTTP/JSON gateway (deferred by ADR-0004);
automatic online upgrade orchestration (default: documented manual
procedure only); and encrypted-at-rest storage (default: operator-level
disk encryption guidance, no product claim).

### 15.11 Requirements traced

R10 is the terminal owning gate for FR-REPL-009, FR-OPS-001, FR-OPS-007,
FR-OPS-008, FR-OPS-009, FR-OPS-012, FR-MKT-004, FR-MKT-005, NFR-DUR-007,
NFR-SEC-007, NFR-SEC-008, NFR-MAINT-001, NFR-MAINT-004, and NFR-MAINT-005.
NFR-MAINT-001 closes here because R10's audit confirms the
failing-test-first discipline held at every gate; NFR-MAINT-004 closes here
because the release run replays every prior gate's accepted evidence; and
NFR-SEC-005's startup permission check, owned by R2, is re-verified by the
install lifecycle test without transferring ownership.

## 16. Requirement-to-Evidence Traceability

This matrix lists every requirement ID in the register — all 108 IDs from
./PRODUCT_REQUIREMENTS.md, each exactly once — with its terminal owning gate
and the named evidence that closes it. Earlier gates may begin a
requirement; only the terminal gate may mark it complete. Named evidence
resolves to test-family matrices in ./OPERATIONS_TEST_PLAN.md and to the
report artifacts named in sections 13–15. A requirement absent from this
table, or present twice, fails the planning-acceptance audit in section 18.

| Requirement | Terminal gate | Named evidence |
| --- | --- | --- |
| FR-QUEUE-001 | R1 | CORE-QUEUE create/validate suite; MODL history with CreateQueue ops. |
| FR-QUEUE-002 | R2 | CRSH ack-durability suite; STOR fsync-before-ack barrier tests (P-01). |
| FR-QUEUE-003 | R1 | CORE-BATCH per-entry result suite; MODL batch histories. |
| FR-QUEUE-004 | R1 | CORE-RECV lease suite; MODL lease-exclusivity check (P-02). |
| FR-QUEUE-005 | R1 | CORE-VIS expiry suite driven by AdvanceTime; SIM visibility scenarios. |
| FR-QUEUE-006 | R1 | CORE-DEL idempotency suite; MODL delete histories (P-06). |
| FR-QUEUE-007 | R1 | CORE-RECEIPT epoch/rejection suite; MODL foreign-handle histories. |
| FR-QUEUE-008 | R1 | CORE-VIS change-visibility suite including zero-return path. |
| FR-QUEUE-009 | R6 | WIRE long-poll suite; OPSX wakeup ordering tests; BENCH wakeup context at R9. |
| FR-QUEUE-010 | R4 | CORE-DELAY per-message suite; SIM delay-boundary scenarios. |
| FR-QUEUE-011 | R4 | CORE-DELAY default-delay precedence suite. |
| FR-QUEUE-012 | R1 | CORE-ATTR typed-attribute suite with 10-attribute boundary cases. |
| FR-QUEUE-013 | R1 | CORE-LIMIT 256 KiB boundary suite with stable oversize error. |
| FR-QUEUE-014 | R4 | CORE-RETAIN retention suite; SIM retention-expiry scenarios. |
| FR-QUEUE-015 | R1 | CORE-PURGE suite including concurrent-purge rejection. |
| FR-QUEUE-016 | R1 | CORE-CAP in-flight cap suite (120,000/20,000) with backpressure error. |
| FR-QUEUE-017 | R4 | FIFO/CORE redrive-policy suite; SIM DLQ-move scenarios (P-03). |
| FR-QUEUE-018 | R4 | CORE-DLQ metadata-preservation suite; MODL byte-identity check (P-10). |
| FR-QUEUE-019 | R4 | CORE-REDRIVE task suite with progress reporting; SIM redrive scenarios. |
| FR-FIFO-001 | R4 | FIFO naming/group-id validation suite. |
| FR-FIFO-002 | R4 | FIFO per-group ordering suite; MODL FIFO checker (P-04). |
| FR-FIFO-003 | R4 | FIFO parallel-group delivery suite; SIM multi-group scenarios. |
| FR-FIFO-004 | R4 | FIFO group-blocking suite covering delete and expiry unblock paths. |
| FR-FIFO-005 | R4 | FIFO content-dedup SHA-256 suite. |
| FR-FIFO-006 | R4 | FIFO explicit-dedup-id override suite. |
| FR-FIFO-007 | R4 | FIFO dedup-window boundary suite at exactly 300 s (P-05). |
| FR-FIFO-008 | R4 | FIFO DLQ/redrive ordering-preservation suite; MODL FIFO histories. |
| FR-TOPIC-001 | R5 | TOPC create/validate suite. |
| FR-TOPIC-002 | R5 | TOPC subscribe suite with filter recorded at subscribe time. |
| FR-TOPIC-003 | R5 | TOPC fanout suite; MODL per-subscription independence check (NG-02). |
| FR-TOPIC-004 | R5 | TOPC filter-matching suite: exact, anything-but, prefix, numeric range, exists. |
| FR-TOPIC-005 | R5 | TOPC invalid-filter field-level rejection suite. |
| FR-TOPIC-006 | R5 | TOPC unsubscribe suite with delivered-copy preservation. |
| FR-TOPIC-007 | R5 | TOPC delete-topic suite; subscribed queues unaffected. |
| FR-TOPIC-008 | R5 | TOPC FIFO-fanout suite composing with FIFO group/dedup evidence. |
| FR-API-001 | R6 | WIRE frame codec suite; byte-level golden frames. |
| FR-API-002 | R6 | FUZZ parser corpus in CI; WIRE bounded-allocation suite. |
| FR-API-003 | R6 | WIRE HMAC auth suite with per-frame verification. |
| FR-API-004 | R6 | WIRE ACL suite with deny-precedence matrix. |
| FR-API-005 | R6 | WIRE quota/rate-limit suite with stable throttle error. |
| FR-API-006 | R6 | WIRE error-taxonomy exhaustiveness suite over every failure path. |
| FR-API-007 | R6 | WIRE connection-multiplexing suite: parked poll blocks nothing. |
| FR-API-008 | R6 | WIRE TLS 1.3 suite; plaintext-loopback-only configuration tests. |
| FR-API-009 | R6 | WIRE version-negotiation suite; unknown version rejected pre-state-change. |
| FR-API-010 | R6 | WIRE limit suite: 1 MiB frame, in-flight bound, slowloris deadlines. |
| FR-REPL-001 | R7 | RAFT election suite with pre-vote; SIM partition scenarios. |
| FR-REPL-002 | R7 | RAFT commit-rule suite; SIM majority-append scenarios. |
| FR-REPL-003 | R7 | SIM failover corpus; MODL no-lost-ack check (P-09). |
| FR-REPL-004 | R7 | SIM partition corpus; MODL no-double-lease check (P-08). |
| FR-REPL-005 | R7 | RAFT snapshot-install suite with state-hash verification. |
| FR-REPL-006 | R7 | RAFT single-server membership suite; SIM change-under-failure scenarios. |
| FR-REPL-007 | R7 | RAFT leader-hint suite; client redirect-follow tests. |
| FR-REPL-008 | R7 | RAFT ReadIndex linearizable-read suite; MODL read histories. |
| FR-REPL-009 | R10 | MIGR mixed-version cluster suite with MODL P-08/P-09 verification. |
| FR-ADMIN-001 | R6 | ADMN describe suite with staleness-labeled approximate counts. |
| FR-ADMIN-002 | R6 | ADMN list/pagination suite with prefix filter and cursor stability. |
| FR-ADMIN-003 | R6 | ADMN tag/untag/list-by-tag suite. |
| FR-ADMIN-004 | R6 | ADMN set-attributes suite with validation and bounded propagation. |
| FR-ADMIN-005 | R6 | ADMN delete-queue terminality suite: handles, subscriptions, storage. |
| FR-ADMIN-006 | R8 | ADMN opcode-coverage check; per-leaf human/JSON output goldens. |
| FR-ADMIN-007 | R8 | ADMN cluster members/health/transfer suite; zero-lost-ack transfer test. |
| FR-ADMIN-008 | R8 | ADMN audit-chain suite; truncation-detection and full-disk tests. |
| FR-OPS-001 | R10 | OPSX static-execution and reproducible-build suites; release manifest. |
| FR-OPS-002 | R6 | WIRE/OPSX config-precedence suite; fail-fast startup validation tests. |
| FR-OPS-003 | R8 | OPSX readiness matrix over every ReadyReason including Raft states. |
| FR-OPS-004 | R8 | OPSX metric inventory and cardinality-budget suites. |
| FR-OPS-005 | R8 | OPSX span-shape suite at the in-process OTLP collector. |
| FR-OPS-006 | R8 | OPSX log-schema suite over the closed field and event sets. |
| FR-OPS-007 | R10 | OPSX backup/restore suites; scripted DR drill with dated green run. |
| FR-OPS-008 | R10 | MIGR upgrade/downgrade rehearsal suites; docs/UPGRADE.md procedures. |
| FR-OPS-009 | R10 | OPSX install-lifecycle VM suite: preserve, purge, unowned-refusal. |
| FR-OPS-010 | R8 | OPSX diagnose-redaction canary suite over the fixed bundle contents. |
| FR-OPS-011 | R9 | docs/reports/CAPACITY_MODEL.md with every constant resolved to BENCH runs. |
| FR-OPS-012 | R10 | OPSX revocation rehearsal; docs/INCIDENT_RESPONSE.md procedures. |
| FR-MKT-001 | R9 | MKT claims audit binding positioning to CORRECTNESS.md P-xx/NG-xx IDs. |
| FR-MKT-002 | R9 | MKT audit: every performance claim resolves to a BENCH report. |
| FR-MKT-003 | R9 | MKT non-guarantee placement test (NG-01 on every flagged surface). |
| FR-MKT-004 | R10 | MKT release-collateral audit over site copy, badges, comparison table. |
| FR-MKT-005 | R10 | MKT signed claims-audit checklist in the release pull request. |
| NFR-DUR-001 | R2 | CRSH acked-send survival corpus (P-01); STOR barrier tests. |
| NFR-DUR-002 | R2 | CRSH replay-to-exact-state suite with state-hash comparison. |
| NFR-DUR-003 | R2 | CRSH torn/partial-write corpus; tail-only truncation assertions. |
| NFR-DUR-004 | R2 | CRSH disk-full corpus: clean write failure, reads continue. |
| NFR-DUR-005 | R2 | STOR fsyncgate suite: injected fsync failure aborts the process. |
| NFR-DUR-006 | R2 | STOR compaction live-data and space-reclaim verification suite. |
| NFR-DUR-007 | R10 | MIGR old-version fixtures; downgrade-policy rehearsal and goldens. |
| NFR-PERF-001 | R9 | BENCH throughput report ≥ 20,000 msg/s in bench/results/. |
| NFR-PERF-002 | R9 | BENCH send-to-ack latency report, pooled HDR p99 ≤ 15 ms. |
| NFR-PERF-003 | R9 | BENCH long-poll wakeup report, p99 ≤ 10 ms with NG-05 framing. |
| NFR-PERF-004 | R9 | BENCH 10 GiB recovery report ≤ 30 s over 5 repetitions. |
| NFR-PERF-005 | R9 | BENCH report-schema validator: provenance on every published number. |
| NFR-AVAIL-001 | R7 | SIM one-node-down corpus; RAFT degraded-serving suite. |
| NFR-AVAIL-002 | R9 | BENCH measured-failover report: p99 gap ≤ 5 s over 20 kills, MODL-clean. |
| NFR-AVAIL-003 | R6 | WIRE overload suite: bounded backpressure and shed under flood. |
| NFR-AVAIL-004 | R6 | OPSX graceful-shutdown drain suite. |
| NFR-SEC-001 | R6 | WIRE receipt-forgery suite; HMAC and epoch single-use tests (P-07). |
| NFR-SEC-002 | R6 | FUZZ targets and corpus gating CI on every wire parser. |
| NFR-SEC-003 | R6 | Redaction canary suites over logs, traces, errors, diagnostics. |
| NFR-SEC-004 | R6 | WIRE constant-time credential-comparison test. |
| NFR-SEC-005 | R2 | STOR startup permission check (0700) with refusal test. |
| NFR-SEC-006 | R6 | WIRE DoS-bound suite: memory caps, deadlines, frame and connection limits. |
| NFR-SEC-007 | R10 | Threat-model re-review record linked in the release pull request. |
| NFR-SEC-008 | R10 | cargo-deny exact-pin audit; published SBOM and provenance attestations. |
| NFR-MAINT-001 | R10 | Per-gate failing-test-first audit records, closed by the R10 audit. |
| NFR-MAINT-002 | R3 | SIM seed-replay determinism suite; failing-seed corpus in CI. |
| NFR-MAINT-003 | R4 | MUT relay-core report ≥ 85% mutants killed. |
| NFR-MAINT-004 | R10 | Release CI run replaying every prior gate's accepted evidence. |
| NFR-MAINT-005 | R10 | Scripted documentation status audit with zero unearned claims. |

## 17. Release-Candidate Readiness Checklist

Cutting 1.0 requires every item below. Items marked *(mechanical)* are
evaluated by the R10.12 checker from the tagged commit; items marked
*(signed)* require a named human sign-off recorded in the release pull
request. A single unmet item blocks the tag.

1. *(mechanical)* Gates R0 through R10 each have an accepted status with a
   linked evidence manifest from a clean commit, and the release CI run
   replays every gate's evidence green (NFR-MAINT-004).
2. *(mechanical)* The deterministic simulation corpus — every checked-in
   failing seed from R3 onward — replays to its recorded outcome
   (NFR-MAINT-002); zero flakes, because a deterministic flake is a bug.
3. *(mechanical)* The crash-injection corpus (CRSH torn-write, disk-full,
   fsync-failure) is green against the release binary.
4. *(mechanical)* Mutation testing on relay-core reports ≥ 85% of mutants
   killed (NFR-MAINT-003), with the report artifact attached.
5. *(mechanical)* Every FUZZ target runs its full corpus with zero findings;
   new corpus entries discovered since R6 are checked in.
6. *(mechanical)* The section 16 traceability matrix validates: 108
   requirement IDs, each exactly once, each terminal gate accepted, each
   named evidence artifact resolvable.
7. *(mechanical)* The documentation status audit passes: every document's
   statuses match gate reality, no claim outruns its gate, and the
   docs/README.md precedence order is intact (NFR-MAINT-005).
8. *(signed)* The release claims audit passes over all launch collateral —
   site copy, README badges, comparison table, announcement text — with the
   MKT checklist signed (FR-MKT-004, FR-MKT-005).
9. *(mechanical)* The DR drill's most recent green run is dated within 30
   days of the tag, and its restored state hash and MODL history matched
   (FR-OPS-007).
10. *(mechanical)* The mixed-version upgrade suite is green against the
    prior released version, and the downgrade rehearsal passed
    pre-finalization (FR-REPL-009, NFR-DUR-007).
11. *(mechanical)* Reproducible-build verification: two builds of the tag
    produce identical artifact digests, and every artifact's SLSA-style
    attestation verifies (FR-OPS-001).
12. *(mechanical)* cargo-deny is green with exact pins; the SBOM is
    generated and its digest is in the release manifest (NFR-SEC-008).
13. *(signed)* The threat-model re-review is complete with zero open
    critical or high findings; accepted residual risks are recorded with
    rationale (NFR-SEC-007).
14. *(mechanical)* The install-lifecycle VM suite is green: install, systemd
    boot, serve, backup, restore, upgrade, rollback, plain uninstall with
    data preserved, purge with named-path removal (FR-OPS-009).
15. *(mechanical)* Benchmark reports for NFR-PERF-001 through NFR-PERF-004
    and the measured failover for NFR-AVAIL-002 are committed from reference
    hardware at a commit within the release's minor series.
16. *(mechanical)* The 24-hour SOAK run is green against a release-series
    build with redaction canaries planted.
17. *(mechanical)* Redaction canary suites report zero secret occurrences
    across logs, traces, errors, diagnose bundles, and pipeline output
    (NFR-SEC-003 surface, extended through R8–R10).
18. *(signed)* The revocation procedure has been rehearsed at least once and
    docs/INCIDENT_RESPONSE.md names the current intake channel and key
    holders (FR-OPS-012).
19. *(mechanical)* The release manifest is generated from the clean signed
    tag, all artifacts and digests enumerated, the upgrade window and
    downgrade policy stated.
20. *(signed)* The announcement is withheld until items 1–19 pass; the
    person cutting the tag signs that no claim in the announcement exceeds
    the audited collateral.

## 18. Feature-Exhaustiveness Audit

This audit walks the entire product surface and confirms that every element
is owned by exactly one gate. Ownership means: that gate's acceptance
evidence is the evidence that the element works; other gates may exercise
it but may not claim it. The table is the closed enumeration; the audit
rule follows it.

| Surface element | Owning gate |
| --- | --- |
| CreateQueue with configuration validation | R1 |
| DeleteQueue terminality (handles, subscriptions, storage) | R6 |
| SetQueueAttributes with bounded propagation | R6 |
| DescribeQueue / DescribeTopic with labeled approximate counts | R6 |
| ListQueues / ListTopics with prefix and cursor pagination | R6 |
| Tag, untag, and list-by-tag | R6 |
| SendMessage durability-bound acknowledgment | R2 |
| SendMessageBatch per-entry results | R1 |
| ReceiveMessage leasing and receipt handles | R1 |
| DeleteMessage idempotency | R1 |
| ChangeMessageVisibility including zero-return | R1 |
| Receipt-handle rejection (expired, superseded, foreign, reused) | R1 |
| Visibility expiry and receive-count increment | R1 |
| PurgeQueue with concurrent-purge rejection | R1 |
| Per-queue in-flight caps and backpressure error | R1 |
| Typed message attributes (String, Number, Binary) | R1 |
| 256 KiB body limit with stable oversize error | R1 |
| Per-message DelaySeconds | R4 |
| Per-queue default delay | R4 |
| Retention expiry and removal | R4 |
| Redrive policy and automatic DLQ move | R4 |
| Dead-letter metadata preservation | R4 |
| StartRedriveTask with progress reporting | R4 |
| Long polling semantics (wait, early return) | R6 |
| FIFO `.fifo` naming and MessageGroupId requirement | R4 |
| FIFO per-group ordering equals acknowledged send order | R4 |
| FIFO parallel delivery across distinct groups | R4 |
| FIFO group blocking while a message is in flight | R4 |
| FIFO content-based deduplication (SHA-256) | R4 |
| FIFO explicit MessageDeduplicationId override | R4 |
| FIFO 300 s dedup window boundary behavior | R4 |
| FIFO ordering through DLQ move and redrive-back | R4 |
| CreateTopic validation | R5 |
| DeleteTopic with subscription removal | R5 |
| Subscribe with filter policy recorded at subscribe time | R5 |
| Unsubscribe without affecting delivered copies | R5 |
| Publish fanout: independent per-subscription copies | R5 |
| Filter matching: exact, anything-but, prefix, numeric range, exists | R5 |
| Filter-policy validation with field-level errors | R5 |
| Fanout into FIFO queues preserving group and dedup semantics | R5 |
| RWP/1 frame codec (magic, length, CRC32C, opcode, request ID) | R6 |
| Bounded parsing with fuzz corpus gating CI | R6 |
| Per-tenant HMAC authentication per frame | R6 |
| Per-queue/per-topic ACLs with deny precedence | R6 |
| Per-tenant quotas and rate limits | R6 |
| Stable machine-readable error taxonomy | R6 |
| Long-poll multiplexing without blocking a connection | R6 |
| TLS 1.3 transport and loopback-only plaintext | R6 |
| Protocol version negotiation and rejection | R6 |
| Wire limits: 1 MiB frames, in-flight bounds, slowloris deadlines | R6 |
| Configuration precedence and fail-fast startup validation | R6 |
| Overload backpressure and shed without collapse | R6 |
| Graceful shutdown drain | R6 |
| Raft leader election with pre-vote and randomized timeouts | R7 |
| Majority-durable commit rule | R7 |
| No-lost-ack across leader failover | R7 |
| Lease linearization; no double-lease across partitions | R7 |
| Snapshot install to verified state | R7 |
| Single-server membership changes | R7 |
| Non-leader write rejection with leader hint | R7 |
| Linearizable reads via ReadIndex | R7 |
| Three-node service with one node down | R7 |
| Prometheus metric inventory and cardinality budget | R8 |
| OTLP request span tree | R8 |
| Structured JSON log conventions | R8 |
| Health and readiness endpoints with Raft-aware reasons | R8 |
| relayctl full command tree, human and JSON output | R8 |
| Cluster administration: members, health, leadership transfer | R8 |
| Administrative audit log with tamper-evident chain | R8 |
| relayctl diagnose redacted support bundle | R8 |
| Operator runbook | R8 |
| Published benchmark reports with provenance | R9 |
| Measured failover (leader kill to first new ack) | R9 |
| Failure-injection report with reproducible seeds | R9 |
| Capacity model from measured constants | R9 |
| Positioning, messaging, and non-guarantee placement | R9 |
| Claims registry and CI claims audit | R9 |
| Static binary with embedded version and provenance | R10 |
| Reproducible release pipeline, attestations, SBOM | R10 |
| Container image with enumerated contents | R10 |
| Install paths and hardened systemd unit | R10 |
| Mixed-version rolling upgrade and window enforcement | R10 |
| Format versioning, finalize-upgrade, downgrade policy | R10 |
| Backup, restore, and the scripted DR drill | R10 |
| Uninstall preserving data; purge with ownership check | R10 |
| Incident response and release revocation | R10 |
| Launch collateral and release claims audit | R10 |

Audit rule: this table and the section 16 matrix are jointly exhaustive
over the product surface defined by ./PRODUCT_REQUIREMENTS.md,
./ARCHITECTURE.md's component inventory, the RWP/1 opcode table, the
relayctl command tree, and ./MARKETING.md's deliverable list. Any surface
element — operation, opcode, command, behavior, deliverable — that cannot
be assigned to exactly one owning gate in this table blocks planning
acceptance: the element is either added here with an owner and a
requirement ID, or removed from the product surface, before any gate that
touches it may be accepted. Two gates claiming one element is the same
failure as none claiming it.
