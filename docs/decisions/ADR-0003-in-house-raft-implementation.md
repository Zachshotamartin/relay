# ADR-0003: In-House Raft Implementation

- Status: accepted
- Date: 2026-08-30
- Related findings or requirements: FR-REPL-001, FR-REPL-002, FR-REPL-003, FR-REPL-004, FR-REPL-005, FR-REPL-006, FR-REPL-007, FR-REPL-008, P-08, P-09

## Context

Relay's replication claims are its hardest: no double-lease across any network
partition (P-08) and no lost acknowledged write across leader failover (P-09).
Gate R7 proves both under deterministic simulation — partitions, clock skew,
and message reorderings injected through `SimNet` and `SimClock` and replayed
exactly from a seed. That works only if the Raft implementation is a pure state
machine that the harness ticks: no internal threads, no runtime-owned timers,
no IO it performs itself. Consensus is also the subsystem where a library's
convenience is most tempting and where the honest argument must be made now,
before R7's budget is committed.

The honest argument: writing Raft from scratch is the expensive path.
Battle-tested libraries exist, Raft's edge cases (stale leaders, membership
churn, snapshot races) have burned every implementer, and an in-house
implementation converts library-integration days into 20–30 focused days of
protocol work. Relay takes that trade deliberately, because the alternative is
a replication story whose safety evidence stops at "the library's tests pass."

## Decision

`crates/relay-raft` implements Raft in-house as a deterministic, IO-free state
machine driven by injected time and transport: pre-vote enabled, heartbeat
100 ms, election timeout 500–1000 ms randomized (in simulated time), ReadIndex
for linearizable reads, single-server membership changes only, and snapshot
install in 1 MiB chunks. Relay accepts that R7 is the longest gate in the build
plan (20–30 focused days) as the direct cost of this decision.

## Alternatives Considered

- **openraft**: rejected for async runtime coupling. Its core spawns tokio
  tasks and owns its timers, so driving it from a single-threaded virtual-time
  executor means fighting the library's own concurrency; deterministic
  seed-replay of a partition scenario (NFR-MAINT-002) becomes harder than
  writing the protocol, which defeats the reason to buy it.
- **raft-rs (TiKV)**: rejected. Its C-style tick/ready API is caller-driven —
  the closest fit — but storage, transport, and snapshot orchestration remain
  the integrator's problem, so most of R7's hard work stays in-house anyway;
  and its maintenance tracks TiKV's internal priorities, a bus-factor risk on
  Relay's most safety-critical dependency.
- **External etcd or ZooKeeper for coordination**: rejected. A second system to
  install, secure, and upgrade contradicts the single-static-binary decision
  (ADR-0009); every lease grant would cross a real network hop that `SimNet`
  cannot capture, making P-08's partition evidence unattainable; and Relay's
  availability ceiling would become the external cluster's.

## Consequences

- Easier: R7's SIM-RAFT scenarios can drive elections, partitions, and
  snapshot races tick-by-tick and shrink failures to minimal seeds; the lease
  state machine and the log live in one crate, so FR-REPL-004's
  leases-through-the-log design has no impedance boundary.
- Harder: Relay owns every known Raft pitfall — configuration-change safety,
  stale-read prevention, snapshot/append races — and the mandatory
  test-first discipline (NFR-MAINT-001) applies to all of it; R7's 20–30 day
  estimate dominates the schedule and slips first if the team is wrong.
- Revisit when: R7's evidence stalls past double its estimate with open safety
  counterexamples — the fallback is raft-rs integration under a superseding
  ADR, not a quiet swap. No OPEN_QUESTIONS entry reopens this decision.
