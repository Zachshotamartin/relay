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
