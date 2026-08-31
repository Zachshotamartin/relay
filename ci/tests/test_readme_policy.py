from __future__ import annotations

import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
README = ROOT / "README.md"
CONTRIBUTING = ROOT / "CONTRIBUTING.md"
HONEST_CLAIM = (
    "Relay is a documented design for a verification-first message queue. "
    "No binary exists, no test exists, no benchmark exists, and no delivery "
    "guarantee has been demonstrated."
)


def normalized(path: Path) -> str:
    lines = (
        line.removeprefix("> ")
        for line in path.read_text(encoding="utf-8").splitlines()
    )
    return " ".join("\n".join(lines).split())


class ReadmePolicyTests(unittest.TestCase):
    def test_r0_readme_01_uses_the_exact_honest_claim_without_unearned_headline(self) -> None:
        readme = normalized(README)

        self.assertEqual(1, readme.count(HONEST_CLAIM))
        self.assertNotIn(
            "Relay delivers at-least-once, and that guarantee is machine-checked.",
            readme,
        )
        self.assertIn("| R0 | in progress |", readme)
        for gate in range(1, 11):
            self.assertIn(f"| R{gate} | planned |", readme)

    def test_r0_readme_02_links_binding_docs_and_has_contributor_only_setup(self) -> None:
        readme = README.read_text(encoding="utf-8")

        for target in (
            "docs/README.md",
            "docs/PRODUCT_REQUIREMENTS.md",
            "docs/BUILD_PLAN.md",
            "docs/ARCHITECTURE.md",
            "docs/CORRECTNESS.md",
            "docs/OPERATIONS_TEST_PLAN.md",
            "CONTRIBUTING.md",
        ):
            self.assertIn(f"]({target}", readme)
        self.assertIn(
            "rustup toolchain install 1.85.0 --profile minimal --component clippy --component rustfmt",
            readme,
        )
        self.assertIn("cargo build --workspace --locked", readme)
        self.assertIn("cargo test --workspace --locked", readme)
        self.assertNotIn("## Installation", readme)
        self.assertNotIn("cargo install relay", readme)

    def test_r0_readme_03_contributing_records_merge_order_and_zero_flake_rule(self) -> None:
        contributing = normalized(CONTRIBUTING).lower()

        expected_order = (
            "failing deterministic test",
            "typed boundary",
            "smallest implementation",
            "property-based coverage",
            "adversarial and interruption",
            "replay every accepted earlier gate",
            "update statuses",
        )
        cursor = 0
        for phrase in expected_order:
            position = contributing.find(phrase, cursor)
            self.assertGreaterEqual(position, 0, phrase)
            cursor = position + len(phrase)
        self.assertIn("a flake is a bug", contributing)
        self.assertIn("fixtures/seeds/", contributing)
        self.assertIn("lowercase evidence-family prefix", contributing)
        self.assertIn("core_", contributing)
        self.assertIn("mkt_", contributing)
        self.assertIn("operations_test_plan.md#10-detailed-verification-matrices", contributing)


if __name__ == "__main__":
    unittest.main()
