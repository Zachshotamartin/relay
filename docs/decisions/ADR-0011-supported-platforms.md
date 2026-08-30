# ADR-0011: Supported Platforms

- Status: accepted
- Date: 2026-08-30
- Related findings or requirements: NFR-DUR-001, NFR-DUR-005, NFR-SEC-005, FR-OPS-001, P-01

## Context

Relay's durability contract is only as strong as its knowledge of the
platform's fsync, rename, and directory-sync semantics (ADR-0008,
NFR-DUR-001, NFR-DUR-005). Every supported platform multiplies the CI
fault-injection matrix — CRSH- and STOR- evidence must be produced per
platform, or the durability claim silently narrows to "tested on the
maintainer's laptop." Platform tiers are therefore a correctness decision,
not a distribution preference, and they are fixed before R0 configures CI.

## Decision

- **Tier 1 — Linux x86_64 and Linux aarch64**: full support. All guarantees
  (P-01 through P-10) are claimed and CI-verified on both architectures;
  release binaries are built, crash-injected, and benchmarked on both.
  io_uring is an optional IO backend behind a runtime flag; a portable
  read/write/fsync fallback is required and remains the default, so no
  guarantee ever depends on io_uring being present or the kernel being
  recent.
- **Tier 2 — macOS aarch64, development only**: `relayd` and the test suite
  build and run for contributor workflows. No durability claim is made on
  macOS: its fsync does not guarantee media persistence without
  `fcntl(F_FULLFSYNC)`, and Relay does not certify that path. Production
  deployment on macOS is unsupported and documentation says so.
- **Windows: unsupported at 1.0.** No build target, no CI lane, no claims.

## Alternatives Considered

- **Windows as a supported target**: rejected for 1.0. Durable-write
  semantics differ end to end — `FlushFileBuffers` behavior, no POSIX
  atomic-rename guarantees for the segment-swap path, a different
  file-locking and permissions model that invalidates the 0700 data-dir
  check (NFR-SEC-005) — so honest support means a parallel CRSH- evidence
  matrix and a second storage abstraction, roughly doubling R2's cost for a
  deployment population Relay's early adopters do not represent.
- **macOS as tier 1**: rejected — certifying durability on macOS requires
  F_FULLFSYNC (an order-of-magnitude fsync slowdown) plus crash-injection
  infrastructure on Apple hardware in CI; the cost buys production support
  for a platform where self-hosted queues do not run in production.
- **io_uring as the mandatory IO path**: rejected — it would raise the
  minimum kernel version, complicate the seccomp/hardening story for
  security-conscious operators, and tie correctness to a subsystem with a
  history of kernel CVEs; as an optional backend it is pure upside.
- **musl-only static linking for tier 1**: rejected as a blanket rule —
  static musl builds are published for portability, but the allocator
  performance gap under multithreaded load is real, so glibc-based builds on
  the reference platform remain the benchmarked configuration; both are
  produced from the same source with provenance (FR-OPS-001).

## Consequences

- Easier: the CRSH-/STOR- matrix stays two-platforms wide; the threat model
  and hardening guidance target one OS family; benchmark numbers (NFR-PERF)
  are stated against a single well-defined reference platform.
- Harder: Windows-shop adoption is deferred; contributors on macOS must trust
  Linux CI for durability results, and any macOS-only test pass is explicitly
  not evidence (spine §1's rule that deferred work is forbidden as completion
  evidence applies).
- Revisit when: OQ-6 in [OPEN_QUESTIONS.md](../OPEN_QUESTIONS.md) — Windows
  support demand post-1.0 — is answered with real user demand; reopening it
  requires a superseding ADR that budgets the full per-platform durability
  evidence, never a build-target-only port.
