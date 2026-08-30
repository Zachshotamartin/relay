# Relay: Installation, Testing, Operations, and Release Plan

Document status: normative lifecycle, verification, and release specification
for Relay. Every gate, matrix row, procedure, and budget in this document is
`planned`: nothing is built, and no claim in this plan is current until its
named evidence passes on the supported matrix.

Last revised: 2026-08-30.

Companion sources of truth:

- [Product requirements](./PRODUCT_REQUIREMENTS.md)
- [Exhaustive build plan](./BUILD_PLAN.md)
- [Architecture](./ARCHITECTURE.md)
- [Correctness properties and non-guarantees](./CORRECTNESS.md)
- [Threat model](./THREAT_MODEL.md)
- [Benchmark plan](./BENCHMARK_PLAN.md)
- [Marketing and claims policy](./MARKETING.md)
- [ADR-0001: Rust language and toolchain](./decisions/ADR-0001-rust-language-and-toolchain.md)
- [ADR-0007: JSONL histories and linearizability oracle](./decisions/ADR-0007-jsonl-histories-and-linearizability-oracle.md)
- [ADR-0008: fsync-before-ack durability contract](./decisions/ADR-0008-fsync-before-ack-durability-contract.md)
- [ADR-0011: Supported platforms](./decisions/ADR-0011-supported-platforms.md)

This plan governs the complete lifecycle of Relay as a self-hosted message
queue: developer bootstrap, repository protection, dependencies, local data,
configuration, test design, shared harnesses, verification matrices, CI,
performance budgets, packaging, installation, upgrade, rollback, backup,
restore, diagnostics, privacy, incident response, and R0–R10 release evidence.

Relay's product thesis is that delivery guarantees are machine-checked, not
asserted. This document is therefore the enforcement arm of that thesis: it
names, for every requirement in the register, the deterministic test family
that proves it, the gate at which the proof is due, and the evidence artifact
that records the proof. Where [CORRECTNESS.md](./CORRECTNESS.md) states a
property P-01 through P-10, this plan names the matrix rows that check it;
where it states a non-guarantee NG-01 through NG-10, this plan forbids any
test, benchmark, or release note from implying otherwise.

## 1. Operating Model and Lifecycle Invariants

### 1.1 Verification-first lifecycle

Relay development is verification-first: the failing test, simulation seed,
model-check profile, or fuzz target exists before the code that satisfies it,
and the code is `accepted` only when its named gate replays that evidence green
on mainline. The lifecycle this document controls:

1. Create and protect the Git repository and its evidence conventions.
2. Bootstrap a clean developer machine to a full green local suite.
3. Review, pin, install, update, and remove dependencies with provenance.
4. Create and protect local data, configuration, keys, and state.
5. Validate configuration fail-fast before any listener or disk write.
6. Test the core state machine, WAL, crash recovery, simulation, model
   checking, FIFO, topics, wire protocol, Raft, administration, migration,
   soak, benchmarks, mutation coverage, and marketing claims.
7. Run deterministic CI on every pull request and deep suites nightly.
8. Measure throughput, latency, recovery time, memory, and binary size.
9. Build reproducible static artifacts with SBOM and provenance.
10. Install, bootstrap clusters, upgrade, roll back, uninstall, and purge.
11. Back up, restore, and rehearse disaster recovery every release.
12. Diagnose failures with redacted support bundles.
13. Apply explicit retention, privacy, and purge rules to message data.
14. Respond to correctness, security, and supply-chain incidents, including
    release revocation.
15. Close gates R0 through R10 only with requirement-linked evidence.

### 1.2 Nothing is accepted without its named gate

- A feature, guarantee, or number is current only after its named matrix rows
  pass at its owning gate from a single mainline commit.
- The status vocabulary is fixed: `accepted` (implemented on mainline, backed
  by its named automated gate), `in progress` (present on a branch, not a
  claim), `planned` (specified, not implemented), `deferred` (outside the
  named phase; forbidden as completion evidence). A package, type, stub, or
  happy-path unit test is never completion.
- At the start of this project nothing is built; every gate is `planned`, and
  the ADRs are `accepted` because they are decisions, not code.
- A later gate inherits every earlier gate's regression, packaging, privacy,
  and claim constraint; rerunning only a failed job does not erase the first
  failure, and the gate record links both runs with a cause classification
  (code defect, harness defect, or infrastructure).
- No claim is ever promoted across a boundary: in-memory correctness is not
  durability, single-node behavior is not replication, and a simulated fault
  is not production hardening. Each promotion has its own gate.

### 1.3 Determinism invariants

These invariants make "a flake is a bug" enforceable rather than aspirational:

- `relay-core` is pure: no IO, no clock, no random source, no thread access.
  `apply(state, entry)` returns a new `Applied` value; state is never mutated
  in place. An architecture check (R0) rejects any `std::time`, `std::fs`,
  `std::net`, `std::thread`, or `rand` path inside the crate.
- All time inside the state machine comes from `AdvanceTime` log entries
  (ADR-0005). Lease expiry, delay maturation, retention sweeps, and dedup
  window boundaries are exact functions of applied log time.
- All environment access in `relayd` flows through the injected `Clock`,
  `Rng`, `Disk`, and `Net` traits. Production binds tokio-backed adapters;
  simulation binds `SimClock`, `SimNet`, `SimDisk`, and `SimRng` on a
  single-threaded virtual-time executor.
- Every simulation and property failure prints its seed; the same seed
  replays the identical schedule, byte for byte, on any machine of the same
  target triple. A divergence between two runs of one seed is itself a
  release-blocking defect (SIM-002).
- Iteration order is deterministic everywhere it can reach an output:
  `BTreeMap`/`BTreeSet` or explicitly sorted collections in `relay-core`,
  `relay-wal`, and `relay-raft`; `HashMap` is permitted only behind
  boundaries proven not to affect observable ordering.
- Tests never weaken production validation to obtain deterministic output;
  the production code path is the tested code path.

### 1.4 Evidence artifacts

Every gate produces `evidence/R<n>/manifest.json` containing: the mainline
commit SHA, the CI run URLs for each required check, the list of matrix row
IDs proven at this gate, SHA-256 hashes of retained artifacts (histories,
seeds, benchmark JSON, DR-drill records), and the sign-off record. The
manifest format is versioned (`evidence/1`) and validated in CI by
`just evidence-check`, which fails if any row claimed by a gate lacks a
resolvable green run. Evidence artifacts are retained for 400 days minimum
and for the life of the release for anything cited by a public claim.

## 2. Supported Environments

### 2.1 Support tiers (ADR-0011)

- **Tier 1 — Linux x86_64 and Linux aarch64.** Every pull request runs the
  deterministic suites here; every release candidate additionally runs
  packaging, installation, cluster, soak, and benchmark evidence here.
  Production support claims exist only for Tier 1.
- **Tier 2 — macOS aarch64, development only.** The deterministic suites
  (unit, sim corpus, model check, fuzz smoke) must pass so that developers on
  macOS get trustworthy local signal. No production claim, no packaging
  artifact, no performance number, and no durability claim is made for
  macOS; `relayd` on macOS prints a dev-only warning at startup.
- **Unsupported — Windows, other architectures.** Startup may work from
  source; Relay makes no claim and issue reports require reproduction on a
  supported target. Windows is unsupported at 1.0.

### 2.2 Toolchain

| Component | Pinned value | Enforcement |
| --- | --- | --- |
| Rust toolchain | 1.85.0 (MSRV; `rust-toolchain.toml` pins exactly) | CI installs only the pinned toolchain; a toolchain bump is a reviewed PR that reruns the full deep suite. |
| Rust edition | 2024 | Workspace `Cargo.toml`; checked by `cargo deny` config lint job. |
| Lint baseline | `-D warnings`, clippy pedantic baseline per ADR-0001 | `lint-deny` CI job; no `#[allow]` without an adjacent justification comment naming the rule and reason. |
| cargo-nextest | 0.9.87 | Pinned in `just bootstrap`; CI uses the same version. |
| cargo-deny | 0.16.4 | Same. |
| cargo-fuzz | 0.12.0 (libFuzzer) | Same; fuzz targets build only on Linux Tier 1. |
| cargo-mutants | 25.0.0 | Same; nightly job only. |
| just | 1.36.0 | Task runner; every CI job body is a `just` recipe so local and CI execution are identical. |

### 2.3 Capability matrix

| Platform | Architecture | Tier | Required behavior |
| --- | --- | --- | --- |
| Linux, kernel 5.15+ (validated on Ubuntu 24.04 LTS and Debian 12) | x86_64 | Tier 1 | Full deterministic suites, crash-injection suites with real `fsync` semantics, static musl packaging, systemd install, 3-node live smoke, soak, benchmarks. io_uring backend optional; the portable `pwritev2`/`fsync` fallback is always built and always tested. |
| Linux, kernel 5.15+ | aarch64 | Tier 1 | Same as x86_64; benchmarks are published per-architecture, never cross-attributed. |
| macOS 15+ | aarch64 | Tier 2 (dev-only) | Unit, sim-corpus, model-check, and fuzz-smoke suites pass; `F_FULLFSYNC` is used wherever the durability tests run locally; no packaging, production, or performance claim. |
| Windows, any | any | Unsupported | No claim at 1.0; revisit is an OPEN_QUESTIONS item with a fail-closed default. |

A newly released OS or kernel line enters the matrix only through a reviewed
PR that runs the complete applicable suite on it; "the distro released" never
adds support by itself.

## 3. Repository, Git, and Branch Protection

### 3.1 Repository layout obligations

The repository is `github.com/Zachshotamartin/relay` with root
`/Users/zacharymartin/Desktop/portfolio_projects/relay/`. The workspace
contains exactly the crates named in [ARCHITECTURE.md](./ARCHITECTURE.md):
`relay-core`, `relay-wal`, `relay-raft`, `relay-sim`, `relay-model`,
`relay-wire`, `relay-server`, `relay-client`, `relay-cli`, `relay-bench`,
plus `xtask` for repository automation not expressible as a `just` recipe.
Checked-in test inputs live under `sim-corpus/`, `fuzz/corpus/`,
`testdata/fixtures/`, and `testdata/adversarial/`; evidence manifests live
under `evidence/`. No generated file is committed except reviewed golden
files and corpus entries.

### 3.2 Default branch protection

`main` is protected from R0 onward:

- pull requests only; no direct pushes, including by administrators;
- required status checks (Section 11.4) must be green on the merge commit;
- one approving review minimum; stale approvals are dismissed on new pushes;
- linear history (squash or rebase merges only); force pushes and branch
  deletion disabled;
- merge queue enabled once more than one PR per day is typical, so required
  checks run against the actual merge result.

### 3.3 Evidence-bearing pull requests

Every PR description must state, in a fixed template:

1. the requirement IDs and matrix row IDs the change affects;
2. the test written first and the commit in which it failed (NFR-MAINT-001);
3. whether any golden file, corpus entry, or fixture changed, with the
   semantic reason (a golden diff without a stated reason is a review reject);
4. for dependency changes, the Section 5 review checklist output;
5. for on-disk or wire format changes, the version bump and the MIGR- fixture
   added.

A PR that changes `relay-core`, `relay-wal`, or `relay-raft` without touching
any test file fails the `docs-policy` CI job unless it carries the label
`refactor-no-behavior` and the reviewer confirms behavioral equivalence is
already pinned by existing tests.

### 3.4 Conventional commits

Commit messages use `<type>: <description>` with types exactly:
`feat`, `fix`, `refactor`, `docs`, `test`, `chore`, `perf`, `ci`. The
`docs-policy` job lints the merge commit title. Reversals of accepted
decisions are new ADRs, not silent edits.

## 4. Exact Developer Bootstrap

### 4.1 From clone to full local suite

All commands run from the repository root on a clean machine. Steps 1–4 are
one-time; steps 5–11 are the full local suite and must be green before any PR.

1. `git clone git@github.com:Zachshotamartin/relay.git && cd relay`
2. `rustup toolchain install 1.85.0 --profile minimal --component clippy --component rustfmt`
   — `rust-toolchain.toml` pins `1.85.0`, so all later `cargo` invocations
   resolve to it automatically; a different active toolchain is an error, not
   a fallback.
3. `cargo install cargo-nextest@0.9.87 cargo-deny@0.16.4 cargo-fuzz@0.12.0 cargo-mutants@25.0.0 just@1.36.0 --locked`
4. Linux only: `rustup target add x86_64-unknown-linux-musl` (and
   `aarch64-unknown-linux-musl` on arm hosts) for packaging recipes.
5. `cargo build --workspace --all-targets --locked`
6. `just lint` — `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and the architecture check that forbids IO/clock/rng paths in `relay-core`.
7. `just deny` — `cargo deny check advisories bans licenses sources`.
8. `cargo nextest run --workspace --locked` — all unit and integration tests.
9. `just sim-corpus` — replays every seed under `sim-corpus/` through
   `relay-sim` and checks histories with `relay-model`; any divergence from
   the recorded verdict fails.
10. `just fuzz-smoke` — runs each cargo-fuzz target for 60 seconds over its
    checked-in corpus with `-runs`-bounded execution; any crash, leak, or
    timeout fails.
11. `just model-smoke` — runs the linearizability checker over the checked-in
    known-good and known-bad history fixtures within the per-history budget.

`just ci-local` runs steps 6–11 in sequence and is the definition of "full
local suite" everywhere this document uses the phrase.

### 4.2 Task list

The `justfile` is the single entry point; CI jobs call these same recipes:

| Recipe | Purpose |
| --- | --- |
| `just lint` | fmt, clippy `-D warnings`, architecture checks |
| `just deny` | cargo-deny advisories/bans/licenses/sources |
| `just test` | `cargo nextest run --workspace --locked` |
| `just sim-corpus` | replay checked-in simulation corpus, verify verdicts |
| `just sim-sweep seeds=N` | fresh randomized simulation sweep of N seeds |
| `just model-smoke` / `just model-deep` | budgeted / extended model checking |
| `just fuzz-smoke` / `just fuzz-deep` | 60 s/target / 4 h/target fuzzing |
| `just soak hours=H` | churn soak harness against a local 3-node cluster |
| `just bench` | criterion micro-benchmarks (macro benches: `relay-bench`) |
| `just mutants` | cargo-mutants over `relay-core` (nightly scope) |
| `just package` | static musl build, tarball, container image, SBOM |
| `just audit-surface` | feature-exhaustiveness audit (Section 22) |
| `just dr-drill` | scripted backup/restore disaster-recovery drill |
| `just evidence-check` | validate `evidence/*/manifest.json` resolvability |
| `just ci-local` | lint + deny + test + sim-corpus + fuzz-smoke + model-smoke |

### 4.3 Expected durations

Measured targets on the reference developer machine (8 performance cores,
32 GiB RAM, NVMe); CI budgets in Section 12 are set from these plus margin.

| Step | Cold | Warm |
| --- | --- | --- |
| `cargo build --workspace --all-targets` | ≤ 6 min | ≤ 60 s |
| `just lint` | ≤ 3 min | ≤ 45 s |
| `just deny` | ≤ 30 s | ≤ 10 s |
| `cargo nextest run --workspace` | ≤ 5 min | ≤ 3 min |
| `just sim-corpus` | ≤ 8 min | ≤ 8 min |
| `just fuzz-smoke` | ≤ 6 min | ≤ 6 min |
| `just model-smoke` | ≤ 2 min | ≤ 2 min |
| `just ci-local` (total) | ≤ 25 min | ≤ 20 min |

A recipe exceeding its budget by more than 25% on the reference machine is a
`perf:` defect against the harness, not a reason to raise the budget.

## 5. Dependency and Supply-Chain Policy

### 5.1 Rules

- Every dependency is exact-pinned in `Cargo.lock`, which is committed; CI
  builds with `--locked` everywhere, and a lockfile drift fails the build.
- `Cargo.toml` version requirements use `=x.y.z` exact pins for direct
  dependencies so a lockfile regeneration cannot silently float.
- New direct dependencies require the Section 5.3 review checklist in the PR
  and an entry in the allowed-crate baseline (Section 5.4). Transitive
  additions are reviewed via the `cargo deny` diff printed in CI.
- Build scripts (`build.rs`) and procedural macros are supply-chain
  execution: any crate that adds either is called out explicitly in review,
  and `cargo deny` bans crates whose build scripts fetch from the network.
  No lifecycle script may download binaries; vendored source only.
- Prebuilt binary artifacts in dependencies are forbidden
  (`deny = ["*-sys-prebuilt-style crates"]` reviewed case by case; the
  baseline contains none).
- Dependency updates are batched at most weekly, run the full deep nightly
  suite before merge, and are `chore:` commits with the advisory diff pasted.
- NFR-SEC-008 closes at R10: the release SBOM (Section 13.3) must account
  for every crate in the lockfile with license and source provenance.

### 5.2 cargo-deny configuration

`deny.toml` at the repository root, checked by the `lint-deny` job:

```toml
[graph]
targets = [
  "x86_64-unknown-linux-gnu",
  "aarch64-unknown-linux-gnu",
  "x86_64-unknown-linux-musl",
  "aarch64-unknown-linux-musl",
  "aarch64-apple-darwin",
]

[advisories]
db-path = "~/.cargo/advisory-db"
db-urls = ["https://github.com/rustsec/advisory-db"]
yanked = "deny"
unmaintained = "workspace"
ignore = [] # every ignore requires an inline comment with issue link and expiry date

[licenses]
allow = ["MIT", "Apache-2.0", "Apache-2.0 WITH LLVM-exception", "BSD-2-Clause", "BSD-3-Clause", "ISC", "Unicode-3.0", "Zlib"]
confidence-threshold = 0.93

[bans]
multiple-versions = "deny"
wildcards = "deny"
deny = [
  { crate = "prost", reason = "no gRPC/protobuf on the wire per ADR-0004" },
  { crate = "tonic", reason = "no gRPC per ADR-0004" },
  { crate = "protobuf", reason = "no protobuf per ADR-0004" },
  { crate = "openssl", reason = "TLS is rustls-only; no C TLS stack" },
  { crate = "openssl-sys", reason = "TLS is rustls-only" },
  { crate = "rocksdb", reason = "storage is the hand-rolled WAL per ADR-0002" },
  { crate = "rusqlite", reason = "storage is the hand-rolled WAL per ADR-0002" },
  { crate = "chrono", reason = "state-machine time is log-applied per ADR-0005; std/time in adapters only" },
  { crate = "ulid", reason = "ULID codec is in-house per ADR-0006 (log-applied time component)" },
]
skip = []
skip-tree = []

[sources]
unknown-registry = "deny"
unknown-git = "deny"
allow-registry = ["https://github.com/rust-lang/crates.io-index"]
allow-git = []
```

### 5.3 Per-dependency review checklist

Recorded in the PR for every new direct dependency:

1. What boundary does it serve, and why is in-house implementation worse?
2. Download counts, maintenance cadence, bus factor, and repository health.
3. `unsafe` surface: count and character of `unsafe` blocks; is a
   `#![forbid(unsafe_code)]` alternative viable?
4. Build script and proc-macro presence; what do they execute?
5. Transitive additions (`cargo tree --edges normal -i <crate>` output).
6. License compatibility with the Section 5.2 allowlist.
7. Determinism impact: does it introduce hidden threads, clocks, or
   randomness that could reach `relay-core`, `relay-sim`, or `relay-model`?
   (Any yes is a rejection for those crates.)
8. Removal plan: what would replacing it cost later?

### 5.4 Allowed-crate baseline

| Crate | Scope | Justification |
| --- | --- | --- |
| `tokio` (=1.x pinned) | relay-server, relay-client, relay-cli production adapters only | Production async runtime; forbidden in relay-core/relay-sim/relay-model (simulation is single-threaded virtual time). |
| `bytes` | wire and WAL buffers | Ref-counted buffer type; no unsafe exposure at our boundary. |
| `crc32c` | WAL records, segments, RWP frames | Hardware-accelerated CRC32C with software fallback. |
| `sha2` | snapshot footer hash, content dedup, body identity | RustCrypto; pure Rust. |
| `hmac` | receipt handles (ADR-0006), request auth (FR-API-003) | RustCrypto; constant-time via `subtle`. |
| `subtle` | credential and tag comparison | Constant-time equality (NFR-SEC-004). |
| `rustls` + `tokio-rustls` | TLS 1.3 transport (FR-API-008) | Memory-safe TLS; the only TLS stack. |
| `serde`, `serde_json` | JSONL histories, evidence manifests, relayctl JSON output | Not used on the RWP wire (bodies are fixed layouts per ADR-0004). |
| `toml` | configuration parsing (Section 7) | Config file format. |
| `clap` (=4.x pinned) | relayd/relayctl argument parsing | Derive-based CLI. |
| `tracing`, `tracing-subscriber` | structured logs and spans (ADR-0010) | JSON log emission. |
| `opentelemetry`, `opentelemetry-otlp` | OTLP export (FR-OPS-005) | relay-server only. |
| `criterion` (dev) | micro-benchmark harness | Statistical treatment for BENCH- micro rows. |
| `proptest` (dev) | property tests with printed seeds | CORE-/STOR- property rows. |
| `libfuzzer-sys` via cargo-fuzz (dev) | FUZZ- targets | Coverage-guided fuzzing. |
| `tempfile` (dev) | isolated test roots | Unique per-test directories. |

ULID encoding/decoding is implemented in `relay-core` (approximately 80 lines,
Crockford base32, time component from the log-applied clock) per ADR-0006;
the external `ulid` crate is banned because it reads wall time. `prost` and
all gRPC stacks are banned per ADR-0004. Anything not in this table requires
the full Section 5.3 review before first use.

## 6. Local Data, Configuration, and State Locations

### 6.1 Data directory tree

Default `/var/lib/relay` in production, `./relay-data` in development
(`RELAY_DATA_DIR` or `--data-dir` overrides). Exact owned layout:

```text
/var/lib/relay/
  LOCK                        # exclusive flock; see 6.4
  meta/
    node.json                 # node id, cluster id, format versions, created-at
  wal/
    wal-<seq:016x>.seg        # 64 MiB target segments (spine format, ARCHITECTURE §WAL)
  snap/
    snap-<lsn:016x>.rsnap     # chunked snapshots with SHA-256 footer
  keys/
    receipt.key               # per-cluster HMAC receipt key, epoch-versioned, 0600
    tenants/<tenant>.key      # per-tenant auth keys, 0600
  audit/
    audit-<yyyymmdd>.jsonl    # admin mutation audit log (FR-ADMIN-008)
  tmp/
    <uuid>/                   # staging for atomic rename; cleared at startup
```

Relayd creates, reads, and deletes only inside this tree plus its configured
log destination. Uninstall and purge (Section 14) enumerate exactly this tree.

### 6.2 Configuration file and environment variables

Configuration file default `/etc/relay/relay.toml` (`--config` or
`RELAY_CONFIG` overrides). Environment variables map to config keys as
`RELAY_<SECTION>__<KEY>` with `__` as the section separator, uppercased,
for example `RELAY_STORAGE__DATA_DIR`, `RELAY_LISTEN__API`,
`RELAY_OBSERVABILITY__LOG_LEVEL`. Additionally reserved short forms:
`RELAY_CONFIG`, `RELAY_DATA_DIR`, `RELAY_NODE_ID`, `RELAY_LOG_LEVEL`,
`RELAY_LOG_FORMAT`. Unknown `RELAY_*` variables are a fail-fast startup error
(`RELAY-CFG-0009`), never silently ignored.

### 6.3 File permission requirements

- Data directory mode `0700`, owned by the `relay` service user. Startup
  verifies owner and mode and refuses to start otherwise
  (`RELAY-CFG-0012`, NFR-SEC-005; verified by STOR-013).
- Key files under `keys/` mode `0600`; group or world access is fatal.
- Config file may be `0644`; it must never contain raw tenant secrets —
  it references key file paths, and a literal secret value in config is a
  validation error (`RELAY-CFG-0013`).
- Audit logs are `0600` and append-only from the process's perspective.

### 6.4 Lock file semantics

`LOCK` is held with an exclusive `flock(2)` for the whole process lifetime.
The file body records `pid`, `node_id`, `binary_version`, and start time as
JSON for diagnostics only; liveness is the flock itself, never a PID probe,
so a stale file after SIGKILL is harmless and taken over silently. A second
relayd against the same data dir exits with `RELAY-CFG-0014` and the holder's
recorded identity within 1 second. relayctl subcommands that read on-disk
state offline (`relayctl fsck`, `relayctl backup`) take a shared lock and
refuse if an exclusive holder exists, except `relayctl backup` in hot mode,
which coordinates through the running server instead of the lock.

## 7. Configuration Schema and Startup Validation

### 7.1 Precedence

Effective configuration is resolved once at startup, in fixed precedence:
**command-line flags > environment variables > configuration file >
built-in defaults** (FR-OPS-002). The resolved configuration, with every
value tagged by its winning source, is printed at `info` level (secrets
elided as `<set>`) and embedded in `relayctl diagnose` output. There is no
runtime re-read; configuration changes require restart, except queue and
topic attributes, which are data-plane state, not process configuration.

### 7.2 Annotated schema

`relay.toml`, complete surface. Every key shows its default; a key omitted
from the file takes the default shown.

```toml
[node]
# Stable node identity within the cluster. Required for cluster mode;
# defaults to "node-1" for single-node. Regex ^[a-z0-9-]{1,32}$.
id = "node-1"
# Cluster name; all members must agree. Regex ^[a-z0-9-]{1,32}$.
cluster = "relay"

[listen]
# Client API (RWP/1). Default port 7414.
api = "0.0.0.0:7414"
# Metrics + health/readiness HTTP. Default port 7415.
metrics = "127.0.0.1:7415"
# Raft inter-node transport. Default port 7416. Ignored in single-node mode.
raft = "0.0.0.0:7416"
# Advertised address other nodes and clients use to reach this node.
# Required in cluster mode; must resolve and must not be 0.0.0.0.
advertise = "10.0.0.11:7414"

[tls]
# TLS 1.3 is required for non-loopback API and Raft listeners (FR-API-008).
# Plaintext is permitted only when the listener binds a loopback address
# AND allow_plaintext_loopback = true. Any other plaintext combination is
# fail-fast error RELAY-CFG-0020.
cert_file = "/etc/relay/tls/server.crt"
key_file = "/etc/relay/tls/server.key"      # must be mode 0600
client_ca_file = ""                          # optional mTLS for raft peers
allow_plaintext_loopback = false

[storage]
data_dir = "/var/lib/relay"
# WAL segment target size in bytes. Range 16 MiB - 1 GiB. Default 64 MiB.
segment_bytes = 67108864
# Adaptive group-commit window cap in microseconds (ADR-0008). Range
# 0-2000; 0 = fsync every append. Default 2000 (2 ms hard cap).
group_commit_cap_us = 2000
# Compaction trigger: reclaimable ratio in a closed segment. Range 0.1-0.9.
compact_reclaim_ratio = 0.5
# Snapshot every N applied log bytes. Range 64 MiB - 8 GiB. Default 1 GiB.
snapshot_interval_bytes = 1073741824
# io_uring on supporting Linux kernels; portable fallback otherwise.
io_backend = "auto"                          # "auto" | "uring" | "portable"

[raft]
# Single-node mode when members lists only this node.
members = ["node-1"]
# Heartbeat interval in ms. Fixed default 100; range 10-1000.
heartbeat_ms = 100
# Election timeout randomized in [min,max] ms. Defaults 500-1000.
election_timeout_min_ms = 500
election_timeout_max_ms = 1000
# Snapshot install chunk size. Fixed 1 MiB; not configurable at 1.0 —
# present here as documentation, rejected if set to any other value.
snapshot_chunk_bytes = 1048576

[limits]
# Wire and object limits. Values shown are the fixed 1.0 contract; a value
# above the fixed maximum is RELAY-CFG-0031. Lowering is permitted.
max_frame_bytes = 1048576          # 1 MiB RWP frame cap
max_body_bytes = 262144            # 256 KiB message body (NG-06)
max_batch_entries = 10
max_attributes = 10
max_inflight_per_conn = 128        # bounded in-flight requests per connection
read_deadline_ms = 30000           # slowloris: header/body read deadline
write_deadline_ms = 30000
max_connections = 10000
inflight_cap_standard = 120000     # per-queue in-flight caps (FR-QUEUE-016)
inflight_cap_fifo = 20000

[quotas]
# Per-tenant defaults; per-tenant overrides live in the data plane.
default_send_per_sec = 5000
default_receive_per_sec = 5000
default_admin_per_sec = 50
burst_multiplier = 2.0             # token bucket burst = rate * multiplier

[observability]
log_level = "info"                 # "error"|"warn"|"info"|"debug"|"trace"
log_format = "json"                # "json"|"text"; json required in prod claims
# OTLP gRPC-less HTTP/protobuf exporter endpoint; empty disables tracing.
otlp_endpoint = ""
trace_sample_ratio = 0.01          # range 0.0-1.0
# Metrics cardinality budget enforcement (Section 16.2): relayd refuses to
# register series beyond the budget rather than emitting unbounded labels.
max_metric_series = 5000
```

### 7.3 Fail-fast validation and stable error codes

Startup validation runs completely before any listener binds or any byte is
written. Every failure exits with code 78 (EX_CONFIG), a single-line JSON
error on stderr, and one stable code. The code set is closed; adding a code
is a reviewed change to this table.

| Code | Condition |
| --- | --- |
| RELAY-CFG-0001 | Config file missing at an explicitly given path. |
| RELAY-CFG-0002 | TOML syntax error (line/column included). |
| RELAY-CFG-0003 | Unknown key or section (typo protection; no silent ignore). |
| RELAY-CFG-0004 | Type mismatch for a known key. |
| RELAY-CFG-0005 | Value out of documented range (range echoed in message). |
| RELAY-CFG-0006 | Invalid node/cluster id or member name (regex echoed). |
| RELAY-CFG-0007 | Listen or advertise address unparseable, or advertise is 0.0.0.0/unspecified. |
| RELAY-CFG-0008 | `node.id` not present in `raft.members`. |
| RELAY-CFG-0009 | Unknown `RELAY_*` environment variable present. |
| RELAY-CFG-0010 | Flag/env/file conflict that precedence cannot resolve (duplicate flag). |
| RELAY-CFG-0011 | Data dir missing and not creatable, or not a directory. |
| RELAY-CFG-0012 | Data dir ownership or mode is not 0700 (NFR-SEC-005). |
| RELAY-CFG-0013 | Literal secret material found in config (key files are referenced by path only). |
| RELAY-CFG-0014 | Data dir lock held by another live process (holder identity echoed). |
| RELAY-CFG-0020 | Plaintext listener on a non-loopback address, or `allow_plaintext_loopback` without loopback bind. |
| RELAY-CFG-0021 | TLS cert/key missing, unreadable, mismatched pair, or key not 0600. |
| RELAY-CFG-0030 | Raft timing invalid (min ≥ max, heartbeat ≥ election min/3). |
| RELAY-CFG-0031 | A `[limits]` value above its fixed 1.0 maximum. |
| RELAY-CFG-0040 | On-disk format version newer than this binary supports (downgrade refusal; see MIGR-003). |

Validation is deterministic and covered by OPSX-007: a golden table of
invalid configurations maps each to its exact code and message shape.

## 8. Test Policy and Harness Rules

### 8.1 Test-first policy

Every parser, reducer, and state transition has its failing deterministic
test written first (NFR-MAINT-001). The PR template (Section 3.3) records
where the test failed; the gate audits (Section 22) sample-verify the claim.
Coverage percentage is not evidence: the evidence unit is the named matrix
row with its stated oracle. Tests assert trusted state — core state hashes,
WAL bytes, history verdicts, metric values, exit codes — not log prose.

### 8.2 Harness inventory

| Harness | Runs | What it proves |
| --- | --- | --- |
| cargo-nextest | unit and integration tests in every crate | Pure-function contracts, WAL behavior on real temp filesystems, wire codec, config validation. |
| relay-sim runner (`just sim-corpus`, `just sim-sweep`) | whole-system deterministic simulation: relay-core + relay-wal + relay-raft + relay-wire driven by SimClock/SimNet/SimDisk/SimRng on virtual time | Fault-schedule behavior, liveness, crash-recovery equivalence, seed reproducibility; emits JSONL histories. |
| relay-model checker (`just model-smoke`, `just model-deep`) | Wing–Gong linearizability checking of JSONL histories against the reference model (ADR-0007) | P-02, P-04, P-06, P-09, P-10 on captured histories; counterexample minimization. |
| cargo-fuzz (libFuzzer) | `fuzz/` targets over frames, opcode bodies, WAL records, filter policies | Bounded parsing under arbitrary bytes (FR-API-002, NFR-SEC-002). |
| live 3-node smoke harness (`just soak`, live smoke recipe) | packaged relayd binaries as real processes on loopback with real disks and TLS | Packaging truth, systemd-adjacent lifecycle, soak, leak detection, real-kill recovery. |
| criterion + relay-bench | micro (criterion) and macro (relay-bench open-loop driver) benchmarks on the dedicated runner | NFR-PERF numbers with statistical treatment (BENCHMARK_PLAN.md owns method). |
| cargo-mutants | mutation testing over relay-core (nightly) | NFR-MAINT-003 ≥ 85% mutant kill. |

### 8.3 Per-suite rules

For every suite: harness, fixture policy, runtime budget, when it gates, and
flake policy. Deterministic suites are zero-flake: **a flake is a bug**, it is
filed as a defect against the code or harness, and quarantine is forbidden —
the suite stays red until the nondeterminism is removed. Only the live
3-node smoke/soak suites (real processes, real kernel scheduling) may
quarantine a test, and a quarantined test has a named owner, a linked issue,
and a 7-day fix SLA; on day 8 the quarantine itself turns the pipeline red.

| Suite | Harness | Fixture policy | Runtime budget | Gating | Flake policy |
| --- | --- | --- | --- | --- | --- |
| Unit + property | cargo-nextest, proptest with printed seeds | Inline constructors; no shared mutable state; property seeds from CI are committed to the regression corpus when they find bugs | ≤ 5 min total | Every PR | Zero-flake |
| Storage integration | cargo-nextest on real temp dirs + SimDisk fault schedules | `tempfile` roots; fault schedules are named constants in the test | ≤ 6 min total | Every PR | Zero-flake |
| Crash injection | nextest-driven child-process kill + SimDisk torn-write sweeps | Checked-in WAL byte fixtures under `testdata/fixtures/wal/` | ≤ 8 min total | Every PR | Zero-flake |
| Sim corpus | relay-sim runner | `sim-corpus/<category>/<seed>.toml` (Section 9.2); additions by PR only | ≤ 10 min in CI | Every PR | Zero-flake (SIM-001 enforces) |
| Sim sweep | relay-sim runner, fresh seeds | Failing fresh seeds are minimized and committed to the corpus before the fix merges | 2 h nightly | Nightly | Zero-flake; a nightly failure is a real bug with a replayable seed |
| Model check | relay-model | History fixtures under `testdata/histories/`; sim-emitted histories | ≤ 5 min budgeted in CI; extended nightly 1 h | Every PR (budgeted) | Zero-flake |
| Fuzz smoke | cargo-fuzz | `fuzz/corpus/<target>/`; every crash becomes a corpus entry in the fixing PR | ≤ 6 min | Every PR | Zero-flake (fixed corpus, bounded runs) |
| Fuzz deep | cargo-fuzz | Same corpus, coverage-guided exploration | 4 h/target nightly | Nightly | New findings are bugs, never flakes |
| Live 3-node smoke | live harness, packaged binaries | Config templates under `testdata/live/`; unique ports per run | ≤ 10 min | Main + release | Quarantine permitted, 7-day SLA |
| Soak | live harness | Workload profiles under `testdata/soak/` | 24 h nightly on dedicated runner | Nightly + release | Quarantine permitted, 7-day SLA |
| Benchmarks | criterion + relay-bench on dedicated runner | Workload profiles in BENCHMARK_PLAN.md | ≤ 90 min nightly | Nightly + R9/R10 | Not flake-eligible: regressions beyond noise bands fail |
| Mutation | cargo-mutants | Scope file `mutants.toml` limits to relay-core | ≤ 4 h nightly | Nightly + R4 evidence | Deterministic; zero-flake |

### 8.4 Isolation and cleanup

Every test receives a unique temporary root, unique ports (OS-assigned),
and a unique data dir; nothing shares mutable singletons. Cleanup removes
only the verified per-test root, reaps child processes, and reports residual
processes, file handles, or directories as test failures. A suite-level leak
detector runs at process exit in the live harness. Cleanup success can never
turn a failed assertion green.

## 9. Shared Test Harnesses and Oracles

### 9.1 SimEnv construction

`relay_sim::SimEnv::from_spec(spec: &SimSpec, seed: u64)` builds one
deterministic world: N simulated nodes each with its own `SimDisk` byte
store, a `SimNet` with per-link fault schedules, one `SimClock` advancing
virtual time, and `SimRng` derived by splitting the run seed per component
(`seed`, component id, instance id → SplitMix64), so adding a component
never perturbs another component's stream. The executor is single-threaded:
all futures are polled in a deterministic order keyed by (virtual time,
tiebreak id). Client workloads are generator programs in the spec, not
threads. Every externally visible operation (invoke and return) is appended
to the JSONL history with virtual-time nanos, per the record format in
[ARCHITECTURE.md](./ARCHITECTURE.md) §histories.

### 9.2 Seed corpus layout

```text
sim-corpus/
  smoke/<seed>.toml         # fast broad scenarios, every PR
  crash/<seed>.toml         # crash-restart schedules (CRSH support)
  net/<seed>.toml           # drop/delay/dup/reorder schedules
  partition/<seed>.toml     # multi-node partition schedules (R7+)
  fifo/<seed>.toml          # group-ordering workloads
  regression/<seed>.toml    # minimized seeds from every fixed bug, named
                            # by issue: regression/issue-0142-a3f9.toml
```

Each corpus file records the `SimSpec`, the seed, the expected verdict
(`pass`, or `expected-detect:<property>` for known-bad meta-tests), the
relay version range it applies to, and the issue link for regressions.
Corpus entries are append-only; removing one requires a `docs-policy`
justification in the PR.

### 9.3 History capture and the linearizability oracle

The oracle entry point is:

```rust
relay_model::check_history(path: &Path, profile: CheckProfile, budget: Duration)
    -> CheckVerdict // Linearizable | Violation(MinimizedCounterexample) | BudgetExceeded
```

`CheckProfile` selects the property set: `QueueBasic` (P-02, P-06, P-10),
`FifoOrder` (adds P-04), `Failover` (adds P-09). Per ADR-0007 the checker
partitions histories per queue, memoizes Wing–Gong search states, and
enforces a wall-clock budget per history; `BudgetExceeded` is never treated
as a pass — it escalates to the nightly extended run with a 10x budget, and
an entry that exceeds even the extended budget is restructured (smaller
history) rather than waived. Violations emit a minimized counterexample
(the shortest failing subhistory the minimizer reaches within 60 s) plus the
seed, and the minimized form is committed under `testdata/histories/bad/`.

### 9.4 Golden-file policy

Golden files cover: relayctl human and JSON output, config validation
error table, log field shapes, metrics inventory, RWP error-code taxonomy,
and evidence manifest schema. Updating a golden requires
`just golden-review`, which prints the semantic diff; the PR must state the
reason. A golden update in the same PR as the behavior change it blesses is
mandatory — goldens never drift in follow-ups.

### 9.5 Fixture directories

```text
testdata/
  fixtures/wal/           # hand-built and generated WAL byte images, incl.
                          # torn tails at every offset class and bad CRCs
  fixtures/snap/          # snapshot images incl. truncated and bad-footer
  fixtures/onDisk/v<N>/   # complete old-version data dirs for MIGR- rows:
                          # each is a frozen output of released version N's
                          # generator, with a manifest of expected recovered
                          # state; never regenerated after freeze
  histories/good/         # linearizable JSONL histories (checker sanity)
  histories/bad/          # seeded violations: lost-ack, double-lease,
                          # invented-message, order-break (checker must catch)
  adversarial/            # hostile wire frames, forged receipts, malformed
                          # filter policies, with provenance and expected
                          # classification
  live/                   # 3-node config templates for the live harness
  soak/                   # soak workload profiles
```

Old-version on-disk fixtures (`fixtures/onDisk/v<N>/`) are created at each
release by `just freeze-fixture` and are the ground truth for MIGR-001 and
MIGR-002; a release is incomplete until its frozen fixture is committed.

## 10. Detailed Verification Matrices

Each matrix row is a required named test family. "Pass" means the stated
oracle holds on every Tier 1 target assigned to the row and no leak detector
reports residual state. "Earliest gate" is the first gate that must prove the
row; every later gate keeps the row as regression coverage. All rows are
`planned`. Row IDs are stable forever; a superseded row is marked retired in
place, never renumbered.

### 10.1 Core queue semantics matrix (CORE-)

Harness: cargo-nextest over `relay-core` with proptest where noted; time
advances only via `AdvanceTime` entries.

| ID | Mechanics | Pass criteria | Earliest gate |
| --- | --- | --- | --- |
| CORE-001 | CreateQueue with valid names at regex boundaries (1 and 80 chars), invalid characters, duplicate name, `.fifo` suffix on standard config, and every config field at min/max/out-of-range. | Valid configs produce an empty queue with exact attributes; every invalid case returns its stable error code with the offending field named; duplicate create is rejected without state change. | R1 |
| CORE-002 | Send bodies of 0, 1, 262143, 262144, and 262145 bytes plus binary and invalid-UTF-8 content. | Bodies through 262144 bytes are stored byte-identically; 262145 returns the stable oversize error (FR-QUEUE-013, NG-06); body content is never interpreted. | R1 |
| CORE-003 | SendBatch with 1, 10, and 11 entries; mixed valid/invalid entries; duplicate ids inside one batch. | 10 is accepted with independent per-entry results in request order; entry 3 failing never affects entry 4; 11 entries rejects the whole batch before any state change. | R1 |
| CORE-004 | Receive with max 1, 10, and 11; empty queue; fewer available than requested; visibility at 0 s, 30 s default, and 12 h max. | At most 10 messages lease atomically with per-message receipt handles carrying incremented lease epochs; max 11 is rejected; empty receive returns empty without error. | R1 |
| CORE-005 | Advance time to exactly lease expiry minus 1 ns, exactly expiry, and past expiry for leased messages. | At expiry-1 ns the lease holds; at expiry the message transitions InFlight→Available and receive count increments exactly once (FR-QUEUE-005); redelivery yields a new lease epoch. | R1 |
| CORE-006 | Delete with a valid handle once, twice, and after redelivery of the same message. | First delete removes the message permanently; second delete of the same handle is an idempotent success (P-06); a handle from a superseded delivery is rejected (epoch mismatch). | R1 |
| CORE-007 | Present expired handles, handles from other queues, handles from deleted queues, and epoch-stale handles to delete and change-visibility. | Every foreign/expired/superseded handle is rejected with its stable code and zero state change; handles are single-use per delivery (FR-QUEUE-007). | R1 |
| CORE-008 | ChangeMessageVisibility extending to max 12 h, shortening, setting exactly 0, and targeting a non-leased message. | Extension and shortening move expiry exactly; zero returns the message to Available immediately; non-leased target is a stable error (FR-QUEUE-008). | R1 |
| CORE-009 | Per-message DelaySeconds 0, 900, and 901; per-queue default delay with and without per-message override; delay maturation at the exact boundary tick. | Delayed messages are invisible to receive until exactly their maturation time; 901 rejects; per-message value overrides queue default (FR-QUEUE-010/011). | R4 |
| CORE-010 | Attributes: each of String/Number/Binary round-trip, 10 attributes accepted, 11 rejected, invalid Number content, empty names, name length limits. | Typed values round-trip exactly; the 11th attribute rejects the send with the attribute name in the error; type violations fail closed (FR-QUEUE-012). | R1 |
| CORE-011 | Purge with available, delayed, and in-flight messages; delete against an in-flight handle after purge; second purge while one is active. | All messages including in-flight are removed; post-purge handle operations return the stable purged/missing error; concurrent purge is rejected while one is active (FR-QUEUE-015). | R1 |
| CORE-012 | Drive in-flight to 119999/120000/120001 (standard) via receives without deletes. | Receive at the cap returns the stable backpressure error, leaves state unchanged, and succeeds again after one delete (FR-QUEUE-016). | R1 |
| CORE-013 | Retention at 60 s min, 14 d max, 4 d default: advance time across expiry for available, delayed, and in-flight messages. | Expired messages are removed by the retention sweep on the next AdvanceTime at/after the boundary; in-flight messages past retention expire per specification without resurrecting (FR-QUEUE-014). | R4 |
| CORE-014 | Redrive policy with maxReceiveCount 1 and 1000: exhaust receives; inspect the dead-lettered message. | The move to the DLQ happens on the receive that would exceed the count; body and attributes are byte-identical and the DLQ record carries source queue, receive count, and move time (FR-QUEUE-017/018). | R4 |
| CORE-015 | StartRedrive from a DLQ with 0, 1, and 10,000 messages; source queue deleted mid-task; second StartRedrive while active. | Messages return to the source queue with progress reported per batch; a deleted source fails the task cleanly with progress preserved; concurrent redrive on one DLQ is rejected (FR-QUEUE-019). | R4 |
| CORE-016 | Property test: apply the same `(CoreState, LogEntry)` twice and compare; apply 10,000 random valid command sequences twice from the same seed. | `apply` is a pure function: identical inputs give byte-identical `Applied` values; the input state is unmodified (immutability); no wall-clock or rng influence exists. | R1 |

### 10.2 WAL storage matrix (STOR-)

Harness: cargo-nextest over `relay-wal` with real temp filesystems and
SimDisk fault schedules; byte fixtures from `testdata/fixtures/wal/`.

| ID | Mechanics | Pass criteria | Earliest gate |
| --- | --- | --- | --- |
| STOR-001 | Encode/decode every record type at payload sizes 0, 1, 4 KiB, and max; verify CRC32C covers type..payload exactly by flipping each header and payload byte class. | Round-trip is byte-identical; every single-bit corruption in the covered range is detected; corruption in `len` is caught by bounds or CRC before any allocation beyond the cap. | R2 |
| STOR-002 | Parse records with len below minimum, len above segment remainder, unknown type, nonzero reserved bits, and truncated payload. | Every malformed record is rejected with a typed error naming the offset; the parser allocates nothing larger than the declared cap; no panic on any fixture. | R2 |
| STOR-003 | Parse segment headers: wrong magic, unsupported format version, seq/base-lsn mismatch with filename, corrupt header CRC, short header. | Only an exact `RWALSEG1` header with valid CRC and consistent identity is accepted; each failure mode has a distinct typed error; a bad header quarantines the segment rather than truncating it. | R2 |
| STOR-004 | Fill a segment past the 64 MiB target; verify rotation creates `wal-<seq+1:016x>.seg`, writes its header, fsyncs the file and directory, then seals the predecessor. | Rotation order is exactly create→header→fsync file→fsync dir→switch; a record never spans segments; the sealed segment is never appended to again. | R2 |
| STOR-005 | Recover from a cleanly shut-down directory with 1, 2, and 50 segments plus a snapshot. | `Wal::recover` returns state identical (hash-compared) to the pre-shutdown in-memory state; recovery reads only sealed bytes plus the tail scan. | R2 |
| STOR-006 | Recover from fixtures with a torn final record: truncated at every byte-class boundary (mid-len, mid-crc, mid-payload) of the last record. | The torn tail is detected by CRC/length, truncated at the last valid record boundary, and recovery succeeds; only tail truncation ever occurs (NFR-DUR-003). | R2 |
| STOR-007 | Recover from fixtures with a corrupt record in the middle of a sealed segment (bit flip in payload and in header). | Recovery fails with a corruption error naming segment and offset; no silent truncation of valid later data; the operator remediation path (restore from backup) is named in the error. | R2 |
| STOR-008 | Recover from a directory with a missing segment in the sequence and with a duplicate segment seq. | Both are detected by the seq/base-lsn chain and fail recovery with a typed gap error; recovery never skips a gap. | R2 |
| STOR-009 | Group commit: issue concurrent appends; measure that `sync()` batches them within the adaptive window capped at 2 ms; verify the returned Lsn covers every batched record. | No append waits longer than 2 ms for its fsync batch; `sync()`'s returned Lsn is durable — a SimDisk crash after return loses nothing at or below it (ADR-0008). | R2 |
| STOR-010 | Compact segments containing mixtures of live and dead records under a `LiveSet`; verify the `CompactionReport` and post-compaction recovery. | No live record is lost (recovery state identical before/after); reclaimed bytes match the report; compaction of the active segment is refused (NFR-DUR-006). | R2 |
| STOR-011 | Drive the disk to full via SimDisk quota during append, rotation, and compaction. | Writes fail with a clean typed error and no partial state visible after recovery; reads and receives continue to serve existing data (NFR-DUR-004); freeing space restores writes without restart. | R2 |
| STOR-012 | Write and reload local checkpoint snapshots: valid, truncated mid-chunk, corrupt chunk CRC, corrupt footer SHA-256. | Only a snapshot with every chunk CRC and the footer full-state SHA-256 valid is loaded; invalid snapshots are ignored in favor of the previous snapshot plus WAL replay; a snapshot is published only via tmp-write→fsync→rename→dir-fsync. | R2 |
| STOR-013 | Start relayd with data dir modes 0755, 0770, 0700, and wrong owner. | Only 0700 with the correct owner starts; others exit RELAY-CFG-0012 before any listener binds or byte is written (NFR-SEC-005). | R2 |
| STOR-014 | Launch a second relayd (and offline relayctl fsck) against a locked data dir; SIGKILL the holder and relaunch. | The second process exits RELAY-CFG-0014 within 1 s naming the holder; after SIGKILL the stale lock is taken over silently via flock semantics. | R2 |

### 10.3 Crash and torn-write injection matrix (CRSH-)

Harness: child-process SIGKILL at instrumented fault points plus SimDisk
crash schedules; the oracle is recovery equivalence against the model of
acknowledged operations.

| ID | Mechanics | Pass criteria | Earliest gate |
| --- | --- | --- | --- |
| CRSH-001 | SIGKILL between WAL append and fsync for a batch of sends; recover; compare against the acked set. | No acked send is missing (P-01); unacked sends may be absent or present (NG-09 permits both); recovered state equals the model applied to the surviving prefix. | R2 |
| CRSH-002 | SIGKILL during fsync itself (SimDisk crash-mid-fsync leaves an arbitrary prefix of the batch durable). | Recovery accepts any durable prefix, truncates the torn tail, and never surfaces a message whose record did not fully survive; acked messages are always in the durable prefix. | R2 |
| CRSH-003 | SIGKILL at each step of rotation: after new-segment create, after header write, before file fsync, before dir fsync, before predecessor seal. | Every intermediate state recovers: an unreferenced or headerless new segment is discarded or adopted deterministically; no acked record is lost; the seq chain remains gapless. | R2 |
| CRSH-004 | SIGKILL mid-compaction: after writing the compacted segment, before dir fsync, after deleting some source segments. | Recovery lands on exactly one of the two valid worlds (pre- or post-compaction); live records exist in either world; partially deleted sources never produce a gap error for live data. | R2 |
| CRSH-005 | SIGKILL mid-snapshot: partial `.rsnap` in tmp/, complete file before rename, after rename before dir fsync. | Partial snapshots are ignored and cleaned at startup; a renamed-but-unfsynced snapshot either exists completely or not at all after recovery; WAL replay covers the difference. | R2 |
| CRSH-006 | Torn-tail sweep: for a fixed workload, truncate the final segment at every offset in the last 8 KiB and recover each image. | Every truncation point yields a successful recovery to the longest valid record prefix; the sweep is exhaustive, not sampled, and runs in ≤ 90 s. | R2 |
| CRSH-007 | Inject an fsync error (EIO) from SimDisk and from a real-file wrapper during group commit. | The process aborts immediately (fsyncgate rule, NFR-DUR-005); it never retries fsync, never acks the batch, and the abort path emits a final log line naming the failed file. | R2 |
| CRSH-008 | Recovery equivalence: run 500 randomized workload+crash schedules; after each crash, compare recovered `CoreState` hash to the reference model applied to the acked history prefix. | All 500 schedules produce hash-identical states (NFR-DUR-002); any divergence dumps both states, the seed, and the WAL image as evidence. | R2 |
| CRSH-009 | Double crash: SIGKILL during recovery itself (during tail scan, during snapshot load, during truncation). | The second recovery succeeds from the same on-disk state; recovery is idempotent and truncation is re-appliable. | R2 |
| CRSH-010 | SIGKILL after fsync completes but before the client ack is transmitted. | On recovery the message exists and will be delivered; the client, having no ack, may resend, producing a duplicate — expected under at-least-once (NG-01); the test asserts no loss and documents the duplicate. | R2 |
| CRSH-011 | Kill -9 the live packaged relayd (real process, real disk) 50 times at random points under load; restart and fsck each time. | Every restart recovers within the R2 recovery budget, `relayctl fsck` reports zero inconsistencies, and no acked message is lost across the full run. | R2 |

### 10.4 Deterministic simulation matrix (SIM-)

Harness: relay-sim runner; specs and seeds from `sim-corpus/`; verdicts via
relay-model where the row names a property.

| ID | Mechanics | Pass criteria | Earliest gate |
| --- | --- | --- | --- |
| SIM-001 | Reproducibility meta-test: run 20 corpus seeds twice each on the same binary; compare full event traces and emitted histories. | Every pair is byte-identical, including log ordering and virtual timestamps; any diff fails the suite and prints the first divergent event. | R3 |
| SIM-002 | Divergence alarm: deliberately introduce a HashMap-ordered iteration and a wall-clock read behind a test feature flag; run the meta-test. | The alarm catches both plants (proving SIM-001 has teeth); the feature-flagged plants are compiled out of release builds; the detector also runs continuously in nightly sweeps. | R3 |
| SIM-003 | Message-drop sweep: client↔server link drops at 1%, 10%, and 50% across 200 seeds with retrying clients. | All histories check linearizable under `QueueBasic`; no acked send is lost; throughput degrades but liveness holds under fairness. | R3 |
| SIM-004 | Delay sweep: latency distributions up to 5 s virtual, including asymmetric request/response delays across 200 seeds. | Visibility and long-poll semantics hold on virtual time exactly; no timeout fires early; histories check clean. | R3 |
| SIM-005 | Duplication sweep: SimNet duplicates 1–10% of frames, including acks and receive responses. | Duplicate frames never create duplicate state transitions (request IDs dedup at the wire layer); histories check clean; receipt single-use holds. | R3 |
| SIM-006 | Reorder sweep: per-link reordering windows up to 100 frames across 200 seeds. | Out-of-order delivery never violates P-02/P-06/P-10; FIFO specs (R4 corpus) preserve P-04 under reorder. | R3 |
| SIM-007 | Partition sweep: symmetric, asymmetric, and partial partitions between cluster nodes with client traffic on both sides, 300 seeds. | No double-lease across any partition (P-08); minority sides reject writes with leader hints; healing converges without lost acks. | R7 |
| SIM-008 | Slow-node sweep: one node's executor is stalled 1–30 s virtual (GC-pause analogue) at random points. | The cluster does not elect dueling leaders on resume (pre-vote), the stalled node rejoins without disrupting commits, and no acked write is lost. | R7 |
| SIM-009 | Clock-jump: wall clock jumps ±1 h and monotonic stays sane; state-machine time comes only from AdvanceTime. | No lease, delay, retention, or dedup boundary shifts (ADR-0005); only log-applied time affects the state machine; wall time appears solely in logs. | R3 |
| SIM-010 | Liveness: under fair schedules (bounded drop, eventual delivery) every sent message reaches Deleted or DeadLettered within a computed virtual-time bound. | The bound holds for all 200 seeds (P-03); a message stuck past the bound dumps its full lifecycle trace. | R3 |
| SIM-011 | Corpus replay: every seed under `sim-corpus/` runs with its recorded verdict on every PR. | 100% of entries reproduce their verdict; a changed verdict is a release-blocking defect even if the new verdict is "pass" (the spec or corpus must be updated deliberately). | R3 |
| SIM-012 | Crash-restart inside simulation: SimDisk persists across simulated process restarts; schedules kill and restart nodes mid-workload. | Recovery inside the simulation matches CRSH- semantics; histories spanning restarts check linearizable; restart count up to 20 per run. | R3 |
| SIM-013 | Long-poll under simulation: receives with WaitTimeSeconds 0–20 while sends arrive at controlled virtual offsets. | Wakeup happens at the exact virtual tick of a matching send; timeout fires at exactly the deadline otherwise; no unrelated request on the same connection is blocked. | R6 |
| SIM-014 | Shrinker: take 10 seeded failing schedules (from the known-bad plant set) and minimize. | The minimizer produces a strictly smaller schedule that still fails, within 5 minutes each; minimized schedules are stable across reruns. | R3 |

### 10.5 Model checking and linearizability matrix (MODL-)

Harness: relay-model checker over JSONL histories; fixtures from
`testdata/histories/`.

| ID | Mechanics | Pass criteria | Earliest gate |
| --- | --- | --- | --- |
| MODL-001 | Check 25 known-good histories (hand-built and sim-emitted) across all profiles. | Every verdict is `Linearizable`; total runtime within the CI budget; verdicts are deterministic across runs. | R1 |
| MODL-002 | Check seeded known-bad histories: lost ack, double lease, invented message, wrong-body delivery, dedup violation, order break. | Every planted violation is detected with the correct property named; zero false passes; this row is the oracle's own regression suite. | R1 |
| MODL-003 | Compare per-queue partitioned checking against whole-history checking on 10 multi-queue histories. | Verdicts are identical; partitioning is a pure performance optimization with a proof obligation, not a semantics change (ADR-0007). | R1 |
| MODL-004 | Minimize counterexamples for each known-bad fixture. | Each minimized subhistory is at most 12 operations, still fails, and names the violated property; minimization completes within 60 s per history. | R1 |
| MODL-005 | Budget enforcement: feed a pathological wide-concurrency history that exceeds the CI budget. | The checker returns `BudgetExceeded` within budget + 5%, never a false `Linearizable`; CI treats it as escalate-to-nightly, and the nightly 10x budget result is recorded. | R1 |
| MODL-006 | Check lease-exclusivity (P-02) on sim histories with aggressive visibility churn: 100 histories from SIM net-fault sweeps. | No history shows two live leases on one message; violations would reproduce from the printed seed. | R3 |
| MODL-007 | Check FIFO profile (P-04) on R4 group-workload histories, including interleaved groups and redrive. | Per-group delivery order equals acked send order in all histories; cross-group order is unconstrained and not asserted (NG-03). | R4 |
| MODL-008 | NO-INVENTION (P-10): every delivered body's SHA-256 in every history matches a prior send. | Zero inventions across the full corpus; the check also validates receipt/message id referential integrity. | R1 |
| MODL-009 | Failover profile (P-09) on R7 partition/failover histories: 100 histories including leader kills at commit boundaries. | Every acked write appears in post-failover reads; unacked writes may appear or not (NG-09); no history requires a rollback of an acked op to linearize. | R7 |

### 10.6 FIFO, deduplication, and group matrix (FIFO-)

Harness: cargo-nextest over relay-core FIFO logic plus sim FIFO corpus;
model profile `FifoOrder`.

| ID | Mechanics | Pass criteria | Earliest gate |
| --- | --- | --- | --- |
| FIFO-001 | Create queues named with and without `.fifo`; send with and without MessageGroupId to each. | `.fifo` requires MessageGroupId (stable error otherwise); standard queues reject FIFO-only parameters; suffix is excluded from the 80-char name budget. | R4 |
| FIFO-002 | Send 10,000 messages across 100 groups from interleaved producers; receive and delete with concurrent consumers (simulated). | Within each group, delivery order equals acknowledged send order (P-04) for every interleaving tested; MODL-007 verifies the histories. | R4 |
| FIFO-003 | Block one group with an undeleted in-flight message while other groups continue. | Distinct groups deliver in parallel (FR-FIFO-003); the blocked group's throughput is zero while others are unaffected. | R4 |
| FIFO-004 | With one message of a group in flight, attempt receives that would deliver later messages of that group. | No later message of the group is delivered until the in-flight message is deleted or its visibility expires (FR-FIFO-004); expiry then redelivers the head first (order preserved). | R4 |
| FIFO-005 | Content-based dedup enabled: send identical bodies, bodies differing in one byte, and identical bodies with different attributes. | Dedup key is SHA-256 of the body exactly (FR-FIFO-005): identical bodies within the window dedup; one-byte difference does not; attribute differences do not affect the content key. | R4 |
| FIFO-006 | Provide explicit MessageDeduplicationId with content dedup both enabled and disabled. | The explicit id always wins over content hashing (FR-FIFO-006); two different bodies with one id dedup; identical bodies with distinct ids do not. | R4 |
| FIFO-007 | Boundary sweep: duplicate sends at window+0 ns, window−1 ns, and exactly 300 s after the original, via AdvanceTime. | The 300 s window holds exactly at both boundaries (P-05): at 299.999999999 s the send dedups, at 300 s it is a new message; behavior is bit-deterministic. | R4 |
| FIFO-008 | Inspect the response of a deduplicated send. | The duplicate send succeeds and returns the original message ID (FR-FIFO-007), distinguishable by the dedup marker in the response; no second message exists. | R4 |
| FIFO-009 | Exhaust receives on a FIFO group so messages dead-letter; then StartRedrive back. | The DLQ move preserves group identity and relative order; redrive-back re-enqueues in original per-group order and downstream delivery honors it (FR-FIFO-008). | R4 |
| FIFO-010 | Drive FIFO in-flight to 19999/20000/20001 across many groups. | The 20,000 FIFO in-flight cap returns the stable backpressure error at exactly the cap and recovers after deletes (FR-QUEUE-016). | R4 |
| FIFO-011 | Visibility expiry inside a blocked group with 3 queued successors; also delete-after-expiry races via stale handles. | After expiry the head redelivers before any successor; the stale first-delivery handle is rejected by epoch; order never inverts. | R4 |

### 10.7 Topics, subscriptions, and fanout matrix (TOPC-)

Harness: cargo-nextest over relay-core topic logic plus sim topic corpus.

| ID | Mechanics | Pass criteria | Earliest gate |
| --- | --- | --- | --- |
| TOPC-001 | Create/delete topics at name-regex boundaries; delete a topic with 0, 1, and 50 subscriptions; then operate the previously subscribed queues. | DeleteTopic removes exactly its subscriptions; subscribed queues and their already-delivered messages are untouched (FR-TOPIC-007); republish to the deleted topic fails with the stable missing-topic error. | R5 |
| TOPC-002 | Publish to a topic with 1, 10, and 100 matching subscriptions; delete one copy from one queue. | Every matching subscription receives an independent copy with its own message ID and lifecycle (FR-TOPIC-003); deleting one copy never affects another queue's copy. | R5 |
| TOPC-003 | Table-driven filter evaluation: exact, anything-but, prefix, numeric range (open/closed/half bounds), and exists, over String and Number attributes, including missing attributes and type mismatches. | Every table row matches the specified verdict from PRODUCT_REQUIREMENTS; unknown operator content cannot be created (rejected at subscribe); evaluation is side-effect free. | R5 |
| TOPC-004 | Subscribe with invalid policies: unknown operator, wrong value type, empty policy object, nesting past the documented depth, oversized policy. | Every invalid policy is rejected at subscribe time with field-level errors naming the exact path (FR-TOPIC-005); no partial subscription is recorded. | R5 |
| TOPC-005 | Subscribe with policy P1, publish, replace the subscription's policy with P2, publish again. | Each publish is evaluated against the policy recorded at its evaluation time; the recorded-at-subscribe-time contract holds and policy replacement has a bounded, observable effective point (FR-TOPIC-002). | R5 |
| TOPC-006 | Race unsubscribe against concurrent publishes in simulation: 100 seeds with unsubscribe interleaved at every publish stage. | Each publish either delivers a complete copy or none to the unsubscribing queue — never a partial record; already-delivered copies remain (FR-TOPIC-006). | R5 |
| TOPC-007 | Publish while one subscribed queue is at its in-flight/backpressure limit or storage-degraded, with 9 healthy subscriptions. | The healthy 9 receive copies; the constrained queue's failure is isolated and reported per-subscription; fanout is never cross-queue atomic (NG-02) and never blocks on the slowest queue. | R5 |
| TOPC-008 | Fan out into a `.fifo` queue with MessageGroupId and dedup id supplied via publish parameters, duplicate publishes within the window. | Group ordering and the 300 s dedup window behave exactly as direct FIFO sends (FR-TOPIC-008); duplicates dedup per destination queue semantics. | R5 |
| TOPC-009 | Publish to a topic with zero subscriptions and to a topic whose only subscription filters the message out. | Both publishes succeed with a delivered-count of 0; no storage grows; the response distinguishes "no match" from error. | R5 |

### 10.8 Wire protocol and fuzzing matrix (WIRE-/FUZZ-)

Harness: cargo-nextest over relay-wire and relay-server with a scripted
loopback client; adversarial fixtures from `testdata/adversarial/`;
cargo-fuzz for FUZZ- rows.

| ID | Mechanics | Pass criteria | Earliest gate |
| --- | --- | --- | --- |
| WIRE-001 | Parse frames with len 0, 1, 1048576, and 1048577; wrong magic; length disagreeing with delivered bytes. | The 1 MiB cap is enforced before any allocation (FR-API-010); over-cap frames fail with the stable frame-too-large error and connection policy applies; wrong magic closes the connection. | R6 |
| WIRE-002 | Flip each byte class of valid frames (magic, len, crc, opcode, flags, request_id, body) and submit. | CRC32C rejects every body/header corruption it covers; corruption is detected before dispatch; the error carries no echo of the corrupt bytes. | R6 |
| WIRE-003 | For every opcode, round-trip its fixed body layout at field min/max, then submit truncated bodies, oversized bodies, and out-of-range length-prefixed variable fields. | Every opcode body parses only its exact layout; every malformed variant returns the stable per-opcode validation error before any state change; no general-purpose deserializer exists on the path (ADR-0004). | R6 |
| WIRE-004 | Send unknown opcodes and unsupported protocol versions during and after negotiation. | Version negotiation rejects unknown versions with the stable error before any state change (FR-API-009); unknown opcodes within a valid version return the stable unknown-opcode error and do not close the connection unless repeated past the abuse threshold. | R6 |
| WIRE-005 | Authenticate frames with valid HMAC, wrong key, wrong tenant, truncated tag, and bit-flipped tag; measure comparison timing over 100k trials. | Only valid tags authenticate (FR-API-003); comparison is constant-time within measurement noise (NFR-SEC-004, via `subtle`); failures are indistinguishable in timing and error content. | R6 |
| WIRE-006 | Exercise ACL combinations: allow, deny, both (deny precedence), unlisted resource, wildcard scope, and cross-tenant access attempts. | Deny always wins (FR-API-004); cross-tenant access is denied with no existence leak (identical error for missing vs forbidden); every decision is auditable. | R6 |
| WIRE-007 | Drive a tenant to its send/receive/admin rate limits and burst caps; continue past them; wait for refill. | Exactly the stable throttle error is returned with a retry-after hint (FR-API-005); other tenants are unaffected; enforcement is per-tenant token bucket per config. | R6 |
| WIRE-008 | Issue a 20 s long-poll receive, then pipeline 50 other requests on the same connection; also 128 concurrent long polls on one connection. | Long polls never block unrelated requests on the connection (FR-API-007); responses match request IDs; the 129th in-flight request hits the stable in-flight cap error. | R6 |
| WIRE-009 | Connect with TLS 1.2, TLS 1.3, plaintext to a TLS listener, and plaintext to a loopback-plaintext listener. | Only TLS 1.3 succeeds on TLS listeners (FR-API-008); plaintext works solely under the explicit loopback configuration; handshake failures are classified and rate-limited. | R6 |
| WIRE-010 | Slowloris: open max_connections, dribble one byte per 25 s, hold half-open TLS handshakes, and stall reads mid-frame. | Read/write deadlines (30 s) reap every stalled connection; per-connection memory stays under the documented cap; healthy clients on remaining slots are unaffected (NFR-SEC-006, NFR-AVAIL-003). | R6 |
| WIRE-011 | Forge receipt handles: flip each field (queue_id, message_id, lease_epoch, expiry, tag), replay a pre-expiry handle after redelivery, and splice fields between valid handles. | Every forgery and splice is rejected by HMAC or epoch validation (P-07, NFR-SEC-001); a replayed handle from a superseded delivery fails on epoch; validation is constant-time on the tag. | R6 |
| WIRE-012 | Trigger every error path in the taxonomy via scripted requests; diff the produced code set against the documented taxonomy. | Every failure maps to exactly one stable machine-readable code (FR-API-006); the golden taxonomy file matches the produced set bidirectionally — no undocumented codes, no unreachable documented codes. | R6 |
| FUZZ-001 | `fuzz_frame_parse`: arbitrary bytes into the frame parser; smoke 60 s per PR, 4 h nightly. | No crash, hang, OOM, or over-cap allocation ever; corpus and dictionary are checked in; nightly coverage report shows every parser branch reached. | R6 |
| FUZZ-002 | `fuzz_opcode_bodies`: structure-aware fuzzing of every opcode body layout behind a valid frame. | No crash or state mutation from any malformed body; every rejection is a typed error; new coverage plateaus are reviewed monthly. | R6 |
| FUZZ-003 | `fuzz_wal_record`: arbitrary bytes into WAL record and segment-header parsers. | No crash, no unbounded allocation, no acceptance of any corrupt record; runs from R2 onward since the parser exists before the wire does. | R2 |
| FUZZ-004 | `fuzz_filter_policy`: arbitrary bytes into filter-policy parsing and evaluation against fuzzed attributes. | No crash; parse and evaluation are total functions; rejected policies never partially apply. | R5 |
| FUZZ-005 | Corpus gate: replay every checked-in fuzz corpus entry as a deterministic test on every PR; verify every historical crash input is present. | 100% corpus replay green; a fix PR for any fuzz finding is rejected by `docs-policy` unless it adds the crash input to the corpus (NFR-SEC-002, FR-API-002). | R6 |

### 10.9 Raft replication matrix (RAFT-)

Harness: relay-sim multi-node schedules (deterministic) plus the live
3-node smoke harness for the packaged-binary rows; histories checked under
the `Failover` profile.

| ID | Mechanics | Pass criteria | Earliest gate |
| --- | --- | --- | --- |
| RAFT-001 | Elections from cold start, leader crash, and leader isolation across 300 seeds with randomized 500–1000 ms timeouts. | Exactly one leader per term; elections converge within 3 election timeouts in 99% of seeds and within 10 in all; no seed livelocks. | R7 |
| RAFT-002 | A partitioned node increments internally, heals, and rejoins; with and without pre-vote (pre-vote off exists only as a test configuration). | With pre-vote on (the only shipping mode), the rejoining node does not depose a healthy leader (FR-REPL-001); the pre-vote-off control demonstrates the harness detects deposition. | R7 |
| RAFT-003 | Commit-rule scenarios including the Raft figure-8 case: a leader replicates an old-term entry to a majority, crashes, and successors act. | An entry is committed only via the current-term commit rule after majority durable append (FR-REPL-002); no committed entry is ever overwritten in any seed. | R7 |
| RAFT-004 | Kill the leader at every stage between client send and ack across 500 seeds; clients retry against the new leader. | No acknowledged write is lost (P-09, FR-REPL-003); unacked writes may or may not survive (NG-09); MODL-009 verifies the histories. | R7 |
| RAFT-005 | Grant leases while partitioning the leader mid-grant; both partition sides serve receives, 300 seeds. | No two consumers ever hold a live lease on one message across any partition (P-08, FR-REPL-004): lease grants linearize through the replicated log, and a deposed leader cannot mint acks. | R7 |
| RAFT-006 | Send writes to followers and candidates; follow the returned hint; hint chains during an election. | Non-leaders reject writes with a current-leader hint (FR-REPL-007); the relay-client follows hints to success within a bounded hop count; during elections the stable no-leader error is returned. | R7 |
| RAFT-007 | Snapshot install to a lagging follower and a brand-new node: verify chunking, resumption, and final state hash. | Install streams 1 MiB chunks, resumes after interruption, and the installed state's SHA-256 matches the snapshot footer before the node serves (FR-REPL-005). | R7 |
| RAFT-008 | Crash the installing follower and the sending leader at each chunk boundary; corrupt one chunk in flight. | Corrupt chunks are detected by per-chunk CRC and re-fetched; partial installs never become serving state; a fresh leader resumes or restarts the install deterministically. | R7 |
| RAFT-009 | Add and remove one member at a time, including under concurrent leader failure and partition; attempt a two-at-once change. | Single-server changes commit safely under every schedule (FR-REPL-006); a second concurrent change is rejected until the first commits; quorum is never ambiguous. | R7 |
| RAFT-010 | Linearizable reads via ReadIndex from the leader and from a deposed-but-unaware leader behind a partition. | ReadIndex reads reflect every committed write at the read point (FR-REPL-008); the deposed leader cannot serve a stale read — it fails the heartbeat confirmation round. | R7 |
| RAFT-011 | Stop one node of a live packaged 3-node cluster; run the full client workload for 10 minutes; restart the node. | Reads and writes continue throughout with one node down (NFR-AVAIL-001); the restarted node catches up via log or snapshot and rejoins without a client-visible error burst. | R7 |
| RAFT-012 | Clean-kill the live leader 20 times; measure kill-to-first-new-acked-write in simulation (exact virtual time) and record live wall time. | Simulated failover-to-ack is ≤ 5 s in every trial (NFR-AVAIL-002 simulated form); live measurements are recorded for R9's measured claim, not asserted here. | R7 |
| RAFT-013 | Run a mixed-version cluster: N and N+1 binaries in all 3-choose combinations through the full workload plus a rolling upgrade and one rollback step. | The cluster operates correctly within the one-minor-version window (FR-REPL-009); wire and log compatibility hold in both directions; the window's boundaries (N with N+2) refuse cleanly. | R10 |

### 10.10 Administration and operability matrix (ADMN-/OPSX-)

Harness: cargo-nextest against relay-server admin surface; relayctl golden
tests; live harness for endpoint rows.

| ID | Mechanics | Pass criteria | Earliest gate |
| --- | --- | --- | --- |
| ADMN-001 | DescribeQueue/DescribeTopic during active traffic; compare returned approximate counts to the exact simulated truth. | Configuration is exact; counts are labeled approximate with a staleness bound (FR-ADMIN-001), and the true value lies within the documented bound in every trial. | R6 |
| ADMN-002 | ListQueues/ListTopics with 0, 1, 999, and 1,000 resources, prefix filters at match boundaries, and cursors resumed after creates/deletes between pages. | Pagination is complete and duplicate-free for a stable set; concurrent mutation yields the documented at-least-once page semantics; cursors are opaque and expire cleanly (FR-ADMIN-002). | R6 |
| ADMN-003 | Tag, retag, untag resources; list by tag with multiple matches; exceed the per-resource tag limit. | Tag operations are idempotent where specified, list-by-tag is exact, and the limit produces its stable error (FR-ADMIN-003). | R6 |
| ADMN-004 | SetQueueAttributes for every mutable attribute at range boundaries; invalid values; measure propagation to the data plane. | Valid changes apply within the documented propagation bound; invalid values are rejected atomically with field-level errors; immutable attributes (fifo-ness) are rejected explicitly (FR-ADMIN-004). | R6 |
| ADMN-005 | DeleteQueue with live messages, in-flight leases, topic subscriptions, and a DLQ role; then use stale handles and republish to the topic. | Deletion is terminal: handles invalidate, subscriptions are removed, storage is reclaimed by compaction (verified), and the DLQ-role dependency is handled per specification (FR-ADMIN-005). | R6 |
| ADMN-006 | Execute every administrative operation through relayctl in human and `--json` modes against a live node; diff against golden output. | relayctl covers 100% of the admin surface (enforced by the Section 22 audit); JSON output is schema-stable and script-safe; human output carries no load-bearing information absent from JSON (FR-ADMIN-006). | R8 |
| OPSX-001 | Probe `/healthz` and `/readyz` on port 7415 through startup, steady state, lost quorum, minority partition, leadership change, and shutdown drain. | Health reflects process liveness only; readiness reflects Raft membership and leadership state truthfully at every stage (FR-OPS-003); probes never block on the data plane. | R8 |
| OPSX-002 | Create 1,000 queues, 100 topics, and 10 tenants; scrape metrics; count series and diff label sets against the inventory. | Total series stay ≤ 5,000 (the ADR-0010 cardinality budget); no forbidden label (message id, group id, receipt, raw tenant secret) appears; over-budget registration is refused and counted, not emitted (FR-OPS-004). | R8 |
| OPSX-003 | Perform every admin mutation via API and relayctl; inspect the audit log; attempt a mutation that fails authorization. | Every mutation writes exactly one audit record with actor, tenant, resource, before/after summary, and result; denied attempts are audited as denied; the log is append-only JSONL (FR-ADMIN-008). | R8 |
| OPSX-004 | Run `relayctl diagnose` against a loaded node seeded with secret canaries in config paths, tenant keys, and message bodies. | The bundle contains the Section 16.3 inventory and nothing else; zero canaries appear in any file (NFR-SEC-003); message bodies and attributes are never included. | R8 |
| OPSX-005 | SIGTERM under load: in-flight receives, long polls, and a compaction running. | Shutdown drains in-flight work within the 30 s drain budget, rejects new work with the stable draining error, fsyncs, releases the lock, and exits 0 (NFR-AVAIL-004); SIGKILL-after-drain-budget leaves a recoverable directory. | R6 |
| OPSX-006 | Emit every log event class; validate each line against the field convention schema (Section 16.1). | Every line is single-line JSON with the mandatory fields; levels are correct; no line exceeds 16 KiB; golden schemas match (FR-OPS-006). | R8 |
| OPSX-007 | Start relayd under the full config-precedence grid (flag vs env vs file vs default for representative keys) and the complete invalid-config table. | Precedence is exactly flags > env > file > defaults; every invalid case exits code 78 with its exact RELAY-CFG code from Section 7.3; no listener binds before validation completes (FR-OPS-002). | R6 |
| OPSX-008 | Execute a client request lifecycle with OTLP export to a local collector; inspect spans for send, receive, delete, raft append, fsync, and long-poll wait. | Spans cover the documented lifecycle with correct parentage and the trace ids in logs match (FR-OPS-005); disabling the endpoint removes all export attempts. | R8 |
| OPSX-009 | Run the Section 14 uninstall and purge procedures on a live install; scan the filesystem before and after. | Uninstall leaves data intact and removes exactly binaries and unit files; purge removes every path in the Section 6.1 tree and nothing outside it; both report each path acted on (FR-OPS-009). | R10 |

### 10.11 Migration and soak matrix (MIGR-/SOAK-)

Harness: frozen fixtures from `testdata/fixtures/onDisk/v<N>/` for MIGR-;
live 3-node harness on the dedicated nightly runner for SOAK-.

| ID | Mechanics | Pass criteria | Earliest gate |
| --- | --- | --- | --- |
| MIGR-001 | Start the current binary on every supported frozen old-version WAL data dir; run the workload; compare recovered state to the fixture manifest. | Recovery replays old-format WAL exactly to the manifest state (NFR-DUR-007); new writes use the current format; the fixture is never modified in place before a successful format upgrade step. | R10 |
| MIGR-002 | Load frozen old-version snapshots, including one requiring WAL replay on top. | Old snapshots load with footer verification; replay on top produces the manifest state; the combined path matches a fresh-format equivalent hash. | R10 |
| MIGR-003 | Start an old binary against a data dir written by a newer format version. | The old binary refuses with RELAY-CFG-0040 before any write (downgrade refusal policy: downgrade is supported only across releases that share an on-disk format version, otherwise restore from backup); the error names both versions and the remediation. | R10 |
| MIGR-004 | Feed the current binary config files written for the previous minor version, including keys since renamed or newly required. | The documented config compatibility window (one minor version, with deprecation warnings) holds; removed keys fail with RELAY-CFG-0003 plus a migration hint; no silent reinterpretation occurs. | R10 |
| MIGR-005 | Execute the Section 14.4 rolling upgrade on a live 3-node cluster N→N+1 under client load, then roll one node back to N within the window. | Zero client-visible ack loss during the roll (ties RAFT-013); the mixed-version window holds; the rollback node rejoins and serves; evidence records per-step cluster health. | R10 |
| SOAK-001 | 24 h nightly churn: mixed send/receive/delete/purge/redrive/subscribe workload at 30% of benchmark throughput across standard, FIFO, and topic paths, with invariant scans every 15 min. | All invariant scans pass (counts reconcile, no stuck message beyond its bound, fsck clean); the run completes with zero quarantine-eligible failures outstanding past SLA. | R8 |
| SOAK-002 | Sample RSS, file descriptors, thread count, and disk usage every 60 s across the soak; fit a slope after warmup. | Post-warmup RSS slope ≤ 1 MiB/h, fd count stable within ±10, thread count constant; any breach fails the nightly with the series attached. | R8 |
| SOAK-003 | During the soak, SIGKILL a random node every 10 minutes (144 kills); each restart must recover and rejoin. | Every recovery completes within the recovery budget, no acked message is lost across the entire run (continuous P-01 audit), and the cluster never loses availability beyond single-node windows. | R8 |
| SOAK-004 | Run retention expiry and compaction continuously over the soak with 4 h retention on churn queues. | Disk usage plateaus (bounded sawtooth) rather than growing monotonically; compaction reports reclaim matching expiry volume; no live message is reclaimed (spot-audited). | R8 |

### 10.12 Benchmarks, mutation, and claims-audit matrix (BENCH-/MUT-/MKT-)

Harness: relay-bench and criterion on the dedicated benchmark runner
(reference hardware: 8 vCPU / 16 GiB / local NVMe, Linux 6.x); cargo-mutants
nightly; claims audit scripts under `xtask`.

| ID | Mechanics | Pass criteria | Earliest gate |
| --- | --- | --- | --- |
| BENCH-001 | Harness validity checks: verify open-loop load generation for latency runs, coordinated-omission-safe timestamping (send-schedule based), warmup discard, run-to-run variance across 5 repeats, and a null-server calibration. | Latency numbers are coordinated-omission-safe; 5-repeat coefficient of variation ≤ 5% for throughput and ≤ 10% for p99, else the run is invalid; calibration overhead is published with results (NFR-PERF-005). | R9 |
| BENCH-002 | Sustained send+receive+delete at 256-byte bodies on a single node for 30 minutes. | Throughput ≥ 20,000 msg/s sustained with fsync-before-ack on (NFR-PERF-001); the result JSON records hardware, config, and statistical treatment. | R9 |
| BENCH-003 | Send-to-ack latency distribution at the NFR-PERF-001 load point. | p99 ≤ 15 ms and the full distribution (p50/p90/p99/p999/max) is published (NFR-PERF-002); no clamping or trimming of the tail. | R9 |
| BENCH-004 | Long-poll wakeup: idle pollers, matching sends at controlled instants, wakeup latency measured externally. | p99 wakeup ≤ 10 ms after the matching send's ack (NFR-PERF-003), published as a goal-backed measurement, never as a contract (NG-05). | R9 |
| BENCH-005 | Build a 10 GiB WAL under recorded workload; crash; measure recovery to serving-ready. | Recovery ≤ 30 s (NFR-PERF-004) across 5 repeats; the recovery profile (scan, replay, index) is broken down in the result. | R9 |
| BENCH-006 | Live 3-node cluster: clean-kill the leader 20 times under client load; measure kill-to-first-new-acked-write wall time. | Measured failover ≤ 5 s in every trial on reference hardware (NFR-AVAIL-002 measured form); distribution published alongside the RAFT-012 simulated evidence. | R9 |
| MUT-001 | cargo-mutants over relay-core with the pinned toolchain, nightly; the mutant list is diffed against the previous run. | ≥ 85% of generated mutants are killed (NFR-MAINT-003); every surviving mutant is triaged as equivalent (documented) or gets a killing test before the R4 evidence closes. | R4 |
| MUT-002 | cargo-mutants over the relay-wal recovery and truncation paths (scoped run). | ≥ 80% kill rate on the scoped paths; survivors on the truncation logic specifically are treated as release-blocking regardless of the aggregate. | R4 |
| MKT-001 | Claims audit: extract every quantitative or guarantee claim from README, MARKETING.md, and site copy; resolve each to a P-xx property, NG-xx statement, or BENCH result id. | 100% of public claims resolve to evidence (FR-MKT-001/002); a claim with no resolvable evidence fails the audit; the audit output is a checked-in report. | R9 |
| MKT-002 | Scan marketing and docs for exactly-once implications (phrase list plus reviewer pass) and verify the NG list placement. | Every surface where exactly-once could be inferred carries the at-least-once statement and NG-01 reference (FR-MKT-003); the phrase scan has zero unreviewed hits. | R9 |
| MKT-003 | Run the release-announcement checklist against the R10 candidate's collateral: badges, comparison table, launch copy. | Every checklist item passes and the signed checklist is in the R10 evidence manifest (FR-MKT-004/005); comparison-table rows cite sources per MARKETING.md rules. | R10 |

## 11. Continuous Integration

### 11.1 Workflow inventory

GitHub Actions, all job bodies delegated to `just` recipes so CI equals
local execution:

| Workflow | Trigger | Jobs | Wall budget |
| --- | --- | --- | --- |
| `lint-deny.yml` | every PR, main | `just lint`, `just deny`, docs-policy checks (PR template, status vocabulary, conventional-commit title, golden-change justification) | 8 min |
| `unit.yml` | every PR, main | `cargo nextest run --workspace --locked` on Linux x86_64 and Linux aarch64; macOS aarch64 on main only | 12 min |
| `sim-corpus.yml` | every PR, main | `just sim-corpus` (full corpus replay + verdict check) | 12 min |
| `model-check.yml` | every PR, main | `just model-smoke` (budgeted checking of history fixtures and PR-emitted histories) | 8 min |
| `fuzz-smoke.yml` | every PR, main | `just fuzz-smoke` (60 s/target over checked-in corpus, corpus-replay gate FUZZ-005) | 10 min |
| `package-smoke.yml` | main, release branches | `just package` then install/run/uninstall smoke of the tarball and container on both Tier 1 architectures | 20 min |
| `nightly-deep.yml` | nightly cron | `just sim-sweep seeds=20000` (2 h), `just fuzz-deep` (4 h/target, parallel), `just model-deep` (1 h), `just mutants` (4 h) | 6 h aggregate |
| `nightly-soak.yml` | nightly cron, dedicated runner | `just soak hours=24` (SOAK-001..004) | 25 h |
| `nightly-bench.yml` | nightly cron, dedicated benchmark runner | `just bench` plus relay-bench macro runs with regression bands | 90 min |

### 11.2 Caching

- `Swatinem/rust-cache` keyed on lockfile hash plus toolchain for target
  and registry caches; fuzz and mutants jobs use separate cache keys.
- Caches are never used for correctness-relevant inputs: corpus, fixtures,
  and golden files always come from the checkout.
- A cache-poisoning drill (delete all caches, rerun main) runs monthly;
  a cold run must still pass inside 2x the wall budget.

### 11.3 Runners

PR and main jobs run on GitHub-hosted `ubuntu-24.04` (x86_64) and
`ubuntu-24.04-arm` (aarch64). Soak and benchmark jobs run on one dedicated
self-hosted runner matching the reference hardware, labeled `relay-perf`,
which runs nothing else; benchmark numbers from any other machine are
non-evidence by definition.

### 11.4 Required checks

Merging to `main` requires green: `lint-deny`, `unit (linux-x86_64)`,
`unit (linux-aarch64)`, `sim-corpus`, `model-check`, `fuzz-smoke`. From R6
onward `package-smoke` is added to the required set on release branches;
from R7 the live 3-node smoke joins main's post-merge required set (its
quarantine ledger, if non-empty, appears in every gate review). Nightly
failures do not block individual merges but block the next gate closure
until triaged, and any nightly failure with a deterministic seed becomes a
required corpus entry.

### 11.5 Evidence artifact retention

Every run uploads: nextest JUnit XML, sim verdict JSON with seeds, model
verdicts with any counterexamples, fuzz corpus deltas and coverage
summaries, benchmark result JSON, and soak time-series. Retention: PR
artifacts 30 days; main and nightly artifacts 90 days; anything referenced
by an `evidence/R<n>/manifest.json` is re-uploaded to the `evidence/`
release storage and retained ≥ 400 days and for the life of any release
that cites it. Artifacts never contain message bodies, tenant keys, or
secret canaries; the redaction scan (OPSX-004 machinery) runs over uploaded
artifacts on release branches.

## 12. Performance and Resource Budgets

### 12.1 Product performance budgets (NFR-PERF)

Owned by [BENCHMARK_PLAN.md](./BENCHMARK_PLAN.md) for method; restated here
as release gates. Reference hardware: 8 vCPU / 16 GiB / local NVMe,
Linux 6.x. All `planned`; measured at R9.

| Budget | Value | Evidence |
| --- | --- | --- |
| Sustained single-node throughput, 256 B bodies, fsync-before-ack | ≥ 20,000 msg/s (send+receive+delete) | BENCH-002 |
| Send-to-ack latency at that load | p99 ≤ 15 ms | BENCH-003 |
| Long-poll wakeup after matching send | p99 ≤ 10 ms (goal-backed, NG-05) | BENCH-004 |
| Crash recovery, 10 GiB WAL | ≤ 30 s | BENCH-005 |
| Clean leader kill to first new acked write | ≤ 5 s | RAFT-012 (simulated), BENCH-006 (measured) |

### 12.2 Process resource budgets

| Resource | Budget | Enforcement |
| --- | --- | --- |
| relayd steady-state RSS at benchmark load | ≤ 2 GiB | nightly-bench assertion |
| relayd idle RSS, empty single node | ≤ 128 MiB | package-smoke assertion |
| Per-connection memory cap | ≤ 512 KiB excluding the in-flight frame | WIRE-010 |
| Soak RSS slope after warmup | ≤ 1 MiB/h | SOAK-002 |
| relayd stripped static binary | ≤ 30 MiB | package job hard fail |
| relayctl stripped static binary | ≤ 15 MiB | package job hard fail |
| Container image (relayd, from scratch) | ≤ 40 MiB | package job hard fail |

### 12.3 Suite and CI wall budgets

Per-suite runtime budgets are in Section 8.3; workflow wall budgets in
Section 11.1. The aggregate PR pipeline (all required checks, parallel)
must complete in ≤ 15 minutes at the median and ≤ 25 minutes at p99 over a
rolling month; breaching the p99 for two consecutive weeks opens a `perf:`
defect against the slowest job. Nightly aggregate stays under 6 hours
excluding the 24 h soak, which owns its dedicated runner.

## 13. Packaging and Artifact Construction

### 13.1 Static build

`just package` produces, per Tier 1 architecture:

1. `relayd` and `relayctl` built with `--release --locked` for
   `x86_64-unknown-linux-musl` and `aarch64-unknown-linux-musl`: fully
   static, no dynamic interpreter (`file` and `ldd` checks in the job),
   `panic = "abort"`, symbols stripped after a split debuginfo artifact is
   archived.
2. `relay-<version>-<arch>-linux-musl.tar.zst` containing `relayd`,
   `relayctl`, `relay.toml.example`, `relay.service` (Section 14.1), the
   license file, and `SHA256SUMS`.
3. Both binaries embed version, git SHA, build timestamp (from
   `SOURCE_DATE_EPOCH`), rustc version, and target triple, printed by
   `relayd --version` and included in logs at startup (FR-OPS-001).

### 13.2 Container image

Built from `scratch` (distroless-equivalent, no shell, no package manager).
Exact contents, verified by a manifest test that lists every layer file:

```text
/usr/local/bin/relayd
/usr/local/bin/relayctl
/etc/relay/relay.toml            # example config, plaintext-loopback dev profile
/etc/ssl/certs/ca-certificates.crt  # for OTLP export TLS only
/var/lib/relay/                  # empty volume mount point, declared VOLUME
```

`USER 7414:7414`, `EXPOSE 7414 7415 7416`, entrypoint
`["/usr/local/bin/relayd"]`. Any file in the image outside this list fails
the package job. Images are tagged `vX.Y.Z` and by digest; `latest` points
only at the newest non-revoked release.

### 13.3 SBOM and provenance

- SBOM: CycloneDX JSON generated from `Cargo.lock` (cargo-auditable data
  embedded in the binaries, plus a syft pass over the container), attached
  to the GitHub release and referenced from the evidence manifest.
- Provenance: SLSA build provenance via GitHub Actions artifact
  attestations (`actions/attest-build-provenance`) for every tarball and
  image digest; verification command documented in the release notes
  (`gh attestation verify`).
- Signing: `SHA256SUMS` is signed with the project's minisign key; the
  public key is in the repository and pinned in the docs.

### 13.4 Reproducible-build verification

Release candidacy requires bit-for-bit reproducibility:

1. CI builds the release artifacts twice on independent runners with
   `SOURCE_DATE_EPOCH` set from the release commit time, locked toolchain,
   `--locked`, and normalized build paths (`--remap-path-prefix`).
2. `sha256sum` of every binary and tarball must match across the two
   builds; a mismatch blocks the release and is triaged as a defect.
3. The verification procedure is documented so a third party can reproduce:
   checkout tag, run `just package-repro`, compare against published
   `SHA256SUMS`. The R10 evidence manifest records one external-style
   reproduction performed from a clean clone.

## 14. Installation, First Run, Update, Rollback, Uninstall, and Purge

All procedures are numbered, idempotent where stated, and exercised by
package-smoke (single-node paths) and the live harness (cluster paths).

### 14.1 Install (single node, systemd)

1. Download `relay-<version>-<arch>-linux-musl.tar.zst`, `SHA256SUMS`, and
   `SHA256SUMS.minisig`; verify: `minisign -Vm SHA256SUMS && sha256sum -c SHA256SUMS --ignore-missing`.
2. `sudo tar --zstd -xf relay-<version>-<arch>-linux-musl.tar.zst -C /usr/local/bin relayd relayctl`
3. `sudo useradd --system --home /var/lib/relay --shell /usr/sbin/nologin relay` (idempotent: skip if present).
4. `sudo install -d -m 0700 -o relay -g relay /var/lib/relay`
5. `sudo install -d -m 0755 /etc/relay && sudo install -m 0644 relay.toml.example /etc/relay/relay.toml`
   then edit `[listen]`, `[tls]`, and `[node]` for the host.
6. Install the unit below to `/etc/systemd/system/relay.service`, then
   `sudo systemctl daemon-reload && sudo systemctl enable --now relay`.
7. Verify: `relayctl --addr 127.0.0.1:7414 cluster health` reports the node
   serving, and `curl -fsS http://127.0.0.1:7415/readyz` returns ready.

```ini
[Unit]
Description=Relay message queue (relayd)
Documentation=https://github.com/Zachshotamartin/relay
After=network-online.target
Wants=network-online.target

[Service]
Type=notify
User=relay
Group=relay
ExecStart=/usr/local/bin/relayd --config /etc/relay/relay.toml
Restart=on-failure
RestartSec=2
# fsync failure aborts by design (ADR-0008); systemd restarts into recovery.
LimitNOFILE=65536
NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/var/lib/relay
PrivateTmp=true
ProtectKernelTunables=true
ProtectControlGroups=true
RestrictSUIDSGID=true
MemoryDenyWriteExecute=true
SystemCallArchitectures=native

[Install]
WantedBy=multi-user.target
```

### 14.2 First run

1. On first start, relayd validates config (Section 7.3), creates the
   Section 6.1 tree, generates `keys/receipt.key` (0600), takes the lock,
   and logs its embedded version and resolved config sources.
2. `relayctl queue create smoke-test && relayctl queue send smoke-test --body ok && relayctl queue receive smoke-test --max 1`
   round-trips one message; `relayctl queue delete-message` with the
   printed receipt, then `relayctl queue delete smoke-test` cleans up.
3. First run makes no outbound connection except configured OTLP; this is
   asserted by the package-smoke network observer.

### 14.3 3-node cluster bootstrap

1. Install per Section 14.1 on all three hosts with unique `node.id`
   (`node-1`..`node-3`), identical `node.cluster`, `raft.members` listing
   all three ids, and correct `listen.advertise` per host.
2. Start node-1 with `--bootstrap` (initializes the Raft log for the
   configured membership exactly once; a second `--bootstrap` against an
   initialized directory is a fail-fast error).
3. Start node-2 and node-3 normally; they join via the configured members.
4. Verify: `relayctl cluster members` shows 3 voters with one leader;
   `relayctl cluster health` is green; `/readyz` is ready on all nodes.
5. Kill any single node and verify writes still succeed (NFR-AVAIL-001
   spot check) before putting traffic on the cluster.

### 14.4 Rolling upgrade (mixed-version window)

Supported window: one minor version (N ↔ N+1) per FR-REPL-009. Procedure,
per node, followers first, leader last:

1. Confirm the target version is within the window
   (`relayctl version --cluster` prints every member's version and flags
   any out-of-window pair).
2. `relayctl cluster drain <node>` (moves leadership if needed; waits for
   in-flight drains).
3. Stop the unit, replace binaries per Section 14.1 steps 1–2, start.
4. Wait for `/readyz` and `relayctl cluster health` green, and for the
   node's replication lag to return to zero.
5. Repeat for the next node. At most one node is down at any time.
6. After all nodes run N+1, run `relayctl fsck --online` and record the
   report in the operations log.

MIGR-005 exercises this exact procedure under load, including one rollback
step, before it is claimed.

### 14.5 Verified rollback

Rollback to the previous release is supported while the on-disk format
version is shared (checked before starting):

1. `relayctl version --cluster` and the release notes confirm N and N−1
   share the on-disk format; if not, rollback is restore-from-backup
   (Section 15.2), never a downgrade start (MIGR-003 enforces refusal).
2. Reverse the Section 14.4 roll: per node, drain, stop, reinstall N−1
   binaries (kept in `/usr/local/lib/relay/previous/` by the install step),
   start, verify readiness and lag.
3. Verify with the same smoke round-trip as Section 14.2 and record the
   rollback in the operations log with the triggering defect id.

### 14.6 Uninstall

Removes software, preserves data:

1. `sudo systemctl disable --now relay`
2. `sudo rm /etc/systemd/system/relay.service && sudo systemctl daemon-reload`
3. `sudo rm /usr/local/bin/relayd /usr/local/bin/relayctl`
4. `/var/lib/relay`, `/etc/relay`, and the `relay` user remain, and the
   procedure prints exactly what it removed and what it preserved.

### 14.7 Purge

Removes everything, irreversibly, after uninstall:

1. `sudo rm -rf /var/lib/relay` — every queue, message, key, and audit log
   on this node; the operator is warned that acked data is destroyed.
2. `sudo rm -rf /etc/relay`
3. `sudo userdel relay`
4. Purge enumerates the Section 6.1 tree before deleting, refuses if
   `data_dir` was configured outside `/var/lib/relay` without an explicit
   `--data-dir` confirmation, and touches nothing outside the enumerated
   paths (OPSX-009 verifies both procedures).

## 15. Backup, Restore, and Disaster Recovery

### 15.1 Backup procedure

Backup = consistent snapshot + WAL archive (FR-OPS-007). Targets: RPO = 0
for acknowledged messages; RTO ≤ 15 minutes for a single node from local
backup media.

1. `relayctl backup create --dest /backup/relay/<date>/` (hot, against the
   running server): the server writes a checkpoint snapshot
   (`snap-<lsn>.rsnap`, footer-verified), then hard-links or copies every
   sealed WAL segment with `lsn > snapshot lsn`, then records a backup
   manifest (`backup.json`: cluster id, node id, snapshot lsn, segment
   list with SHA-256 per file, format versions, wall time).
2. Continuous WAL archiving (optional but required for RPO = 0 off-host):
   `archive_command`-style hook copies each segment on seal; the backup
   manifest names the archive location and last archived seq.
3. `relayctl backup verify /backup/relay/<date>/` re-hashes every file
   against the manifest and replays headers; verification is part of the
   backup job, not an optional extra.
4. Backup runs are scheduled at least daily in the reference runbook;
   backups are restorable across the same on-disk format versions as
   rollback (Section 14.5), and one minor version forward via MIGR-001
   machinery.

### 15.2 Restore procedure

1. Provision a clean host per Section 14.1 steps 1–6 but do not start.
2. `relayctl restore --from /backup/relay/<date>/ --data-dir /var/lib/relay`:
   verifies the manifest hashes, lays down snapshot and segments, replays
   the WAL tail, prints the recovered high-water LSN and message counts,
   and refuses a data dir that is not empty.
3. For a cluster: restore the most recent backup on one node, start it
   with `--bootstrap-from-restore` (re-forms a single-voter cluster at the
   restored state), then add the remaining nodes as new members
   (Section 14.3 pattern); they sync via snapshot install (RAFT-007 path).
4. Verify: `relayctl fsck` clean, smoke round-trip, and reconciliation of
   restored counts against the backup manifest.

### 15.3 Scripted disaster-recovery drill

`just dr-drill` runs the full loop and is a release requirement, not an
operator suggestion:

1. Stand up a single node, drive a recorded workload to a known state
   (message counts and content hashes captured).
2. Take a hot backup mid-load, then continue load, then SIGKILL.
3. Restore onto a fresh directory from backup + archived WAL; start; fsck.
4. Assert: zero acked messages lost (RPO = 0 for acked), recovered counts
   and hashes match the oracle, and wall-clock restore time ≤ 15 min for
   the reference 10 GiB dataset (RTO).
5. Write `evidence/R<n>/dr-drill-<date>.json` with timings, counts, and
   hashes. The drill runs green for every release from R10 onward and its
   record is a mandatory entry in the release evidence manifest.

## 16. Diagnostics, Logs, and Support Bundles

### 16.1 Log field conventions

Structured JSON, one event per line (FR-OPS-006). Mandatory fields on every
line: `ts` (RFC 3339 UTC, wall clock — logs are the only place wall time is
authoritative), `level`, `event` (stable snake_case identifier), `node`,
`msg`. Contextual fields when applicable: `queue`, `topic`, `tenant`
(opaque tenant id, never a key), `request_id`, `trace_id`, `span_id`,
`opcode`, `error_code`, `lsn`, `term`, `duration_us`. Forbidden in logs
forever: message bodies, message attributes, receipt handles, HMAC keys or
tags, TLS private material, and full config dumps containing key paths'
contents. `event` names are a golden inventory (OPSX-006); a new event name
is a reviewed golden change. Lines are ≤ 16 KiB; longer payloads are
referenced by id, never inlined.

### 16.2 Metric inventory and cardinality budget

The Prometheus inventory is a checked-in golden
(`docs/generated/metrics.md`, regenerated by `just golden-review`) naming
every metric, type, unit, labels, and meaning. The ADR-0010 cardinality
budget, enforced at runtime and by OPSX-002: ≤ 5,000 active series per node
at the reference shape of 1,000 queues, 100 topics, 10 tenants. Label
allowlist: `queue`, `topic`, `opcode`, `result`, `node`, `peer`, `tenant`
(opaque id). Forbidden labels: message id, group id, dedup id, receipt,
client address, or any unbounded value. relayd refuses over-budget series
registration and increments `relay_metrics_dropped_series_total` instead.

### 16.3 relayctl diagnose

`relayctl diagnose --out bundle.tar.zst` collects, and collects only:

- resolved configuration with every secret-bearing value replaced by
  `<set>` and file paths retained;
- version/build provenance block, uptime, host kernel and arch;
- the last 10,000 log lines (already redaction-safe by Section 16.1);
- full metrics scrape, health/readiness bodies, Raft status
  (`members`, term, commit/applied LSN, lag);
- storage summary: segment list with sizes and seqs, snapshot list,
  disk usage, lock holder identity;
- queue/topic inventory with configuration and approximate counts —
  names and settings, never contents;
- the audit log's most recent 1,000 records;
- `bundle-manifest.json` listing every included file with SHA-256.

### 16.4 Redaction guarantees and canary tests

The bundle never contains message bodies, message attributes, receipt
handles, tenant or receipt keys, or TLS private keys (NFR-SEC-003). This is
tested, not asserted: OPSX-004 seeds unique canary values into tenant keys,
TLS keys, and message bodies, runs a workload, produces a bundle, and scans
every byte of it (raw, base64, hex, and URL-encoded forms of each canary).
One hit fails the release. The same canary scan runs against logs, traces,
error responses, and CI-uploaded artifacts on release branches.

## 17. Data Retention, Privacy, and Purge

### 17.1 Message data lifecycle

Message bodies and attributes exist in exactly three places: the WAL/
snapshot storage under `data_dir`, in-memory state, and the wire between
authenticated client and server. Relay never sends message content to any
other destination — not logs, not traces, not metrics, not diagnostics,
not crash output (the abort handler emits only structural context).

### 17.2 Retention sweep

Per-queue retention (60 s–14 d, default 4 d) removes expired messages via
the retention sweep driven by log-applied time (CORE-013). Deleted and
expired records become dead space reclaimed by compaction (STOR-010); after
compaction plus snapshot turnover, expired content is physically absent
from live files. Relay makes no secure-erase claim for underlying media;
operators with hardware-level requirements are pointed to full-disk
encryption in the runbook.

### 17.3 Operator purge

- `PurgeQueue` removes all of one queue's messages including in-flight
  (CORE-011); `DeleteQueue` is terminal for the queue (ADMN-005).
- Node purge (Section 14.7) destroys all local data irreversibly.
- Backups are the operator's custody: purge procedures remind, and the
  runbook documents, that copies under `/backup` are not touched by any
  Relay purge and must be handled by the operator's own retention policy.

### 17.4 What diagnostics may never contain

Restating the enforced set (OPSX-004 canaries): message bodies, message
attributes, receipt handles, HMAC keys and tags, TLS private keys, raw
tenant credentials, and any unredacted secret-bearing config value. There
is no debug flag that overrides this for bundles intended to leave the
host; deep local debugging uses the operator's own direct file access,
never a widened bundle.

## 18. Incident Response and Release Revocation

### 18.1 Severity ladder

| Severity | Definition | Response clock |
| --- | --- | --- |
| SEV-1 | Any violation of a CORRECTNESS.md property P-01..P-10 observed or reasonably suspected in any released build ("in the wild"), any data-loss or double-delivery-beyond-contract report, or a remotely exploitable security defect. | Acknowledge ≤ 4 h; mitigation guidance ≤ 24 h; public disclosure ≤ 72 h. |
| SEV-2 | Security defect requiring local access or misconfiguration; availability defect that survives the documented recovery procedures; wrong stable error semantics that could mislead automation. | Acknowledge ≤ 24 h; fix in the next patch release. |
| SEV-3 | Defect with a documented workaround; performance regression beyond published bands. | Triage ≤ 1 week; fix scheduled to a named release. |
| SEV-4 | Cosmetic or docs defect. | Normal backlog. |

### 18.2 Correctness-bug protocol

Any P-xx violation in the wild is SEV-1 with public disclosure, no
exceptions and no reclassification downward:

1. Reproduce deterministically: capture the reporter's environment,
   attempt a sim seed or history reconstruction; the fix PR must land the
   reproduction in `sim-corpus/regression/` or `testdata/histories/bad/`.
2. Publish a correctness advisory: affected versions, the violated
   property by P-id, exact conditions, operator detection guidance
   (`relayctl fsck` where applicable), and remediation.
3. Update CORRECTNESS.md status for the property to reflect the gap window
   and the restoring evidence; MARKETING.md claims citing the property are
   suspended until the evidence is green again (FR-MKT rules).
4. Post-incident review within 14 days answers: which matrix row should
   have caught this, and which row now does. A new or strengthened row is
   a mandatory review output.

### 18.3 Release revocation (yank) steps

1. Mark the GitHub release as revoked: title prefixed `[REVOKED]`, body
   updated with the advisory link; artifacts remain downloadable for
   forensics but `SHA256SUMS` is superseded by a `REVOKED` marker file.
2. Update the machine-readable release index (`releases.json` on the
   project site) setting `revoked: true` with the advisory id; `relayctl
   version --check` and the docs install snippet read this index and warn.
3. Re-point the container `latest` tag to the newest non-revoked release;
   the revoked version tags remain but their descriptions carry the
   advisory.
4. Publish a GitHub Security Advisory (GHSA) for security-class causes.
5. Ship the fixed release with the regression evidence, then run MKT-003
   before any announcement.
6. FR-OPS-012 closes at R10 with a tabletop drill of this exact procedure
   recorded in the evidence manifest.

## 19. Release Gates R0–R10

[BUILD_PLAN.md](./BUILD_PLAN.md) §5–§15 defines the feature work per gate;
this section defines each gate's operational and evidence closure. Status:
every gate below is `planned`. A later gate inherits every earlier gate's
constraints; "matrices green" always means from a single mainline commit,
recorded in `evidence/R<n>/manifest.json` with the Section 1.4 contents and
a named sign-off (maintainer identity and date).

### 19.1 R0 — Repository, toolchain, CI, and architecture checks

R0 passes when:

- the repository exists with the Section 3 protections, PR template, and
  conventional-commit linting active;
- the pinned toolchain (1.85.0), `deny.toml`, and `justfile` recipes build
  an empty-but-structured workspace with `just ci-local` green;
- the architecture check proves `relay-core` cannot reach IO, clocks,
  randomness, or threads (a deliberate violation branch fails it);
- `lint-deny`, `unit`, and docs-policy are required checks on `main`;
- `evidence/R0/manifest.json` records the runs and sign-off.

Entry criteria: none (first gate). Required matrices: none yet — R0's
evidence is the harness skeleton itself plus the architecture check.

### 19.2 R1 — Core queue semantics under the model checker

Entry: R0 closed. R1 passes when:

- CORE-001..008, CORE-010..012, CORE-016 are green;
- MODL-001..005, MODL-008 are green, including the known-bad detection
  suite (the oracle catches every planted violation);
- histories emitted by the in-memory harness check `Linearizable` under
  `QueueBasic` within CI budget;
- the requirement rows with terminal gate R1 in Section 20 all resolve;
- evidence manifest and sign-off recorded.

### 19.3 R2 — Durable WAL survives crash, torn-write, and disk-full

Entry: R1 closed. R2 passes when:

- STOR-001..014, CRSH-001..011, and FUZZ-003 are green on both Tier 1
  architectures;
- recovery-equivalence (CRSH-008) has run its full 500 schedules;
- the fsyncgate abort path (CRSH-007) is demonstrated on real files, not
  only SimDisk;
- data-dir permission and lock semantics (STOR-013/014) hold;
- FR-QUEUE-002 and NFR-DUR-001..006 rows resolve; evidence recorded.

### 19.4 R3 — Deterministic simulation with a checked-in corpus

Entry: R2 closed. R3 passes when:

- SIM-001..006, SIM-009..012, SIM-014, and MODL-006 are green;
- the reproducibility meta-test and divergence alarm (SIM-001/002) pass,
  including the planted-nondeterminism detection;
- `sim-corpus/` is populated (≥ 60 seeds across smoke/crash/net
  categories) and `sim-corpus.yml` is a required check;
- NFR-MAINT-002 resolves: every past failing seed replays in CI;
- evidence recorded.

### 19.5 R4 — FIFO, deduplication, delay, DLQ, and redrive

Entry: R3 closed. R4 passes when:

- FIFO-001..011, CORE-009, CORE-013..015, MODL-007, MUT-001, MUT-002 are
  green;
- the dedup boundary sweep (FIFO-007) demonstrates exact 300 s window
  behavior at ±1 ns;
- mutation testing reaches ≥ 85% kill on relay-core with survivors
  triaged (NFR-MAINT-003);
- all FR-FIFO and R4-terminal FR-QUEUE rows resolve; evidence recorded.

### 19.6 R5 — Topics, subscriptions, and filter fanout

Entry: R4 closed. R5 passes when:

- TOPC-001..009 and FUZZ-004 are green, including the unsubscribe race
  seeds and FIFO fanout semantics;
- all FR-TOPIC rows resolve; evidence recorded.

### 19.7 R6 — Bounded, fuzzed wire API with auth, quotas, long polling

Entry: R5 closed. R6 passes when:

- WIRE-001..012, FUZZ-001/002/005, SIM-013, OPSX-005, OPSX-007, and
  ADMN-001..005 are green;
- fuzz targets have completed at least 7 nightly deep runs with zero open
  findings and checked-in corpora gate CI;
- receipt forgery, constant-time comparison, TLS-only, and slowloris rows
  hold (NFR-SEC-001/002/004/006);
- `package-smoke` joins the required checks; graceful shutdown and
  backpressure (NFR-AVAIL-003/004) resolve;
- all FR-API rows, R6-terminal FR-ADMIN and FR-QUEUE-009 rows resolve;
  evidence recorded.

### 19.8 R7 — Raft replication: partition and failover safety

Entry: R6 closed. R7 passes when:

- RAFT-001..012, SIM-007, SIM-008, and MODL-009 are green, including the
  500-seed failover sweep and 300-seed partition sweep;
- the live 3-node smoke harness runs the packaged binaries through
  RAFT-011/012 and joins main's post-merge required set;
- no-double-lease (P-08) and no-lost-ack (P-09) histories check clean
  across the full corpus;
- FR-REPL-001..008 and NFR-AVAIL-001 resolve; evidence recorded.

### 19.9 R8 — Operable: metrics, tracing, admin surface, runbook

Entry: R7 closed. R8 passes when:

- ADMN-006, OPSX-001..004, OPSX-006, OPSX-008, and SOAK-001..004 are
  green, including a 24 h soak with kill-churn (SOAK-003) on the dedicated
  runner;
- the metrics inventory golden exists and the cardinality budget is
  enforced at runtime (OPSX-002);
- the diagnose bundle passes the full canary scan (OPSX-004);
- the operator runbook (install, upgrade, backup, incident basics) is
  published in docs and referenced by relayd error output;
- FR-ADMIN-006..008, FR-OPS-003..006, FR-OPS-010 resolve; evidence
  recorded.

### 19.10 R9 — Published benchmarks and evidence-bound claims

Entry: R8 closed. R9 passes when:

- BENCH-001..006 are green on the dedicated reference runner with the
  statistical validity checks (BENCH-001) passing first;
- every published number carries hardware, workload, and statistical
  treatment (NFR-PERF-005) and lands in BENCHMARK_PLAN.md result records;
- MKT-001 and MKT-002 audits pass over all public copy;
- the failure-injection report (crash/partition evidence summary drawn
  from CRSH-/SIM-/RAFT- artifacts) is published;
- NFR-PERF-001..005, NFR-AVAIL-002, FR-OPS-011, FR-MKT-001..003 resolve;
  evidence recorded.

### 19.11 R10 — 1.0: packaging, upgrade, rollback, backup/restore

Entry: R9 closed. R10 passes when:

- MIGR-001..005, RAFT-013, OPSX-009, and MKT-003 are green;
- reproducible-build verification (Section 13.4) matches bit-for-bit and
  SBOM + provenance attach to the candidate;
- the DR drill (Section 15.3) runs green with RPO = 0 for acked messages
  and RTO ≤ 15 min recorded;
- install, first-run, 3-node bootstrap, rolling upgrade, verified
  rollback, uninstall, and purge procedures have each been executed
  end-to-end by the live harness or a recorded manual run;
- the threat model re-review (NFR-SEC-007) and dependency provenance
  (NFR-SEC-008) are signed off; the incident-response tabletop drill
  (FR-OPS-012) is recorded;
- NFR-MAINT-004 holds: the R10 pipeline replays the accepted evidence of
  every prior gate green from the release commit;
- every remaining Section 20 row resolves — at R10 the traceability table
  has no unresolved requirement;
- evidence manifest, claims audit, and sign-off recorded; only then may
  1.0 be announced.

## 20. Requirement-to-Evidence Traceability

### 20.1 Literal requirement map

The tables below are the auditable requirement registry for this plan. They
list all 103 requirement IDs from the register in
[PRODUCT_REQUIREMENTS.md](./PRODUCT_REQUIREMENTS.md), each exactly once;
no future registry is needed to discover whether an ID has a test and
evidence owner. "Terminal gate" is the gate at which the requirement's
evidence completes, exactly as the register assigns; earlier gates may
begin it as described in [BUILD_PLAN.md](./BUILD_PLAN.md) §16, which must
agree with these rows ID-for-ID and gate-for-gate. "Evidence families"
resolve to Section 10 rows and named procedures in this document. When
`evidence/requirements.json` is implemented it is a machine-readable mirror
of these rows, not a replacement: its validator must prove a bijection with
this literal set and reject duplicates and omissions.

#### Core queue semantics (FR-QUEUE)

| Requirement | Terminal gate | Evidence families |
| --- | --- | --- |
| FR-QUEUE-001 | R1 | CORE-001; MODL-001 |
| FR-QUEUE-002 | R2 | STOR-009; CRSH-001, CRSH-008, CRSH-010 |
| FR-QUEUE-003 | R1 | CORE-003 |
| FR-QUEUE-004 | R1 | CORE-004; MODL-006 |
| FR-QUEUE-005 | R1 | CORE-005; MODL-006; SIM-010 |
| FR-QUEUE-006 | R1 | CORE-006; MODL-002 |
| FR-QUEUE-007 | R1 | CORE-007; WIRE-011 |
| FR-QUEUE-008 | R1 | CORE-008 |
| FR-QUEUE-009 | R6 | WIRE-008; SIM-013 |
| FR-QUEUE-010 | R4 | CORE-009 |
| FR-QUEUE-011 | R4 | CORE-009 |
| FR-QUEUE-012 | R1 | CORE-010 |
| FR-QUEUE-013 | R1 | CORE-002 |
| FR-QUEUE-014 | R4 | CORE-013; SOAK-004 |
| FR-QUEUE-015 | R1 | CORE-011 |
| FR-QUEUE-016 | R1 | CORE-012; FIFO-010 |
| FR-QUEUE-017 | R4 | CORE-014; SIM-010 |
| FR-QUEUE-018 | R4 | CORE-014 |
| FR-QUEUE-019 | R4 | CORE-015 |

#### FIFO queues (FR-FIFO)

| Requirement | Terminal gate | Evidence families |
| --- | --- | --- |
| FR-FIFO-001 | R4 | FIFO-001 |
| FR-FIFO-002 | R4 | FIFO-002; MODL-007 |
| FR-FIFO-003 | R4 | FIFO-003 |
| FR-FIFO-004 | R4 | FIFO-004, FIFO-011 |
| FR-FIFO-005 | R4 | FIFO-005 |
| FR-FIFO-006 | R4 | FIFO-006 |
| FR-FIFO-007 | R4 | FIFO-007, FIFO-008 |
| FR-FIFO-008 | R4 | FIFO-009; MODL-007 |

#### Topics and fanout (FR-TOPIC)

| Requirement | Terminal gate | Evidence families |
| --- | --- | --- |
| FR-TOPIC-001 | R5 | TOPC-001 |
| FR-TOPIC-002 | R5 | TOPC-005 |
| FR-TOPIC-003 | R5 | TOPC-002, TOPC-007 |
| FR-TOPIC-004 | R5 | TOPC-003 |
| FR-TOPIC-005 | R5 | TOPC-004; FUZZ-004 |
| FR-TOPIC-006 | R5 | TOPC-006 |
| FR-TOPIC-007 | R5 | TOPC-001 |
| FR-TOPIC-008 | R5 | TOPC-008 |

#### Wire API (FR-API)

| Requirement | Terminal gate | Evidence families |
| --- | --- | --- |
| FR-API-001 | R6 | WIRE-001, WIRE-002, WIRE-003 |
| FR-API-002 | R6 | WIRE-001; FUZZ-001, FUZZ-002, FUZZ-005 |
| FR-API-003 | R6 | WIRE-005 |
| FR-API-004 | R6 | WIRE-006 |
| FR-API-005 | R6 | WIRE-007 |
| FR-API-006 | R6 | WIRE-012 |
| FR-API-007 | R6 | WIRE-008; SIM-013 |
| FR-API-008 | R6 | WIRE-009 |
| FR-API-009 | R6 | WIRE-004 |
| FR-API-010 | R6 | WIRE-001, WIRE-010 |

#### Replication (FR-REPL)

| Requirement | Terminal gate | Evidence families |
| --- | --- | --- |
| FR-REPL-001 | R7 | RAFT-001, RAFT-002 |
| FR-REPL-002 | R7 | RAFT-003 |
| FR-REPL-003 | R7 | RAFT-004; MODL-009 |
| FR-REPL-004 | R7 | RAFT-005; SIM-007 |
| FR-REPL-005 | R7 | RAFT-007, RAFT-008 |
| FR-REPL-006 | R7 | RAFT-009 |
| FR-REPL-007 | R7 | RAFT-006 |
| FR-REPL-008 | R7 | RAFT-010 |
| FR-REPL-009 | R10 | RAFT-013; MIGR-005 |

#### Administration (FR-ADMIN)

| Requirement | Terminal gate | Evidence families |
| --- | --- | --- |
| FR-ADMIN-001 | R6 | ADMN-001 |
| FR-ADMIN-002 | R6 | ADMN-002 |
| FR-ADMIN-003 | R6 | ADMN-003 |
| FR-ADMIN-004 | R6 | ADMN-004 |
| FR-ADMIN-005 | R6 | ADMN-005 |
| FR-ADMIN-006 | R8 | ADMN-006; Section 22 surface audit |
| FR-ADMIN-007 | R8 | ADMN-006; OPSX-001 |
| FR-ADMIN-008 | R8 | OPSX-003 |

#### Operations (FR-OPS)

| Requirement | Terminal gate | Evidence families |
| --- | --- | --- |
| FR-OPS-001 | R10 | Section 13.1 embedded-provenance checks; package-smoke; Section 13.4 reproducibility |
| FR-OPS-002 | R6 | OPSX-007 |
| FR-OPS-003 | R8 | OPSX-001 |
| FR-OPS-004 | R8 | OPSX-002 |
| FR-OPS-005 | R8 | OPSX-008 |
| FR-OPS-006 | R8 | OPSX-006 |
| FR-OPS-007 | R10 | Section 15.3 DR drill; STOR-012 |
| FR-OPS-008 | R10 | MIGR-005; RAFT-013; Section 14.4/14.5 live-harness runs |
| FR-OPS-009 | R10 | OPSX-009 |
| FR-OPS-010 | R8 | OPSX-004 |
| FR-OPS-011 | R9 | BENCH-002..005; capacity model in BENCHMARK_PLAN.md |
| FR-OPS-012 | R10 | Section 18.3 tabletop drill record in the R10 manifest |

#### Marketing claims (FR-MKT)

| Requirement | Terminal gate | Evidence families |
| --- | --- | --- |
| FR-MKT-001 | R9 | MKT-001 |
| FR-MKT-002 | R9 | MKT-001; BENCH-001..006 |
| FR-MKT-003 | R9 | MKT-002 |
| FR-MKT-004 | R10 | MKT-003 |
| FR-MKT-005 | R10 | MKT-003 |

#### Durability (NFR-DUR)

| Requirement | Terminal gate | Evidence families |
| --- | --- | --- |
| NFR-DUR-001 | R2 | CRSH-001, CRSH-010, CRSH-011; STOR-009 |
| NFR-DUR-002 | R2 | CRSH-008, CRSH-009 |
| NFR-DUR-003 | R2 | STOR-006, STOR-007; CRSH-006 |
| NFR-DUR-004 | R2 | STOR-011 |
| NFR-DUR-005 | R2 | CRSH-007 |
| NFR-DUR-006 | R2 | STOR-010; CRSH-004 |
| NFR-DUR-007 | R10 | MIGR-001, MIGR-002, MIGR-003 |

#### Performance (NFR-PERF)

| Requirement | Terminal gate | Evidence families |
| --- | --- | --- |
| NFR-PERF-001 | R9 | BENCH-002 |
| NFR-PERF-002 | R9 | BENCH-003 |
| NFR-PERF-003 | R9 | BENCH-004 |
| NFR-PERF-004 | R9 | BENCH-005 |
| NFR-PERF-005 | R9 | BENCH-001; MKT-001 |

#### Availability (NFR-AVAIL)

| Requirement | Terminal gate | Evidence families |
| --- | --- | --- |
| NFR-AVAIL-001 | R7 | RAFT-011 |
| NFR-AVAIL-002 | R9 | RAFT-012 (simulated); BENCH-006 (measured) |
| NFR-AVAIL-003 | R6 | WIRE-007, WIRE-010; CORE-012 |
| NFR-AVAIL-004 | R6 | OPSX-005 |

#### Security (NFR-SEC)

| Requirement | Terminal gate | Evidence families |
| --- | --- | --- |
| NFR-SEC-001 | R6 | WIRE-011; CORE-007 |
| NFR-SEC-002 | R6 | FUZZ-001, FUZZ-002, FUZZ-005 |
| NFR-SEC-003 | R6 | OPSX-004 canary machinery (wire/log scope at R6, bundle scope closes with R8 regression retained); Section 16.4 |
| NFR-SEC-004 | R6 | WIRE-005 |
| NFR-SEC-005 | R2 | STOR-013 |
| NFR-SEC-006 | R6 | WIRE-010 |
| NFR-SEC-007 | R10 | Per-gate threat-model review items in Section 19; R10 sign-off |
| NFR-SEC-008 | R10 | Section 5 policy + lint-deny job; Section 13.3 SBOM and provenance |

#### Maintainability (NFR-MAINT)

| Requirement | Terminal gate | Evidence families |
| --- | --- | --- |
| NFR-MAINT-001 | R10 | Section 3.3 PR evidence rule; Section 22 per-gate audit; MUT-001 |
| NFR-MAINT-002 | R3 | SIM-001, SIM-011 |
| NFR-MAINT-003 | R4 | MUT-001 |
| NFR-MAINT-004 | R10 | Section 11.4 required-check replay at R10 (Section 19.11) |
| NFR-MAINT-005 | R10 | docs-policy CI job; Section 19 status discipline; MKT-001 |

### 20.2 Consistency obligations

`just evidence-check` validates, on every PR that touches this file or
BUILD_PLAN.md: every register ID appears exactly once above; every terminal
gate matches the register; every evidence family resolves to an existing
Section 10 row or named section; and BUILD_PLAN.md §16 agrees row-for-row.
A drift in any direction fails CI.

## 21. Release-Candidate Readiness

The release-candidate checklist, executed for every tagged candidate from
R6 onward and in full at R10. Every item must be checked from the candidate
commit:

- [ ] All required checks green (Section 11.4) and no quarantined live-test
      past its 7-day SLA.
- [ ] Nightly deep suites (sim sweep, fuzz deep, model deep, mutants,
      soak, bench) green within the last 72 hours on the candidate lineage.
- [ ] `just evidence-check` passes; the gate manifest for every closed gate
      resolves from this commit (NFR-MAINT-004).
- [ ] Packaging: reproducible build verified, SBOM and provenance
      attached, binary-size budgets met, container manifest exact.
- [ ] Install/upgrade/rollback/uninstall/purge procedures executed against
      the candidate artifacts.
- [ ] DR drill green with recorded RPO/RTO (R10 scope).
- [ ] Redaction canary scan green over bundle, logs, and CI artifacts.
- [ ] Claims audit (MKT-001/002, plus MKT-003 at R10) green over README,
      docs, and site copy.
- [ ] CHANGELOG and docs distinguish `accepted` from `planned` for every
      user-visible statement (NFR-MAINT-005).
- [ ] No open SEV-1/SEV-2 against the shipped scope.

Consistency rule: this checklist and [BUILD_PLAN.md](./BUILD_PLAN.md) §17
are the same list maintained in two places by deliberate redundancy; a PR
changing either must change both identically, enforced by a docs-policy
diff check. Where they could ever disagree, BUILD_PLAN.md §17 controls
implementation order and this section controls release mechanics, per the
precedence order in [README.md](./README.md).

## 22. Feature-Exhaustiveness Audit Rule

Matrices rot when the product surface grows past them. At every gate
closure, `just audit-surface` regenerates the product-surface inventory and
diffs it against this document:

1. **Enumerate the surface** from code and specs: every `Command` variant
   in `relay-core`, every RWP opcode in `relay-wire`, every relayctl
   subcommand, every configuration key in Section 7.2, every stable error
   code, every lifecycle state and transition
   (`Delayed → Available → InFlight → Deleted` and the lease lifecycle),
   every metric in the inventory, and every documented procedure in
   Sections 14–15.
2. **Map each surface element** to at least one Section 10 matrix row, one
   Section 20 requirement row, or an explicit deferral recorded in
   [OPEN_QUESTIONS.md](./OPEN_QUESTIONS.md) with a fail-closed default and
   a reopen trigger.
3. **Fail the gate** on any unmapped element: an opcode without a WIRE-
   row, a relayctl subcommand outside ADMN-006's golden set, a config key
   without OPSX-007 coverage, or a lifecycle transition no CORE- row
   exercises blocks closure until a row is added or a deferral recorded.
4. **Audit the audit**: the gate reviewer samples three requirement rows
   from Section 20 and walks each to its green run and artifact hash in
   the evidence manifest; a dangling reference reopens the gate.
5. The audit report (`evidence/R<n>/surface-audit.json`) lists the full
   inventory, the mapping, and the deferral set, and is a mandatory
   manifest entry from R1 onward.

This rule is what keeps the centerpiece matrices in Section 10 exhaustive
by construction rather than by good intentions: nothing ships a surface
the matrices cannot name, and nothing in the matrices survives as prose
that no longer corresponds to the product.

