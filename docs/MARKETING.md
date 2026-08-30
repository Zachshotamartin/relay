# Relay: Marketing and Claims Plan

Document status: normative. Last revised 2026-08-30.

Implementation status: nothing described here has shipped; every deliverable
in this plan is `planned`. No copy block below may be published before the
gate named next to it passes, and none at all until it survives the §8 audit.

## 1. Purpose and Governing Rule

This document plans marketing generation for Relay as gated engineering
work, per FR-MKT-001..005 in [PRODUCT_REQUIREMENTS.md](./PRODUCT_REQUIREMENTS.md):
positioning derives only from verified guarantees in
[CORRECTNESS.md](./CORRECTNESS.md); every public performance claim cites a
[BENCHMARK_PLAN.md](./BENCHMARK_PLAN.md) result with hardware and
statistical treatment; copy states the non-guarantee list wherever
exactly-once could be inferred; launch collateral is reviewed against the
claims audit; and a claims-audit checklist gates every announcement.

The governing rule, stated once and applied everywhere:

> Every outbound sentence that asserts something about Relay resolves to a
> P-xx guarantee, an NG-xx honest limitation, or a BENCH-xx measured result.
> A sentence that resolves to none of these is not published.

Precedence: per [README.md](./README.md), this document sits at tier 7 of
the conflict-and-status order. [CORRECTNESS.md](./CORRECTNESS.md) (tier 3)
controls guarantee and non-guarantee claims; [THREAT_MODEL.md](./THREAT_MODEL.md)
and [BENCHMARK_PLAN.md](./BENCHMARK_PLAN.md) bind security and performance
claims at tier 7. MARKETING.md may never strengthen a claim beyond what
those documents prove; where they disagree, this document is wrong.

Corollaries: marketing never promotes status (in-memory is not durability,
single-node is not replication, a simulated fault is not production
hardening — the gate behind a claim is named or the claim is withheld);
marketing never rounds up ("no lost acks across 10^7 simulated failovers"
is publishable after R7; "never loses data" is not publishable ever, per
NG-09); and NG-01..NG-10 are content, not fine print.

## 2. Audiences

Planning assumptions; none imply existing users.

1. **Backend engineers burned by queue edge cases.** They have debugged a
   3 a.m. duplicate delivery; they read skeptically and reward
   specificity. Primary audience for Pillar 2: the plainly stated
   non-guarantee list earns their trust. Copy leads with failure behavior.
2. **SRE and platform operators.** They will run relayd, page on it, and
   restore it from backup; they care whether the failure they will see was
   already a test case. Copy cites CRSH-, SIM-, and OPSX- evidence and
   links the R8 runbook.
3. **Correctness-community readers.** Adjacent to Jepsen, TigerBeetle, and
   FoundationDB testing culture; they verify claims and amplify
   reproducible evidence. Copy names methods precisely — simulation, JSONL
   histories, the Wing–Gong checker (ADR-0007) — never implying affiliation.
4. **Hiring evaluators reading the repository as a portfolio artifact.**
   This document is itself evidence for them: claims discipline designed
   in before the first line of code. The one rule: the README never
   claims more than CI proves on the day it is read.

## 3. Positioning Statement and Category

Category: **verification-first infrastructure** — self-hosted systems
software whose central claims are machine-checked by evidence anyone can
re-run, and whose limitations are published as prominently as its
guarantees. Relay is a queue and pub/sub service in that category.

Positioning statement (internal; guides copy, not published verbatim):

> For teams that need a queue they can actually trust, Relay is the
> self-hosted message queue and pub/sub service whose delivery guarantees
> are machine-checked by deterministic simulation and model checking rather
> than asserted in documentation, and whose non-guarantees are listed as
> plainly as its guarantees.

### 3.1 One-liner (10 words)

> The self-hosted queue whose delivery guarantees are machine-checked, not
> asserted.

Resolution: "machine-checked" is the P-01..P-10 proving-test mapping in
[CORRECTNESS.md](./CORRECTNESS.md), running in CI. Partial use R3; full R7.

### 3.2 25-word variant

> Relay is a self-hosted message queue and pub/sub service. Every delivery
> guarantee is checked by deterministic simulation and a linearizability
> oracle; every limitation is documented.

Resolution: SIM- and MODL- families; the NG list. Earliest publication R3,
with replication-related guarantees excluded until R7.

### 3.3 100-word boilerplate

> Relay is a self-hosted message queue and pub/sub service written in Rust.
> Its delivery guarantees — durable acknowledgment, lease exclusivity,
> per-group FIFO order, exact deduplication windows, and no lost
> acknowledgments across failover — are machine-checked: a deterministic
> simulator injects crashes, partitions, and disk faults, and a
> linearizability checker validates every operation history against a
> reference model. Any failure reproduces exactly from its seed. Relay is
> honest about what it does not do: delivery is at-least-once, never
> exactly-once, and its full non-guarantee list ships in the documentation.
> Published performance numbers carry their hardware, workload, and
> statistical treatment.

Resolution, sentence by sentence: P-01, P-02, P-04, P-05, P-09; SIM-/MODL-;
NFR-MAINT-002; NG-01; NFR-PERF-005. Earliest full publication R9; a reduced
boilerplate dropping the failover and performance sentences may run from R4.

## 4. Messaging Pillars

Exactly four pillars; every piece of outbound copy hangs off one. Each
lists its statement, evidence, draft proof-point copy (draft-pending-audit),
and forbidden phrasings — wordings banned even though they sound adjacent.

### 4.1 Pillar 1 — Guarantees you can replay

Pillar statement: Relay's guarantees are named properties with named
tests, and any failure those tests ever find reproduces from a seed.

Evidence: P-01 DURABLE-ACK (CRSH-, SIM-), P-09 NO-LOST-ACK (SIM-RAFT,
MODL-), NFR-MAINT-002 (seed reproducibility; failing-seed corpus in CI),
ADR-0007 (JSONL histories and the linearizability oracle).

Proof-point copy blocks (draft-pending-audit):

> **Kill it. Then check.** `kill -9` relayd mid-write, restart it, and every
> acknowledged message is still there. That sentence is property P-01 in our
> correctness document, enforced by crash-injection tests (CRSH-) that run
> on every commit. If you don't believe it, run the tests.
>
> **Failures come with a seed.** When Relay's simulator finds a bug, it
> prints one number. Feed that seed back in and you get the identical
> execution — same arrivals, partitions, and crash timing. Every seed
> that ever found a bug lives in a corpus CI replays forever.
>
> **The history doesn't lie.** Every simulated run emits a JSONL operation
> history, and a linearizability checker validates it against a reference
> model. If Relay ever delivered a message it shouldn't have, the checker
> would say so — mechanically, not rhetorically.

Forbidden phrasings: "proven correct" / "formally verified" (the oracle
checks histories, not an end-to-end proof); "bug-free" or "cannot lose
data" (acked sends, crash-stop only; NG-07, NG-09); "battle-tested in
production" (no deployments exist).

### 4.2 Pillar 2 — Honest about at-least-once

Pillar statement: Relay is at-least-once and says so everywhere it
matters; the non-guarantee list is a first-class deliverable and a selling
point — a queue precise about its limits is safer to build on.

Evidence: NG-01 (no exactly-once delivery), NG-04 (visibility expiry is
"not before", never exact-instant), P-02 LEASE-EXCL, P-06 DELETE-IDEM.

Proof-point copy blocks (draft-pending-audit):

> **We will not sell you exactly-once.** Relay delivers at-least-once.
> Under failure, you can receive a message twice; your consumers must be
> idempotent. Every queue vendor faces this physics — we're just telling
> you before you deploy, not after your incident review.
>
> **What we do guarantee instead.** While a lease is live, no second
> consumer holds that message (P-02, model-checked); deleting with the
> same receipt handle twice is harmless (P-06). Those properties make
> idempotent consumption tractable — machine-checked, not promised.
>
> **"Not before" is the honest contract.** A visibility timeout means the
> message will not reappear before the deadline — not that it reappears at
> the deadline instant (NG-04). We document the difference because your
> retry logic depends on it.

Forbidden phrasings: "exactly-once" in any construction, including
"effectively exactly-once"; "guaranteed delivery" without the
at-least-once qualifier in the same sentence or visual block; any wording
implying exact-instant expiry.

### 4.3 Pillar 3 — Failure is a test case

Pillar statement: the failures other queues first meet in production —
partitions, torn writes, disk-full, leader crashes — are simulator inputs,
and every failure the simulator ever found is a permanent regression test.

Evidence: the simulation harness (crates/relay-sim: SimClock, SimNet,
SimDisk, SimRng, single-threaded virtual-time executor; SIM-), the
failing-seed corpus (NFR-MAINT-002, CI-gated from R3), P-08 (SIM-RAFT).

Proof-point copy blocks (draft-pending-audit):

> **The network is a parameter.** Relay's simulator owns the clock,
> network, disk, and randomness. Partitions, reordering, torn writes, and
> crash timing are inputs we sweep, not accidents we await. A whole
> cluster's failures run in one deterministic thread.
>
> **Split-brain is on the syllabus.** Partition the cluster any way you
> like: no two nodes ever grant a live lease on the same message (P-08).
> We check this across partition schedules in simulation — because the one
> partition pattern you don't test is the one you get.
>
> **Bugs don't get discharged.** Every seed that ever produced a failure is
> checked into a corpus, and CI replays the corpus on every change. Relay's
> past bugs are its permanent test suite.

Forbidden phrasings: "handles every failure" / "fault-proof" (crash-stop
faults only, NG-07); "chaos-tested in production" (simulation is not
production); "survives anything" (no multi-region replication, NG-08).

### 4.4 Pillar 4 — Numbers with methods

Pillar statement: Relay publishes no performance number without hardware,
workload, and statistical treatment attached — a figure without its method
is an anecdote.

Evidence: [BENCHMARK_PLAN.md](./BENCHMARK_PLAN.md) statistical treatment
(reference hardware, warmup, repeated runs, percentiles; BENCH- family)
and NFR-PERF-005.

Proof-point copy blocks (draft-pending-audit):

> **Every number has a footnote, and the footnote is the point.** When we
> publish throughput or latency, you get the hardware, message size,
> workload shape, run count, and percentile method next to the number. If
> a figure arrives without its method, it didn't come from us.
>
> **Percentiles, not peaks.** We report sustained rates and tail latencies
> from repeated runs — not the best second we ever saw. The benchmark
> harness and its configuration are in the repository; the numbers are
> re-runnable on your own hardware.
>
> **fsync included.** Relay's latency numbers are measured with
> fsync-before-ack enabled — the durability contract you'd actually run —
> not with durability quietly disabled for the benchmark.

Forbidden phrasings: any specific throughput or latency figure before its
BENCH- result exists at R9 (targets stated only as targets, gate named);
"fastest" or unquantified "low latency"; any comparative performance
statement about another product (§5.2 rule 4).

## 5. The Claims Register

The complete list of approved outbound claims: outbound copy may contain
only sentences that restate a registered claim without strengthening it.
Adding a row requires evidence existing in [CORRECTNESS.md](./CORRECTNESS.md),
[BENCHMARK_PLAN.md](./BENCHMARK_PLAN.md), or the requirement register, plus
a §8 audit re-run. "Earliest gate" is the first gate after whose acceptance
the claim may be published; before it, it appears only labeled `planned`.

### 5.1 Registered claims

| Claim ID | Approved wording | Evidence | Earliest gate |
| --- | --- | --- | --- |
| CLM-001 | An acknowledged message survives `kill -9`: WAL fsync precedes every ack. | P-01, NFR-DUR-001, CRSH- family, ADR-0008 | R2 |
| CLM-002 | Crash recovery replays the WAL to the exact pre-crash acknowledged state. | NFR-DUR-002, CRSH- family | R2 |
| CLM-003 | Torn and partial writes are detected by CRC and truncated only at the log tail. | NFR-DUR-003, CRSH- family | R2 |
| CLM-004 | Disk-full fails writes cleanly with no corruption; reads continue. | NFR-DUR-004, CRSH- family | R2 |
| CLM-005 | On fsync failure the process crashes rather than pretending durability (fsyncgate rule). | NFR-DUR-005, ADR-0008 | R2 |
| CLM-006 | No two consumers hold a live lease on one message. | P-02, MODL-, SIM- families | R1 (single node); R7 (cluster wording) |
| CLM-007 | Deleting the same receipt handle twice is idempotent. | P-06, CORE-, MODL- families | R1 |
| CLM-008 | Every delivered message was previously sent, byte-identical; Relay never invents or alters payloads. | P-10, MODL- history check | R1 |
| CLM-009 | Within a message group, delivery order equals acknowledged send order. | P-04, FR-FIFO-002, FIFO- family | R4 |
| CLM-010 | The 5-minute deduplication window holds exactly at both boundaries; a duplicate send returns the original message ID. | P-05, FR-FIFO-007, CORE- boundary tests | R4 |
| CLM-011 | Every message is eventually delivered or dead-lettered; none is silently dropped inside Relay. | P-03, FR-QUEUE-017, SIM- liveness | R4 |
| CLM-012 | Any simulation failure reproduces exactly from its printed seed. | NFR-MAINT-002, SIM- family | R3 |
| CLM-013 | The failing-seed corpus is checked in and replayed by CI on every change. | NFR-MAINT-002, SIM- family | R3 |
| CLM-014 | Every operation history is validated by a linearizability checker against a reference model. | ADR-0007, MODL- family | R3 |
| CLM-015 | Publish delivers an independent copy to every matching subscription; filter policies are enforced at delivery. | FR-TOPIC-003, FR-TOPIC-004, TOPC- family | R5 |
| CLM-016 | The wire parser is bounded and fuzz-gated: every length is checked before allocation, and the fuzz corpus gates CI. | FR-API-002, NFR-SEC-002, FUZZ- family | R6 |
| CLM-017 | Receipt handles are unforgeable (HMAC-SHA256) and single-use per delivery. | P-07, NFR-SEC-001, WIRE- security tests | R6 |
| CLM-018 | Overload produces bounded backpressure and shed, never collapse. | NFR-AVAIL-003, WIRE- family | R6 |
| CLM-019 | No double-lease across any network partition. | P-08, SIM-RAFT | R7 |
| CLM-020 | No acknowledged write is lost across leader failover. | P-09, FR-REPL-003, SIM-RAFT, MODL- | R7 |
| CLM-021 | A 3-node cluster serves reads and writes with one node down. | NFR-AVAIL-001, RAFT- family | R7 |
| CLM-022 | Sustained single-node throughput at 256-byte bodies: pending measured values (target ≥ 20,000 msg/s send+receive+delete). | BENCH- throughput result per NFR-PERF-001, NFR-PERF-005 | R9 |
| CLM-023 | p99 send-to-ack latency with fsync-before-ack: pending measured values (target ≤ 15 ms). | BENCH- latency result per NFR-PERF-002, NFR-PERF-005 | R9 |
| CLM-024 | Long-poll wakeup after a matching send: pending measured values (target ≤ 10 ms; a goal, not a contract — NG-05). | BENCH- wakeup result per NFR-PERF-003, NG-05 | R9 |
| CLM-025 | Crash recovery of a 10 GiB WAL: pending measured values (target ≤ 30 s). | BENCH- recovery result per NFR-PERF-004 | R9 |
| CLM-026 | Clean leader kill to first new acknowledged write: pending measured values (target ≤ 5 s; simulated at R7, measured at R9). | BENCH- failover result per NFR-AVAIL-002 | R9 |
| CLM-027 | Mutation testing on relay-core kills ≥ 85% of mutants. | NFR-MAINT-003, MUT- family | R4 |
| CLM-028 | Relay is at-least-once, never exactly-once; consumers must be idempotent. | NG-01 | R0 (non-guarantees may be stated at any time) |

CLM-022..CLM-026 must be rewritten with measured values, hardware, and
statistical treatment before publication; a pending row is unpublishable.

### 5.2 Forbidden claims

Forbidden in all outbound copy, in any wording, at any gate:

1. **"Exactly-once"** in any construction, including hedged forms (NG-01);
   where inferable, the at-least-once contract is stated (FR-MKT-003).
2. **"Zero data loss" unqualified** — durability wording is always scoped
   to acknowledged sends and crash-stop faults (P-01, NG-07, NG-09).
3. **"Infinitely scalable"** or any unbounded scaling claim (NG-08).
4. **Comparative benchmarks without a same-hardware head-to-head** — no
   performance comparison against any named product unless we measured
   both systems on identical hardware with methods published for both,
   under [BENCHMARK_PLAN.md](./BENCHMARK_PLAN.md) rules; no exceptions
   for numbers taken from the other product's own site.
5. **Global ordering claims** — ordering is per message group only (NG-03).
6. **Latency contracts** — wakeup and delivery targets are goals (NG-05).
7. **Byzantine or adversarial-node tolerance** (NG-07).
8. **Any implication of a production track record** before real deployments
   exist and consent to be cited.

## 6. Deliverables by Gate (Generation Plan)

Nothing is announced before its gate. Each deliverable below is `planned`
and carries the claims it may use.

### 6.1 R6 — README quick start and honest FAQ

**README quick start** (structure fixed; commands finalized at R6): install
the static binary, start `relayd`, create a queue with `relayctl`, send one
message, `kill -9` relayd, restart, and receive the message that survived.
It ends on CLM-001 deliberately: the first thing a reader does with Relay
is watch a guarantee hold.

**Honest FAQ** (draft-pending-audit; answers cite only registered claims):

> **Is delivery exactly-once?** No — at-least-once (CLM-028); under
> failure you can receive a message twice. Relay makes idempotent consumers
> practical: exclusive live leases (CLM-006), idempotent deletes (CLM-007).
>
> **What happens if the process is killed mid-write?** Every acknowledged
> send was fsynced before its ack, so it survives (CLM-001); recovery
> replays the WAL to the exact pre-crash state (CLM-002). Unacknowledged
> sends may be lost — the ack is the durability boundary.
>
> **What happens when the disk fills up?** Writes fail cleanly with a
> stable error; nothing is corrupted; reads continue (CLM-004).
>
> **How fast is it?** We don't publish unmeasured numbers; benchmarks ship
> at R9 with hardware, workload, and statistical treatment attached.
>
> **What doesn't Relay do?** No exactly-once, no cross-queue atomicity, no
> global ordering, no exact-instant visibility expiry, no bounded latency
> contract, no messages over 256 KiB, no Byzantine tolerance, no
> multi-region replication, no durability for unacknowledged sends, no
> promise that FIFO matches standard throughput (NG-01..NG-10 in
> [CORRECTNESS.md](./CORRECTNESS.md)).

### 6.2 R9 — launch collateral

#### 6.2.1 Launch blog post outline

Title (working): "A message queue that shows its work." Sections:

1. **The thesis.** Queue documentation is full of adjectives; incidents
   are full of specifics. Relay inverts this: named properties with named
   tests, limitations beside them. States the §1 governing rule verbatim.
2. **A reproduced failure walkthrough.** One real bug from the failing-seed
   corpus, walked end to end: the property violated, the printed seed, the
   replay command (`cargo run -p relay-sim -- --seed <the real seed>`), the
   minimized schedule, and the fix. A reader with the repository replays
   the exact failure. If the corpus holds no violation at R9, the
   walkthrough uses an injected regression and says so.
3. **Benchmark tables.** Measured results for CLM-022..CLM-026, each with
   hardware, workload, run count, and percentile method per NFR-PERF-005.
   No comparative numbers.
4. **The non-guarantee list, verbatim.** NG-01..NG-10 in full from
   [CORRECTNESS.md](./CORRECTNESS.md), introduced with: "Here is what
   Relay does not do. We think publishing this list is the point."
5. **How to check us.** Clone, run the deterministic suites, replay the
   corpus, re-run the benchmarks. The closing call to action is
   verification, not adoption.

#### 6.2.2 Comparison-table rules

Any comparison table (site or README) obeys all of the following:

1. Rows are feature presence/absence only — never performance, reliability
   adjectives, or quality ratings.
2. Every competitor cell cites that competitor's own current public docs
   by link, with an access date.
3. Relay cells cite registered CLM- or NG- IDs.
4. Absence of a feature in Relay is stated as plainly as presence.
5. No performance comparisons, per §5.2 rule 4, regardless of source.
6. The table is re-audited at every release; a stale competitor citation
   is a claims-audit violation (§8.3).

#### 6.2.3 Demo script — "kill the leader live" runbook

A rehearsed, scripted live demo (recorded fallback required). Steps:

1. Start a 3-node local cluster with `relayctl`; show members and leader.
   Start a visible producer sending sequenced messages and an acking
   consumer; show the sequence counter advancing.
2. `kill -9` the leader process on camera.
3. Show the consumer stall and recover as a new leader is elected; state
   the measured failover figure from CLM-026 while it happens.
4. Stop the producer; drain the consumer; verify no acknowledged sequence
   number is missing and count any duplicates.
5. Say the honest sentence, scripted verbatim: "Note what happened: nothing
   acknowledged was lost — that's the guarantee (CLM-020) — and you may
   have seen a duplicate — that's at-least-once (CLM-028). Both behaviors
   are the contract."
6. Replay a corpus seed on screen to show the same failure class running
   deterministically in the simulator (CLM-012).

The demo never edits output, never trims the stall, and never re-takes to
hide a duplicate delivery.

#### 6.2.4 Site copy blocks

Full draft copy, status draft-pending-audit; each block must pass the R9
claims audit before any page ships (no website before 1.0, §9 — drafting
now means the audit gates the copy, not the deadline).

**Hero block:**

> **Relay**
>
> The self-hosted queue whose delivery guarantees are machine-checked, not
> asserted.
>
> Every guarantee is a named property with a named test. Every failure our
> simulator ever found replays from a seed. Every limitation is published
> next to the guarantees. At-least-once delivery, honestly.
>
> [Read the guarantees] [Read the non-guarantees]

Both buttons carry equal visual weight — a design requirement, not a
suggestion (FR-MKT-003).

**Feature section 1 — Durability you can interrupt:**

> **Pull the plug. We already did.**
>
> An acknowledged send is fsynced before you hear about it (ADR-0008).
> `kill -9` the process and every acked message survives; recovery replays
> the log to the exact pre-crash state. Torn writes are detected and
> truncated only at the tail; disk-full fails cleanly; a failing fsync
> crashes the process rather than lying to you — all enforced by
> crash-injection tests on every commit. (CLM-001..CLM-005)

**Feature section 2 — Failure is a test case:**

> **Your outage, as a unit test.**
>
> Relay's deterministic simulator owns the clock, network, disk, and
> randomness. Partitions, reordering, crash timing — inputs, not accidents.
> No double-lease across any partition; no lost ack across failover; and
> when a run fails, it prints a seed that reproduces it byte-for-byte,
> forever. The failing-seed corpus is checked in and CI replays it on
> every change. (CLM-012, CLM-013, CLM-019, CLM-020)

**Feature section 3 — Numbers with methods:**

> **Benchmarks you can re-run.**
>
> Every published number carries its hardware, workload, run count, and
> percentile method — and fsync-before-ack stays on, because that's the
> configuration you'd actually run. The harness is in the repository;
> results reproduce on your machines. No comparisons against other systems
> unless we measured both on identical hardware — so far, we haven't, so
> there aren't any. (CLM-022..CLM-026, NFR-PERF-005)

**Honest-limits section:**

> **What Relay does not do.**
>
> No exactly-once delivery — Relay is at-least-once and your consumers
> must be idempotent. No cross-queue atomicity. No global ordering across
> groups, queues, or topics. Visibility expiry is "not before", never
> exact-instant. Latency targets are goals, not contracts. No messages
> over 256 KiB. No Byzantine fault tolerance — crash-stop faults only. No
> multi-region replication. Unacknowledged sends may be lost: the ack is
> the durability boundary. FIFO throughput is not promised to match
> standard throughput.
>
> This list (NG-01..NG-10) ships in our correctness documentation and here
> on purpose. A queue precise about its limits is one you can build on.

### 6.3 R10 — release deliverables

**Launch checklist** (every item blocks the announcement):

1. All copy scheduled for publication passes the MKT- automated check.
2. Human claims audit completed and signed (§8.2, FR-MKT-004/005).
3. CLM-022..CLM-026 rewritten with measured values; no "pending measured
   values" wording anywhere in outbound copy.
4. Non-guarantee list present verbatim in the announcement and README.
5. Any comparison table re-verified within 14 days; access dates updated.
6. Every link in outbound copy resolves to the released tag, not a branch.
7. Demo recording verified unedited per §6.2.3.

**Release-announcement template** (structure fixed; bracketed fields are
the only variable content, each carrying a claim ID or gate reference):

1. Headline: "Relay [version] is released."
2. What is machine-checked in this release: [CLM- IDs whose gates are
   accepted, each with a one-line restatement of its approved wording].
3. What is measured: [benchmark table per NFR-PERF-005].
4. What Relay does not do: [NG-01..NG-10 verbatim].
5. What changed since [previous version]: [changes tagged with gate or ADR].
6. How to verify: [commands to run the deterministic suites, replay the
   corpus, and re-run benchmarks].

**HN post draft** (draft-pending-audit; bracketed annotations are stripped
only after the audit confirms each one):

> Show HN: Relay — a self-hosted message queue whose delivery guarantees
> are machine-checked
>
> I built Relay because every queue I've operated made claims its
> incidents contradicted. Every guarantee is a named property with a named
> proving test [governing rule, §1]; a deterministic simulator injects
> crashes, partitions, and torn writes, and any failure replays exactly
> from a seed [CLM-012]; a linearizability checker validates operation
> histories against a reference model [CLM-014]; the non-guarantee list is
> published as prominently as the guarantees [FR-MKT-003]. It is
> at-least-once, not exactly-once — consumers must be idempotent
> [CLM-028]. Acked messages survive kill -9 [CLM-001]; no lost acks across
> failover [CLM-020]; no double-lease across partitions [CLM-019].
> Benchmarks with full methods are in the repo [CLM-022..CLM-026]. I'd
> value attempts to falsify any of these — the seeds and checker are in
> the repository.

**Reddit post draft** (r/programming or r/rust; same annotation rule):

> Relay: a message queue where the correctness claims are CI jobs
>
> Instead of writing "reliable" in the README, Relay names ten properties
> (durable ack, lease exclusivity, FIFO-per-group, exact dedup windows, no
> lost ack across failover, and five more) mapped to tests that run on
> every commit [CLM-001..CLM-021], and ten non-guarantees, starting with
> "no exactly-once delivery" [CLM-028] — pretending otherwise is how
> queues earn their incident reports. Rust, hand-rolled WAL and Raft
> [ADR-0002, ADR-0003], deterministic simulation with seed replay
> [CLM-012]. Criticism of the method is as welcome as of the code.

**Badge policy for the README:**

1. Permitted badges: CI status on the default branch, latest release
   version, license, MSRV; each links to the live source of truth.
2. Every additional badge must map to a registered CLM- claim or a live CI
   job (a fuzz-corpus badge maps to CLM-016 only while fuzzing gates CI).
3. Forbidden: "production ready" badges; download- or star-count vanity
   badges; coverage badges without a CI-published report; badges for
   suites not currently green; third-party quality scores we cannot cite.
4. A badge whose backing job is removed or goes persistently red is
   removed in the same change; a red badge left up is an audit violation.

## 7. Voice and Style Rules

1. Plain, falsifiable sentences: every marketing sentence should be one a
   reader could in principle prove wrong; otherwise it is cut.
2. Every superlative is banned: "fastest", "best", "most reliable",
   "unmatched", "blazing", "rock-solid", "bulletproof", "enterprise-grade",
   "world-class". A measured number with its method replaces all of them.
3. "We prove" appears only where a named test or checker is cited in the
   same block; otherwise "we test" (narrower than a proof) or "we plan"
   (gate not passed). Never "we ensure" — proof claimed, nothing cited.
4. Guarantees are stated with their scope in the same breath: "acknowledged
   sends survive crashes" — never "your data is safe".
5. Jargon is defined on first use in any artifact or linked to
   [GLOSSARY.md](./GLOSSARY.md): linearizability, at-least-once, lease,
   visibility timeout, WAL, fsync, deterministic simulation, seed,
   crash-stop. A queue novice must be able to follow the hero copy.
6. No fear-based copy about competitors and no disparagement; §6.2.2 is
   the only sanctioned form of comparison.
7. Numbers appear with units, hardware, and treatment, or not at all.
8. Public status vocabulary matches the repository's: `accepted`,
   `in progress`, `planned`, `deferred` — same meanings.

## 8. The Claims-Audit Process

The MKT- test family (registered in
[OPERATIONS_TEST_PLAN.md](./OPERATIONS_TEST_PLAN.md), standard row format
`| ID | Mechanics | Pass criteria | Earliest gate |`) makes claims
discipline executable.

### 8.1 Automated check: MKT-CLAIMS-SCAN

Mechanics: every outbound-copy source file (README, site copy under
`marketing/`, announcement templates) marks each claim-bearing passage
with a grep-able marker (`<!-- CLM-001 CLM-002 -->`) adjacent to the
passage it covers. The scanner:

1. verifies every marker's claim IDs exist in the §5.1 register;
2. verifies no forbidden phrase from §5.2 or §7 rule 2 appears anywhere in
   scanned sources (fixed case-insensitive deny-list kept beside the
   scanner);
3. verifies each cited claim's earliest gate is at or before the release
   gate being built, and that "pending measured values" appears in no
   publishable file;
4. fails the build on any violation, printing file, line, and rule.

Pass criteria: zero violations. Earliest gate: R6 (first outbound copy);
CI-gating from R6 onward; deterministic, zero-flake — a flake is a bug.
The scanner cannot judge whether prose subtly strengthens a claim; that is
the human audit's job, which starts from this mechanically clean baseline.

### 8.2 Human audit checklist (run at R9 and R10; FR-MKT-004, FR-MKT-005)

For every artifact scheduled for publication, a reviewer who is not its
author answers each item in writing in the release record:

1. Does every claim-bearing sentence restate its registered CLM- wording
   without strengthening scope, dropping qualifiers, or promoting status?
2. Is the at-least-once contract stated wherever exactly-once could be
   inferred (FR-MKT-003)?
3. Does every performance figure carry hardware, workload, and statistical
   treatment (FR-MKT-002, NFR-PERF-005)?
4. Are all gates behind cited claims `accepted` on mainline today?
5. Does any comparison obey all six rules of §6.2.2?
6. Do any images, diagrams, or demo edits imply unregistered behavior?
7. Is the non-guarantee list present where §6 requires it, verbatim?

Any "no" blocks publication of that artifact until corrected and
re-audited.

### 8.3 Violation handling

If published copy is found to violate the register, at any time:

1. Pull the copy immediately — unpublish, or where impossible (a mailed
   announcement, an aggregator post), publish a correction in the same
   channel within one business day.
2. File a correctness-of-claims incident under the
   [OPERATIONS_TEST_PLAN.md](./OPERATIONS_TEST_PLAN.md) incident procedure,
   recording the violating text, the register row contradicted, how it
   passed audit, and the process change that prevents recurrence.
3. Add the violating phrasing to the MKT-CLAIMS-SCAN deny-list where a
   mechanical pattern exists.
4. Record the incident as a required regression-review item in the next
   release's audit.

A claims violation is treated as seriously as a correctness bug because it
is one: the product's thesis is that its claims are trustworthy.

## 9. Explicit Deferrals

Deferred until after 1.0 (R10 acceptance); each is fail-closed — until
reopened, the answer is no. All reopen via OQ-10 in [OPEN_QUESTIONS.md](./OPEN_QUESTIONS.md).

1. **No paid channels.** No advertising, sponsorships, paid placement, or
   paid newsletters.
2. **No logo or brand system.** The wordmark is the plain product name; no
   logo, palette, or brand guidelines are commissioned.
3. **No website build.** The §6.2.4 site copy is drafted and audited, but
   no site is built or hosted before 1.0; the README is the sole public
   surface.
4. **No analytics.** No site tracking, download telemetry, or install-base
   measurement; any post-1.0 proposal must specify what is collected, why,
   and the disclosure text.

Deferral does not defer the discipline: when OQ-10 reopens any of these,
the resulting artifacts enter this document's register, scanner scope, and
audit process before anything ships.
