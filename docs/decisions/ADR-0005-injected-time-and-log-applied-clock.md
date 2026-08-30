# ADR-0005: Injected Time and the Log-Applied Clock

- Status: accepted
- Date: 2026-08-30
- Related findings or requirements: FR-QUEUE-005, FR-QUEUE-010, FR-QUEUE-014, FR-FIFO-007, NFR-MAINT-002, P-02, P-05

## Context

Nearly every Relay guarantee is time-dependent: visibility expiry
(FR-QUEUE-005), delay (FR-QUEUE-010), retention (FR-QUEUE-014), and the
5-minute dedup window that must hold exactly at both boundaries (FR-FIFO-007,
P-05). If `relay-core` read a wall clock, two problems follow immediately:
simulation replays stop being exact (NFR-MAINT-002), and under replication
each replica would evaluate expiries against its own clock, so replicas
applying the identical log could disagree about whether a lease is live —
which is how double-lease bugs (violations of P-02) are born.

## Decision

Time is an injected dependency everywhere and a log entry inside the state
machine. The binding design:

- `relay-core` contains no clock, RNG, or IO access. The sole source of time
  inside the state machine is the `Command::AdvanceTime(Nanos)` log entry;
  applying it moves the state machine's clock monotonically forward and fires
  every timer-driven transition (visibility expiry, delay elapse, retention
  expiry, dedup-window eviction, lease expiry) whose deadline is ≤ the new
  time, deterministically and in a fixed order.
- The server's driver samples the injected `Clock` trait and appends
  `AdvanceTime` entries into the log (through Raft once R7 lands), so every
  replica applies the identical time at the identical log position: the
  log-applied clock. Lease expiry is therefore a replicated, deterministic
  fact, not a local-clock opinion.
- In production the `Clock` is monotonic-time backed; in simulation `SimClock`
  advances virtual time under harness control, and the same `AdvanceTime`
  entries appear in the log, making every timer path seed-replayable.
- Regressions of the clock are impossible by construction: an `AdvanceTime`
  entry with a value ≤ the current state-machine time is applied as a no-op.

## Alternatives Considered

- **Wall-clock reads inside `relay-core`**: rejected. Non-replayable
  (identical seeds produce different histories), and replicas diverge on
  expiry decisions under replication — a direct P-02/P-08 hazard the model
  checker could not even test deterministically.
- **Timestamps only on traffic-carrying entries** (derive time from `Send` and
  `Receive` entries, no dedicated tick): rejected because time then advances
  only when traffic arrives — on an idle queue, leases and retention never
  expire, violating FR-QUEUE-005 liveness; explicit `AdvanceTime` ticks solve
  this at the cost of periodic log entries.
- **Hybrid logical clocks**: rejected as solving the wrong problem. HLCs give
  causal ordering across nodes, but Relay's ordering already comes from log
  position; HLCs still read physical clocks, so they reintroduce the
  determinism and divergence problems while adding merge logic.

## Consequences

- Easier: every timer edge case (dedup boundary at exactly 300 s, visibility
  set to 0, retention at 14 d) is a deterministic CORE-/MODL- test; simulation
  can compress days of virtual time into milliseconds of wall time.
- Harder: the log carries periodic `AdvanceTime` entries even when idle
  (bounded by batching them with the group-commit window of ADR-0008); all
  "not before" semantics must be documented, since expiry fires at the next
  applied tick, never at the exact instant — consistent with NG-04.
- Revisit when: R7 measurement shows tick traffic materially affects log
  throughput, in which case tick frequency (not the mechanism) may be tuned.
  No OPEN_QUESTIONS entry reopens this decision.
