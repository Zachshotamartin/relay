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
    python3 -m unittest discover -s ci/tests -p 'test_*.py'
    python3 ci/check_pr_policy.py

test:
    cargo build --workspace --locked
    python3 ci/check_test_names.py .
    python3 ci/check_canaries.py --capture -- cargo test --workspace --locked
    python3 ci/check_canaries.py --capture -- python3 -m unittest discover -s ci/tests -p 'test_*.py'

pr-policy:
    python3 ci/check_pr_policy.py

gates:
    python3 ci/run_gates.py

ci-local: fmt lint msrv deny arch test gates
