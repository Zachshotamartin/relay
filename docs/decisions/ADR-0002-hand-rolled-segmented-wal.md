# ADR-0002: Hand-Rolled Segmented WAL Storage Engine

- Status: accepted
- Date: 2026-08-30
- Related findings or requirements: FR-QUEUE-002, NFR-DUR-001, NFR-DUR-002, NFR-DUR-003, NFR-DUR-004, NFR-DUR-005, NFR-DUR-006, NFR-DUR-007, P-01

## Context

The durability contract (ADR-0008) says an acknowledged send is durable across
any single-process crash (P-01). Relay's product thesis is that this claim is
machine-checked: gate R2 requires the storage engine to survive crash,
torn-write, and disk-full injection, with every fault delivered through the
`Disk` trait so that `SimDisk` can inject it deterministically and replay it
from a seed. That requirement rules out any storage engine whose write path,
fsync ordering, and background work Relay does not fully own. The storage
engine is the second decision made because the WAL format is a versioned
on-disk contract (NFR-DUR-007) that other documents must cite byte-for-byte.

## Decision

`crates/relay-wal` implements a hand-rolled segmented write-ahead log as
Relay's only queue store, exposing `recover`, `append` (buffered), `sync`
(durable up to the returned LSN), and `compact`, all performing IO exclusively
through the injected `Disk` trait.

Binding formats (normative, versioned under NFR-DUR-007):

- Record: `[len u32 LE][crc32c u32][type u8][flags u8][reserved u16][lsn u64][payload len bytes]`;
  the CRC covers `type` through `payload`.
- Segment file: 64 MiB target size, named `wal-<seq:016x>.seg`, with a 4 KiB
  header: magic `RWALSEG1`, format version u16, segment seq u64, base LSN u64,
  created wall time, header CRC.
- Snapshot file: `snap-<lsn:016x>.rsnap`, magic `RSNAP1`, chunked with
  per-chunk CRC and a footer carrying the full-state SHA-256.

Recovery replays segments in sequence order, truncates only at the log tail on
CRC mismatch (NFR-DUR-003), and refuses to start on mid-log corruption.

## Alternatives Considered

- **RocksDB**: rejected. Compaction is opaque — its background threads issue
  writes, fsyncs, and file deletions on their own schedule, outside the `Disk`
  trait, so `SimDisk` cannot enumerate or replay its crash states and R2's
  torn-write injection cannot reach the bytes that matter. A determinism bug in
  a vendored C++ engine is also undiagnosable within Relay's toolchain.
- **SQLite**: rejected. Its page-store B-tree model mismatches an append-only
  queue log, paying page-rewrite amplification for no benefit; and its fsync
  semantics are checkpoint-oriented — SQLite decides when the WAL is
  checkpointed and synced, so Relay cannot bind one client ack to one completed
  fsync as ADR-0008 requires.
- **Hosted database as the queue store** (managed Postgres, DynamoDB, or
  similar): rejected, recorded explicitly. (1) Unverifiable durability chain —
  Relay cannot inject torn writes, disk-full, or crash faults into storage it
  does not run, so P-01 would rest on a vendor SLA instead of CRSH-* evidence.
  (2) Non-deterministic testing — every operation crosses a real network to an
  external system, making seed-exact replay (NFR-MAINT-002) impossible.
  (3) Product-thesis violation — Relay's claim is first-party-verified
  durability; delegating the durability boundary to a third party makes the
  headline claim someone else's marketing.

## Consequences

- Easier: R2's fault matrix (CRSH-*, STOR-*) can address every byte the engine
  writes; fsync ordering is exactly what ADR-0008 specifies; recovery time is
  measurable against NFR-PERF-004 (≤ 30 s for a 10 GiB WAL) with no foreign
  tuning knobs.
- Harder: Relay owns compaction correctness (NFR-DUR-006), format migration
  fixtures (MIGR- family), and every performance property RocksDB would have
  supplied for free; R2 carries 12–18 focused days largely because of this ADR.
- Revisit when: R2's evidence shows the format cannot meet NFR-PERF-001 on
  reference hardware, which would force a format-version bump under
  NFR-DUR-007's migration rules — never a silent rewrite. No OPEN_QUESTIONS
  entry reopens this decision.
