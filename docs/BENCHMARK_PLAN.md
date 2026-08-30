# Relay: Benchmark Plan

Last revised: 2026-08-30. Status: planned (gate R9). This document is normative.

This plan defines every performance measurement Relay is permitted to publish,
the hardware and harness that produce it, the statistical treatment it must
carry, and the exact claims each number supports. No benchmark in this document
has been run. Every result described here is `planned` until the R9 gate in
[BUILD_PLAN.md](./BUILD_PLAN.md) §14 is accepted. This document sits at
precedence tier 7 of the conflict rules in [docs/README.md](./README.md): it
controls performance-claim binding and nothing else.

## 1. Purpose and Claims Discipline

Relay's performance numbers exist for exactly one reason: to support the
specific claims enumerated in §6 of this document. They support no other
claims. The rules:

1. A number may be published only if it was produced by a benchmark defined in
   §4, on hardware conforming to §2, by the harness in §3, under the
   statistical treatment in §5.
2. Every published number carries, inseparably, its hardware description, its
   workload identity (benchmark ID plus parameter set plus workload seed), and
   its statistical treatment. This is requirement NFR-PERF-005 and it admits no
   exception: a number quoted without all three attachments is a documentation
   defect and is treated as an unearned claim under NFR-MAINT-005.
3. [MARKETING.md](./MARKETING.md) may cite performance only from this
   document's published results (FR-MKT-002). Marketing may weaken or omit a
   claim; it may never strengthen, extrapolate, or round one upward.
4. A benchmark result is evidence for its claim row in §6 and for nothing
   adjacent. BENCH-01 latency says nothing about FIFO latency; BENCH-02
   single-node throughput says nothing about replicated throughput.
5. Targets in this plan (drawn from NFR-PERF-001 through NFR-PERF-004 and
   NFR-AVAIL-002) are acceptance thresholds for the R9 gate, not published
   claims. Until a benchmark runs and meets its target on reference hardware,
   the only honest public statement is the target itself, labeled as a target.
6. Non-guarantee NG-05 stands over this entire document: long-poll wakeup and
   all latency figures are measured goals, never contractual delivery-latency
   bounds. Publication of a p99 does not create a latency SLA.

## 2. Reference Hardware

All acceptance runs execute on one fixed reference profile. Numbers from any
other machine are exploratory and unpublishable.

### 2.1 Reference profile

| Dimension | Requirement |
| --- | --- |
| CPU | 8 vCPU (4 physical cores with SMT, or 8 physical cores), x86_64, fixed frequency governor `performance` |
| Memory | 16 GiB, no swap enabled during runs |
| Storage | Local NVMe SSD, directly attached; data directory on a dedicated filesystem (ext4, default journal) on that device |
| OS | Linux 6.x kernel, 64-bit |
| Network (multi-node benchmarks) | ≥ 10 Gbit/s between nodes, same placement group / rack, RTT < 0.5 ms |

Exact cloud analog: AWS `c6id.2xlarge` (8 vCPU, 16 GiB, one local 474 GB NVMe
instance store), Amazon Linux 2023 or Ubuntu 24.04 with a 6.x kernel. This
instance type is the canonical reference; a bare-metal machine matching the
profile above is equally valid and must be documented to the same field level
in the hardware manifest (§7.2).

### 2.2 Disqualification rules

A run is void — not "lower quality," void — if any of the following holds:

- The instance is burstable or CPU-shared (any AWS `t`-family, GCP `e2`
  shared-core, or equivalent). Credit-based CPU makes tail latency fiction.
- Storage is network-attached (EBS, persistent disk, NFS) or any tier where
  fsync durability semantics are mediated by a remote service.
- The run is page-cache-only: fsync must reach stable media. Before every
  acceptance session the harness runs an fsync-latency sanity probe (1,000
  serial 4 KiB append+fsync operations); if median fsync latency is below
  50 µs on the reference device class, the device is presumed to be
  acknowledging from volatile cache and the session is disqualified until
  write-cache behavior is verified and documented in the manifest.
- Another tenant workload, container, or CI job shares the machine during the
  run.
- Swap is enabled, the CPU governor is not `performance`, or thermal/frequency
  throttling is observed in the recorded system telemetry.
- The relayd build is not a release build (`--release`, LTO settings as pinned
  in the workspace) at an exact tagged commit.

Disqualified runs are still archived (§5.6 forbids discarding data) but are
marked `disqualified` with the triggering rule, and are never aggregated.

## 3. Harness

The harness is the `crates/relay-bench` workspace crate. It is the only
program permitted to produce publishable numbers.

### 3.1 Load generation

- Open-loop by default. The generator computes each request's intended start
  time from the target rate schedule before the run begins, and measures every
  request's latency from its intended start time, not from its actual send
  time. A stalled server therefore accrues the stall into latency instead of
  silently thinning the arrival stream. This is the coordinated-omission rule
  and it applies to every latency benchmark in §4.
- BENCH-02 is the deliberate exception: it is closed-loop by design, because
  its measured quantity is sustainable throughput of a full message lifecycle,
  and it is labeled closed-loop wherever its results appear.
- The generator runs on a separate machine of at least the reference profile,
  connected per §2.1. Loopback load generation is permitted only for BENCH-06
  (crash recovery), which measures the server alone.
- Client connections use `relay-client` over RWP/1 with TLS 1.3, matching
  production configuration (FR-API-008). Plaintext-loopback runs are
  exploratory only.

### 3.2 Measurement

- Latencies are recorded in HDR histograms with 3 significant digits, range
  1 µs to 60 s, one histogram per operation type per run, serialized in the
  standard HDR interval-log format.
- Timestamps come from `CLOCK_MONOTONIC`. Cross-machine latency is never
  computed from two clocks; every latency is measured on the machine that
  initiated the operation.
- System telemetry (CPU, memory, disk queue depth, frequency, temperature) is
  sampled at 1 Hz for the full run and archived with the histograms.

### 3.3 Run structure

- Warmup: the first 60 seconds of every run are executed at full target load
  and discarded from all statistics. Warmup data is archived but never
  aggregated.
- Measured length: 300 seconds per run after warmup, except BENCH-06 (runs to
  recovery completion) and BENCH-08 (600 seconds to contain the failover
  event with stable load on both sides).
- Repetition: n = 5 runs per benchmark per parameter set, executed on the
  same freshly provisioned machine, with a full relayd restart and data
  directory wipe between runs (except BENCH-06, which prepares its WAL per
  run as specified).
- Seed pinning: every workload (message bodies, attribute choices, group ID
  assignment, arrival jitter) derives from a single recorded 64-bit seed per
  run. The seed appears in the results file; re-running with the same seed on
  the same build reproduces the same request stream byte-for-byte.
- Abort rule: a run in which relayd crashes, a client observes a non-throttle
  error, or telemetry shows disqualifying conditions (§2.2) is recorded as
  `failed` with its evidence, and counts against the no-cherry-picking record
  in §5.6. Failed runs are not silently rerun into the aggregate; the failure
  is investigated first.

## 4. Workloads

Eight benchmarks. Common parameters unless a row overrides them: standard
queue, 256-byte bodies, no message attributes, visibility timeout 30 s,
durability contract active (fsync-before-ack per ADR-0008,
[decisions/ADR-0008-fsync-before-ack-durability-contract.md](./decisions/ADR-0008-fsync-before-ack-durability-contract.md)),
single tenant, authentication enabled.

### BENCH-01 — Send-only latency, 256 B, fsync-on

- Topology: single node. Load: open-loop SendMessage at 10,000 msg/s (50% of
  the NFR-PERF-001 throughput floor) from 32 connections, one queue.
- Measured: send-to-ack latency (intended-start to durable-ack receipt at the
  client), full distribution.
- Target: p99 ≤ 15 ms (NFR-PERF-002).
- Secondary sweep (reported, no target): the same workload at 2,500, 5,000,
  and 20,000 msg/s to expose the latency/throughput curve.

### BENCH-02 — Closed-loop send+receive+delete throughput

- Topology: single node. Load: closed-loop; 64 producer workers each
  send-then-await-ack, 64 consumer workers each loop
  ReceiveMessage(max 10) → DeleteMessage per message, one queue.
- Measured: sustained completed-lifecycle rate (messages sent, received, and
  deleted per second) over the measured window; backlog depth must be
  bounded (steady state, final backlog < 10 s of throughput).
- Target: ≥ 20,000 msg/s (NFR-PERF-001).

### BENCH-03 — Long-poll wakeup latency

- Topology: single node. 256 consumers parked in ReceiveMessage with
  WaitTimeSeconds = 20 on one queue; an open-loop trickle of sends at
  100 msg/s, each send matched to exactly one waiting consumer.
- Measured: wakeup latency — durable-ack time of the send to first byte of
  the matching non-empty receive response at the consumer, per FR-QUEUE-009.
- Target: p99 ≤ 10 ms (NFR-PERF-003). NG-05 applies: this is a measured goal,
  not a delivery-latency contract.

### BENCH-04 — FIFO contention: 1 group versus 1,000 groups

- Topology: single node, one `.fifo` queue, content-based dedup off, explicit
  MessageDeduplicationId per message.
- Variant A: all messages carry one MessageGroupId (fully serialized per
  FR-FIFO-004). Variant B: MessageGroupIds drawn uniformly from 1,000 groups.
  Both variants: closed-loop lifecycle as in BENCH-02 with 16 producers and
  16 consumers.
- Measured: sustained lifecycle throughput per variant, and the B:A ratio.
- Target: none. NG-10 explicitly declines to promise FIFO throughput parity;
  this benchmark exists to publish the honest cost of group serialization.

### BENCH-05 — Fanout, 1 topic → 10 subscriptions

- Topology: single node; one topic, 10 standard queues subscribed, no filter
  policies. Open-loop Publish at 2,000 msg/s (20,000 deliveries/s downstream).
- Measured: publish-to-ack latency distribution; per-subscription delivery
  completeness (every matching queue receives its independent copy per
  FR-TOPIC-003); aggregate downstream delivery rate.
- Target: none numeric. Pass condition for R9: zero missing and zero
  cross-queue-coupled deliveries over the full run.

### BENCH-06 — Crash-recovery time versus WAL size

- Topology: single node. Preparation per run: load the WAL to 1 GiB, 5 GiB,
  or 10 GiB of live records at 256 B bodies with a seed-pinned mixture of
  sent, in-flight, and deleted messages; then SIGKILL relayd mid-load.
- Measured: wall time from relayd process start to readiness endpoint
  (FR-OPS-003) reporting ready with the exact pre-crash acknowledged state
  (verified against the recorded model per NFR-DUR-002).
- Target: ≤ 30 s at 10 GiB (NFR-PERF-004). The 1 GiB and 5 GiB points are
  published to show scaling shape; they carry no independent target.

### BENCH-07 — 3-node replicated send p99 versus single node

- Topology: 3-node Raft cluster per §2.1 network rules; open-loop
  SendMessage at 10,000 msg/s against the leader, parameters otherwise
  identical to BENCH-01.
- Measured: send-to-ack p99 with majority-durable commit (FR-REPL-002), and
  the delta versus the same release's BENCH-01 p99.
- Target: none numeric at 1.0; the replication overhead is published as an
  honest delta, never netted out. The single-node NFR-PERF-002 target is
  explicitly not applied to this benchmark.

### BENCH-08 — Failover time under load

- Topology: 3-node cluster; open-loop mixed load (70% send / 30%
  receive+delete) at 10,000 msg/s total. At t = 300 s of the measured
  window, the leader process receives SIGKILL.
- Measured: (a) wall time from leader kill to the first newly acknowledged
  write on the new leader; (b) client-observed error/throttle window;
  (c) post-run verification of no lost acknowledged write (P-09) and no
  double-lease (P-08) against recorded histories.
- Target: (a) ≤ 5 s (NFR-AVAIL-002, the R9 wall-clock measurement of the
  property simulated at R7). (c) must be zero-violation for the run to pass.

## 5. Statistical Treatment

### 5.1 Aggregation

Each benchmark's headline number per parameter set is the median of its 5
valid runs' headline quantity (p99 for latency benchmarks, sustained rate for
throughput benchmarks, wall time for BENCH-06/08). Per-run values for all 5
runs are published alongside the median; the median is never presented alone.

### 5.2 Reported quantiles

Every latency benchmark reports p50, p90, p99, p99.9, and the observed
maximum, from the merged post-warmup HDR histograms, per run and merged
across runs. Throughput benchmarks additionally report the 1-second-window
rate distribution (min, p50, max) to expose stalls that a mean would hide.

### 5.3 Confidence

For each headline quantity, a 95% confidence interval is computed by
bootstrap over the 5 per-run values (10,000 resamples, percentile method,
seed recorded). The CI is published with the number. Five runs give a wide
interval; that width is published honestly rather than narrowed by dropping
runs.

### 5.4 Regression definition

A regression is a > 5% degradation of a benchmark's headline p99 (or, for
throughput benchmarks, a > 5% drop in headline rate; for BENCH-06/08, a > 5%
increase in wall time) versus the recorded baseline for the previous release
on the same reference profile. Regressions block release until either fixed
or accepted in a new ADR that revises the target.

### 5.5 Baselines

The first accepted R9 run set becomes baseline v1. Baselines are stored in
the results archive (§7) and are immutable; a new baseline is a new versioned
entry, never an edit.

### 5.6 No cherry-picking

All runs are published: valid, failed, and disqualified alike, each labeled.
Selecting the best 5 of N runs is forbidden; the 5 aggregated runs are the 5
scheduled runs, in execution order. If a run fails, the failure is diagnosed
and the entire 5-run session is repeated from run 1, with the failed session
retained in the archive.

## 6. What the Numbers Do and Do Not Support

### 6.1 Supported claims

| Supported claim (exact scope) | Evidence |
| --- | --- |
| Single-node p99 send-to-ack latency at 256 B with fsync-before-ack, at the stated rate, on reference hardware | BENCH-01 |
| Single-node sustained send+receive+delete throughput at 256 B, closed-loop, on reference hardware | BENCH-02 |
| Long-poll wakeup latency distribution at the stated fan-in, on reference hardware (goal, not contract; NG-05) | BENCH-03 |
| Measured FIFO throughput cost of group serialization, single node | BENCH-04 |
| Correct 10-way fanout under stated load, with measured publish latency | BENCH-05 |
| Crash-recovery wall time as a function of WAL size up to 10 GiB | BENCH-06 |
| Measured latency overhead of 3-node majority-commit replication versus single node, same release | BENCH-07 |
| Measured failover time under stated load, with verified no-lost-ack and no-double-lease | BENCH-08 |

### 6.2 Forbidden extrapolations

- No "faster than X" or comparative claim of any kind against another system
  without a same-hardware, same-workload, same-statistical-treatment
  head-to-head executed under this plan and published in full. Quoting
  another project's published numbers next to Relay's is a comparison and is
  forbidden by this rule.
- No extrapolation across body size, message count, node count, tenancy, or
  hardware: a 256 B result claims nothing about 256 KiB bodies; a 3-node
  result claims nothing about 5 nodes.
- Simulated-time results from `crates/relay-sim` (R3, R7 evidence) are
  correctness evidence only and are never reported as wall-time performance,
  never converted to wall-time figures, and never mixed into a table with
  wall-time results. NFR-AVAIL-002 illustrates the boundary: the failover
  bound is simulated at R7 and becomes a performance figure only via
  BENCH-08 at R9.
- No latency figure implies a delivery-latency guarantee (NG-05), and no
  throughput figure implies exactly-once semantics (NG-01) or cross-queue
  atomicity (NG-02).
- Exploratory numbers from non-reference machines, dev laptops, or CI runners
  never leave engineering notes.

## 7. Publication Format

### 7.1 Results file

Each release publishes one versioned results file,
`bench/results/<release-tag>.json`, containing for every benchmark ×
parameter set: benchmark ID and plan revision, build commit and tag, run
seeds, per-run headline values, merged quantiles per §5.2, medians, bootstrap
CIs with bootstrap seed, run verdicts (valid/failed/disqualified with
reasons), and baseline deltas per §5.4. The file is append-only across
releases; corrections are new entries flagged `supersedes`.

### 7.2 Hardware manifest

Each results file references a hardware manifest recording: instance type or
bare-metal identity, CPU model and governor, memory, NVMe device model and
firmware, write-cache configuration and the fsync sanity-probe result (§2.2),
kernel version, filesystem and mount options, network topology for multi-node
runs, and relayd configuration deltas from defaults.

### 7.3 Raw archives

Raw HDR histogram interval logs, per-second telemetry, harness logs, and
workload seeds for every run — including warmup, failed, and disqualified
runs — are archived under `bench/archive/<release-tag>/` and retained for
the supported life of the release. A published number whose raw archive is
missing is withdrawn.

### 7.4 Claims audit hook

The MKT- claims audit (FR-MKT-005, [MARKETING.md](./MARKETING.md)) verifies
before every release announcement that each cited figure resolves to a
results-file entry with its manifest and archive intact, and that the citing
text does not exceed the claim scope in §6.1.
