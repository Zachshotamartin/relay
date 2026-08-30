from __future__ import annotations

import unittest

from ci import check_pr_policy


VALID_TEMPLATE = """
## Requirements and evidence
Requirement IDs:
Matrix row IDs:

## Test-first record
Failing-test commit:

## Artifact changes
Golden files / corpus entries / fixtures changed:
Semantic reason:

## Dependency review
Dependency review checklist:

## Format changes
Version bump:
MIGR fixture:
"""


class PullRequestPolicyTests(unittest.TestCase):
    def test_r0_pr_01_accepts_conventional_title(self) -> None:
        self.assertEqual([], check_pr_policy.validate_title("ci: enforce the R0 gate graph"))

    def test_r0_pr_02_rejects_nonconventional_title(self) -> None:
        errors = check_pr_policy.validate_title("Add CI")

        self.assertEqual(1, len(errors))
        self.assertIn("conventional", errors[0])

    def test_r0_pr_03_accepts_complete_template(self) -> None:
        self.assertEqual([], check_pr_policy.validate_template(VALID_TEMPLATE))

    def test_r0_pr_04_reports_each_missing_template_field(self) -> None:
        errors = check_pr_policy.validate_template("## Requirements and evidence\n")

        self.assertIn("Failing-test commit:", "\n".join(errors))
        self.assertIn("MIGR fixture:", "\n".join(errors))


if __name__ == "__main__":
    unittest.main()
