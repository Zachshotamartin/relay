#![forbid(unsafe_code)]
//! Repository architecture policy checks.

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Violation {
    pub line: usize,
    pub message: String,
}

#[must_use]
pub fn check_exact_requirements(_manifest: &str) -> Vec<Violation> {
    Vec::new()
}

#[must_use]
pub fn scan_source(_source: &str, _tokens: &[String]) -> Vec<Violation> {
    Vec::new()
}

#[must_use]
pub fn validate_gates(_source: &str) -> Vec<Violation> {
    Vec::new()
}

#[must_use]
pub fn validate_relative_links(_source: &str, _known_paths: &[String]) -> Vec<Violation> {
    Vec::new()
}

#[must_use]
pub fn validate_test_names(_source: &str) -> Vec<Violation> {
    Vec::new()
}

#[must_use]
pub fn scan_canaries(_source: &str) -> Vec<Violation> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};
    use std::process::{Command, Output};

    fn fixture_path(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/r0_04")
            .join(name)
    }

    fn run_arch_fixture(metadata: &str, config: &Path) -> Output {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let workspace_root = manifest_dir
            .parent()
            .and_then(Path::parent)
            .expect("arch-check must live at tools/arch-check");
        Command::new(env!("CARGO"))
            .args([
                "run",
                "--quiet",
                "--locked",
                "-p",
                "arch-check",
                "--",
                "--metadata-fixture",
            ])
            .arg(fixture_path(metadata))
            .arg("--config")
            .arg(config)
            .current_dir(workspace_root)
            .output()
            .expect("fixture invocation must start")
    }

    #[test]
    fn arch_exact_pin_rejects_floating_requirement() {
        let violations = check_exact_requirements("[dependencies]\nbytes = \"1\"\n");
        assert_eq!(violations.len(), 1);
        assert!(violations[0].message.contains("bytes"));
    }

    #[test]
    fn arch_exact_pin_accepts_pinned_and_workspace_requirements() {
        let source = "[dependencies]\nbytes = \"=1.9.0\"\nrelay-core = { workspace = true }\n";
        assert!(check_exact_requirements(source).is_empty());
    }

    #[test]
    fn arch_dependency_graph_accepts_allowlisted_fixture() {
        let output = run_arch_fixture("metadata-allowed.json", &fixture_path("arch-valid.toml"));
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            String::from_utf8_lossy(&output.stdout).contains("architecture checks passed"),
            "fixture mode did not run the dependency check"
        );
    }

    #[test]
    fn arch_dependency_graph_rejects_forbidden_dependency() {
        let output = run_arch_fixture("metadata-forbidden.json", &fixture_path("arch-valid.toml"));
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(!output.status.success(), "forbidden graph was accepted");
        assert!(stderr.contains("relay-core"), "{stderr}");
        assert!(stderr.contains("rand"), "{stderr}");
    }

    #[test]
    fn arch_config_rejects_malformed_policy() {
        let output = run_arch_fixture("metadata-allowed.json", &fixture_path("arch-malformed.toml"));
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(!output.status.success(), "malformed arch.toml was accepted");
        assert!(stderr.contains("arch-malformed.toml"), "{stderr}");
        assert!(stderr.contains("line"), "{stderr}");
    }

    #[test]
    fn arch_config_rejects_empty_crate_list() {
        let output = run_arch_fixture("metadata-allowed.json", &fixture_path("arch-empty.toml"));
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(!output.status.success(), "empty crate policy was accepted");
        assert!(stderr.contains("no crate policies"), "{stderr}");
    }

    #[test]
    fn arch_config_rejects_unreadable_input() {
        let unreadable = fixture_path("unreadable-directory");
        let output = run_arch_fixture("metadata-allowed.json", &unreadable);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(!output.status.success(), "unreadable config input was skipped");
        assert!(stderr.contains("unreadable-directory"), "{stderr}");
        assert!(stderr.contains("cannot read"), "{stderr}");
    }

    #[test]
    fn arch_purity_reports_code_but_ignores_comments_and_cfg_test_module() {
        let source = "// SystemTime::now is forbidden\nfn bad() { SystemTime::now(); }\n#[cfg(test)]\nmod tests { fn allowed() { SystemTime::now(); } }\n";
        let violations = scan_source(source, &["SystemTime::now".to_owned()]);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].line, 2);
    }

    #[test]
    fn arch_gate_parser_requires_all_gates_and_accepted_commands() {
        let malformed = "schema = 1\n[gate.R0]\nstatus = \"accepted\"\ncommands = []\n";
        let violations = validate_gates(malformed);
        assert!(violations.iter().any(|item| item.message.contains("R0")));
        assert!(violations.iter().any(|item| item.message.contains("R10")));
    }

    #[test]
    fn arch_links_report_missing_relative_target() {
        let violations = validate_relative_links(
            "See [missing](./missing.md) and [ok](./ok.md).",
            &["ok.md".to_owned()],
        );
        assert_eq!(violations.len(), 1);
        assert!(violations[0].message.contains("missing.md"));
    }

    #[test]
    fn arch_test_name_enforces_evidence_family_prefix() {
        let violations = validate_test_names("#[test]\nfn unnamed_test() {}\n");
        assert_eq!(violations.len(), 1);
        assert!(validate_test_names("#[test]\nfn core_001_body_limit() {}\n").is_empty());
    }

    #[test]
    fn arch_canary_scan_rejects_captured_secret_marker() {
        let violations = scan_canaries("ordinary output\nRELAY_CANARY_secret\n");
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].line, 2);
    }
}
