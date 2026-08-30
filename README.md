# Relay

Relay is a self-hosted message queue and pub/sub service whose delivery
guarantees are machine-checked by deterministic simulation and model
checking, not asserted in documentation. It is designed as a single static
binary (`relayd`, administered by `relayctl`) offering standard and FIFO
queues, visibility-timeout leases, dead-letter queues, topics with filtered
fanout, a bounded fuzzed binary wire protocol, and Raft replication — with
every one of those guarantees backed by a named, replayable test before it
may be claimed.

The guarantee headline, stated once and never softened or inflated:

> **Relay delivers at-least-once, and that guarantee is machine-checked.**
> **Relay does not and will not claim exactly-once delivery.** Consumers
> must be idempotent. The full guarantee list (P-01–P-10) and the equally
> binding non-guarantee list (NG-01–NG-10) live in
> [docs/CORRECTNESS.md](docs/CORRECTNESS.md), which is the only document
> allowed to define them.

> **Implementation status:** nothing is built. This repository contains a
> complete planning documentation set and eleven accepted architecture
> decision records — and that is all. There is no code, no binary, no test,
> no benchmark, and no CI pipeline. Every build gate R0–R10 is `planned`.
> The ADRs are `accepted` because they are decisions, not implementation.
> Any sentence anywhere in this repository that reads as if Relay runs
> today is a documentation defect.

## What Exists Today Versus What Is Planned

Exists today:

- The documentation set under [docs/](docs/README.md): product requirements,
  build plan, architecture, correctness, operations and test plan, threat
  model, benchmark plan, marketing, glossary, and open questions.
- Eleven accepted ADRs under [docs/decisions/](docs/README.md#architecture-decisions).

Does not exist today: everything else. No crate compiles because no crate
has been created. No number in [docs/BENCHMARK_PLAN.md](docs/BENCHMARK_PLAN.md)
has been measured. No property in [docs/CORRECTNESS.md](docs/CORRECTNESS.md)
has been checked.

## Implementation Status Snapshot

Each gate is defined in [docs/BUILD_PLAN.md](docs/BUILD_PLAN.md). A gate's
evidence is unlocked only when the gate is accepted on mainline with its
named automated checks green. Today, every row is `planned`.

| Gate | Status | Evidence unlocked when accepted |
| --- | --- | --- |
| R0 | planned | Repository, Rust toolchain, CI, and architecture checks exist and are green. |
| R1 | planned | Single-node in-memory core queue semantics are correct under the model checker. |
| R2 | planned | The durable WAL storage engine survives crash, torn-write, and disk-full injection. |
| R3 | planned | Deterministic simulation reproduces any failure from a seed and runs in CI with a checked-in corpus. |
| R4 | planned | FIFO groups, deduplication, delay, DLQ, and redrive behave exactly to specification. |
| R5 | planned | Topics, subscriptions, and filter policies fan out correctly. |
| R6 | planned | A bounded, fuzzed wire API with authentication, quotas, and long polling serves real clients. |
| R7 | planned | Raft replication survives partition and failover with no double-lease and no lost ack. |
| R8 | planned | Metrics, tracing, the admin surface, and a runbook make Relay operable. |
| R9 | planned | Published benchmarks, a failure-injection report, and evidence-bound marketing support stated claims and no more. |
| R10 | planned | Packaging, deployment, upgrade, rollback, and backup/restore satisfy the 1.0 release gate. |

Status vocabulary is fixed across all documents: `accepted` (implemented on
mainline, backed by its named automated gate), `in progress` (present on a
branch, not a claim), `planned` (specified, not implemented), `deferred`
(outside the named phase; forbidden as completion evidence). A package, a
type, a stub, or a happy-path unit test is never completion.

## Quick Start

### Today

The only runnable thing in this repository is reading. Start at
[docs/README.md](docs/README.md) and follow its reading order:
requirements, then the build plan, then architecture, then correctness.

### Planned (post-R6) — interface sketch, not yet real

The following is a **PLANNED interface sketch**. None of these commands,
binaries, flags, or outputs exist. They are requirements bound to the R6
gate (wire API and clients) and the R8 gate (`relayctl` coverage), shown
here only so readers can see the intended shape. They may change through
the normal ADR process before they are ever implemented.

```text
# PLANNED — no such binary exists today
$ relayctl queue create orders
created queue "orders"

$ relayctl send orders --body "order-1042"
message id 01JD4M6E8ZK9W3R5T7XYVBNQAC acknowledged (durable)

$ relayctl receive orders --max 1 --wait 20
message 01JD4M6E8ZK9W3R5T7XYVBNQAC  receipt rh1_...  body "order-1042"

$ relayctl delete orders --receipt rh1_...
deleted

$ relayctl queue describe orders
name=orders  available≈0  in-flight≈0  (counts approximate, staleness labeled)
```

The planned server listens on port 7414 (API), 7415 (metrics/health), and
7416 (Raft), reads `/etc/relay/relay.toml`, and stores data under
`/var/lib/relay` (`./relay-data` in development). All of that is
specification, not description.

## What Relay Guarantees — and Refuses to Pretend

Planned, machine-checked guarantees (owned by
[docs/CORRECTNESS.md](docs/CORRECTNESS.md), each mapped to named proving
tests): acknowledged sends survive crashes; no two consumers hold a live
lease on one message; every message is eventually delivered or
dead-lettered; per-group FIFO order; exact dedup-window boundaries;
idempotent delete; unforgeable single-use receipt handles; no double-lease
across partitions; no lost acknowledged write across failover; no invented
messages.

Explicit non-guarantees, repeated wherever they could be misread otherwise:
no exactly-once delivery; no cross-queue atomicity; no global ordering; no
exact-instant visibility expiry; no bounded delivery latency; no bodies
over 256 KiB; no Byzantine fault tolerance; no multi-region replication;
unacknowledged sends may be lost; FIFO throughput is not promised to match
standard-queue throughput.

Performance targets (they are targets, not results — nothing has been
measured) are defined and disciplined in
[docs/BENCHMARK_PLAN.md](docs/BENCHMARK_PLAN.md): 20,000 msg/s sustained
single-node lifecycle throughput at 256-byte bodies, p99 send-to-ack of
15 ms with fsync-before-ack, 10 ms long-poll wakeup, and 30-second crash
recovery of a 10 GiB WAL, all on the fixed reference hardware profile.

## Documentation

Everything normative lives under [docs/](docs/README.md):

- [Documentation index and conflict rules](docs/README.md)
- [Product requirements](docs/PRODUCT_REQUIREMENTS.md)
- [Build plan (gates R0–R10)](docs/BUILD_PLAN.md)
- [Architecture](docs/ARCHITECTURE.md)
- [Correctness: guarantees and non-guarantees](docs/CORRECTNESS.md)
- [Operations and test plan](docs/OPERATIONS_TEST_PLAN.md)
- [Threat model](docs/THREAT_MODEL.md)
- [Benchmark plan](docs/BENCHMARK_PLAN.md)
- [Marketing and claims audit](docs/MARKETING.md)
- [Glossary](docs/GLOSSARY.md)
- [Open questions](docs/OPEN_QUESTIONS.md)
- [Decision records](docs/decisions/)

When documents disagree, the precedence order in
[docs/README.md](docs/README.md#conflict-and-status-rules) controls.

## Status Discipline

This README may never claim ahead of the gates. Concretely:

- Every implementation statement in this file must name its gate, and no
  statement may present a `planned` item as existing.
- When a gate is accepted, this README's snapshot table is updated in the
  same change that lands the acceptance evidence — never before.
- In-memory evidence is never promoted to a durability claim, single-node
  evidence is never promoted to a replication claim, and simulated faults
  are never promoted to production hardening.
- Performance may be quoted here only as targets until
  [docs/BENCHMARK_PLAN.md](docs/BENCHMARK_PLAN.md) results exist, and then
  only with hardware, workload, and statistical treatment attached.
- Reversals of any accepted decision are recorded as new ADRs, not edited
  into this file.

If this README and the documents under [docs/](docs/README.md) ever
disagree, this README is wrong.
