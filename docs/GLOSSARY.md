# Relay: Glossary

Canonical definitions for the controlled vocabulary used across the planning documents. When another document appears to conflict with a definition here, fix the conflict rather than redefining the term locally. Guarantee semantics are controlled by [CORRECTNESS.md](./CORRECTNESS.md); this glossary defines words, not claims.

## Delivery Semantics

- **Ack (acknowledgment)** — the successful return of an operation to the client. The ack is Relay's durability boundary: an acked send is covered by P-01; an unacked send may be lost (NG-09).
- **At-least-once** — Relay's delivery contract: every acked message is delivered one or more times; duplicates on redelivery are possible and consumers must be idempotent (NG-01).
- **Delivery** — the act of a `receive` returning a message to a consumer under a fresh lease. Each delivery increments the receive count and the lease epoch.
- **Delivery order** — the sequence of first-time deliveries within one FIFO message group; guaranteed equal to acked send order (P-04). No order is promised anywhere else (NG-03).
- **Backpressure** — the bounded-refusal behavior under overload: a stable error (`inflight_cap_exceeded`, `throttled`) instead of queuing without limit or collapsing (NFR-AVAIL-003).
- **Long polling** — a `receive` that waits up to WaitTimeSeconds (0–20 s) for a message to arrive before returning empty. A server-layer behavior at R6; the reference model never blocks.
- **Non-guarantee** — a promise Relay explicitly refuses to make, listed as NG-01..NG-10 in CORRECTNESS.md §3. Permanent design boundaries, not open items; marketing must repeat them where a stronger inference is plausible.
- **Property** — one of the ten machine-checked guarantees P-01..P-10 defined in CORRECTNESS.md §2, each bound to named tests in §9. A property exists as a claim only while its tests pass in CI.

## Queue and Message Lifecycle

- **Available set** — the messages of a queue in state `Available` with `visible_at ≤ now`; the only messages a `receive` may return, further restricted by FIFO group heads and the in-flight cap.
- **Delay** — the per-message (0–900 s) or per-queue-default deferral of first availability; a delayed message sits in state `Delayed` until its visibility instant.
- **In-flight** — a message currently under a live lease (state `InFlight`). Per-queue caps: 120,000 standard, 20,000 FIFO; exceeding the cap shrinks receive eligibility.
- **Lease** — time-bounded exclusive ownership of one delivery by one consumer, measured in log-applied time. Lifecycle: `Granted → Extended* → (Released | Expired | Consumed)`.
- **Lease epoch** — the per-message counter incremented on every delivery. Receipt handles embed it; `delete` and `change-visibility` require epoch equality, which makes handles single-use per delivery (P-07) and stale handles rejectable.
- **Message ID** — the ULID assigned at send, unique per queue, with its time component drawn from the log-applied clock (ADR-0006).
- **Purge** — the removal of all messages from a queue, including in-flight ones; a second purge within the 60 s cooldown is rejected (`purge_in_progress`).
- **Receive count** — the number of deliveries a message has had. When it reaches the queue's maxReceiveCount under a redrive policy, the next lease resolution dead-letters the message.
- **Receipt handle** — the opaque single-use token returned per delivery: `rh1_` + base64url(version ‖ queue id ‖ message id ‖ lease epoch ‖ expiry ‖ HMAC-SHA256 tag). Required for delete and change-visibility; unforgeable by construction (P-07).
- **Retention** — the per-queue lifetime (60 s–14 d, default 4 d) after which a message is removed regardless of delivery state; the terminal state `Expired`.
- **Visibility timeout** — the lease duration (0 s–12 h, default 30 s) during which a delivered message is invisible to other consumers. Expiry is "not before", never exact-instant (NG-04); setting it to 0 returns the message immediately.

## FIFO, Deduplication, and Dead-Lettering

- **FIFO group** — the ordered stream within a FIFO queue identified by a message group ID. Order holds within a group; distinct groups deliver in parallel; a group with an in-flight message blocks its later messages.
- **Message group ID** — the producer-supplied key (≤ 128 bytes, mandatory on FIFO sends) that assigns a message to its FIFO group.
- **Deduplication ID** — the key the dedup window matches on: the explicit `MessageDeduplicationId` when present, else the content hash when content-based deduplication is enabled. Explicit always overrides content-based.
- **Content-based deduplication** — the queue option that derives the deduplication ID as SHA-256 of the message body when no explicit ID is supplied.
- **Dedup window** — the fixed 300 s half-open interval `[t0, t0 + 300 s)` in log-applied time during which a repeated deduplication ID returns the original message ID with no state change. Exact at both boundaries (P-05).
- **DLQ (dead-letter queue)** — the queue named by a redrive policy that receives messages whose receive count exhausted maxReceiveCount, with body and attributes preserved and source metadata recorded.
- **Dead-lettering** — the automatic move of an exhausted message to the DLQ at lease resolution; the `DeadLettered` terminal-ish state (the message lives on in the DLQ as an ordinary message).
- **Redrive** — the `StartRedriveTask` operation moving messages from a DLQ back to their source queue with receive count reset, one-at-a-time (never cross-queue atomic), each message in exactly one queue at all times.

## Topics and Fanout

- **Topic** — a named pub/sub distribution point holding a set of subscriptions; publishing to a topic fans out copies to subscribed queues.
- **Subscription** — the binding of one queue to one topic, with an optional filter policy recorded verbatim at subscribe time.
- **Filter policy** — the attribute-matching predicate of a subscription: exact, anything-but, prefix, numeric range, exists. Invalid policies are rejected at subscribe time with field-level errors.
- **Fanout** — the per-subscription enqueue of independent copies (distinct message IDs) on publish. Per-subscription, never cross-queue atomic (NG-02); FIFO targets keep group and dedup semantics.

## Durability and Storage

- **WAL (write-ahead log)** — the segmented append-only log in `relay-wal` that records every state change before it is acknowledged; recovery replays it to the exact pre-crash acked state.
- **LSN (log sequence number)** — the monotonically increasing position of a record in the WAL; `Wal::sync` returns the highest durable LSN.
- **Segment** — one WAL file (`wal-<seq:016x>.seg`, 64 MiB target) with a 4 KiB header carrying magic, format version, sequence, base LSN, and a header CRC.
- **fsync-before-ack** — the ADR-0008 durability contract: an ack is emitted if and only if the WAL record and its fsync have completed. The enforcement point of P-01.
- **Group commit** — batching multiple pending appends into one fsync within an adaptive window capped at 2 ms, trading bounded latency for throughput without weakening fsync-before-ack.
- **fsyncgate rule** — the ADR-0008 decision that an fsync failure is fatal: the process aborts rather than retrying, because a failed fsync leaves page-cache state unknowable (NFR-DUR-005).
- **Torn write** — a partially persisted record at the WAL tail after a crash. Detected by per-record CRC and truncated safely at the tail only; never surfaced as data (NFR-DUR-003, P-10).
- **CRC32C** — the checksum guarding every WAL record, snapshot chunk, and RWP frame; detection, not prevention, of corruption.
- **Compaction** — reclaiming WAL space by discarding records no longer needed to reconstruct live state; verified to never remove live data (NFR-DUR-006).
- **Snapshot** — a checkpoint of full state (`snap-<lsn:016x>.rsnap`, chunked, per-chunk CRC, footer with full-state SHA-256) that bounds recovery replay and brings lagging Raft followers up to date.

## Replication and Consensus

- **Quorum** — a majority of the cluster's voting members. An entry commits only after a quorum has durably appended it (FR-REPL-002).
- **Partition (network)** — a fault splitting the cluster into node sets that cannot exchange messages. Relay's properties P-08 and P-09 are quantified over arbitrary partition schedules within the crash-stop fault model.
- **Split-brain** — the failure mode where both sides of a partition act as authority. The queue-level symptom Relay checks against is the double-lease (P-08).
- **Pre-vote** — the Raft extension in which a candidate first polls electability without incrementing terms, preventing partitioned nodes from disrupting a stable leader on rejoin.
- **ReadIndex** — the Raft mechanism for linearizable reads without log writes: the leader confirms leadership with a quorum, then serves the read at or after the confirmed commit index (FR-REPL-008).
- **Leader hint** — the referral returned when a non-leader rejects a write, telling the client which node to retry against (FR-REPL-007).
- **Membership change** — adding or removing one server at a time (single-server rule), the only reconfiguration mode Relay's Raft supports.
- **Crash-stop** — the fault model: processes fail only by halting (and may restart); no process ever lies. The boundary that excludes Byzantine behavior (NG-07).

## Wire Protocol and Security

- **RWP (Relay Wire Protocol)** — the custom framed binary protocol (`RWP/1`): magic `RWP1`, length (max 1 MiB), CRC32C, opcode, flags, request ID, per-opcode fixed body layouts. Not SQS-compatible (ADR-0004).
- **Bounded parser** — a parser that checks every length against declared limits before allocating; the mandatory discipline for all wire input (FR-API-002), gated by fuzz corpus in CI.
- **Error taxonomy** — the fixed set of stable machine-readable error codes to which every failure maps exactly one member (FR-API-006); the reference-model subset is listed in CORRECTNESS.md §5.1.
- **Canary** — a seeded high-entropy synthetic secret used to prove leak absence across logs, traces, errors, and diagnostics (NFR-SEC-003).

## Verification Apparatus

- **Reference model** — the complete executable specification in `relay-model` (CORRECTNESS.md §4): explicit state plus precondition → postcondition → return rules for every operation. The oracle's ground truth; when model and implementation disagree, the implementation is wrong unless an ADR amends the model.
- **History** — the JSONL record of one concurrent execution: one operation per line with op ID, client, call, invoke/return timestamps, result, and generating seed. Captured client-side in tests and by wire-tap in simulation.
- **Wire-tap** — the SimNet capture point that records every RWP request/response pair into a history, so protocol-level faults appear exactly as a client would experience them.
- **Linearizability** — the correctness condition the oracle checks: a total order of operations exists, consistent with real-time precedence (return-before-invoke), under which every recorded result matches the reference model.
- **Wing–Gong checker** — the linearizability search algorithm (ADR-0007): repeatedly linearize a real-time-minimal candidate whose model result matches the record, backtracking on failure, with per-queue partitioning and memoized (linearized-set, state-hash) configurations.
- **Oracle** — the checking apparatus as a whole: reference model plus Wing–Gong checker plus structural pre-checks, deciding pass/fail for a history.
- **Model checker** — the MODL- harness: seeded workload generators producing concurrent histories that the oracle checks; CORRECTNESS.md §7 fixes its profiles and budgets.
- **Counterexample** — the oracle's failure output: the history shrunk (ddmin-style) to a minimal non-linearizable form, plus the generating seed and the first non-linearizable frontier.
- **Invariant checker** — a predicate evaluated continuously during simulation against harness-maintained ground truth; each maps to one property (CORRECTNESS.md §8.4) and aborts the run on violation.
- **Liveness** — a property of the form "something eventually happens" (P-03); checked by a simulation watchdog with a computed bound, not by the linearizability oracle.
- **Bounded fairness** — the scheduling assumption liveness needs: every ready task eventually runs, enforced by construction in the virtual-time executor.

## Simulation and Determinism

- **Simulation** — the `relay-sim` harness running the whole system in one process, one thread, under virtual time, with all environment access through injected `Clock`/`Net`/`Disk`/`Rng` traits.
- **Virtual time** — simulated time that jumps discretely between scheduled events; no real sleeping. Distinct from log-applied time, which is the state machine's own clock.
- **Log-applied time** — the deterministic clock inside `relay-core`, advanced only by applied `AdvanceTime` log entries (ADR-0005). All leases, delays, windows, and retention are measured in it.
- **AdvanceTime** — the log-entry command that is the sole source of time in the state machine; makes expiry deterministic and boundary tests exact to the nanosecond.
- **Seed** — the 64-bit value that fully determines a simulation or generator run: workload, network schedule, disk behavior, fault schedule, and tie-breaking all derive from it via split substreams.
- **Fault schedule** — the timed list of injected faults (crashes, restarts, partitions, fsync errors, disk-full, torn writes) derived from a seed's fault substream before the run starts.
- **Reproducibility contract** — same seed ⇒ byte-identical trace (SHA-256-hashed executor event log). Divergence is its own bug class (`DET-DIVERGE`) as severe as a property violation.
- **Failing-seed corpus** — the append-only, checked-in set of seeds (plus minimized histories) that ever produced a failure; every CI run replays all of it (NFR-MAINT-002); removal requires an ADR.

## Process and Status

- **Gate** — one of the phased evidence milestones R0–R10 in [BUILD_PLAN.md](./BUILD_PLAN.md); each unlocks a named class of claims and no more.
- **Gate statuses** — the four-word vocabulary used identically in every document: `accepted` (implemented on mainline, backed by its named automated gate), `in progress` (present on a branch, not a claim), `planned` (specified, not implemented), `deferred` (outside the named phase; forbidden as completion evidence).
- **Terminal owning gate** — the gate at which a requirement's evidence completes; earlier gates may begin it, but only the terminal gate may claim it done (spine requirement register rule).
- **Evidence** — a named automated test or measured artifact backing a claim. A package, type, stub, or happy-path unit test is never evidence of completion.
- **Claims audit** — the MKT- checklist gating every release announcement: each public claim traces to a P-xx/NG-xx entry or a benchmark result, and no copy strengthens a claim beyond CORRECTNESS.md or THREAT_MODEL.md.
- **ULID** — the 128-bit lexicographically sortable identifier (Crockford base32) used for message IDs, with the time component from the log-applied clock.
