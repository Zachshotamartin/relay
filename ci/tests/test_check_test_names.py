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

    def test_r0_name_04_ignores_attributes_inside_literals_and_nested_comments(self) -> None:
        source = r'''
const NORMAL: &str = "#[test] fn not_a_string_test() {}";
const RAW: &str = r##"#[tokio::test] async fn not_a_raw_test() {}"##;
const BYTE: &[u8] = br#"#[test] fn not_a_byte_string_test() {}"#;
/* outer
   /* #[test] fn not_a_nested_comment_test() {} */
*/
// #[test] fn not_a_line_comment_test() {}
'''
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "tests" / "literals.rs"
            path.parent.mkdir()
            path.write_text(source, encoding="utf-8")

            self.assertEqual([], check_test_names.find_violations(Path(directory)))

    def test_r0_name_05_recognizes_multiline_qualified_test_attribute(self) -> None:
        source = """#[
    tokio :: test
]
#[ignore]
pub async fn missing_family_prefix() {}
"""
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "tests" / "qualified.rs"
            path.parent.mkdir()
            path.write_text(source, encoding="utf-8")

            violations = check_test_names.find_violations(Path(directory))

        self.assertEqual(1, len(violations))
        self.assertEqual("missing_family_prefix", violations[0].name)
        self.assertEqual(5, violations[0].line)

    def test_r0_name_06_uses_exact_spine_family_prefixes(self) -> None:
        families = (
            "core",
            "stor",
            "crsh",
            "sim",
            "modl",
            "fifo",
            "topc",
            "wire",
            "fuzz",
            "raft",
            "admn",
            "opsx",
            "migr",
            "soak",
            "bench",
            "mut",
            "mkt",
        )
        accepted = "\n".join(
            f"#[test]\nfn {family}_named_evidence() {{}}" for family in families
        )
        source = f"{accepted}\n#[test]\nfn corex_not_the_core_family() {{}}\n"
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "tests" / "families.rs"
            path.parent.mkdir()
            path.write_text(source, encoding="utf-8")

            violations = check_test_names.find_violations(Path(directory))

        self.assertEqual(["corex_not_the_core_family"], [item.name for item in violations])

    def test_r0_name_07_reports_in_deterministic_path_order(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            later = root / "z" / "tests" / "z.rs"
            earlier = root / "a" / "tests" / "a.rs"
            later.parent.mkdir(parents=True)
            earlier.parent.mkdir(parents=True)
            later.write_text("#[test]\nfn later() {}\n", encoding="utf-8")
            earlier.write_text("#[test]\nfn earlier() {}\n", encoding="utf-8")

            violations = check_test_names.find_violations(root)

        self.assertEqual(["earlier", "later"], [item.name for item in violations])

    def test_r0_name_08_fails_closed_on_non_utf8_rust_source(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "tests" / "invalid.rs"
            path.parent.mkdir()
            path.write_bytes(b"#[test]\nfn hidden() {}\n\xff")

            with self.assertRaisesRegex(check_test_names.ScanError, "UTF-8"):
                check_test_names.find_violations(Path(directory))

    def test_r0_name_09_ignores_rust_files_outside_tests_directories(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            path = root / "src" / "tests_support.rs"
            path.parent.mkdir()
            path.write_text("#[test]\nfn internal_unit_test() {}\n", encoding="utf-8")

            self.assertEqual([], check_test_names.find_violations(root))

    def test_r0_name_10_detects_cfg_attr_tests_and_dynamic_macro_names(self) -> None:
        source = """#[cfg_attr(all(), test)]
fn conditional_missing_family() {}

#[cfg_attr(test, ignore)]
fn helper_is_not_a_test() {}

macro_rules! make_test {
    ($name:ident) => {
        #[test]
        fn $name() {}
    };
}
"""
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "tests" / "generated.rs"
            path.parent.mkdir()
            path.write_text(source, encoding="utf-8")

            violations = check_test_names.find_violations(Path(directory))

        self.assertEqual(
            ["conditional_missing_family", "<dynamic-test-name>"],
            [violation.name for violation in violations],
        )
        self.assertEqual([2, 9], [violation.line for violation in violations])

    def test_r0_name_11_rejects_pasted_macro_test_names(self) -> None:
        source = """macro_rules! make_test {
    ($name:ident) => {
        #[test]
        fn [<$name _case>]() {}
    };
}
"""
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "tests" / "generated.rs"
            path.parent.mkdir()
            path.write_text(source, encoding="utf-8")

            violations = check_test_names.find_violations(Path(directory))

        self.assertEqual(["<dynamic-test-name>"], [item.name for item in violations])
        self.assertEqual([3], [item.line for item in violations])

    def test_r0_name_12_rejects_macro_indirected_test_attributes(self) -> None:
        source = """macro_rules! mark {
    ($attr:meta) => {
        #[$attr]
        fn bad_name() {}
    };
}
mark!(test);
"""
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "tests" / "generated_attribute.rs"
            path.parent.mkdir()
            path.write_text(source, encoding="utf-8")

            violations = check_test_names.find_violations(Path(directory))

        self.assertEqual(["bad_name"], [item.name for item in violations])
        self.assertEqual([4], [item.line for item in violations])


if __name__ == "__main__":
    unittest.main()
