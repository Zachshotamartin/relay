# ADR-0007: JSONL Operation Histories and an In-House Wing–Gong Linearizability Oracle

- Status: accepted
- Date: 2026-08-30
- Related findings or requirements: NFR-MAINT-002, P-02, P-04, P-06, P-09, P-10

## Context

"Machine-checked delivery guarantees" needs a concrete mechanism: something
that takes what Relay actually did and decides whether it was legal. Relay's
choice must run in CI on every pull request with zero flakes (a flake is a
bug), reproduce any failure from a seed, and check the real Rust
implementation — not a model of it. Gate R1 requires the checker before any
queue semantics count as correct, so the history format and checking algorithm
are fixed now. Linearizability checking is NP-complete in general; the
decision must therefore also fix the search strategy and its budget rules,
or the oracle becomes the flaky part of CI.

## Decision

`crates/relay-model` defines a JSONL operation-history format — one operation
per line, with `op`, `client`, `call`, `invoke_ns`, `return_ns`, `result`, and
`seed` fields exactly as fixed in the spine's history record — emitted by
every simulation and model-check run. An in-house Wing–Gong linearizability
checker decides each history against the reference model (a sequential queue
specification in the same crate), proving P-02, P-04, P-06, P-09, and P-10
over real execution traces.

Search strategy and budget rules (binding):

- **Per-queue partitioning**: operations on distinct queues commute in the
  reference model, so each queue's subhistory is checked independently,
  turning one large search into many small ones.
- **Memoized Wing–Gong**: the checker caches visited (linearized-set,
  model-state) pairs to prune re-exploration.
- **Wall-clock budget per history**: each history gets a fixed budget stated
  in OPERATIONS_TEST_PLAN. A history the checker cannot decide within budget
  is a test failure, never a skip — the harness shrinks the generating seed
  until the history is decidable or the bug is exposed. Fail closed.

## Alternatives Considered

- **TLA+ only**: rejected for the spec-to-code gap. A TLA+ spec verifies the
  spec; a Rust reducer that diverges from it passes every TLA+ run while
  shipping the bug. TLA+ may still be used as a design aid, but it is
  evidence for nothing in the traceability matrix.
- **Jepsen only**: rejected as non-deterministic and non-CI. Jepsen runs real
  clusters under real time: failures are minutes long, not seed-reproducible
  (violating NFR-MAINT-002), and cannot gate merges under a zero-flake
  policy. Its history model is the right idea — Relay keeps the idea and
  makes generation deterministic.
- **Off-the-shelf checkers (Knossos, Porcupine, Elle)**: rejected on
  boundary grounds — Clojure and Go implementations cannot link into the Rust
  test harness without a subprocess-and-serialize seam, and the checker must
  share the reference model's exact Rust types so that model drift between
  checker and simulation is impossible.
- **Bare Wing–Gong without partitioning or memoization**: rejected because
  worst-case exponential search over full multi-queue histories blows any CI
  budget; the partitioning exploits queue independence that the reference
  model guarantees.

## Consequences

- Easier: every MODL- and SIM- test emits the same JSONL artifact, so a CI
  failure ships its own evidence; P-10 (no invention) falls out of the same
  history check; histories are diffable, archivable fixtures for regression
  suites.
- Harder: the reference model becomes load-bearing — a bug in it silently
  weakens every proof, so the model itself needs its own CORE-level tests and
  mutation coverage (NFR-MAINT-003); checker performance is now Relay's
  problem forever.
- Revisit when: real histories from R7 routinely exhaust the budget even
  after shrinking — the escalation is a stronger search strategy (finer
  commutativity partitioning, just-in-time linearization), recorded as a
  superseding ADR, never a budget-raise-until-green. No OPEN_QUESTIONS entry
  reopens this decision.
