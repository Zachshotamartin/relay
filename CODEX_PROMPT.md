# One-shot implementation prompt for Relay

Copy everything below the line into Codex Ultra, run from the `relay/` repository root.

---

You are implementing **Relay**, a self-hosted message queue and pub/sub service whose
delivery guarantees are machine-checked, not asserted. The complete normative
specification already exists in this repository and is binding — you write code to
satisfy it, you do not redesign it.

## Ground rules (non-negotiable)

1. Read first, in this order, before writing any code: `docs/README.md`,
   `docs/PRODUCT_REQUIREMENTS.md`, `docs/BUILD_PLAN.md`, `docs/ARCHITECTURE.md`,
   `docs/CORRECTNESS.md`, `docs/OPERATIONS_TEST_PLAN.md`, and every ADR in
   `docs/decisions/`. When documents disagree, the precedence order in
   `docs/README.md` §Conflict and Status Rules controls.
2. Execute `docs/BUILD_PLAN.md` gates **strictly in order: R0 → R1 → R2 → …**
   Inside each gate, follow its `X.5` ticket sequence. Never start gate N+1 while
   gate N's `X.9` acceptance evidence is not green. A finished, green earlier gate
   is worth more than a sprawling broken later one.
3. **Test-first.** For every parser, reducer, state transition, and error category,
   write the failing deterministic test named in the gate's `X.6` evidence matrix
   (and in OPERATIONS_TEST_PLAN §10) before the implementation. Do not delete or
   weaken a specified test to make it pass; fix the implementation.
4. **Determinism discipline** (ADR-0005): `relay-core` is a pure state machine —
   no `std::time`, no `rand`, no IO, no threads, no tokio anywhere inside it. All
   time enters as `AdvanceTime` log entries; all nondeterminism enters through the
   injected `Clock`/`Rng`/`Net`/`Disk` traits defined in `docs/ARCHITECTURE.md` §4.
   Add the CI lint that enforces this and keep it green.
5. **Formats are frozen.** WAL record/segment layout, RSNAP1 snapshot, RWP/1 frame,
   receipt-handle construction, ULID IDs, and every numeric limit (256 KiB body,
   300 s dedup window, 0–20 s long poll, visibility 0 s–12 h default 30 s, batch 10,
   maxReceiveCount 1–1,000, in-flight caps 120,000/20,000) are specified byte- and
   field-level in `docs/ARCHITECTURE.md` §6/§8/§10 and the ADRs. Implement them
   exactly; if you believe one is wrong, record a new ADR — never silently deviate.
6. **No unearned claims.** In-memory results never justify a durability claim;
   single-node results never justify a replication claim; a simulated fault is
   never production hardening. Statuses are exactly `accepted | in progress |
   planned | deferred`; a stub or happy-path test is never completion.
7. Toolchain per ADR-0001: Rust stable, edition 2024, MSRV 1.85 pinned,
   `cargo clippy -D warnings`, `cargo-deny` with exact-pinned dependencies only.
   Workspace crates exactly as `docs/ARCHITECTURE.md` §2: relay-core, relay-wal,
   relay-raft, relay-sim, relay-model, relay-wire, relay-server, relay-client,
   relay-cli, relay-bench.
8. The verification apparatus is product, not test scaffolding: `relay-sim`
   (deterministic simulation — same seed ⇒ byte-identical trace, divergence is a
   bug) and `relay-model` (JSONL histories + Wing–Gong linearizability oracle per
   `docs/CORRECTNESS.md` §5–§6) are built at their own gates (R3, R1) with their
   own acceptance evidence. Failing seeds go into `sim-corpus/` and replay in CI.
9. Immutability style: state transitions return new state (`apply(&CoreState,
   &LogEntry) -> Applied`); no in-place mutation of domain state. Handle every
   error explicitly; never swallow one. Validate every input at every boundary.
10. Do not touch `docs/` except to flip statuses you have genuinely earned
    (with the named green gate as evidence) and to add new ADRs for any reversal.

## Definition of one-shot success

Work gate by gate as far as quality allows within your run. Hard floor for this
run: **R0 through R2 fully accepted** (repo + CI green; core semantics green under
the model checker; WAL survives crash/torn-write/disk-full injection), with R3
attempted. Every completed gate must have: all its `X.5` tickets done, all `X.6`
matrix rows implemented as named passing tests, its `X.9` checklist satisfiable by
running commands you provide, and its `X.10` deferrals still honestly deferred.

## Required final report (print at the end, nothing else after it)

1. Per-gate status table (gate → accepted/in progress/planned) with the exact
   commands that prove each accepted gate (they must pass from a clean checkout).
2. Test inventory: count of tests per family (CORE-, STOR-, CRSH-, SIM-, MODL-, …)
   implemented vs specified in OPERATIONS_TEST_PLAN §10.
3. Requirement IDs (FR-*/NFR-*) whose terminal gate you completed, per
   BUILD_PLAN §16 — claim only terminal completions.
4. Every deviation from the docs, however small, with its justification or new ADR.
5. What you deliberately did not build and which gate owns it.

Do not summarize the docs back to me, do not ask questions, do not stop at a plan —
read, then build, then report.
