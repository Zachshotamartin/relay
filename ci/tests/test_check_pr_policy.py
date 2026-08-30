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

        self.assertNotEqual([], check_pr_policy.validate_title("build: unsupported type"))
        self.assertNotEqual([], check_pr_policy.validate_title("ci: first line\nsecond line"))

    def test_r0_pr_03_accepts_complete_template(self) -> None:
        self.assertEqual([], check_pr_policy.validate_template(VALID_TEMPLATE))

    def test_r0_pr_04_reports_each_missing_template_field(self) -> None:
        errors = check_pr_policy.validate_template("## Requirements and evidence\n")

        self.assertIn("Failing-test commit:", "\n".join(errors))
        self.assertIn("MIGR fixture:", "\n".join(errors))

    def test_r0_pr_05_requires_completed_evidence_fields(self) -> None:
        empty_errors = check_pr_policy.validate_pr_body(VALID_TEMPLATE)

        self.assertIn("Requirement IDs:", "\n".join(empty_errors))
        self.assertIn("Failing-test commit:", "\n".join(empty_errors))

        completed = VALID_TEMPLATE.replace("Requirement IDs:", "Requirement IDs: None — R0")
        completed = completed.replace("Matrix row IDs:", "Matrix row IDs: R0 repository policy")
        completed = completed.replace("Failing-test commit:", "Failing-test commit: deadbeef")
        completed = completed.replace(
            "Golden files / corpus entries / fixtures changed:",
            "Golden files / corpus entries / fixtures changed: None",
        )
        completed = completed.replace("Semantic reason:", "Semantic reason: No artifacts changed")
        completed = completed.replace(
            "Dependency review checklist:",
            "Dependency review checklist: No dependency changes",
        )
        completed = completed.replace("Version bump:", "Version bump: None")
        completed = completed.replace("MIGR fixture:", "MIGR fixture: None")

        self.assertEqual([], check_pr_policy.validate_pr_body(completed))


if __name__ == "__main__":
    unittest.main()
