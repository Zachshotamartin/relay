# ADR-0001: Rust Language and Toolchain

- Status: accepted
- Date: 2026-08-30
- Related findings or requirements: NFR-PERF-001, NFR-PERF-002, NFR-MAINT-002, NFR-SEC-002, NFR-SEC-008, P-01

## Context

Relay's thesis is that delivery guarantees are machine-checked, not asserted. That
thesis imposes three hard constraints on the implementation language before any
code exists. First, the deterministic simulation in `crates/relay-sim` requires a
single-threaded virtual-time executor whose interleavings the harness fully
controls; a language runtime with its own scheduler or collector injects
nondeterminism the harness cannot replay from a seed (NFR-MAINT-002). Second, the
performance targets (NFR-PERF-001: ≥ 20,000 msg/s sustained; NFR-PERF-002: p99
send-to-ack ≤ 15 ms with fsync-before-ack) leave no budget for stop-the-world
pauses in the ack path. Third, the wire parser handles untrusted input under a
fuzz-gated CI contract (NFR-SEC-002), so memory-unsafety bug classes must be
excluded statically, not chased with sanitizers.

Nothing is built. This is the first binding decision because every other ADR
assumes the workspace crate boundaries (`relay-core`, `relay-wal`, `relay-raft`,
`relay-sim`, `relay-model`, `relay-wire`, `relay-server`, `relay-client`,
`relay-cli`, `relay-bench`) and the toolchain gates that hold them apart.

## Decision

Relay is written in stable Rust, edition 2024, with the MSRV pinned at 1.85 and
raised only by a superseding ADR. The repository is a single Cargo workspace of
the ten crates above. CI enforces `-D warnings` on every build, a
`clippy::pedantic` baseline (individual lints allowed only with an inline
justification comment), `rustfmt` with the default profile, and `cargo-deny`
checking exact-pinned dependencies, the license allowlist, and the RustSec
advisory database on every pull request (NFR-SEC-008).

`relay-core` is forbidden from depending on tokio, the system clock, the system
RNG, or any IO crate; an architecture check in CI (gate R0) fails the build if
those dependencies appear in its tree.

## Alternatives Considered

- **Go**: rejected. GC pauses land directly in the p99 send-to-ack path and make
  NFR-PERF-002 a fight against the runtime rather than the disk. The goroutine
  scheduler cannot be driven deterministically from a test harness without
  forking the runtime, so seed-reproducible simulation (NFR-MAINT-002) degrades
  to statistical stress testing — exactly the weak fault-injection determinism
  Relay exists to avoid.
- **Zig**: rejected on ecosystem and maturity grounds. No stable 1.0 release to
  pin an MSRV-equivalent against, no mature TLS 1.3 implementation for
  FR-API-008, and an async model still in flux, which would put the project's
  most schedule-sensitive infrastructure on a moving target.
- **C++**: rejected for safety burden. The parser boundary (NFR-SEC-002) exposes
  use-after-free, out-of-bounds, and uninitialized-read bug classes that Rust
  excludes at compile time; recovering equivalent assurance requires permanent
  sanitizer and fuzzing discipline across the whole codebase, plus bespoke
  dependency auditing where `cargo-deny` gives Relay a single enforced tool.

## Consequences

- Easier: deterministic simulation with full interleaving control; refactoring
  under model-checker churn with the compiler enforcing invariants; one
  dependency-audit tool (`cargo-deny`) satisfying NFR-SEC-008.
- Harder: compile times grow with the workspace; async ecosystem coupling to
  tokio must be actively confined to the edge crates; contributor pool is
  narrower than Go's.
- Revisit when: an MSRV raise is needed for a language feature (superseding ADR
  required, with a stated migration window), or if `relay-core`'s no-IO
  discipline is ever violated — that is an architecture-check failure, not a
  judgment call. No OPEN_QUESTIONS entry reopens this decision.
