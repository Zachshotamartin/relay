from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from ci import check_test_names


class TestNameConventionTests(unittest.TestCase):
    def test_r0_name_01_accepts_spine_family_prefixes(self) -> None:
        source = """
#[test]
fn core_receive_visibility() {}

#[tokio::test]
async fn wire_partial_frame() {}
"""
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "tests" / "named.rs"
            path.parent.mkdir()
            path.write_text(source, encoding="utf-8")

            self.assertEqual([], check_test_names.find_violations(Path(directory)))

    def test_r0_name_02_rejects_unprefixed_test_in_tests_tree(self) -> None:
        source = """
#[test]
fn receipt_is_rejected() {}
"""
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "crates" / "relay-core" / "tests" / "receipt.rs"
            path.parent.mkdir(parents=True)
            path.write_text(source, encoding="utf-8")

            violations = check_test_names.find_violations(Path(directory))

        self.assertEqual(1, len(violations))
        self.assertEqual("receipt_is_rejected", violations[0].name)
        self.assertEqual(3, violations[0].line)

    def test_r0_name_03_ignores_test_attribute_inside_comment(self) -> None:
        source = """
/*
#[test]
fn not_a_real_test() {}
*/
"""
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "tests" / "comment.rs"
            path.parent.mkdir()
            path.write_text(source, encoding="utf-8")

            self.assertEqual([], check_test_names.find_violations(Path(directory)))


if __name__ == "__main__":
    unittest.main()
