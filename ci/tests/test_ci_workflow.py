from __future__ import annotations

import re
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
WORKFLOW = ROOT / ".github" / "workflows" / "ci.yml"
JUSTFILE = ROOT / "justfile"
JOBS = ("fmt", "lint", "msrv", "deny", "arch", "test", "gates")


def source(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def mapping_block(document: str, key: str, *, indent: int) -> str:
    marker = f"{' ' * indent}{key}:"
    lines = document.splitlines()
    for index, line in enumerate(lines):
        if line != marker:
            continue
        end = index + 1
        while end < len(lines):
            candidate = lines[end]
            if candidate and not candidate.startswith(" " * (indent + 1)):
                break
            end += 1
        return "\n".join(lines[index:end]) + "\n"
    raise AssertionError(f"missing mapping {key!r} at indentation {indent}")


def direct_mapping_keys(block: str, *, indent: int) -> list[str]:
    pattern = re.compile(rf"^ {{{indent}}}([A-Za-z][A-Za-z0-9_-]*):(?:\s|$)")
    return [match.group(1) for line in block.splitlines() if (match := pattern.match(line))]


class WorkflowGraphTests(unittest.TestCase):
    def test_r0_ci_01_has_exact_ordered_seven_job_graph(self) -> None:
        workflow = source(WORKFLOW)
        jobs = mapping_block(workflow, "jobs", indent=0)

        self.assertEqual(list(JOBS), direct_mapping_keys(jobs, indent=2))
        for index, job in enumerate(JOBS):
            block = mapping_block(jobs, job, indent=2)
            expected_need = JOBS[index - 1] if index else None
            needs = re.findall(r"^    needs:\s*([^\s#]+)", block, flags=re.MULTILINE)
            self.assertEqual([] if expected_need is None else [expected_need], needs)
            self.assertRegex(block, rf"(?m)^      - run: just {job}$")

    def test_r0_ci_02_uses_only_full_sha_pinned_external_actions(self) -> None:
        workflow = source(WORKFLOW)
        action_refs = re.findall(r"(?m)^\s*- uses:\s*([^\s#]+)", workflow)

        self.assertGreater(len(action_refs), 0)
        for action_ref in action_refs:
            if action_ref.startswith("./"):
                continue
            self.assertRegex(action_ref, r"^[^@\s]+@[0-9a-f]{40}$")
        self.assertNotIn("${{ secrets.", workflow)
        self.assertNotRegex(workflow, r"(?i)(?:^|/)cache(?:@|/)")

    def test_r0_ci_03_has_required_linux_and_advisory_macos_matrix(self) -> None:
        workflow = source(WORKFLOW)
        test_job = mapping_block(mapping_block(workflow, "jobs", indent=0), "test", indent=2)

        self.assertIn("runner: ubuntu-24.04", test_job)
        self.assertIn("architecture: x86_64", test_job)
        self.assertIn("runner: ubuntu-24.04-arm", test_job)
        self.assertIn("architecture: aarch64", test_job)
        self.assertIn("runner: macos-15", test_job)
        self.assertIn("tier: advisory", test_job)
        self.assertIn("continue-on-error: ${{ matrix.advisory }}", test_job)

    def test_r0_ci_04_has_safe_triggers_permissions_and_uncached_gate_replay(self) -> None:
        workflow = source(WORKFLOW)
        gates = mapping_block(mapping_block(workflow, "jobs", indent=0), "gates", indent=2)

        self.assertRegex(workflow, r"(?m)^  pull_request:$")
        self.assertRegex(workflow, r"(?m)^  push:$")
        self.assertRegex(workflow, r"(?m)^      - main$")
        self.assertRegex(workflow, r"(?m)^permissions:\n  contents: read$")
        self.assertNotIn("pull_request_target", workflow)
        self.assertNotRegex(gates, r"(?i)cache")
        self.assertRegex(gates, r"(?m)^      - run: just gates$")

    def test_r0_ci_05_just_recipes_preserve_locked_gate_commands(self) -> None:
        justfile = source(JUSTFILE)
        required = {
            "fmt": "cargo fmt --all -- --check",
            "lint": "cargo clippy --workspace --all-targets --locked -- -D warnings",
            "msrv": "cargo check --workspace --locked",
            "deny": "cargo deny check",
            "arch": "cargo run -p arch-check --locked",
            "test": "cargo test --workspace --locked",
            "gates": "python3 ci/run_gates.py",
        }
        for recipe, command in required.items():
            self.assertRegex(justfile, rf"(?m)^{recipe}:\n    {re.escape(command)}$")
        self.assertRegex(
            justfile,
            r"(?m)^ci-local: fmt lint msrv deny arch test gates$",
        )


if __name__ == "__main__":
    unittest.main()
