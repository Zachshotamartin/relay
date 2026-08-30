# Relay: System Architecture

Document status: normative target architecture. Nothing described here is
implemented; every component, format, and mechanism below is `planned` and is
earned only at the BUILD_PLAN gate that names it (see [./BUILD_PLAN.md](./BUILD_PLAN.md)).
This document controls component boundaries and formats per the precedence
order in [./README.md](./README.md); accepted ADRs control the decisions they
record; [./CORRECTNESS.md](./CORRECTNESS.md) controls guarantee claims.

Last revised: 2026-08-30.

## 1. Architectural thesis and constraints

Relay is a self-hosted message queue and pub/sub service whose delivery
guarantees are machine-checked by deterministic simulation and model checking,
not asserted in documentation. The architecture exists to make that checking
possible, cheap, and permanent. Every structural decision below follows from
four constraints, and any change that violates one of them requires a new ADR
that supersedes the relevant accepted decision.

**Constraint 1 — every nondeterministic input is injected.** No component
outside the environment implementations may read the wall clock, the monotonic
clock, an entropy source, the filesystem, the network, or spawn a thread
directly. All of these arrive through the trait bundle in §4 (`Clock`, `Rng`,
`Net`, `Disk`, `Spawner`). Production wires a Tokio-backed implementation;
simulation wires a single-threaded virtual-time implementation seeded from one
`u64`. Because the code under test is byte-identical in both wirings, a failure
found by the simulator at seed S is a real bug, and replaying seed S reproduces
it exactly (NFR-MAINT-002). A CI lint (§4.4) rejects any source file outside
the environment crates that names a forbidden API.

**Constraint 2 — one pure state machine.** All queue, topic, FIFO, lease,
dedup, tenant, and administrative semantics live in a single pure function,
`relay_core::apply` (§5). It takes a state value and a log entry and returns a
new state value plus outputs. It performs no IO, reads no clock (time arrives
as `AdvanceTime` log entries per
[./decisions/ADR-0005-injected-time-and-log-applied-clock.md](./decisions/ADR-0005-injected-time-and-log-applied-clock.md)),
draws no randomness, and never mutates its input. This is what makes single-node
Relay, replicated Relay, the simulator, and the reference model agree: they are
all drivers of the same function. The Raft layer (§9) replicates the log that
feeds `apply`; it adds no semantics of its own.

**Constraint 3 — crash-only.** Relay has no clean-shutdown path that
correctness depends on. The only startup path is recovery (§6.7); a graceful
stop is merely a crash with less data to replay. Every durable structure is
designed to be interrupted at any byte: WAL records carry CRC32C and are
truncated only at the tail (NFR-DUR-003), snapshots become visible only by
atomic rename after fsync (§6.6), and an fsync failure aborts the process
rather than retrying against a lying kernel
([./decisions/ADR-0008-fsync-before-ack-durability-contract.md](./decisions/ADR-0008-fsync-before-ack-durability-contract.md),
NFR-DUR-005). Crash injection is therefore a routine test, not an exceptional
event: the simulator kills nodes at arbitrary instruction boundaries between
disk operations and asserts P-01 (DURABLE-ACK) on every recovery.

**Constraint 4 — verification is a component, not a phase.** `relay-sim` and
`relay-model` (§11) are first-class crates with public APIs, versioned
formats (the JSONL history format, §11.3), CI budgets, and their own failing
tests. The correctness properties P-01 through P-10 and the non-guarantees
NG-01 through NG-10 defined in [./CORRECTNESS.md](./CORRECTNESS.md) are the
product; the queue is the artifact that satisfies them. Consequences that shape
everything below:

- State updates are immutable: `apply` returns a new `CoreState` built with
  structurally shared persistent maps, so the simulator can hold arbitrary
  historical states for divergence comparison at O(log n) cost per retained
  version, and torn intermediate states are unrepresentable.
- Everything is bounded: frame sizes, batch sizes, attribute counts, name
  lengths, in-flight caps, channel depths, and parser allocations all have
  fixed limits from spine-level configuration (§10.1, FR-API-002,
  NFR-SEC-006). Unbounded queues are forbidden in every crate.
- Determinism is testable: iteration is over ordered maps only, no floating
  point appears in `relay-core`, and the simulator re-executes seeds and
  compares state hashes to detect any nondeterminism regression (§11.2).
- Formats are versioned from the first byte written (NFR-DUR-007, §12).

## 2. Component boundaries

The Cargo workspace contains exactly the ten crates fixed by the project
identity: `relay-core`, `relay-wal`, `relay-raft`, `relay-sim`, `relay-model`,
`relay-wire`, `relay-server`, `relay-client`, `relay-cli`, `relay-bench`.
Rust stable, edition 2024, MSRV 1.85
([./decisions/ADR-0001-rust-language-and-toolchain.md](./decisions/ADR-0001-rust-language-and-toolchain.md)).
Each subsection states the crate's responsibility, a public API sketch, and its
allowed and forbidden dependencies. The dependency lists are enforced by the R0
architecture check in CI, not by convention.

### 2.1 relay-core — pure deterministic state machine

Responsibility: all message-queue semantics as pure data transformation. Owns
`CoreState`, `Command`, `Output`, `apply`, the environment trait definitions
(§4 — trait *definitions* are pure interfaces; `relay-core` contains no
implementation of them and `apply` cannot receive one), identifier newtypes
(`Nanos`, `WallClock`, `Lsn`, `QueueId`, `MessageId`, `TenantId`), limit
constants, the error-code taxonomy, and the canonical binary encoding of
commands and state (used by the WAL payloads and snapshots).

```rust
// crates/relay-core/src/lib.rs (public surface, abbreviated)
pub fn apply(state: &CoreState, entry: &LogEntry) -> Applied;

pub struct Applied { pub state: CoreState, pub outputs: Vec<Output> }

pub struct LogEntry {
    pub lsn: Lsn,
    pub term: u64,               // Raft term; 0 in single-node mode
    pub client: Option<ClientToken>, // ack-dedup identity (§9.8)
    pub command: Command,
}

pub mod env;      // Clock, Rng, Net, Disk, Spawner trait definitions (§4)
pub mod limits;   // MAX_BODY_BYTES = 262_144, MAX_BATCH = 10, ... (spine §6)
pub mod encoding; // canonical LE binary encode/decode for Command, CoreState
pub mod errors;   // ErrorCode (stable u16 discriminants, FR-API-006)
```

Allowed dependencies: `core`, `alloc`, `std` collections, the `im` persistent
collection crate (exact-pinned per NFR-SEC-008), `sha2` (content dedup,
FR-FIFO-005), `bytes`. FORBIDDEN: any IO crate, `tokio`, `rand`, `std::time`,
`std::fs`, `std::net`, `std::thread`, `std::process`, `std::env`, atomics used
for shared mutability, floating point, `HashMap`/`HashSet` (nondeterministic
iteration order). `relay-core` depends on no other workspace crate.

### 2.2 relay-wal — segmented write-ahead log

Responsibility: durable, crash-truncatable, CRC-verified storage of log
entries and snapshots per the formats in §6
([./decisions/ADR-0002-hand-rolled-segmented-wal.md](./decisions/ADR-0002-hand-rolled-segmented-wal.md)).
Owns segment lifecycle, group commit, recovery scanning, snapshot write/read,
manifest maintenance, compaction, and disk-space accounting.

```rust
// crates/relay-wal/src/lib.rs (public surface, abbreviated)
impl Wal {
    pub fn recover(disk: Arc<dyn Disk>, dir: DiskPath, opts: WalOptions)
        -> Result<(Wal, RecoveredState), WalError>;
    pub fn append(&mut self, records: &[Record]) -> Result<Lsn, WalError>; // buffered
    pub fn sync(&mut self) -> Result<Lsn, WalError>; // durable up to returned Lsn
    pub fn compact(&mut self, upto: Lsn, live: &LiveSet)
        -> Result<CompactionReport, WalError>;
    pub fn write_snapshot(&mut self, state: &SnapshotPayload)
        -> Result<SnapshotId, WalError>;
    pub fn read_snapshot(&self, id: SnapshotId) -> Result<SnapshotPayload, WalError>;
    pub fn space(&self) -> SpaceReport; // §6.9 disk accounting
}

pub struct Record { pub kind: RecordKind, pub lsn: Lsn, pub payload: Bytes }
pub enum RecordKind { Command = 0x01, RaftHardState = 0x02, RaftConfig = 0x03,
                      SnapshotMarker = 0x04, SegmentSeal = 0x05 }
pub struct RecoveredState {
    pub last_durable_lsn: Lsn,
    pub truncated_tail_bytes: u64,       // torn bytes discarded (NFR-DUR-003)
    pub snapshot: Option<SnapshotId>,
    pub raft: RecoveredRaftState,
}
```

Allowed dependencies: `relay-core` (types, encoding, `env::Disk` trait only),
`crc32c`, `bytes`. FORBIDDEN: `std::fs`, `std::io` file APIs, `tokio`, `Net`,
`Clock` (wall time in segment headers is passed in by the caller), `Rng`,
`relay-raft`, `relay-server`. Every byte `relay-wal` touches goes through the
`Disk` trait; this is what makes torn-write and disk-full injection (CRSH-*
family) possible without kernel tricks.

### 2.3 relay-raft — in-house Raft

Responsibility: leader election with pre-vote, log replication, commit-index
advancement, ReadIndex, snapshot install, and single-server membership change
([./decisions/ADR-0003-in-house-raft-implementation.md](./decisions/ADR-0003-in-house-raft-implementation.md)).
Structured as a pure protocol core plus a driver: `RaftCore` is a
sans-IO state machine (messages in, messages/actions out) so the model checker
and simulator can drive it exhaustively; `RaftDriver` binds it to `Net`,
`Clock`, `Rng`, and the WAL.

```rust
// crates/relay-raft/src/lib.rs (public surface, abbreviated)
pub struct RaftCore { /* persistent + volatile Raft state, no IO */ }
impl RaftCore {
    pub fn step(&self, input: RaftInput) -> RaftStep; // pure: new core + actions
}
pub struct RaftStep { pub core: RaftCore, pub actions: Vec<RaftAction> }
pub enum RaftInput {
    Tick(Nanos), Message(NodeId, RaftMessage), Propose(ProposalId, Command),
    ReadIndex(ReadId), StorageAppended(Lsn), SnapshotInstalled(Lsn),
}
pub enum RaftAction {
    AppendToWal(Vec<Record>), SendMessage(NodeId, RaftMessage),
    CommitUpTo(Lsn), BecomeLeader(u64), BecomeFollower(u64, Option<NodeId>),
    RequestSnapshot(NodeId), GrantRead(ReadId, Lsn),
    RejectProposal(ProposalId, NotLeader),
}
pub struct RaftDriver { /* owns RaftCore; wired to Env + Wal (§3.2) */ }
```

Allowed dependencies: `relay-core` (types + env traits), `relay-wal`.
FORBIDDEN: `tokio`, `std::time`, `rand`, `relay-wire`, `relay-server`. Election
randomness comes only from the injected `Rng`; timers only from `Spawner`
sleeps driven by `Clock`.

### 2.4 relay-wire — RWP/1 codec

Responsibility: encode and decode RWP/1 frames and per-opcode bodies (§10)
with every length checked against limits before allocation (FR-API-002), plus
the fuzz targets and corpus that gate CI (FUZZ- family, NFR-SEC-002).

```rust
// crates/relay-wire/src/lib.rs (public surface, abbreviated)
pub struct FrameHeader { pub len: u32, pub crc32c: u32, pub opcode: Opcode,
                         pub flags: FrameFlags, pub request_id: u64 }
pub fn decode_header(buf: &[u8; 20]) -> Result<FrameHeader, WireError>;
pub fn decode_body(op: Opcode, body: &[u8], limits: &WireLimits)
    -> Result<RequestBody, WireError>;
pub fn encode_frame(op: Opcode, flags: FrameFlags, request_id: u64,
                    body: &BodyWriter) -> Bytes;
pub fn verify_auth(frame: &DecodedFrame, session: &SessionAuth,
                   key: &TenantKey) -> Result<(), WireError>; // constant-time (NFR-SEC-004)
```

Allowed dependencies: `relay-core` (types, limits, error codes), `crc32c`,
`bytes`, `subtle` (constant-time comparison), `hmac`/`sha2`. FORBIDDEN: any
IO, `tokio`, allocation proportional to attacker-controlled lengths before
validation, any general-purpose serde on the wire (ADR-0004).

### 2.5 relay-sim — deterministic simulation

Responsibility: the virtual-time single-threaded executor, `SimClock`,
`SimNet`, `SimDisk`, `SimRng`, fault injection, multi-node cluster harness,
trace recording, and divergence detection (§11.1–§11.2). Allowed
dependencies: `relay-core` (env traits), `relay-server` (library target, to
construct whole nodes), `relay-model` (to check histories in-loop),
`relay-client`. FORBIDDEN: `tokio` in the executor path, real sockets, real
files, wall-clock reads anywhere except the top-level wall-budget watchdog.
`relay-server` MUST NOT depend on `relay-sim`; the dependency arrow points
into the server, never out.

### 2.6 relay-model — reference model and linearizability checker

Responsibility: the simplified reference semantics, the JSONL operation-history
format (spine §5, reproduced in §11.3), and the Wing–Gong linearizability
checker with per-queue partitioning
([./decisions/ADR-0007-jsonl-histories-and-linearizability-oracle.md](./decisions/ADR-0007-jsonl-histories-and-linearizability-oracle.md)).
Allowed dependencies: `relay-core` (types), `serde`/`serde_json` (histories are
offline artifacts, not wire traffic), `sha2`. FORBIDDEN: `relay-server`,
`relay-wal`, `relay-raft`, `tokio`. The model must never import the
implementation it judges.

### 2.7 relay-server — relayd

Responsibility: the `relayd` binary and the `Node` library that both
production `main` and `relay-sim` construct: configuration loading
(FR-OPS-002), the process model in §3, the Tokio-backed environment
(`TokioEnv`), TLS termination (FR-API-008), authentication and quota
enforcement at the edge, long-poll parking, metrics/health on 7415
(FR-OPS-003/004), and receipt-handle mint/verify (§8).

```rust
// crates/relay-server/src/lib.rs (public surface, abbreviated)
pub struct Node { /* one relayd instance bound to an Env */ }
impl Node {
    pub fn start(env: Env, config: NodeConfig) -> Result<Node, StartError>;
    pub fn shutdown(self) -> ShutdownReport; // drain, then stop (NFR-AVAIL-004)
}
pub struct TokioEnv;   // production Env implementation (§4.3)
```

Allowed dependencies: `relay-core`, `relay-wal`, `relay-raft`, `relay-wire`,
`tokio` (inside `TokioEnv` and task plumbing only), `rustls`, `prometheus`
client, `toml`. FORBIDDEN: `relay-sim`, `relay-model`, `relay-client`,
`relay-cli`; direct `std::time`/`rand` use outside `TokioEnv` (§4.4 lint).

### 2.8 relay-client — client library

Responsibility: connection management against the `Net` trait (so the same
client runs under simulation), RWP/1 request/response correlation, leader-hint
following and retry rules (§9.8), receipt-handle opacity (clients never parse
handles), and typed operation methods used by `relayctl`, tests, and
`relay-bench`. Allowed dependencies: `relay-core` (types + `env::Net`),
`relay-wire`. FORBIDDEN: `relay-server`, `relay-wal`, `tokio` outside its own
`TokioNet` convenience constructor.

### 2.9 relay-cli — relayctl

Responsibility: every administrative operation with human and JSON output
(FR-ADMIN-006), `relayctl diagnose` (FR-OPS-010), cluster administration
(FR-ADMIN-007). Allowed dependencies: `relay-client`, `relay-core` (types),
`clap`, `serde_json`. FORBIDDEN: `relay-server`, `relay-wal`, `relay-raft` —
the CLI speaks only RWP/1; it has no side door into state.

### 2.10 relay-bench — benchmark harness

Responsibility: the workloads, statistical treatment, and report generation
behind every published number (NFR-PERF-005, BENCH- family). Allowed
dependencies: `relay-client`, `relay-server` (to launch local single-node
targets), `hdrhistogram`. FORBIDDEN: modifying server internals; benchmarks
measure the same binary users run.

### 2.11 Dependency graph

Arrows point from dependent to dependency. Every edge not drawn is forbidden.

```text
relay-cli ──► relay-client ──► relay-wire ──► relay-core
relay-bench ─► relay-client                        ▲
    │                                              │
    └────────► relay-server ──► relay-raft ──► relay-wal ──► relay-core
                    │   │                                        ▲
                    │   └──────► relay-wire ─────────────────────┤
                    └──────────► relay-core ─────────────────────┘
relay-sim ──► relay-server (lib), relay-client, relay-model, relay-core
relay-model ──► relay-core
```

## 3. Process model

### 3.1 One process, fixed task set

`relayd` is a single static binary and a single OS process
([./decisions/ADR-0009-single-static-binary-deployment.md](./decisions/ADR-0009-single-static-binary-deployment.md)).
It runs a fixed, named set of tasks; no task is created per message, and the
per-connection task count is bounded by the connection limit (NFR-SEC-006).
All tasks are spawned through the `Spawner` trait with a static name, which is
what lets the simulator schedule them deterministically and lets metrics
attribute CPU by task.

| Task | Cardinality | Owns | Communicates via |
| --- | --- | --- | --- |
| `accept-api` | 1 | API listener on port 7414 (TLS 1.3) | spawns `conn-*` tasks |
| `conn-<id>` | ≤ max_connections (default 4,096) | one client connection: frame read, auth, decode, response write, long-poll parking | bounded `mpsc` to `proposal`; per-connection reply slots |
| `accept-raft` | 1 | Raft listener on port 7416 | spawns `raftconn-*` |
| `raft-driver` | 1 | `RaftCore`, election/heartbeat timers, peer connections | proposals in; `RaftAction`s out to `wal-commit` and `apply` |
| `wal-commit` | 1 | the `Wal` value; group commit (append, adaptive ≤ 2 ms window, fsync) | append requests in; durable-LSN watermark out |
| `apply` | 1 | the current `CoreState`; calls `relay_core::apply` on committed entries in LSN order | committed entries in; `Output`s routed to waiting connections, long-poll wakeups, metrics |
| `compaction` | 1 | snapshot writing and segment reclamation (§6.8) | reads state snapshots from `apply` via watch channel |
| `timekeeper` | 1 | proposes `AdvanceTime` entries from `Clock::monotonic` (§7.2) | proposals to `raft-driver` |
| `metrics-http` | 1 | health/readiness/Prometheus on port 7415 | read-only views |

The single `apply` task is load-bearing: exactly one task ever holds
`CoreState`, entries apply in LSN order, no lock protects queue semantics, and
replay equals live execution. Throughput comes from pipelining — decode, WAL
fsync, and apply overlap across entries — not from parallel application.

### 3.2 Commit pipeline (single-node and replicated)

```text
conn-<id>: read frame → verify CRC → verify HMAC → decode body → quota check
    → build Command + ClientToken → send Proposal on bounded channel
raft-driver: assign (term, lsn) → RaftAction::AppendToWal + SendMessage(peers)
wal-commit: batch appends → fsync (group commit, ≤ 2 ms adaptive window)
    → advance durable watermark → notify raft-driver (StorageAppended)
raft-driver: on majority durable (single-node: local durable) → CommitUpTo(lsn)
apply: apply(state, entry) → new state + outputs → route outputs:
    ack/response → conn-<id> reply slot → client
    delivery wakeup → parked long-poll receivers on that queue
    dead-letter / purge progress → metrics + audit
```

In single-node mode the raft-driver runs the same code with a one-member
configuration: "majority" is the local durable watermark, so the ack rule is
exactly ADR-0008 (record fsynced before ack, FR-QUEUE-002, NFR-DUR-001). In
replicated mode the ack rule strengthens to majority-durable (§9.4). No code
path acks from memory in either mode.

### 3.3 Simulation vs production: the same code

`Node::start(env, config)` is the only way a Relay node comes into existence.
Production `main` calls it with `TokioEnv` (multi-threaded Tokio runtime, real
sockets, real files, a dedicated blocking thread for fsync). `relay-sim` calls
the identical function with `SimEnv` (§11.1): every task from §3.1 becomes a
future on one OS thread, `Spawner::sleep_until` parks it on the virtual-time
wheel, `SimNet` delivers frames with seeded latency/loss/partition, and
`SimDisk` injects torn writes and disk-full. There is no `#[cfg(sim)]` in
`relay-server`, `relay-wal`, `relay-raft`, or `relay-core`; the difference
between a production cluster and a simulated one is which `Env` value was
passed to the same constructor. This identity is asserted by SIM- tests that
run the production `Node` under both environments against the same scripted
workload and compare state hashes.

### 3.4 Backpressure paths

Every queue between tasks is bounded, and every bound has a defined overflow
behavior (NFR-AVAIL-003 — bounded backpressure and shed, never collapse):

1. Socket → `conn-<id>`: the connection task reads at most one frame ahead per
   in-flight slot; at the in-flight cap (128 requests per connection,
   FR-API-010) it stops reading, letting TCP flow control push back.
2. `conn-<id>` → proposal channel: bounded (default 8,192 proposals). A full
   channel yields `ErrorCode::Throttled` (retryable) to the client after a
   bounded wait of 10 ms; the connection never blocks other connections
   (FR-API-007).
3. Proposal → WAL: the group-commit batch has a byte budget (default 8 MiB);
   when WAL fsync latency rises, batches fill, the proposal channel fills, and
   shed happens at step 2 with `Throttled`, not by unbounded queueing.
4. Per-queue in-flight caps (120,000 standard / 20,000 FIFO, FR-QUEUE-016)
   are enforced inside `apply`, deterministically, returning
   `ErrorCode::InFlightCapExceeded` as an `Output::Rejected`.
5. Long-poll parking is bounded per queue (default 4,096 parked receivers);
   beyond that, `Receive` with a wait returns empty immediately with a
   `parking_exhausted` detail rather than parking (FR-QUEUE-009 is a wait
   *up to*, so an early empty return is contract-legal, NG-05).
6. `apply` → connection reply slots are single-entry per request id; outputs
   for a dead connection are dropped after audit counting — the client's
   retry path (§9.8) covers the lost reply.

Every shed increments a Prometheus counter labeled by shed point (FR-OPS-004).

## 4. The deterministic environment

### 4.1 The Env bundle

All nondeterminism enters through one value, constructed exactly once per
node, in `main` (production) or the simulator (test):

```rust
// crates/relay-core/src/env.rs
#[derive(Clone)]
pub struct Env {
    pub clock: Arc<dyn Clock>,
    pub rng: Arc<Mutex<dyn Rng>>,
    pub net: Arc<dyn Net>,
    pub disk: Arc<dyn Disk>,
    pub spawner: Arc<dyn Spawner>,
}
```

`relay-core::apply` cannot receive an `Env` — its signature takes only state
and entry — so purity of the state machine is structural, not disciplinary.
The traits are defined in `relay-core::env` because trait definitions are pure
interfaces; no implementation lives there.

### 4.2 Spine trait signatures (verbatim)

The following signatures are fixed by the project spine and frozen at gate R1
(§12); they are reproduced verbatim:

```rust
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
```

### 4.3 Extensions to the spine signatures

The extended surface completes the bundle. These types are part of the same
R1 freeze:

```rust
/// Monotonic duration/instant in nanoseconds since an arbitrary per-boot
/// (or per-simulation) origin. Never compared across boots.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Nanos(pub u64);
impl Nanos {
    pub const ZERO: Nanos = Nanos(0);
    pub fn saturating_add(self, d: Nanos) -> Nanos { Nanos(self.0.saturating_add(d.0)) }
    pub fn checked_sub(self, d: Nanos) -> Option<Nanos> { self.0.checked_sub(d.0).map(Nanos) }
}

/// Wall-clock time: nanoseconds since the Unix epoch, informational only.
/// Never drives state-machine decisions (ADR-0005). May jump backwards.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct WallClock(pub i64);

pub trait Spawner: Send + Sync {
    /// Spawn a named task. Names are static so the simulator's schedule and
    /// the metrics task table are stable across runs.
    fn spawn(&self, name: &'static str, fut: BoxFuture<'static, ()>) -> TaskHandle;
    /// Sleep until the given monotonic deadline. In SimEnv this parks the
    /// task on the virtual-time wheel; virtual time advances only when no
    /// task is runnable.
    fn sleep_until(&self, deadline: Nanos) -> BoxFuture<'static, ()>;
    /// Deterministic cooperative yield point.
    fn yield_now(&self) -> BoxFuture<'static, ()>;
}

pub struct TaskHandle { /* opaque; abort() and join() */ }

pub trait Listener: Send + Sync {
    fn accept(&self) -> BoxFuture<'static, Result<(Conn, NodeAddr), NetError>>;
    fn local_addr(&self) -> NodeAddr;
}

pub trait ConnT: Send + Sync {
    fn read(&self, buf_len: u32) -> BoxFuture<'static, Result<Bytes, NetError>>;
    fn write_all(&self, data: Bytes) -> BoxFuture<'static, Result<(), NetError>>;
    fn peer(&self) -> NodeAddr;
    fn close(&self) -> BoxFuture<'static, ()>;
}
pub type Conn = Arc<dyn ConnT>;

/// Opaque file identity issued by a Disk implementation. Carries no OS fd
/// in the type; SimDisk maps it to an in-memory file image.
#[derive(Clone)]
pub struct FileHandle(pub(crate) u64);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiskError {
    NotFound,
    AlreadyExists,
    NoSpace,               // disk-full injection lands here (NFR-DUR-004)
    Io { kind: DiskIoKind }, // read/write/metadata failure classes
    FsyncFailed,           // caller MUST abort the process (NFR-DUR-005)
    InvalidPath,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NetError {
    Refused, Reset, Timeout, Unreachable, Closed, AddrInUse,
}
```

`DiskError::FsyncFailed` is deliberately not retryable by type: `relay-wal`
converts it to `WalError::Fatal` and the caller aborts. The fsyncgate failure
mode — retrying fsync after the kernel has already dropped dirty pages — is
made unrepresentable by policy and asserted by a CRSH- test that injects one
fsync failure and requires process death, never a retry syscall.

### 4.4 Rules for authors, enforced by CI

1. No source file outside `TokioEnv` (in `relay-server`) and `relay-sim` may
   contain `std::time`, `SystemTime`, `Instant::now`, `rand::`, `getrandom`,
   `tokio::time`, `tokio::fs`, `tokio::net`, `std::fs`, `std::net`,
   `std::thread::sleep`, or `std::thread::spawn`.
2. The R0 architecture check enforces this twice: clippy
   `disallowed-methods`/`disallowed-types` in `clippy.toml`, plus a workspace
   lint script that greps the token list with an allowlist of exactly the
   `TokioEnv` module path and `relay-sim`. A hit anywhere else fails CI.
3. `cargo-deny` bans transitive dependencies that reach entropy or clocks in
   library code paths of `relay-core`, `relay-wal`, `relay-raft`,
   `relay-wire`, and `relay-model`.
4. New environment capabilities are added by extending the trait bundle and
   both implementations in the same change, never by direct OS calls "just
   for now". The freeze rules in §12 govern signature changes.

## 5. Core state machine

### 5.1 CoreState

`CoreState` is a persistent (structurally shared) value. All maps are ordered
(`im::OrdMap`, `im::OrdSet`) so iteration is deterministic; hash maps are
forbidden in `relay-core` (§2.1). Cloning `CoreState` is O(1); updating a
message is O(log n) with sharing of untouched subtrees. This is the immutable
update discipline the whole system relies on.

```rust
// crates/relay-core/src/state.rs
#[derive(Clone)]
pub struct CoreState {
    pub format_version: u32,          // snapshot-encoding version (§12)
    pub applied_lsn: Lsn,             // last entry folded into this state
    pub now: Nanos,                   // log-applied clock; moves ONLY on AdvanceTime
    pub queues: OrdMap<QueueId, Queue>,
    pub queue_names: OrdMap<QueueName, QueueId>,
    pub topics: OrdMap<TopicId, Topic>,
    pub topic_names: OrdMap<TopicName, TopicId>,
    pub subscriptions: OrdMap<SubscriptionId, Subscription>,
    pub subs_by_topic: OrdMap<TopicId, OrdSet<SubscriptionId>>,
    pub tenants: OrdMap<TenantId, Tenant>,
    pub tags: OrdMap<ResourceRef, OrdMap<TagKey, TagValue>>,
    pub redrive_tasks: OrdMap<RedriveTaskId, RedriveTask>,
    pub client_dedup: OrdMap<ClientId, ClientAckWindow>, // §9.8 ack dedup
    pub ulid_seed: [u8; 32],          // §8.1 deterministic ULID derivation
}

#[derive(Clone)]
pub struct Queue {
    pub id: QueueId,
    pub name: QueueName,
    pub config: QueueConfig,          // visibility default, delay default,
                                      // retention, redrive policy, caps
    pub created_at: Nanos,
    pub kind: QueueKind,
    pub store: MessageStore,
    pub purge: Option<PurgeTask>,     // FR-QUEUE-015: at most one active
}

#[derive(Clone)]
pub enum QueueKind { Standard, Fifo(FifoState) }

#[derive(Clone)]
pub struct MessageStore {
    /// Every live message body + attributes + metadata, keyed by ULID.
    pub messages: OrdMap<MessageId, Message>,
    /// Deliverable messages ordered by (priority_time, ulid). priority_time
    /// is the instant the message became available (send time, delay expiry,
    /// or visibility-return time), so redelivered messages sort by their
    /// return time, and ULID breaks ties deterministically.
    pub available: OrdSet<(Nanos, MessageId)>,
    /// Delayed messages indexed by the instant they become available.
    pub delayed: OrdSet<(Nanos, MessageId)>,
    /// Active leases keyed by message id.
    pub in_flight: OrdMap<MessageId, Lease>,
    /// Lease-expiry sweep index for AdvanceTime.
    pub in_flight_by_expiry: OrdSet<(Nanos, MessageId)>,
    /// Retention sweep index: (sent_at + retention, id).
    pub retention: OrdSet<(Nanos, MessageId)>,
    pub counts: StoreCounts,          // available/delayed/in_flight totals
}

#[derive(Clone)]
pub struct Message {
    pub id: MessageId,                // ULID (§8.1)
    pub body: Bytes,                  // ≤ 256 KiB (FR-QUEUE-013)
    pub attributes: OrdMap<AttrName, AttrValue>, // ≤ 10, typed (FR-QUEUE-012)
    pub sent_at: Nanos,
    pub receive_count: u32,           // increments on each delivery
    pub group: Option<GroupId>,       // FIFO only
    pub dedup_id: Option<DedupId>,    // FIFO only
    pub dead_letter: Option<DeadLetterMeta>, // FR-QUEUE-018: source queue,
                                             // receive count, move time
}

#[derive(Clone)]
pub struct Lease {
    pub epoch: u64,                   // increments per delivery; receipt
                                      // handles bind to it (§8.2)
    pub expiry: Nanos,                // state.now-relative deadline
    pub consumer: ConsumerId,         // connection-scoped consumer identity
}

#[derive(Clone)]
pub struct FifoState {
    /// Per-group FIFO order: acknowledged send order (FR-FIFO-002).
    pub groups: OrdMap<GroupId, Vector<MessageId>>,
    /// Groups with an in-flight head; later deliveries blocked (FR-FIFO-004).
    pub blocked: OrdSet<GroupId>,
    /// 5-minute dedup ring (FR-FIFO-005..007).
    pub dedup: DedupRing,
}

#[derive(Clone)]
pub struct DedupRing {
    pub by_id: OrdMap<DedupId, DedupEntry>,      // → original MessageId, sent_at
    pub by_expiry: OrdSet<(Nanos, DedupId)>,     // swept at sent_at + 300 s exactly
}

#[derive(Clone)]
pub struct Subscription {
    pub id: SubscriptionId,
    pub topic: TopicId,
    pub queue: QueueId,
    pub raw_policy: Option<FilterPolicy>,        // as supplied (FR-TOPIC-002)
    pub compiled: Option<CompiledFilterPolicy>,  // validated at subscribe time
                                                 // (FR-TOPIC-005); matcher over
                                                 // exact / anything-but / prefix /
                                                 // numeric-range / exists
}

#[derive(Clone)]
pub struct Tenant {
    pub id: TenantId,
    pub key_epochs: OrdMap<u32, KeyFingerprint>, // auth key ids; secret bytes
                                                 // live at the edge, never here
    pub acls: Vector<AclRule>,                   // deny precedence (FR-API-004)
    pub quotas: TenantQuotas,                    // rate/size budgets (FR-API-005)
}

#[derive(Clone)]
pub struct RedriveTask {                          // FR-QUEUE-019
    pub id: RedriveTaskId,
    pub source_dlq: QueueId,
    pub destination: QueueId,
    pub moved: u64,
    pub remaining_hint: u64,
    pub state: RedriveState,                      // Running | Completed | Failed
}

#[derive(Clone)]
pub struct PurgeTask { pub started_at: Nanos, pub purged: u64 }
```

### 5.2 Command and Output

`Command` is fixed by the spine and reproduced verbatim; the payload structs
it references are part of the R1 freeze:

```rust
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
```

Key payloads:

```rust
pub struct SendCommand {
    pub queue: QueueId,
    pub body: Bytes,
    pub attributes: Vec<(AttrName, AttrValue)>,
    pub delay: Option<Nanos>,          // 0–900 s (FR-QUEUE-010)
    pub group: Option<GroupId>,        // required for FIFO (FR-FIFO-001)
    pub dedup_id: Option<DedupId>,     // FR-FIFO-006
}
pub struct ReceiveCommand {
    pub queue: QueueId,
    pub max: u8,                       // 1–10 (FR-QUEUE-004)
    pub visibility: Option<Nanos>,     // per-receive override, 0 s–12 h
    pub consumer: ConsumerId,
}
pub struct DeleteCommand {
    pub queue: QueueId, pub message: MessageId, pub lease_epoch: u64,
}
pub struct ChangeVisibilityCommand {
    pub queue: QueueId, pub message: MessageId, pub lease_epoch: u64,
    pub visibility: Nanos,             // 0 returns immediately (FR-QUEUE-008)
}
```

`Output` extends the spine with the full result vocabulary; every variant is a
fact about a state transition that already happened, never a promise:

```rust
pub enum Output {
    QueueCreated(QueueId), QueueDeleted(QueueId), QueueUpdated(QueueId),
    SendAcked { queue: QueueId, message: MessageId, dedup_of: Option<MessageId> },
    BatchResult { results: Vec<Result<SendAck, ErrorCode>> },      // FR-QUEUE-003
    Delivered { queue: QueueId, deliveries: Vec<Delivery> },       // id, body,
                                                                   // attrs, epoch,
                                                                   // expiry,
                                                                   // receive_count
    ReceiveEmpty { queue: QueueId },   // server may park a long poll on this
    Deleted { queue: QueueId, message: MessageId, already: bool }, // FR-QUEUE-006
    VisibilityChanged { queue: QueueId, message: MessageId, expiry: Nanos },
    ReturnedToAvailable { queue: QueueId, message: MessageId, cause: ReturnCause },
    DeadLettered { from: QueueId, to: QueueId, message: MessageId },
    RetentionExpired { queue: QueueId, message: MessageId },
    Purged { queue: QueueId, removed: u64 },
    RedriveProgress { task: RedriveTaskId, moved: u64, done: bool },
    TopicCreated(TopicId), TopicDeleted(TopicId),
    Subscribed(SubscriptionId), Unsubscribed(SubscriptionId),
    Published { topic: TopicId, fanout: Vec<(SubscriptionId, MessageId)> },
    Tagged(ResourceRef), Untagged(ResourceRef),
    QueueBecameNonEmpty(QueueId),      // edge-triggered long-poll wakeup signal
    DuplicateClientCommand { original: Box<Output> },              // §9.8
    Rejected { code: ErrorCode, detail: ErrorDetail },
}
```

### 5.3 The apply() contract

`apply(state, entry) -> Applied` is governed by five rules; MODL- and MUT-
tests exist for each:

1. **Pure.** No IO, no clock, no randomness, no global state, no logging.
   Everything `apply` knows is in its two arguments.
2. **Total.** `apply` returns for every possible `LogEntry`, including
   malformed or semantically invalid commands: those produce
   `Output::Rejected` with a stable `ErrorCode` (FR-API-006) and a state
   change of exactly `applied_lsn` advancing. `apply` never panics on input;
   a panic is reserved for internal invariant violation, which the simulator
   treats as a checker-level failure.
3. **Deterministic.** Same `(state, entry)` bytes ⇒ same `(state, outputs)`
   bytes, on every platform. No floats, no hash iteration, no pointer-derived
   ordering.
4. **Immutable.** The input state is never modified; the returned state shares
   structure with it. `&CoreState` in, owned `CoreState` out.
5. **LSN-monotonic.** `entry.lsn` must equal `state.applied_lsn + 1`; anything
   else is an internal invariant violation (the WAL and Raft layers guarantee
   ordering; `apply` asserts it).

### 5.4 Message lifecycle transition table

States are exactly `Delayed → Available → InFlight → Deleted`, with
`InFlight → Available`, `InFlight|Available → DeadLettered`, and
`* → Expired` (spine §5). All guards are evaluated against `state.now`.

| # | From | Trigger (command applied) | Guard | To | Side effects |
| --- | --- | --- | --- | --- | --- |
| M1 | (none) | `Send` | body ≤ 256 KiB; attrs ≤ 10; FIFO: group present; dedup miss | `Delayed` if effective delay > 0 else `Available` | ULID minted; retention index entry; FIFO: appended to group vector; dedup ring entry |
| M2 | (none) | `Send` (FIFO dedup hit) | dedup_id in ring, `now < original.sent_at + 300 s` (boundary: at exactly +300 s the entry is already swept, so it is a miss — FR-FIFO-007, P-05) | no new message | `SendAcked{dedup_of: original}` returns the original message ID |
| M3 | `Delayed` | `AdvanceTime(t)` | `t ≥ ready_at` | `Available` | priority_time = ready_at |
| M4 | `Available` | `Receive` | lease cap not exceeded (FR-QUEUE-016); FIFO: group ∉ blocked (FR-FIFO-004) | `InFlight` | lease `{epoch+1, now+visibility, consumer}`; receive_count += 1; FIFO: group added to blocked |
| M5 | `InFlight` | `Delete` | `lease_epoch == lease.epoch` | `Deleted` | message removed from all indexes; FIFO: group unblocked; lease `Consumed` |
| M6 | `InFlight` | `Delete` (repeat, same epoch, message already gone) | delete of an already-deleted handle | `Deleted` (no-op) | `Deleted{already: true}` — idempotent (FR-QUEUE-006, P-06) |
| M7 | `InFlight` | `AdvanceTime(t)` | `t ≥ lease.expiry`; receive_count < maxReceiveCount (or no redrive policy) | `Available` | priority_time = expiry instant; lease `Expired`; FIFO: group unblocked (FR-QUEUE-005) |
| M8 | `InFlight` | `AdvanceTime(t)` | `t ≥ lease.expiry`; redrive policy set; receive_count ≥ maxReceiveCount | `DeadLettered` | moved to DLQ with `DeadLetterMeta`; body/attrs preserved (FR-QUEUE-017/018); FIFO order preserved through the move (FR-FIFO-008) |
| M9 | `InFlight` | `ChangeVisibility(0)` | epoch match | `Available` | immediate return (FR-QUEUE-008); same dead-letter check as M8 applies first |
| M10 | `InFlight` | `ChangeVisibility(v>0)` | epoch match; `v ≤ 12 h` | `InFlight` | lease expiry = `now + v` (lease `Extended`) |
| M11 | `Delayed`/`Available`/`InFlight` | `AdvanceTime(t)` | `t ≥ sent_at + retention` | `Expired` (removed) | FR-QUEUE-014; in-flight expired messages are removed and their lease dies with them |
| M12 | any live state | `Purge` | no purge already active (FR-QUEUE-015) | `Deleted` | all messages including in-flight removed; outstanding handles become invalid |
| M13 | `DeadLettered` (in DLQ, `Available`) | `StartRedrive` progress | task active | `Available` in source queue | receive_count reset to 0; dead_letter meta retained (FR-QUEUE-019) |

Guard-order rule: within one `AdvanceTime` application, sweeps run in the
fixed order retention (M11) → delay promotion (M3) → lease expiry (M7/M8), each
sweep processing its index in key order. This order is part of the frozen
semantics: the model checker replays it exactly.

### 5.5 Lease lifecycle transition table

Lease lifecycle is `Granted → Extended* → (Released | Expired | Consumed)`
(spine §5). A lease exists only while its message is `InFlight`.

| # | From | Trigger | Guard | To | Receipt-handle effect |
| --- | --- | --- | --- | --- | --- |
| L1 | (none) | `Receive` delivers message | see M4 | `Granted` | new handle minted at edge, bound to new epoch (§8.2) |
| L2 | `Granted`/`Extended` | `ChangeVisibility(v>0)` | epoch equality | `Extended` | same handle remains valid; expiry moved |
| L3 | `Granted`/`Extended` | `ChangeVisibility(0)` | epoch equality | `Released` | handle dead: epoch no longer matches any lease |
| L4 | `Granted`/`Extended` | `AdvanceTime ≥ expiry` | — | `Expired` | handle dead; a later redelivery mints epoch+1, so the old handle can never act (FR-QUEUE-007, P-02) |
| L5 | `Granted`/`Extended` | `Delete` | epoch equality | `Consumed` | handle dead; repeat delete is M6 idempotent success |
| L6 | any | command carries stale/foreign epoch | epoch mismatch or unknown message | (unchanged) | `Rejected{ErrorCode::InvalidReceiptHandle}` (FR-QUEUE-007) |

Because the epoch lives inside the committed state and inside the
HMAC-protected handle, single-use enforcement (P-07) needs no per-handle
storage: a handle is valid iff its epoch equals the live lease's epoch, and
every delivery bumps the epoch.

## 6. Storage layout

### 6.1 On-disk directory tree

The data directory (default `/var/lib/relay`, dev `./relay-data`) is owned by
the relay user with mode 0700, verified at startup (NFR-SEC-005). Layout:

```text
<data_dir>/
  meta/
    MANIFEST            # current formats, active snapshot, WAL floor (§6.5)
    MANIFEST.tmp        # staging; never read at recovery
    node.bin            # node id, cluster id, ULID seed, receipt-key epochs
  wal/
    wal-<seq:016x>.seg  # segments, 64 MiB target, monotonic seq
  snap/
    snap-<lsn:016x>.rsnap
  tmp/                  # all staged files before rename; wiped at recovery
```

Every durable mutation follows the same shape: write into `tmp/`, fsync the
file, rename into place, fsync the destination directory. `tmp/` contents are
deleted wholesale during recovery step 2 (§6.7), so a crash mid-stage leaves
no ambiguity.

### 6.2 WAL record layout

The record format is fixed by the spine:

WAL record: `[len u32 LE][crc32c u32][type u8][flags u8][reserved u16][lsn u64][payload len bytes]`;
crc covers type..payload. Segment file: 64 MiB target, name `wal-<seq:016x>.seg`,
4 KiB header: magic `RWALSEG1`, format version u16, segment seq u64, base lsn u64, created wall time, header crc.

Field table (all integers little-endian):

| Offset | Size | Field | Meaning |
| --- | --- | --- | --- |
| 0 | 4 | `len` | Byte count of type..payload, i.e. `12 + payload_len`. Max `1_048_588` (1 MiB payload + fixed 12). |
| 4 | 4 | `crc32c` | CRC32C (Castagnoli) over bytes `[8, 8+len)` — type through end of payload. |
| 8 | 1 | `type` | `0x01` Command, `0x02` RaftHardState, `0x03` RaftConfig, `0x04` SnapshotMarker, `0x05` SegmentSeal. Unknown type ⇒ corruption (recovery step 8). |
| 9 | 1 | `flags` | bit 0: `COMPRESSED` (payload is lz4-framed); bits 1–7 reserved, must be 0. |
| 10 | 2 | `reserved` | Must be 0 on write; must be ignored-if-zero, rejected-if-nonzero on read within format version 1. |
| 12 | 8 | `lsn` | Log sequence number. Strictly `previous + 1` for `Command` records; non-command records carry the current LSN. |
| 20 | `len-12` | `payload` | Type-specific canonical encoding (`relay_core::encoding`). |

Command payloads open with a `cmd_tag u16 LE` identifying the `Command`
variant, followed by that variant's canonical field encoding. `AdvanceTime` is
`cmd_tag 0x0100` + `target Nanos u64 LE`.

### 6.3 Worked byte example

An `AdvanceTime(81_234_000_000 ns)` command record at LSN 42
(`0x2A`): payload is 10 bytes (`cmd_tag` + `u64`), so `len = 22 = 0x16`, and
CRC32C over the 22 bytes type..payload is `0x1AB354EC`. The 30 bytes on disk:

```text
offset  bytes                    field
00      16 00 00 00              len = 22
04      ec 54 b3 1a              crc32c = 0x1AB354EC (LE)
08      01                       type = Command
09      00                       flags = 0
10      00 00                    reserved
12      2a 00 00 00 00 00 00 00  lsn = 42
20      00 01                    cmd_tag = 0x0100 (AdvanceTime, LE)
22      80 78 ec e9 12 00 00 00  target = 81_234_000_000 ns
```

A torn write anywhere in these 30 bytes fails the CRC and, if the record is
the last in the final segment, is truncated at offset 0 of the record
(NFR-DUR-003); anywhere else it is fatal corruption.

### 6.4 Segment header and snapshot layout

Segment header, 4 KiB (offsets in bytes; remainder zero-filled):

| Offset | Size | Field |
| --- | --- | --- |
| 0 | 8 | magic `RWALSEG1` (ASCII) |
| 8 | 2 | format version u16 (currently 1) |
| 10 | 8 | segment seq u64 |
| 18 | 8 | base lsn u64 (LSN of first command record in this segment) |
| 26 | 8 | created wall time (`WallClock` i64 ns; informational only, §7.4) |
| 34 | 4 | header crc32c over bytes 0..34 |
| 38 | 4058 | zero padding to 4096 |

Snapshot file `snap-<lsn:016x>.rsnap`, magic `RSNAP1`, chunked, per-chunk
CRC, footer with full-state SHA-256 (spine §6):

| Region | Layout |
| --- | --- |
| Header (32 B) | magic `RSNAP1\0\0` (8 B) ‖ format version u16 ‖ reserved u16 ‖ snapshot lsn u64 ‖ chunk size u32 (1 MiB) ‖ header crc32c u32 ‖ reserved u32 |
| Chunk × N | `[chunk_len u32][chunk_crc32c u32][chunk bytes]` — consecutive slices of the canonical `CoreState` encoding (`relay_core::encoding`), each ≤ chunk size |
| Footer (48 B) | magic `RSNAPEND` (8 B) ‖ chunk count u32 ‖ footer crc32c u32 ‖ SHA-256 of the complete decoded state encoding (32 B) |

The footer SHA-256 is the same value the simulator uses for divergence
detection (§11.2) and Raft snapshot-install verification (§9.6): one hash
definition, three consumers.

### 6.5 MANIFEST

`meta/MANIFEST` is a single 128-byte binary record, always rewritten whole via
`tmp/` + rename + dir fsync:

| Offset | Size | Field |
| --- | --- | --- |
| 0 | 8 | magic `RMANIFS1` |
| 8 | 2 | manifest format version u16 |
| 10 | 2 | wal format version u16 |
| 12 | 2 | snapshot format version u16 |
| 14 | 2 | reserved |
| 16 | 8 | active snapshot lsn (0 = none) |
| 24 | 8 | wal floor segment seq (segments below this are logically deleted) |
| 32 | 8 | last compaction wall time (informational) |
| 40 | 4 | receipt-key epoch u32 (§8.3) |
| 44 | 76 | zero padding |
| 120 | 4 | crc32c over bytes 0..120 |
| 124 | 4 | reserved |

### 6.6 fsync ordering rules

These rules are the durability contract's mechanical form (ADR-0008); each is
individually tested by a CRSH- case that crashes between the numbered steps:

1. **Record durability precedes ack.** append(records) → `fsync(segment)` →
   only then may any ack for those records leave the process (NFR-DUR-001).
   Group commit batches appends under one fsync with an adaptive window
   capped at 2 ms.
2. **Segment creation precedes use.** create `wal-<seq>.seg` → write 4 KiB
   header → `fsync(file)` → `fsync_dir(wal/)` → only then may records be
   appended and acked from it. A crash before the dir fsync may lose the
   empty segment; that is safe because nothing acked lives there.
3. **Rename publishes, dir fsync commits.** Every rename out of `tmp/`
   (snapshots, MANIFEST) is followed by `fsync_dir(destination)` before the
   renamed file may be referenced by any other durable structure.
4. **MANIFEST ordering.** A snapshot is referenced by MANIFEST only after the
   snapshot's own rename + dir fsync completed (write order: snapshot, then
   MANIFEST). Segment deletion happens only after the MANIFEST naming the new
   floor is durable (delete order: MANIFEST, then segments, then
   `fsync_dir(wal/)`).
5. **fsync failure is fatal.** Any `DiskError::FsyncFailed` aborts the
   process immediately; no retry, no degraded mode (NFR-DUR-005).

### 6.7 Recovery algorithm

Executed by `Wal::recover` on every start; there is no non-recovery start
(crash-only, §1). Numbered steps:

1. Acquire an exclusive advisory lock on `<data_dir>`; a second relayd on the
   same directory fails fast with a distinct error.
2. Verify directory ownership and 0700 mode (NFR-SEC-005); delete everything
   in `tmp/`.
3. Read `meta/MANIFEST`; verify magic and CRC. A missing MANIFEST with an
   empty `wal/` is first boot; a missing or corrupt MANIFEST with a non-empty
   `wal/` is fatal (operator intervention, documented in the runbook).
4. Reject unknown format versions (§12); no best-effort parsing of future
   formats.
5. If MANIFEST names a snapshot: open `snap/snap-<lsn>.rsnap`, verify header,
   every chunk CRC, chunk count, footer CRC, and the footer SHA-256 against
   the decoded state. Any mismatch is fatal — a snapshot is never "partially
   trusted". Decode into `CoreState`; else start from `CoreState::genesis()`.
6. List `wal/`; parse segment names; verify each header magic, version, CRC,
   and that segment seqs are gap-free from the MANIFEST floor. A gap is fatal.
7. Delete any segments below the MANIFEST floor (a crash in compaction step 7
   of §6.8 can leave them); `fsync_dir(wal/)`.
8. Scan records segment by segment in seq order, verifying `len` bounds and
   CRC. On the first invalid record: if it is in the last segment, truncate
   the segment at that record's start offset, `fsync(file)`, and stop the
   scan (torn tail, NFR-DUR-003); if it is in any earlier segment, fail
   fatally (middle corruption is not self-healable and must not be guessed
   away).
9. Rebuild Raft persistent state from the newest `RaftHardState` and
   `RaftConfig` records encountered (§9.3).
10. Replay every `Command` record with `lsn > snapshot lsn` through
    `relay_core::apply`, in LSN order, asserting LSN contiguity. Outputs
    produced during replay are discarded except audit counters — replay must
    not re-ack, re-deliver, or re-fire long polls (NFR-DUR-002).
11. Record `last_durable_lsn`, `truncated_tail_bytes` (0 or the torn count)
    in `RecoveredState`; open a fresh tail segment per rule §6.6-2.
12. Only now may listeners bind. Readiness (FR-OPS-003) reports `ready=false`
    until step 12 completes.

### 6.8 Compaction algorithm

Goal: bound WAL size and recovery time (NFR-PERF-004) without ever removing
live data (NFR-DUR-006). Runs in the `compaction` task; numbered:

1. Trigger when WAL bytes since last snapshot exceed the snapshot threshold
   (default 256 MiB) or the segment count exceeds 64, and no compaction is
   already running.
2. Take the current `CoreState` value from the `apply` task's watch channel
   (O(1): it is a persistent structure) together with its `applied_lsn` = S.
   Raft constraint: S must be ≤ the cluster's committed index; entries above
   the local applied point are never snapshotted.
3. Encode the state; write `tmp/snap-<S>.rsnap` with chunk CRCs and footer
   SHA-256; `fsync(file)`.
4. Rename into `snap/`; `fsync_dir(snap/)`.
5. Compute the new floor: the highest segment seq whose records are ALL
   ≤ S **and** below the minimum LSN any Raft peer still needs for catch-up
   (`LiveSet` from the Raft driver — a lagging follower pins segments until
   snapshot install, §9.6).
6. Rewrite MANIFEST (new snapshot lsn, new floor) via `tmp/` + rename +
   `fsync_dir(meta/)`.
7. Delete segments below the floor; `fsync_dir(wal/)`; delete snapshots older
   than the previous one (retain exactly two: current and previous, for
   backup overlap per FR-OPS-007).
8. Emit a `CompactionReport { snapshot_lsn, segments_deleted, bytes_reclaimed,
   duration }` to metrics and the audit log.

A crash between any two steps is recoverable: before step 6 the old MANIFEST
still governs and the new snapshot is ignored or adopted at the operator's
version; after step 6, recovery step 7 finishes the deletions.

### 6.9 Disk-space accounting

`Wal::space()` returns a `SpaceReport` computed from tracked appends, not
`stat` calls, so simulation and production agree byte-for-byte:

- `wal_bytes` (sum of segment sizes incl. headers), `snap_bytes`,
  `meta_bytes`, `reclaimable_bytes` (segments awaiting step 7).
- A configured budget `storage.max_bytes` (0 = unlimited) is checked before
  each group-commit batch: projected overflow fails the batch's proposals
  with `ErrorCode::StorageExhausted` (retryable=false) while reads, deletes,
  and compaction continue — disk-full fails writes cleanly with no
  corruption (NFR-DUR-004). `DiskError::NoSpace` from the `Disk` trait is
  handled identically even when the budget said otherwise (the OS is the
  final authority).
- The capacity model relating message rate, retention, and disk
  (FR-OPS-011) is published from these same counters by `relay-bench`.

## 7. Time

### 7.1 Wall and monotonic at the edge only

[./decisions/ADR-0005-injected-time-and-log-applied-clock.md](./decisions/ADR-0005-injected-time-and-log-applied-clock.md)
fixes the design: `relay-core` never reads any clock. `state.now` is a
`Nanos` value that changes only when an `AdvanceTime(t)` entry is applied,
and every timer-like behavior — visibility expiry, delay promotion,
retention, the dedup window — is a pure function of `state.now` evaluated
during that application (§5.4 sweep order). `Clock::monotonic` and
`Clock::wall` are consulted only at the edge: in the `timekeeper` task, in
connection deadline enforcement, and in observability timestamps.

### 7.2 AdvanceTime proposal rules

The `timekeeper` task on the leader (or the sole node) proposes
`AdvanceTime` entries under these rules:

1. Tick cadence: propose when `monotonic_now - last_proposed_target ≥ 10 ms`
   (the granularity floor for all timers; NG-04 — expiry is "not before",
   never exact-instant).
2. Monotonic targets: a proposed target must be strictly greater than the
   last proposed target; `apply` treats `AdvanceTime(t)` with
   `t ≤ state.now` as a no-op that still advances `applied_lsn` (total
   function, §5.3 rule 2), so a duplicated or reordered proposal is harmless.
3. Bounded batch effect: one `AdvanceTime` application sweeps every due index
   entry across all queues in the fixed order of §5.4; the sweep is bounded
   per entry (default 10,000 transitions), and an oversized sweep carries
   over to an immediately proposed follow-up `AdvanceTime` with the same
   target — keeping single `apply` calls short and the apply task responsive.
4. In simulation, `SimClock` virtual time drives the same task; time advances
   in the executor only when no task is runnable (§11.1), so a seed fully
   determines the sequence of `AdvanceTime` entries.

### 7.3 Deterministic timers

Because timers fire only via applied entries, every replica computes identical
expiries at identical LSNs: a follower replaying the log reaches the same
lease returns and dead-letter moves the leader computed, with no local clock
involved. This is the mechanism behind the lease-safety argument in §9.7 —
"time passes" is itself a replicated, totally ordered event.

### 7.4 Clock-jump handling

- `Clock::monotonic` is required to be non-decreasing per process by every
  implementation (`TokioEnv` uses a monotonic source; `SimClock` by
  construction). It resets across restarts; nothing persistent stores a raw
  monotonic value except through `state.now`, which is restored by replay.
- `Clock::wall` may jump either direction (NTP step, VM resume). Wall time
  appears only in segment headers, MANIFEST, logs, traces, and
  `DescribeQueue` display fields, all labeled informational. A backwards wall
  jump changes no state-machine behavior and no durability decision.
- `state.now` never regresses: rule §7.2-2 plus the no-op guard make it
  monotone even under leader change, because a new leader's first
  `AdvanceTime` target below the committed `state.now` simply no-ops.
- Simulation includes clock-skew fault injection (per-node monotonic rate
  distortion) to verify that only edge behavior (deadlines, heartbeats)
  shifts, never applied semantics (SIM- family).

## 8. Identifiers and receipt handles

### 8.1 ULID generation rules

Message IDs are ULIDs (128-bit, Crockford base32; spine §6) with the time
component from the log-applied clock
([./decisions/ADR-0006-ulid-ids-and-hmac-receipt-handles.md](./decisions/ADR-0006-ulid-ids-and-hmac-receipt-handles.md)).
Because `apply` may not draw randomness (§5.3), the 80-bit random component
is derived deterministically:

1. `time_ms` (48 bits) = the wall-anchored applied time: `state.now`
   converted to milliseconds against the cluster's genesis wall anchor
   (recorded once in `meta/node.bin` at bootstrap and in the genesis log
   entry, so all replicas share it).
2. `rand` (80 bits) = the first 10 bytes of
   `SHA-256(ulid_seed ‖ lsn_le_u64 ‖ intra_entry_index_le_u16)`, where
   `ulid_seed` is 32 bytes drawn from `Rng` exactly once at cluster
   bootstrap and stored in `CoreState.ulid_seed` (replicated with the
   state). Deterministic per entry, unique per message, and unpredictable
   without the seed.
3. Ordering ties within one millisecond are broken by LSN order, which the
   derivation preserves through the index sets keyed on `(Nanos, MessageId)`.

Every replica therefore mints byte-identical IDs for the same log — required
for P-10 (NO-INVENTION) checking and for dedup answers that return the
original message ID (FR-FIFO-007).

Example ULID from the derivation above (seed and LSN 42 as in §6.3's
worked entry, index 0): bytes `0198f0a1b2c31e9f55dff6bf08e4a1e7`, rendered
`01K3RA3CP33TFNBQZPQW4E98F7`.

### 8.2 Receipt-handle byte layout

Fixed by the spine:

Receipt handle: `rh1_` + base64url( version u8 ‖ queue_id 16B ‖ message_id 16B ‖ lease_epoch u64 ‖ expiry_nanos u64 ‖ HMAC-SHA256 tag 32B ). Single-use: lease_epoch increments each delivery; delete/change-visibility validate epoch equality.

| Offset | Size | Field | Notes |
| --- | --- | --- | --- |
| 0 | 1 | version | `0x01`; selects the key epoch table (§8.3) |
| 1 | 16 | queue_id | binds the handle to one queue (foreign-queue use rejected, FR-QUEUE-007) |
| 17 | 16 | message_id | ULID bytes |
| 33 | 8 | lease_epoch u64 LE | must equal the live lease's epoch (§5.5 L6) |
| 41 | 8 | expiry_nanos u64 LE | advisory copy of the lease expiry; the committed lease is authoritative |
| 49 | 32 | HMAC-SHA256 tag | over bytes 0..49 with the receipt key |

Total 81 bytes → 108 base64url characters (unpadded) → 112-character handle
with the `rh1_` prefix. Handles are minted and verified at the edge
(`relay-server`), never inside `apply`: the server verifies the tag in
constant time (NFR-SEC-004), extracts `(queue_id, message_id, lease_epoch)`,
and proposes `Delete`/`ChangeVisibility` carrying the epoch; `apply` enforces
epoch equality against the committed lease. Forgery is defeated by the HMAC
(P-07); staleness by the epoch; cross-queue replay by the embedded queue id.

### 8.3 Key management

- The receipt key is 32 bytes, generated from `Rng` at cluster bootstrap,
  stored in `meta/node.bin` (mode 0600) on every member, and delivered to
  joining nodes inside snapshot install metadata over the TLS-protected Raft
  channel — never over the client API and never in logs (NFR-SEC-003).
- Rotation: `relayctl cluster rotate-receipt-key` proposes a key-epoch bump;
  MANIFEST records the epoch (§6.5). During a rotation grace window (default
  12 h, the max visibility timeout) the verifier accepts the previous epoch's
  key for tag verification but mints only with the new one; after the window
  the old key is erased. The handle `version` byte namespaces layouts, not
  epochs; the epoch is resolved from cluster state at verification time.
- Keys never appear in `CoreState`, WAL payloads, snapshots, histories, or
  diagnostics; the redaction canary tests (NFR-SEC-003) include a receipt-key
  canary.

### 8.4 Worked example

Inputs: version `0x01`; queue_id `0198f09a4d21a3e08c11b2fa77c4d901`;
message_id `0198f0a1b2c31e9f55dff6bf08e4a1e7` (the ULID from §8.1);
lease_epoch 3; expiry 81,264,000,000 ns; test-only receipt key
`600d5eed` repeated 8× (32 bytes — documentation value, never a real key).

- Preimage (49 bytes):
  `010198f09a4d21a3e08c11b2fa77c4d9010198f0a1b2c31e9f55dff6bf08e4a1e70300000000000000003cb6eb12000000`
- HMAC-SHA256 tag:
  `4041f0993ad053e1461957daa7cc0516ce9fe71d18cf85f7274b80208774ee96`
- Handle (112 chars):
  `rh1_AQGY8JpNIaPgjBGy-nfE2QEBmPChssMen1Xf9r8I5KHnAwAAAAAAAAAAPLbrEgAAAEBB8Jk60FPhRhlX2qfMBRbOn-cdGM-F9ydLgCCHdO6W`

A consumer that deletes with this handle after the message was redelivered
(epoch now 4) receives `ErrorCode::InvalidReceiptHandle`: the tag verifies,
but the epoch comparison inside `apply` fails (transition L6). This is the
single-use property, enforced by committed state rather than handle
bookkeeping.

## 9. Replication design

### 9.1 Raft atop the same apply()

Replication ([./decisions/ADR-0003-in-house-raft-implementation.md](./decisions/ADR-0003-in-house-raft-implementation.md))
adds exactly one thing: agreement on the order of the log that feeds
`relay_core::apply`. The state machine, the WAL formats, the outputs, and the
timers are unchanged from single-node operation; a follower is a node whose
`apply` task consumes committed entries it did not propose. Roles are the
standard three — Leader, Candidate (with Pre-Candidate), Follower — held by
`RaftCore` (§2.3) as pure state.

### 9.2 Parameters (fixed)

Pre-vote on; heartbeat 100 ms; election timeout 500–1000 ms randomized (via
injected `Rng`, in simulated time under `SimEnv`); ReadIndex reads;
single-server membership change; snapshot chunk 1 MiB (spine §6). Raft
traffic runs on port 7416 over mutually authenticated TLS 1.3.

### 9.3 Persistent Raft state in the WAL

Raft reuses the WAL rather than adding a second durable store. Record types
(§6.2):

- `0x02 RaftHardState` — payload `term u64 ‖ voted_for_present u8 ‖
  voted_for NodeId 16B ‖ commit_hint u64`. Written before any vote is cast or
  any higher term is acknowledged; the newest record wins at recovery
  (§6.7 step 9).
- `0x03 RaftConfig` — payload: member count u8 ‖ members (NodeId 16B ‖ addr
  len-prefixed) ‖ config index u64. Written when a membership-change entry is
  appended (not when committed), per single-server change rules.
- `0x01 Command` records ARE the Raft log entries: `LogEntry.term` rides in
  the payload's entry header, and `lsn` is the Raft log index. There is no
  translation layer between "Raft entries" and "WAL records".
- `0x04 SnapshotMarker` — records that a snapshot at LSN S was installed,
  bounding recovery scans after a follower catch-up via snapshot.

### 9.4 Commit and apply pipeline

```text
client ──RWP──► leader conn task ──► proposal channel
                                          │
                          raft-driver: assign (term, index=lsn)
                                          │
              ┌───────────────────────────┼───────────────────────────┐
              ▼                           ▼                           ▼
      local wal-commit          AppendEntries → peer A      AppendEntries → peer B
      append + fsync            peer wal append + fsync     peer wal append + fsync
              │                           │                           │
              └────────── acks ───────────┴───────────────────────────┘
                                          │
                     majority durable ⇒ commit index := lsn
                                          │
                          apply task: apply(state, entry)
                                          │
                        Output routed to the awaiting conn task
                                          │
                                   ack to client
```

The ack rule strengthens ADR-0008: in a cluster, an ack requires the record
fsynced on a majority (FR-REPL-002), then applied on the leader. Followers
apply as their commit index advances; follower outputs are discarded except
metrics (only the leader owns client conversations).

### 9.5 ReadIndex

Linearizable reads (FR-REPL-008) without writing to the log:

1. Leader records `read_index := commit_index` and a `ReadId`.
2. Leader confirms leadership with one heartbeat round acknowledged by a
   majority (or piggybacked on in-flight heartbeats within one interval).
3. When `applied_lsn ≥ read_index`, the read executes against the current
   `CoreState` value and returns.
4. Non-leaders forward ReadIndex to the leader or reply with a leader hint;
   `DescribeQueue` approximate counts (FR-ADMIN-001) may instead be served
   follower-local, labeled with their staleness.

### 9.6 Snapshot install and membership change

Snapshot install (FR-REPL-005): a follower whose next-needed index is below
the leader's WAL floor receives the active `.rsnap` in 1 MiB chunks over the
Raft channel, writes them to `tmp/`, verifies every chunk CRC and the footer
SHA-256, renames into `snap/`, writes a `SnapshotMarker`, resets its state to
the decoded snapshot, and resumes AppendEntries from S+1. A failed hash
discards the whole transfer; there is no partial adoption.

Membership change (FR-REPL-006) is single-server at a time: `AddNode` /
`RemoveNode` are `RaftConfig`-carried transitions; a second change is
rejected until the first commits. New nodes join as non-voting learners
until caught up (within 1,024 entries of the leader), then are promoted by a
second config entry. `relayctl cluster` (FR-ADMIN-007) drives this.

### 9.7 Lease safety across partitions (P-08 / P-09)

The argument that no double-lease exists across any partition, and no
acknowledged write is lost:

1. Every lease grant is a committed `Receive` log entry (FR-REPL-004); every
   lease return is a committed `AdvanceTime` (expiry) or `ChangeVisibility`
   application. Leases have no existence outside `CoreState`.
2. A deposed leader cannot commit: commitment requires majority fsync, and a
   majority has moved to a higher term. So a leader isolated by a partition
   can neither grant a new lease nor observe an expiry — its proposals never
   apply anywhere.
3. Expiry is not a local timeout. A minority-side leader's `timekeeper`
   proposes `AdvanceTime` entries that cannot commit, so `state.now` on the
   majority side is the only clock that moves, and only the majority-side
   leader can re-deliver after expiry. Two live leases on one message would
   require two conflicting committed entries at the same state — impossible
   under Raft's single committed history (P-08, NO-SPLIT-LEASE).
4. The epoch check (§5.5 L6) closes the client-facing gap: a consumer on the
   minority side holding a stale handle finds, after the partition heals,
   that the epoch has advanced if the message was redelivered; its `Delete`
   is rejected rather than silently deleting another consumer's delivery.
5. No lost ack (P-09, FR-REPL-003): an ack is sent only after commit
   (majority durable). Raft leader-completeness guarantees every future
   leader's log contains all committed entries, so failover replays the
   acked send into the new leader's applied state. The SIM-RAFT suite
   crashes leaders between fsync and ack, between ack and follower apply,
   and during partitions, and asserts both properties on every seed.

### 9.8 Client interaction

- **Leader hints (FR-REPL-007).** A non-leader answering any write returns
  `ErrorCode::NotLeader` with the current leader's address in the error body
  (§10.5); `relay-client` re-resolves and retries against the hint, with
  exponential backoff (base 50 ms, cap 2 s, jitter from the client's own
  entropy) and a bounded redirect count (4) before surfacing the error.
- **Retry rules.** Retryable errors: `NotLeader`, `Throttled`, `Timeout`,
  connection loss before response. Non-retryable: validation, authorization,
  `InvalidReceiptHandle`, `StorageExhausted`.
- **Ack dedup by client token.** Every mutating request carries a
  `ClientToken { client_id: u128, seq: u64 }` (§10.4). `CoreState.client_dedup`
  keeps a bounded per-client window (last 1,024 seqs, expiring after 300 s of
  `state.now`). A retried proposal whose token is in the window returns
  `Output::DuplicateClientCommand` wrapping the original result — the client
  gets the original message ID, not a duplicate send. This gives effectively
  exactly-once *acknowledgement* for retries while delivery remains
  at-least-once (NG-01 stands; consumers must still be idempotent).

## 10. Wire protocol

### 10.1 RWP/1 frame layout

Fixed by the spine
([./decisions/ADR-0004-rwp-binary-protocol-no-sqs-compat.md](./decisions/ADR-0004-rwp-binary-protocol-no-sqs-compat.md)):

RWP/1 frame: `[magic "RWP1" 4B][len u32 LE (max 1 MiB)][crc32c u32][opcode u16][flags u16][request_id u64][body]`;
bodies are per-opcode fixed field layouts with length-prefixed variable fields; no general-purpose serde on the wire.

| Offset | Size | Field | Rule |
| --- | --- | --- | --- |
| 0 | 4 | magic | ASCII `RWP1`; mismatch closes the connection |
| 4 | 4 | len u32 LE | byte count of opcode..body (= 12 + body len); max 1,048,588; checked before any body allocation (FR-API-002, FR-API-010) |
| 8 | 4 | crc32c | over bytes opcode..body |
| 12 | 2 | opcode u16 LE | table §10.2 |
| 14 | 2 | flags u16 LE | bit 0 `RESPONSE`; bits 1–15 reserved, must be 0 |
| 16 | 8 | request_id u64 LE | client-chosen, strictly increasing per connection (§10.4); responses echo it |
| 24 | len−12 | body | per-opcode layout (§10.3) |

Variable fields inside bodies are length-prefixed: `u16 len` for names/ids,
`u32 len` for bodies/payloads, each validated against `relay-core::limits`
before allocation. Responses reuse the request opcode with `RESPONSE` set;
failures use opcode `0xFFFF` with `RESPONSE` set and the same `request_id`.

### 10.2 Opcode table

| Opcode | Name | Opcode | Name |
| --- | --- | --- | --- |
| 0x0001 | Hello | 0x0023 | DescribeQueue |
| 0x0002 | Ping | 0x0024 | ListQueues |
| 0x0003 | Goodbye | 0x0025 | PurgeQueue |
| 0x0010 | SendMessage | 0x0026 | StartRedriveTask |
| 0x0011 | SendMessageBatch | 0x0027 | DescribeRedriveTask |
| 0x0012 | Receive | 0x0030 | CreateTopic |
| 0x0013 | Delete | 0x0031 | DeleteTopic |
| 0x0014 | DeleteBatch | 0x0032 | Subscribe |
| 0x0015 | ChangeVisibility | 0x0033 | Unsubscribe |
| 0x0020 | CreateQueue | 0x0034 | Publish |
| 0x0021 | DeleteQueue | 0x0035 | DescribeTopic |
| 0x0022 | SetQueueAttributes | 0x0036 | ListTopics |
| 0x0040 | TagResource | 0x0041 | UntagResource |
| 0x0042 | ListByTag | 0x0050 | ClusterInfo |
| 0xFFFF | Error | | |

Unknown opcodes yield `Error{UnknownOpcode}` without closing the connection.
Opcode numbers are frozen at R6 and never reused (§12).

### 10.3 Per-opcode bodies (the eight most important)

All integers LE. `str16` = `u16 len ‖ UTF-8 bytes`; `bytes32` = `u32 len ‖
bytes`; `attrs` = `u8 count ‖ count × (str16 name ‖ u8 type{1=String,
2=Number, 3=Binary} ‖ bytes32 value)`.

**Hello (0x0001), request** — the only unauthenticated frame:

| Field | Type | Meaning |
| --- | --- | --- |
| min_version | u16 | lowest RWP version the client accepts (1) |
| max_version | u16 | highest offered (1) |
| tenant_key_id | u32 | key epoch of the tenant credential |
| tenant_id | str16 | tenant name |
| client_id | u128 | client identity for ack dedup (§9.8) |
| client_nonce | 16 B | random bytes from the client |
| hello_mac | 32 B | HMAC-SHA256(tenant key, `RWP1` ‖ min ‖ max ‖ tenant_key_id ‖ tenant_id ‖ client_id ‖ client_nonce) |

**Hello response:** `chosen_version u16 ‖ session_id u64 ‖ server_nonce 16 B ‖
server_mac 32 B` where `server_mac` = HMAC(tenant key, session_id ‖
client_nonce ‖ server_nonce), proving the server holds the key too.

**SendMessage (0x0010), request:** `queue str16 ‖ delay_ms u32 (0 =
queue default; sentinel 0xFFFFFFFF = explicit zero) ‖ group str16 (len 0 =
none) ‖ dedup_id str16 (len 0 = none) ‖ attrs ‖ body bytes32 ‖ auth (§10.4)`.
Response: `message_id 16 B ‖ dedup_of_present u8 ‖ dedup_of 16 B`.

**Receive (0x0012), request:** `queue str16 ‖ max u8 (1–10) ‖ visibility_ms
u32 (0 = queue default) ‖ wait_ms u32 (0–20,000; long poll, FR-QUEUE-009) ‖
auth`. Response: `u8 count ‖ count × (message_id 16 B ‖ receipt str16 ‖
receive_count u32 ‖ sent_at_wall i64 ‖ attrs ‖ body bytes32)`.

**Delete (0x0013), request:** `queue str16 ‖ receipt str16 ‖ auth`.
Response: `already_deleted u8` (idempotent success signal, FR-QUEUE-006).

**ChangeVisibility (0x0015), request:** `queue str16 ‖ receipt str16 ‖
visibility_ms u32 ‖ auth`. Response: `new_expiry_wall i64` (informational
wall rendering of the committed expiry).

**Publish (0x0034), request:** `topic str16 ‖ attrs ‖ body bytes32 ‖ auth`.
Response: `matched u16 ‖ matched × (subscription_id 16 B ‖ message_id 16 B)`
(per-subscription copies, FR-TOPIC-003).

**Subscribe (0x0032), request:** `topic str16 ‖ queue str16 ‖
filter_policy bytes32 (canonical filter encoding; len 0 = none) ‖ auth`.
Response: `subscription_id 16 B`. Invalid policies return
`Error{InvalidFilterPolicy}` with field-level detail (FR-TOPIC-005).

**Error (0xFFFF), response body:**

| Field | Type | Meaning |
| --- | --- | --- |
| code | u16 | stable `ErrorCode` discriminant (FR-API-006) |
| retryable | u8 | 0/1 per §9.8 taxonomy |
| leader_hint | str16 | non-empty only for `NotLeader` (FR-REPL-007) |
| message | str16 | human-readable, secret-free (NFR-SEC-003) |
| detail_count | u8 | field-level details |
| details | count × (str16 field ‖ str16 problem) | validation specifics |

### 10.4 Authentication and replay protection

Per-tenant HMAC on every frame (FR-API-003): after Hello, every request body
ends with an `auth` block: `mac 32 B` = HMAC-SHA256(tenant key,
`session_id u64 ‖ request_id u64 ‖ opcode u16 ‖ body-without-mac`). The
server verifies in constant time (NFR-SEC-004) before decoding the rest of
the body beyond bound checks. Replay is defeated by binding to the
per-connection `session_id` (unguessable across connections via the nonce
exchange) and requiring `request_id` strictly increasing per connection; a
regression or reuse closes the connection with `Error{AuthReplay}`.
Authorization (per-queue/per-topic ACL, deny precedence, FR-API-004) and
quota checks (FR-API-005) run at the edge before a proposal is created, so
rejected requests never consume log space.

### 10.5 Version negotiation and connection lifecycle

1. TCP accept on 7414 → TLS 1.3 handshake (FR-API-008; plaintext only via
   explicit loopback configuration) → connection task spawned.
2. The first frame must be Hello within 5 s or the connection closes. Version
   selection: highest version in `[min, max] ∩ server-supported`; empty
   intersection returns `Error{UnsupportedVersion}` and closes before any
   state change (FR-API-009).
3. Steady state: at most 128 in-flight requests per connection; responses may
   complete out of order (long polls do not block unrelated requests on the
   same connection, FR-API-007) and are correlated by `request_id`.
4. Deadlines: 30 s read-progress deadline (a frame begun must complete;
   slowloris defense, FR-API-010), write deadline 30 s, idle timeout 300 s
   answered by client Ping (0x0002).
5. Goodbye (0x0003) drains: the server completes in-flight requests, unparks
   the connection's long polls with empty results, then closes
   (NFR-AVAIL-004).
6. Per-connection memory is capped (frame buffer + in-flight bodies ≤ 8 MiB);
   breach sheds the connection with `Error{Throttled}` (NFR-SEC-006).

## 11. Verification apparatus architecture

### 11.1 relay-sim

`relay-sim` runs entire `Node`s (§3.3) on one OS thread under virtual time.
Public surface:

```rust
// crates/relay-sim/src/lib.rs (public surface, abbreviated)
pub struct SimConfig {
    pub seed: u64,
    pub nodes: u8,                    // 1 (R1–R6 suites) or 3/5 (R7)
    pub workload: WorkloadPlan,       // scripted or generative client ops
    pub faults: FaultPlan,            // schedule of FaultEvent
    pub max_virtual: Nanos,           // virtual-time budget
    pub max_wall: Duration,           // wall watchdog (the one legal wall read)
    pub check: CheckMode,             // InLoop(model) | RecordOnly
}
pub struct Sim { /* executor, nodes, injectors, trace ring */ }
impl Sim {
    pub fn new(cfg: SimConfig) -> Sim;
    pub fn run(self) -> SimReport;
}
pub struct SimReport {
    pub seed: u64,
    pub verdict: Verdict,             // Pass | PropertyViolation(PropertyId, TraceSlice)
                                      // | Diverged(DivergenceReport) | BudgetExhausted
    pub history_path: Option<PathBuf>,// JSONL history for offline checking
    pub state_hashes: Vec<(Lsn, [u8; 32])>, // checkpoint hashes (§6.4 footer hash)
}
pub enum FaultEvent {
    Partition { groups: Vec<Vec<NodeId>> }, Heal,
    CrashNode(NodeId), RestartNode(NodeId),
    NetDrop { link: (NodeId, NodeId), p_millionths: u32 },
    NetDelay { link: (NodeId, NodeId), extra: Nanos },
    NetDuplicate { link: (NodeId, NodeId), p_millionths: u32 },
    TornWrite { node: NodeId, nth_append: u64, keep_bytes: u32 },
    DiskFull { node: NodeId, free_bytes: u64 },
    FsyncFail { node: NodeId, nth_fsync: u64 },
    ClockSkew { node: NodeId, rate_ppm: i32 },
}
```

Mechanics, numbered:

1. **Virtual-time executor.** All tasks are futures polled by one scheduler.
   When every task is blocked, the executor pops the earliest deadline from
   the timer wheel and jumps virtual time to it — a 14-day retention test
   runs in milliseconds of wall time. Runnable-task selection is drawn from
   the scheduler's seeded stream, exploring interleavings across seeds.
2. **Seed → schedule derivation.** The `u64` seed expands via SplitMix64 into
   independent ChaCha20 streams per component: `sched`, `net`, `disk`,
   `workload`, `faults`, and per-node `rng` (feeding each node's injected
   `Rng`). Component isolation means adding a fault type does not perturb
   the scheduler stream, keeping old corpus seeds meaningful.
3. **Trace recording.** Every schedule decision, fault firing, message
   delivery, disk op, and applied LSN is appended to a bounded trace ring;
   on any non-Pass verdict the ring is flushed with the seed to an artifact
   the CI job uploads. The checked-in failing-seed corpus replays in CI
   forever (NFR-MAINT-002, gate R3).
4. **Divergence detection.** Runs mirror `state_hashes` at fixed LSN
   checkpoints against (a) a second execution of the same seed and (b) peer
   nodes at identical applied LSNs. Any mismatch is `Verdict::Diverged` —
   nondeterminism or replica divergence is a build-failing bug even when no
   property was violated.

### 11.2 relay-model

The reference model is a deliberately naive reimplementation of queue
semantics (hundreds of lines, no indexes, no performance) plus the checker:

```rust
// crates/relay-model/src/lib.rs (public surface, abbreviated)
pub struct ModelState { /* naive maps only */ }
pub fn model_apply(state: &ModelState, call: &Call) -> (ModelState, ModelResult);

pub struct History { pub ops: Vec<HistoryOp> }       // parsed JSONL (§11.3)
pub struct CheckerConfig {
    pub budget: Duration,             // wall budget per history (ADR-0007)
    pub partition_by_queue: bool,     // default true
}
pub fn check(history: &History, cfg: &CheckerConfig) -> CheckOutcome;
pub enum CheckOutcome {
    Linearizable,
    Violation { minimal_prefix: Vec<OpId>, explanation: String }, // MODL evidence
    BudgetExhausted { ops_explored: u64 },            // inconclusive ≠ pass
}
```

The checker is Wing–Gong over the reference model
(ADR-0007): it searches for a linearization order of overlapping operations
whose sequential execution through `model_apply` reproduces every observed
result. Per-queue partitioning: operations on distinct queues commute (NG-02
— no cross-queue atomicity is claimed), so a history splits into independent
per-queue sub-histories checked separately, with memoized explored-state sets
bounding the search. `BudgetExhausted` is reported distinctly and a CI
budget-exhaustion is a test-suite sizing bug, never silently treated as a
pass. P-10 (NO-INVENTION) is checked structurally on the history before the
search: every delivered `body_sha256` must match a prior send.

### 11.3 JSONL history format

One operation per line, exactly as fixed by the spine:

```json
{"op":42,"client":3,"call":{"type":"receive","queue":"q1","max":1,"visibility_s":30},
 "invoke_ns":81234000,"return_ns":81239000,
 "result":{"ok":{"messages":[{"id":"01J...","receipt":"rh1_...","body_sha256":"..."}]}},"seed":"0xDEADBEEF"}
```

`invoke_ns`/`return_ns` are virtual-time bounds of the operation interval;
`seed` ties every history line to its reproducing simulation. Histories are
produced by `relay-sim` (in-loop or recorded) and by the R6+ live-cluster
harness through `relay-client` instrumentation, and consumed only by
`relay-model` — the format is the contract between them, frozen at R3 (§12).

## 12. Interface stability and versioning

### 12.1 Freeze schedule

An interface is *frozen* when its gate's acceptance evidence lands; after
that, changing it requires a superseding ADR plus the migration work of
NFR-DUR-007 (old-version fixtures, MIGR- tests, stated downgrade policy).

| Interface | Frozen at | Version field |
| --- | --- | --- |
| `env` traits (§4), `apply` signature, `Command`/`Output`, `CoreState` semantics | R1 | snapshot `format_version` (state encoding) |
| WAL record/segment formats, MANIFEST, snapshot RSNAP1 (§6) | R2 | segment header u16, snapshot header u16, MANIFEST u16 triplet |
| JSONL history format, `SimConfig`/`SimReport`, seed-derivation scheme (§11) | R3 | `"v"` field added to history lines only by bump; seed scheme changes invalidate the corpus and are ADR-level |
| FIFO/dedup/DLQ transition tables (§5.4–5.5 rows M2, M8, M13) | R4 | covered by state encoding version |
| Filter-policy compiled semantics (§5.1 `CompiledFilterPolicy`) | R5 | policy encoding version u16 inside `filter_policy` blob |
| RWP/1 frames, opcodes, bodies, auth block, error codes (§10) | R6 | protocol version negotiated in Hello |
| Raft WAL record payloads, snapshot-install metadata (§9.3, §9.6) | R7 | rides WAL format version; mixed-version window per FR-REPL-009 |
| Receipt-handle layout (§8.2) | R6 | leading version byte (`0x01`) |

### 12.2 Forbidden-change rules

1. Numeric identifiers are never reused: WAL record types, RWP opcodes,
   `ErrorCode` discriminants, and `cmd_tag`s are retired, not recycled.
2. CRC coverage ranges, magic strings, and field offsets within a published
   format version never change; any layout change is a version bump with a
   new magic-adjacent version value and MIGR- fixtures for every prior
   version still inside the support window.
3. Within a version, changes must be additive and ignorable: new optional
   trailing fields may be appended only where a length prefix already scopes
   the structure; reserved fields become meaningful only in a new version.
4. Readers reject unknown *higher* versions with a stable error rather than
   best-effort parsing (recovery step 4, Hello negotiation); writers never
   emit a version the local build cannot itself read back.
5. The frozen `env` trait signatures may gain new traits in the bundle but
   existing method signatures never change — the simulator's corpus validity
   depends on it.
6. A semantic change to any transition-table row in §5.4/§5.5 is a
   correctness-property event: [./CORRECTNESS.md](./CORRECTNESS.md), the
   reference model, and the model corpus change together, and any marketing
   claim bound to the old behavior is invalidated (precedence,
   [./README.md](./README.md)).

Anything this document leaves open — an HTTP/JSON gateway (deferred by
ADR-0004), io_uring adoption within
[./decisions/ADR-0011-supported-platforms.md](./decisions/ADR-0011-supported-platforms.md),
and every other open item — lives in [./OPEN_QUESTIONS.md](./OPEN_QUESTIONS.md)
with a fail-closed default and a reopen trigger, not here.
