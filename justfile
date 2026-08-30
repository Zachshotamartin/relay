set shell := ["bash", "-euo", "pipefail", "-c"]

fmt:
    cargo fmt --all -- --check

lint:
    cargo clippy --workspace --all-targets --locked -- -D warnings

msrv:
    cargo check --workspace --locked

deny:
    cargo deny check

arch:
    cargo run -p arch-check --locked
    python3 -m unittest ci.tests.test_run_gates ci.tests.test_ci_workflow

test:
    cargo test --workspace --locked

gates:
    python3 ci/run_gates.py

ci-local: fmt lint msrv deny arch test gates
