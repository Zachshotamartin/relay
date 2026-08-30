# Relay

> Relay is a documented design for a verification-first message queue. No
> binary exists, no test exists, no benchmark exists, and no delivery
> guarantee has been demonstrated.

That is the binding public product claim from
[BUILD_PLAN.md §2.2](docs/BUILD_PLAN.md#22-current-honest-product-claim).
“No test exists” means that no product-semantics test has earned a queue,
durability, replication, or delivery claim. This branch contains only R0
repository-policy tests and enforcement tooling; those do not implement or
verify a message queue.

## Current status

R0 is `in progress`. The workspace skeleton and its repository checks are
present on this branch, but R0 is not accepted. R1 through R10 remain
`planned`; no later gate has started.

| Gate | Status | Scope when accepted |
| --- | --- | --- |
| R0 | in progress | Repository, toolchain, CI, and architecture checks. |
| R1 | planned | In-memory core semantics and the linearizability model. |
| R2 | planned | WAL durability and crash recovery. |
| R3 | planned | Deterministic simulation and seed replay. |
| R4 | planned | FIFO, delay, retention, dead-lettering, and redrive. |
| R5 | planned | Topics, subscriptions, filters, and fanout. |
| R6 | planned | Wire protocol, clients, authentication, quotas, and fuzzing. |
| R7 | planned | Raft replication and partition/failover evidence. |
| R8 | planned | Operations, observability, admin APIs, and runbooks. |
| R9 | planned | Benchmarks, capacity evidence, and claims audit. |
| R10 | planned | Packaging, release, upgrade, backup, and recovery. |

The only status words are `accepted`, `in progress`, `planned`, and
`deferred`. A crate skeleton, parser stub, happy-path test, simulated fault,
or feature-branch result is not acceptance evidence.

## Contributor setup

Read [the documentation index and conflict rules](docs/README.md) before
changing code, then follow [CONTRIBUTING.md](CONTRIBUTING.md). For R0
repository-tooling work, install the pinned Rust toolchain and run the
workspace checks from the repository root:

```sh
rustup toolchain install 1.85.0 --profile minimal --component clippy --component rustfmt
cargo build --workspace --locked
cargo test --workspace --locked
```

These commands build and test repository enforcement code only. They do not
produce a Relay service.

## Normative documentation

The specification is binding and lives under `docs/`:

- [Documentation index, reading order, and conflict rules](docs/README.md)
- [Product requirements](docs/PRODUCT_REQUIREMENTS.md)
- [Ordered build gates](docs/BUILD_PLAN.md)
- [Architecture and frozen formats](docs/ARCHITECTURE.md)
- [Correctness properties and non-guarantees](docs/CORRECTNESS.md)
- [Operations and test plan](docs/OPERATIONS_TEST_PLAN.md)
- [Threat model](docs/THREAT_MODEL.md)
- [Benchmark plan](docs/BENCHMARK_PLAN.md)
- [Marketing and claims rules](docs/MARKETING.md)
- [Glossary](docs/GLOSSARY.md)
- [Open questions](docs/OPEN_QUESTIONS.md)
- [Architecture decision records](docs/decisions/)

When this README and the normative documents disagree, this README is wrong.
No performance number is a result until R9 evidence exists, and no delivery
guarantee may be stated until its terminal gate is accepted.
