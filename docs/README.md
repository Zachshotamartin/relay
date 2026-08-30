# Relay Documentation Index

Relay is a self-hosted message queue and pub/sub service whose delivery
guarantees are machine-checked by deterministic simulation and model checking,
not asserted in documentation. This directory is the complete planning corpus
for that product. Nothing described here is built: every implementation gate
R0–R10 is `planned`, and the only `accepted` artifacts are the eleven
architecture decision records, which are decisions, not code.

The root [README](../README.md) is the honest status snapshot and quick start.
It distinguishes what exists today (this documentation set and the ADRs) from
everything else (which does not exist).

Every document uses one status vocabulary with identical wording: `accepted`
(implemented on mainline, backed by its named automated gate), `in progress`
(present on a branch, not a claim), `planned` (specified, not implemented),
and `deferred` (outside the named phase; forbidden as completion evidence).
A package, a type, a stub, or a happy-path unit test is never completion. A
document that needs a status other than these four is wrong and must be
fixed, not accommodated.

## Reading Order

Read these documents first and in this order:

1. [Product requirements](PRODUCT_REQUIREMENTS.md) defines Relay's users,
   the full requirement register (FR-QUEUE, FR-FIFO, FR-TOPIC, FR-API,
   FR-REPL, FR-ADMIN, FR-OPS, FR-MKT, NFR-DUR, NFR-PERF, NFR-AVAIL, NFR-SEC,
   NFR-MAINT — 108 requirements), user-visible semantics, limits, error
   behavior, and acceptance criteria for every operation.
2. [Build plan](BUILD_PLAN.md) defines implementation order as eleven gates
   R0–R10, each with prerequisites, owned files, ticket sequence, a
   test-driven evidence matrix, failure and security cases, acceptance
   evidence, and explicit deferrals, plus the requirement-to-evidence
   traceability matrix and the release-readiness checklist.
3. [Architecture](ARCHITECTURE.md) defines the workspace crates, the pure
   deterministic core state machine, the injected Clock/Rng/Disk/Net
   environment, the segmented WAL, Raft integration, wire protocol layering,
   and every on-disk and on-wire format at byte level.
4. [Correctness](CORRECTNESS.md) owns the guarantee list (properties P-01
   through P-10, each mapped to named proving tests) and the non-guarantee
   list (NG-01 through NG-10). Every other document cites it; no other
   document may state a guarantee it does not contain.
5. [Operations and test plan](OPERATIONS_TEST_PLAN.md) defines the test
   family matrices (CORE-, STOR-, CRSH-, SIM-, MODL-, FIFO-, TOPC-, WIRE-,
   FUZZ-, RAFT-, ADMN-, OPSX-, MIGR-, SOAK-, BENCH-, MUT-, MKT-), CI gating
   and flake policy, packaging, upgrade, rollback, backup/restore, and
   release mechanics.
6. [Threat model](THREAT_MODEL.md) defines assets, adversaries, trust
   boundaries, abuse cases, and controls, and binds every security claim to
   a named enforcement point plus adversarial evidence.
7. [Benchmark plan](BENCHMARK_PLAN.md) defines the only performance numbers
   Relay may ever publish: reference hardware, the relay-bench harness,
   workloads BENCH-01 through BENCH-08, statistical treatment, and the
   claim table separating what the numbers support from forbidden
   extrapolations.
8. [Marketing](MARKETING.md) defines positioning, messaging pillars (each
   citing a P-xx/NG-xx property or a BENCH result), the launch plan by gate,
   and the claims-audit checklist. It sits below CORRECTNESS.md and
   THREAT_MODEL.md and may never strengthen a claim beyond them.
9. [Glossary](GLOSSARY.md) gives controlled definitions for every term of
   art (lease, receipt handle, message group, dedup window, gate, evidence)
   so that requirement and test language stays unambiguous.
10. [Open questions](OPEN_QUESTIONS.md) records every deferred decision with
    a fail-closed default position and an explicit reopen trigger. Open
    items live here and nowhere else.
11. [Decision records](decisions/) contain the eleven accepted ADRs listed
    below. Each records one decision, its rejected alternatives, and its
    consequences; reversing any of them requires a new ADR.

Namespace note: the FR-MKT requirement family is an approved, user-directed
extension of the original requirement namespaces, added by explicit decision;
its claims remain evidence-bound like every other family.

## Architecture Decisions

All eleven ADRs are `accepted` as of 2026-08-30. They are decisions, not
implementation claims.

- [ADR-0001](decisions/ADR-0001-rust-language-and-toolchain.md) — Rust
  (edition 2024, MSRV 1.85) for a verification-first queue; rejects Go, Zig,
  and C++.
- [ADR-0002](decisions/ADR-0002-hand-rolled-segmented-wal.md) — hand-rolled
  segmented WAL storage engine; rejects RocksDB, SQLite, and any hosted
  database as the queue store.
- [ADR-0003](decisions/ADR-0003-in-house-raft-implementation.md) — in-house
  Raft; rejects openraft, raft-rs, and external coordination services, and
  accepts that R7 is the longest gate.
- [ADR-0004](decisions/ADR-0004-rwp-binary-protocol-no-sqs-compat.md) —
  custom RWP/1 framed binary protocol; explicitly not SQS-wire-compatible;
  an HTTP/JSON gateway is deferred to open questions.
- [ADR-0005](decisions/ADR-0005-injected-time-and-log-applied-clock.md) —
  injected time; AdvanceTime log entries are the sole source of time inside
  the core state machine.
- [ADR-0006](decisions/ADR-0006-ulid-ids-and-hmac-receipt-handles.md) — ULID
  message IDs and HMAC-SHA256 single-use receipt handles with key rotation
  by epoch.
- [ADR-0007](decisions/ADR-0007-jsonl-histories-and-linearizability-oracle.md)
  — JSONL operation histories checked by an in-house Wing–Gong
  linearizability checker over the reference model; rejects TLA+-only and
  Jepsen-only approaches.
- [ADR-0008](decisions/ADR-0008-fsync-before-ack-durability-contract.md) —
  durability contract: fsync-before-ack with bounded group commit; fsync
  failure crashes the process.
- [ADR-0009](decisions/ADR-0009-single-static-binary-deployment.md) — single
  static binary deployment (relayd plus relayctl); rejects modular services.
- [ADR-0010](decisions/ADR-0010-observability-stack.md) — Prometheus
  metrics, OTLP traces, and structured JSON logs under a named cardinality
  budget.
- [ADR-0011](decisions/ADR-0011-supported-platforms.md) — tier-1 Linux
  x86_64/aarch64; tier-2 macOS aarch64 dev-only; Windows unsupported at 1.0.

## Conflict and Status Rules

When documents disagree:

1. accepted ADRs control the decisions they record;
2. PRODUCT_REQUIREMENTS.md controls user-visible semantics and acceptance;
3. CORRECTNESS.md controls guarantee and non-guarantee claims;
4. BUILD_PLAN.md controls implementation order and gates;
5. ARCHITECTURE.md controls component boundaries and formats;
6. OPERATIONS_TEST_PLAN.md controls test, evidence, packaging, and release
   mechanics;
7. THREAT_MODEL.md controls security-claim binding; BENCHMARK_PLAN.md
   controls performance-claim binding; MARKETING.md may never strengthen a
   claim beyond 3 and 7;
8. implemented code and passing tests control claims about what works today
   (today: nothing).

Documentation must follow these rules:

- Label planned behavior as planned until its named tests and evidence pass.
- Name the release gate behind every implementation claim.
- Never promote in-memory evidence into a durability claim, single-node
  evidence into a replication claim, or a simulated fault into a
  production-hardening claim.
- Bind every security claim to a named enforcement point plus adversarial
  evidence.
- Marketing copy never strengthens a claim beyond what CORRECTNESS.md,
  THREAT_MODEL.md, and BENCHMARK_PLAN.md support.
- Record a reversal of any accepted decision in a new ADR instead of
  silently editing away the earlier one.
