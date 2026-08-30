# ADR-0008: Durability Contract — fsync-Before-Ack with Bounded Group Commit

- Status: accepted
- Date: 2026-08-30
- Related findings or requirements: FR-QUEUE-002, NFR-DUR-001, NFR-DUR-005, NFR-PERF-001, NFR-PERF-002, P-01

## Context

The single most important promise Relay makes is P-01: every acknowledged send
survives any single-process crash. That promise is worthless unless the exact
meaning of "acknowledged" is fixed — which bytes must be where, in what order,
before the client sees success — and unless the failure mode of the fsync
syscall itself is decided. The fsyncgate lesson (PostgreSQL, 2018) is binding
context: on Linux, a failed fsync may clear dirty page state, so retrying
fsync and eventually succeeding can silently lose the very write that failed.
Per-message fsync is also incompatible with NFR-PERF-001's 20,000 msg/s
target on a single NVMe device, so batching must be part of the contract, not
an optimization bolted on later. This contract gates R2 and shapes the WAL API
(ADR-0002), so it is decided before any storage code exists.

## Decision

The binding durability contract:

1. **ack ⇔ record + fsync complete.** A `SendMessage` (or batch entry) is
   acknowledged if and only if its WAL record has been appended and an fsync
   covering that record's LSN has returned success (`Wal::sync` returned an
   LSN ≥ the record's). No code path may emit an ack before that point.
2. **Adaptive group commit, ≤ 2 ms cap.** Pending appends are grouped into one
   fsync. The window is adaptive — it closes immediately when the device is
   idle or the batch is full, and never stays open longer than 2 ms. The cap
   bounds ack latency contribution (fits inside NFR-PERF-002's 15 ms p99);
   adaptivity keeps single-writer latency near raw device fsync time.
3. **fsync failure is fatal.** Any fsync or fsync_dir error aborts the process
   immediately (NFR-DUR-005). No retry, no error return to the caller, no
   degraded mode: after a failed fsync the kernel's page-cache state is
   unknowable, so the only honest recovery is crash and WAL replay from disk.
4. **What a crash may lose: unacknowledged sends only.** Sends in flight —
   received but not yet acked — may vanish (NG-09). Nothing acknowledged may
   ever be lost; that is exactly P-01 and is what CRSH-* injection verifies.

## Alternatives Considered

- **Ack before fsync with periodic background sync**: rejected — a crash
  inside the sync interval loses acknowledged messages, directly violating
  P-01; this converts Relay's headline guarantee into a probability.
- **Retry fsync on failure**: rejected per fsyncgate — after EIO, dirty pages
  may have been marked clean, so a subsequent successful fsync proves nothing
  about the failed write; retry-then-ack can acknowledge data that is not on
  disk. Crash-and-replay re-reads reality from disk instead of trusting it.
- **Per-message fsync (no grouping)**: rejected on throughput — one fsync per
  message caps the system at device fsync rate (~1–5 k/s on NVMe),
  making NFR-PERF-001 unreachable; grouping amortizes without weakening the
  per-message ack condition.
- **O_DIRECT with custom write scheduling**: rejected for 1.0 — it discards
  the page cache's read benefit and adds alignment and device-quirk
  complexity before a portable baseline exists; it may return as an io_uring
  optimization inside the tier-1 platform work (ADR-0011) without changing
  this contract.

## Consequences

- Easier: the ack boundary is a single testable predicate — CRSH- tests kill
  the process at every instruction boundary around `sync` and assert acked
  messages survive; the client contract for retries is simple (unacked send
  after timeout ⇒ resend; dedup handles duplicates on FIFO queues).
- Harder: every deployment pays an fsync per group commit — Relay is honest
  that throughput scales with device sync performance; operators on network
  block storage will see the cap dominate latency, and the capacity model
  (FR-OPS-011) must say so.
- Revisit when: R9 benchmarks show the 2 ms cap is mistuned for reference
  hardware — the cap's value may change by superseding ADR; the ack ⇔ fsync
  equivalence and crash-on-failure rules are not tunable and may never be
  weakened. No OPEN_QUESTIONS entry reopens this decision.
