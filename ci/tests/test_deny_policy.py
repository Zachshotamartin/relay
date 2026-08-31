from __future__ import annotations

import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]


class DenyPolicyTests(unittest.TestCase):
    def test_r0_deny_01_requires_workspace_unmaintained_advisory_policy(self) -> None:
        document = (ROOT / "deny.toml").read_text(encoding="utf-8")
        advisories = document.split("[advisories]\n", 1)[1].split("\n[", 1)[0]

        self.assertIn('unmaintained = "workspace"', advisories.splitlines())


if __name__ == "__main__":
    unittest.main()
