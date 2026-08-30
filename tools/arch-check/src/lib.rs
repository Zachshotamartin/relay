#![forbid(unsafe_code)]
//! Repository architecture policy checks.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const PRODUCT_CRATES: [&str; 10] = [
    "relay-core",
    "relay-wal",
    "relay-raft",
    "relay-sim",
    "relay-model",
    "relay-wire",
    "relay-server",
    "relay-client",
    "relay-cli",
    "relay-bench",
];
const TOOL_CRATE: &str = "arch-check";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Violation {
    pub line: usize,
    pub message: String,
}

impl Violation {
    fn new(line: usize, message: impl Into<String>) -> Self {
        Self {
            line,
            message: message.into(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CratePolicy {
    pub allowed_deps: BTreeSet<String>,
    pub allowed_dev_deps: BTreeSet<String>,
    pub allowed_build_deps: BTreeSet<String>,
    pub forbidden_deps: BTreeSet<String>,
    pub forbidden_tokens: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArchConfig {
    pub crates: BTreeMap<String, CratePolicy>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MetadataPackage {
    pub name: String,
    pub manifest_path: PathBuf,
    pub dependencies: BTreeSet<MetadataDependency>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum DependencyKind {
    Normal,
    Development,
    Build,
}

impl DependencyKind {
    fn label(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Development => "dev",
            Self::Build => "build",
        }
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct MetadataDependency {
    pub name: String,
    pub kind: DependencyKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceMetadata {
    pub packages: BTreeMap<String, MetadataPackage>,
}

#[must_use]
pub fn check_exact_requirements(manifest: &str) -> Vec<Violation> {
    let mut violations = Vec::new();
    let mut section = String::new();
    let mut table_requirements: BTreeMap<String, TableRequirement> = BTreeMap::new();
    let lines: Vec<&str> = manifest.lines().collect();
    let mut index = 0;

    while index < lines.len() {
        let line_number = index + 1;
        let uncommented = strip_toml_comment(lines[index]);
        let line = uncommented.trim();
        if line.is_empty() {
            index += 1;
            continue;
        }
        if line.starts_with('[') {
            if let Some(name) = parse_table_header(line) {
                section = name;
                if let Some(dependency) = dependency_table_name(&section) {
                    table_requirements
                        .entry(dependency)
                        .or_insert_with(|| TableRequirement::new(line_number));
                }
            }
            index += 1;
            continue;
        }

        let Some((key, mut value)) = split_assignment(line) else {
            index += 1;
            continue;
        };
        let Some(assignment) = dependency_assignment(&section, key.trim()) else {
            index += 1;
            continue;
        };
        let mut end = index;
        while delimiters_unbalanced(&value) && end + 1 < lines.len() {
            end += 1;
            value.push('\n');
            value.push_str(strip_toml_comment(lines[end]).trim());
        }
        if delimiters_unbalanced(&value) {
            let dependency = assignment.dependency();
            violations.push(Violation::new(
                line_number,
                format!("dependency {dependency} has an unterminated requirement"),
            ));
            index = end + 1;
            continue;
        }

        process_dependency_assignment(
            assignment,
            &value,
            line_number,
            &mut table_requirements,
            &mut violations,
        );
        index = end + 1;
    }

    for (dependency, requirement) in table_requirements {
        if !requirement.version_seen && !requirement.inherited {
            violations.push(missing_requirement(&dependency, requirement.line));
        }
    }
    violations
}

/// Parses the reviewable `arch.toml` policy format used by R0.
///
/// # Errors
///
/// Returns every syntax and schema violation. An empty crate policy set is an
/// error so a truncated configuration cannot disable the checker.
pub fn parse_arch_config(source: &str) -> Result<ArchConfig, Vec<Violation>> {
    let mut violations = Vec::new();
    let mut builders: BTreeMap<String, PolicyBuilder> = BTreeMap::new();
    let mut current = None;
    let lines: Vec<&str> = source.lines().collect();
    let mut index = 0;

    while index < lines.len() {
        let line_number = index + 1;
        let uncommented = strip_toml_comment(lines[index]);
        let line = uncommented.trim();
        if line.is_empty() {
            index += 1;
            continue;
        }
        if line.starts_with('[') {
            match parse_table_header(line)
                .and_then(|table| table.strip_prefix("crate.").map(str::to_owned))
            {
                Some(name) if !name.is_empty() && !builders.contains_key(&name) => {
                    builders.insert(name.clone(), PolicyBuilder::new(line_number));
                    current = Some(name);
                }
                Some(name) if builders.contains_key(&name) => {
                    violations.push(Violation::new(
                        line_number,
                        format!("duplicate crate policy {name}"),
                    ));
                    current = None;
                }
                _ => {
                    violations.push(Violation::new(
                        line_number,
                        "arch.toml tables must use [crate.<package-name>]",
                    ));
                    current = None;
                }
            }
            index += 1;
            continue;
        }

        let Some((key, mut value)) = split_assignment(line) else {
            violations.push(Violation::new(
                line_number,
                "malformed arch.toml assignment",
            ));
            index += 1;
            continue;
        };
        let mut end = index;
        while delimiters_unbalanced(&value) && end + 1 < lines.len() {
            end += 1;
            value.push('\n');
            value.push_str(strip_toml_comment(lines[end]).trim());
        }
        if delimiters_unbalanced(&value) {
            violations.push(Violation::new(
                line_number,
                format!("unterminated array for {}", key.trim()),
            ));
            index = end + 1;
            continue;
        }
        let values = match parse_string_array(value.trim()) {
            Ok(values) => values,
            Err(message) => {
                violations.push(Violation::new(line_number, message));
                index = end + 1;
                continue;
            }
        };
        match current.as_ref().and_then(|name| builders.get_mut(name)) {
            Some(builder) => builder.set(key.trim(), values, line_number, &mut violations),
            None => violations.push(Violation::new(
                line_number,
                format!("arch.toml field {} is outside a crate policy", key.trim()),
            )),
        }
        index = end + 1;
    }

    if builders.is_empty() {
        violations.push(Violation::new(1, "arch.toml contains no crate policies"));
    }
    let mut crates = BTreeMap::new();
    for (name, builder) in builders {
        if let Some(policy) = builder.finish(&name, &mut violations) {
            crates.insert(name, policy);
        }
    }
    if violations.is_empty() {
        Ok(ArchConfig { crates })
    } else {
        Err(violations)
    }
}

/// Parses the subset of Cargo metadata needed for direct dependency checking.
///
/// # Errors
///
/// Returns an error for malformed JSON, missing required fields, duplicate
/// workspace package names, omitted members, or an empty workspace.
pub fn parse_cargo_metadata(source: &str) -> Result<WorkspaceMetadata, String> {
    let root = JsonParser::new(source).parse()?;
    let root = root
        .as_object()
        .ok_or_else(|| "cargo metadata root must be an object".to_owned())?;
    let member_ids: BTreeSet<String> = root
        .get("workspace_members")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| "cargo metadata is missing workspace_members".to_owned())?
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| "workspace member id must be a string".to_owned())
        })
        .collect::<Result<_, _>>()?;
    if member_ids.is_empty() {
        return Err("cargo metadata contains an empty workspace member list".to_owned());
    }

    let package_values = root
        .get("packages")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| "cargo metadata is missing packages".to_owned())?;
    let mut packages = BTreeMap::new();
    for package_value in package_values {
        let package = package_value
            .as_object()
            .ok_or_else(|| "cargo metadata package must be an object".to_owned())?;
        let id = json_string_field(package, "id")?;
        if !member_ids.contains(id) {
            continue;
        }
        let name = json_string_field(package, "name")?.to_owned();
        let manifest_path = PathBuf::from(json_string_field(package, "manifest_path")?);
        let dependencies = package
            .get("dependencies")
            .and_then(JsonValue::as_array)
            .ok_or_else(|| format!("package {name} is missing dependencies"))?
            .iter()
            .map(|dependency| {
                let object = dependency
                    .as_object()
                    .ok_or_else(|| format!("package {name} dependency is not an object"))?;
                let dependency_name = json_string_field(object, "name")?.to_owned();
                let kind = match object.get("kind") {
                    Some(JsonValue::Null) | None => DependencyKind::Normal,
                    Some(JsonValue::String(kind)) if kind == "dev" => DependencyKind::Development,
                    Some(JsonValue::String(kind)) if kind == "build" => DependencyKind::Build,
                    Some(JsonValue::String(kind)) => {
                        return Err(format!(
                            "package {name} dependency {dependency_name} has unknown kind {kind:?}"
                        ));
                    }
                    Some(_) => {
                        return Err(format!(
                            "package {name} dependency {dependency_name} kind must be null or a string"
                        ));
                    }
                };
                Ok(MetadataDependency {
                    name: dependency_name,
                    kind,
                })
            })
            .collect::<Result<BTreeSet<_>, _>>()?;
        let parsed = MetadataPackage {
            name: name.clone(),
            manifest_path,
            dependencies,
        };
        if packages.insert(name.clone(), parsed).is_some() {
            return Err(format!("duplicate workspace package name {name}"));
        }
    }
    if packages.is_empty() {
        return Err("cargo metadata contains no workspace packages".to_owned());
    }
    if packages.len() != member_ids.len() {
        return Err("cargo metadata omitted one or more workspace packages".to_owned());
    }
    Ok(WorkspaceMetadata { packages })
}

#[must_use]
pub fn validate_dependency_graph(
    config: &ArchConfig,
    metadata: &WorkspaceMetadata,
) -> Vec<Violation> {
    let mut violations = Vec::new();
    let product_crates: BTreeSet<&str> = PRODUCT_CRATES.into_iter().collect();
    let configured_crates: BTreeSet<&str> = config.crates.keys().map(String::as_str).collect();
    let is_full_product_policy = configured_crates == product_crates;

    for configured_crate in configured_crates.difference(&product_crates) {
        violations.push(Violation::new(
            1,
            format!("architecture policy configures rogue crate {configured_crate}"),
        ));
    }
    for package_name in metadata.packages.keys() {
        if package_name == TOOL_CRATE {
            continue;
        }
        if !config.crates.contains_key(package_name) {
            violations.push(Violation::new(
                1,
                format!("workspace package {package_name} has no architecture policy"),
            ));
        }
    }
    if is_full_product_policy && !metadata.packages.contains_key(TOOL_CRATE) {
        violations.push(Violation::new(
            1,
            "real workspace metadata is missing required arch-check package",
        ));
    }

    for (crate_name, policy) in &config.crates {
        let Some(package) = metadata.packages.get(crate_name) else {
            violations.push(Violation::new(
                1,
                format!("configured crate {crate_name} is absent from cargo metadata"),
            ));
            continue;
        };
        for dependency in &package.dependencies {
            if policy.forbidden_deps.contains(&dependency.name) {
                violations.push(Violation::new(
                    1,
                    format!(
                        "crate {crate_name} has forbidden {} dependency {}",
                        dependency.kind.label(),
                        dependency.name
                    ),
                ));
                continue;
            }
            let allowed = policy.allowed_deps.contains(&dependency.name)
                || match dependency.kind {
                    DependencyKind::Normal => false,
                    DependencyKind::Development => {
                        policy.allowed_dev_deps.contains(&dependency.name)
                    }
                    DependencyKind::Build => policy.allowed_build_deps.contains(&dependency.name),
                };
            if !allowed {
                violations.push(Violation::new(
                    1,
                    format!(
                        "crate {crate_name} {} dependency {} is not allowlisted",
                        dependency.kind.label(),
                        dependency.name
                    ),
                ));
            }
        }
    }
    violations
}

/// Runs R0.04 checks using captured metadata and policy fixture files.
///
/// # Errors
///
/// Returns deterministic, file-qualified diagnostics for unreadable or
/// malformed inputs and graph violations.
pub fn check_fixture_files(metadata_path: &Path, config_path: &Path) -> Result<(), Vec<Violation>> {
    let metadata_source = read_file(metadata_path)?;
    let config_source = read_file(config_path)?;
    let metadata = parse_cargo_metadata(&metadata_source).map_err(|message| {
        vec![Violation::new(
            1,
            format!("{} line 1: {message}", metadata_path.display()),
        )]
    })?;
    let config = parse_arch_config(&config_source)
        .map_err(|violations| qualify_violations(config_path, violations))?;
    let violations = qualify_violations(config_path, validate_dependency_graph(&config, &metadata));
    if violations.is_empty() {
        Ok(())
    } else {
        Err(violations)
    }
}

/// Runs the R0.04 dependency graph and exact-pin checks against a workspace.
///
/// # Errors
///
/// Returns all deterministic policy violations. Cargo execution and every file
/// read fail closed.
pub fn check_workspace_r0_04(root: &Path) -> Result<(), Vec<Violation>> {
    let mut violations = match check_workspace_arch_and_source(root) {
        Ok(()) => Vec::new(),
        Err(violations) => violations,
    };
    violations.extend(validate_workspace_r0_06(root));
    sort_violations(&mut violations);
    if violations.is_empty() {
        Ok(())
    } else {
        Err(violations)
    }
}

fn check_workspace_arch_and_source(root: &Path) -> Result<(), Vec<Violation>> {
    let config_path = root.join("tools/arch-check/arch.toml");
    let config_source = read_file(&config_path)?;
    let config = parse_arch_config(&config_source)
        .map_err(|violations| qualify_violations(&config_path, violations))?;
    let output =
        Command::new(std::env::var_os("CARGO").unwrap_or_else(|| OsStr::new("cargo").to_owned()))
            .args(["metadata", "--locked", "--format-version", "1", "--no-deps"])
            .current_dir(root)
            .output()
            .map_err(|error| {
                vec![Violation::new(
                    1,
                    format!("cannot run cargo metadata --locked: {error}"),
                )]
            })?;
    if !output.status.success() {
        return Err(vec![Violation::new(
            1,
            format!(
                "cargo metadata --locked failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ),
        )]);
    }
    let metadata_source = std::str::from_utf8(&output.stdout).map_err(|error| {
        vec![Violation::new(
            1,
            format!("cargo metadata emitted non-UTF-8 output: {error}"),
        )]
    })?;
    let metadata = parse_cargo_metadata(metadata_source).map_err(|message| {
        vec![Violation::new(
            1,
            format!("malformed cargo metadata: {message}"),
        )]
    })?;
    let mut violations = validate_dependency_graph(&config, &metadata);

    let mut manifests = vec![root.join("Cargo.toml")];
    manifests.extend(
        metadata
            .packages
            .values()
            .map(|package| package.manifest_path.clone()),
    );
    manifests.sort();
    manifests.dedup();
    for manifest_path in manifests {
        match fs::read_to_string(&manifest_path) {
            Ok(manifest) => {
                violations.extend(qualify_violations(
                    &manifest_path,
                    check_exact_requirements(&manifest),
                ));
                violations.extend(qualify_violations(
                    &manifest_path,
                    validate_source_layout(&manifest),
                ));
            }
            Err(error) => violations.push(Violation::new(
                1,
                format!(
                    "{} line 1: cannot read required input: {error}",
                    manifest_path.display()
                ),
            )),
        }
    }
    violations.extend(validate_workspace_sources(&config, &metadata));
    if violations.is_empty() {
        Ok(())
    } else {
        Err(violations)
    }
}

#[must_use]
pub fn scan_source(source: &str, tokens: &[String]) -> Vec<Violation> {
    let (mut cleaned, syntax) = lex_rust(source);
    mask_cfg_test_items(&mut cleaned, &syntax);
    let source_tokens = rust_tokens(&cleaned, &syntax);
    let use_paths = expanded_use_paths(&source_tokens);
    let mut violations = Vec::new();
    for window in source_tokens.windows(3) {
        if window[0].text == "#" && window[1].text == "[" && window[2].text == "path" {
            violations.push(Violation::new(
                window[0].line,
                "#[path] source indirection is forbidden; architecture checks require conventional module paths",
            ));
        }
    }
    if tokens_forbid_std_paths(tokens) {
        violations.extend(std_aliases(&source_tokens).into_iter().map(|token| {
            Violation::new(
                token.line,
                "aliasing std is forbidden when protected std paths are denied",
            )
        }));
    }
    for forbidden in tokens {
        if forbidden.is_empty() {
            continue;
        }
        let (pattern_source, pattern_syntax) = lex_rust(forbidden);
        let pattern = rust_tokens(&pattern_source, &pattern_syntax);
        if pattern.is_empty() {
            continue;
        }
        let mut hits = BTreeMap::new();
        if pattern.len() <= source_tokens.len() {
            for window in source_tokens.windows(pattern.len()) {
                if rust_token_texts_equal(window, &pattern) {
                    hits.insert(window[0].offset, window[0].line);
                }
            }
        }
        if let Some(path_pattern) = rust_path_pattern(&pattern) {
            for path in &use_paths {
                for hit in matching_use_path_offsets(path, &path_pattern) {
                    hits.insert(hit.offset, hit.line);
                }
            }
        }
        violations.extend(
            hits.into_values()
                .map(|line| Violation::new(line, format!("forbidden source token {forbidden:?}"))),
        );
    }
    violations
}

fn tokens_forbid_std_paths(tokens: &[String]) -> bool {
    tokens.iter().any(|token| token.starts_with("std::"))
}

fn std_aliases(tokens: &[RustToken]) -> Vec<&RustToken> {
    let mut aliases = Vec::new();
    for (index, token) in tokens.iter().enumerate() {
        if token.text == "use" {
            let mut cursor = index + 1;
            if tokens.get(cursor).is_some_and(|item| item.text == "::") {
                cursor += 1;
            }
            if tokens.get(cursor).is_some_and(|item| item.text == "std")
                && tokens.get(cursor + 1).is_some_and(|item| item.text == "as")
                && tokens
                    .get(cursor + 2)
                    .is_some_and(|item| is_rust_identifier(&item.text))
            {
                aliases.push(&tokens[cursor]);
            } else if tokens.get(cursor).is_some_and(|item| item.text == "std")
                && tokens.get(cursor + 1).is_some_and(|item| item.text == "::")
                && tokens.get(cursor + 2).is_some_and(|item| item.text == "{")
            {
                let mut group_cursor = cursor + 3;
                let mut depth = 1_u32;
                while let Some(item) = tokens.get(group_cursor) {
                    if item.text == "{" {
                        depth += 1;
                    } else if item.text == "}" {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    } else if depth == 1
                        && item.text == "self"
                        && tokens
                            .get(group_cursor + 1)
                            .is_some_and(|next| next.text == "as")
                        && tokens
                            .get(group_cursor + 2)
                            .is_some_and(|alias| is_rust_identifier(&alias.text))
                    {
                        aliases.push(&tokens[cursor]);
                        break;
                    }
                    group_cursor += 1;
                }
            }
        } else if token.text == "extern"
            && tokens
                .get(index + 1)
                .is_some_and(|item| item.text == "crate")
            && tokens.get(index + 2).is_some_and(|item| item.text == "std")
            && tokens.get(index + 3).is_some_and(|item| item.text == "as")
            && tokens
                .get(index + 4)
                .is_some_and(|item| is_rust_identifier(&item.text))
        {
            aliases.push(&tokens[index + 2]);
        }
    }
    aliases
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RustToken {
    text: String,
    line: usize,
    offset: usize,
}

#[derive(Clone, Debug)]
struct RustPathPattern {
    segments: Vec<String>,
    requires_descendant: bool,
}

fn rust_tokens(source: &[u8], syntax: &[bool]) -> Vec<RustToken> {
    let mut tokens = Vec::new();
    let mut index = 0;
    let mut line = 1;
    while index < source.len() {
        if source[index] == b'\n' {
            line += 1;
            index += 1;
            continue;
        }
        if !syntax[index] || source[index].is_ascii_whitespace() {
            index += 1;
            continue;
        }
        let start = index;
        if is_rust_identifier_byte(source[index]) {
            index += 1;
            while index < source.len() && syntax[index] && is_rust_identifier_byte(source[index]) {
                index += 1;
            }
        } else if source.get(index..index + 2) == Some(b"::")
            && syntax.get(index + 1) == Some(&true)
        {
            index += 2;
        } else {
            index += 1;
        }
        tokens.push(RustToken {
            text: String::from_utf8_lossy(&source[start..index]).into_owned(),
            line,
            offset: start,
        });
    }
    tokens
}

fn is_rust_identifier_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || !byte.is_ascii()
}

fn rust_token_texts_equal(left: &[RustToken], right: &[RustToken]) -> bool {
    left.iter()
        .zip(right)
        .all(|(left, right)| left.text == right.text)
}

fn rust_path_pattern(tokens: &[RustToken]) -> Option<RustPathPattern> {
    let mut segments = Vec::new();
    let mut expect_segment = true;
    for token in tokens {
        if expect_segment {
            if !is_rust_identifier(&token.text) {
                return None;
            }
            segments.push(token.text.clone());
        } else if token.text != "::" {
            return None;
        }
        expect_segment = !expect_segment;
    }
    if segments.is_empty() {
        None
    } else {
        Some(RustPathPattern {
            segments,
            requires_descendant: expect_segment,
        })
    }
}

fn is_rust_identifier(value: &str) -> bool {
    value
        .as_bytes()
        .first()
        .is_some_and(|byte| is_rust_identifier_byte(*byte))
        && value
            .as_bytes()
            .iter()
            .all(|byte| is_rust_identifier_byte(*byte))
}

fn matching_use_path_offsets<'a>(
    path: &'a [RustToken],
    pattern: &RustPathPattern,
) -> Vec<&'a RustToken> {
    let mut hits = Vec::new();
    if pattern.segments.len() > path.len() {
        return hits;
    }
    for start in 0..=path.len() - pattern.segments.len() {
        let end = start + pattern.segments.len();
        if pattern.requires_descendant && end == path.len() {
            continue;
        }
        if path[start..end]
            .iter()
            .zip(&pattern.segments)
            .all(|(token, segment)| token.text == *segment)
        {
            hits.push(&path[start]);
        }
    }
    hits
}

fn expanded_use_paths(tokens: &[RustToken]) -> Vec<Vec<RustToken>> {
    let mut paths = Vec::new();
    let mut index = 0;
    while index < tokens.len() {
        if tokens[index].text != "use" {
            index += 1;
            continue;
        }
        index += 1;
        parse_use_tree(tokens, &mut index, &[], &mut paths);
        while index < tokens.len() && tokens[index].text != ";" {
            index += 1;
        }
        index = (index + 1).min(tokens.len());
    }
    paths
}

fn parse_use_tree(
    tokens: &[RustToken],
    index: &mut usize,
    prefix: &[RustToken],
    paths: &mut Vec<Vec<RustToken>>,
) {
    if tokens.get(*index).is_some_and(|token| token.text == "{") {
        *index += 1;
        while *index < tokens.len() && tokens[*index].text != "}" {
            let before = *index;
            parse_use_tree(tokens, index, prefix, paths);
            if tokens.get(*index).is_some_and(|token| token.text == ",") || *index == before {
                *index += 1;
            }
        }
        if tokens.get(*index).is_some_and(|token| token.text == "}") {
            *index += 1;
        }
        return;
    }

    if tokens.get(*index).is_some_and(|token| token.text == "::") {
        *index += 1;
    }
    let mut path = prefix.to_vec();
    while let Some(token) = tokens.get(*index) {
        if token.text == "*" {
            *index += 1;
            break;
        }
        if !is_rust_identifier(&token.text) || token.text == "as" {
            break;
        }
        path.push(token.clone());
        *index += 1;
        if tokens.get(*index).is_none_or(|token| token.text != "::") {
            break;
        }
        *index += 1;
        if tokens.get(*index).is_some_and(|token| token.text == "{") {
            parse_use_tree(tokens, index, &path, paths);
            return;
        }
    }
    if tokens.get(*index).is_some_and(|token| token.text == "as") {
        *index += 1;
        if tokens
            .get(*index)
            .is_some_and(|token| is_rust_identifier(&token.text))
        {
            *index += 1;
        }
    }
    if !path.is_empty() {
        paths.push(path);
    }
}

#[must_use]
pub fn validate_source_layout(manifest: &str) -> Vec<Violation> {
    let mut violations = Vec::new();
    let mut section = String::new();
    for (line_index, raw_line) in manifest.lines().enumerate() {
        let line_number = line_index + 1;
        let line = strip_toml_comment(raw_line).trim();
        if let Some(table) = cargo_table_header(line) {
            section = table;
            continue;
        }
        let Some((key, value)) = split_assignment(line) else {
            continue;
        };
        let key_components = toml_dotted_key_components(key.trim());
        let target_section = matches!(
            section.as_str(),
            "lib" | "bin" | "example" | "test" | "bench"
        );
        let dotted_target_path = section.is_empty()
            && matches!(
                key_components.as_slice(),
                [target, field]
                    if matches!(target.as_str(), "lib" | "bin" | "example" | "test" | "bench")
                        && field == "path"
            );
        if (target_section && key_components.as_slice() == ["path"]) || dotted_target_path {
            violations.push(Violation::new(
                line_number,
                "custom target path is forbidden; architecture checks require Cargo's conventional src layout",
            ));
        } else if ((section == "package" && key_components.as_slice() == ["build"])
            || (section.is_empty() && key_components.as_slice() == ["package", "build"]))
            && parse_toml_string(value.trim()).is_ok()
        {
            violations.push(Violation::new(
                line_number,
                "custom target path for package build script is forbidden",
            ));
        }
    }
    violations
}

fn cargo_table_header(line: &str) -> Option<String> {
    if let Some(inner) = line
        .strip_prefix("[[")
        .and_then(|value| value.strip_suffix("]]"))
    {
        let inner = inner.trim();
        return (!inner.is_empty()).then(|| inner.to_owned());
    }
    parse_table_header(line)
}

/// Runs only the R0.05 source checks against a deterministic fixture tree.
///
/// # Errors
///
/// Returns policy parse failures, missing configured source roots, traversal
/// failures, malformed UTF-8, and forbidden-token hits with file and line.
pub fn check_source_fixture_files(
    source_root: &Path,
    config_path: &Path,
) -> Result<(), Vec<Violation>> {
    let config_source = read_file(config_path)?;
    let config = parse_arch_config(&config_source)
        .map_err(|violations| qualify_violations(config_path, violations))?;
    let roots = config
        .crates
        .keys()
        .map(|crate_name| {
            (
                crate_name.clone(),
                source_root.join("crates").join(crate_name).join("src"),
            )
        })
        .collect();
    let violations = validate_source_roots(&config, &roots);
    if violations.is_empty() {
        Ok(())
    } else {
        Err(violations)
    }
}

#[must_use]
pub fn validate_gates(source: &str) -> Vec<Violation> {
    let mut violations = Vec::new();
    let mut schema = None;
    let mut gates: BTreeMap<String, GateBuilder> = BTreeMap::new();
    let mut current_gate = None;
    let mut entered_table = false;
    let lines: Vec<&str> = source.lines().collect();
    let mut index = 0;

    while index < lines.len() {
        let line_number = index + 1;
        let uncommented = strip_toml_comment(lines[index]);
        let line = uncommented.trim();
        if line.is_empty() {
            index += 1;
            continue;
        }

        if line.starts_with('[') {
            entered_table = true;
            current_gate = start_gate_table(line, line_number, &mut gates, &mut violations);
            index += 1;
            continue;
        }

        let Some((key, mut value)) = split_assignment(line) else {
            violations.push(Violation::new(
                line_number,
                "malformed gate registry assignment",
            ));
            index += 1;
            continue;
        };
        let key = key.trim();
        let mut end = index;
        while delimiters_unbalanced(&value) && end + 1 < lines.len() {
            end += 1;
            value.push('\n');
            value.push_str(strip_toml_comment(lines[end]).trim());
        }
        if delimiters_unbalanced(&value) {
            violations.push(Violation::new(
                line_number,
                format!("unterminated gate registry value for {key}"),
            ));
            index = end + 1;
            continue;
        }

        if current_gate.is_none() {
            if key != "schema" {
                violations.push(Violation::new(
                    line_number,
                    format!("gate registry field {key} is outside a gate section"),
                ));
            } else if entered_table {
                violations.push(Violation::new(
                    line_number,
                    "gate registry schema must precede every gate section",
                ));
            } else if schema.is_some() {
                violations.push(Violation::new(
                    line_number,
                    "duplicate gate registry schema field",
                ));
            } else if value.trim() == "1" {
                schema = Some(line_number);
            } else {
                violations.push(Violation::new(
                    line_number,
                    format!(
                        "unsupported or malformed gate registry schema {:?}; expected 1",
                        value.trim()
                    ),
                ));
                schema = Some(line_number);
            }
        } else if key == "schema" {
            violations.push(Violation::new(
                line_number,
                "gate registry schema must be a single top-level field",
            ));
        } else if let Some(builder) = current_gate
            .as_ref()
            .and_then(|gate_name| gates.get_mut(gate_name))
        {
            builder.set(key, value.trim(), line_number, &mut violations);
        }
        index = end + 1;
    }

    finish_gate_registry(schema, gates, &mut violations);
    violations
}

#[must_use]
pub fn validate_relative_links(source: &str, known_paths: &[String]) -> Vec<Violation> {
    let known_paths: BTreeSet<String> = known_paths
        .iter()
        .filter_map(|path| normalize_relative_target(path).ok())
        .collect();
    let (links, mut violations) = markdown_links(source);
    for link in links {
        let Some(target) = relative_link_path(&link.target) else {
            continue;
        };
        match normalize_relative_target(target) {
            Ok(normalized) if known_paths.contains(&normalized) => {}
            Ok(normalized) => violations.push(Violation::new(
                link.line,
                format!("relative documentation link {normalized:?} does not resolve"),
            )),
            Err(message) => violations.push(Violation::new(link.line, message)),
        }
    }
    violations
}

#[must_use]
pub fn validate_status_discipline(source: &str) -> Vec<Violation> {
    let cleaned = clean_markdown_prose(source);
    let is_adr_template = source
        .lines()
        .take(5)
        .any(|line| line.trim() == "# ADR-NNNN: Title");
    let mut violations = Vec::new();
    let mut status = None;
    let mut current_heading_level = None;
    let mut status_scope_level = None;
    let mut paragraph = ProseBuffer::default();
    let mut table_status_column = None;

    let raw_lines: Vec<&str> = source.lines().collect();
    for (line_index, line) in cleaned.lines().enumerate() {
        let line_number = line_index + 1;
        let raw_line = raw_lines.get(line_index).copied().unwrap_or(line);
        if !is_adr_template && !line.trim().is_empty() {
            if let Some(value) = explicit_status_value(raw_line) {
                paragraph.flush(status, &mut violations);
                match parse_status_word(&value) {
                    Some(parsed) => {
                        status = Some(parsed);
                        status_scope_level = current_heading_level;
                    }
                    None => violations.push(Violation::new(
                        line_number,
                        format!(
                            "status {value:?} is invalid; expected accepted, in progress, planned, or deferred"
                        ),
                    )),
                }
                continue;
            }
        }

        let trimmed = line.trim();
        if trimmed.starts_with('|') && trimmed.ends_with('|') {
            paragraph.flush(status, &mut violations);
            let cells = markdown_table_cells(raw_line.trim());
            if let Some(column) = cells
                .iter()
                .position(|cell| cell.eq_ignore_ascii_case("status"))
            {
                table_status_column = Some(column);
                continue;
            }
            if cells.iter().all(|cell| is_markdown_table_separator(cell)) {
                continue;
            }
            let mut row_status = status;
            if let Some(column) = table_status_column {
                if let Some(value) = cells.get(column) {
                    let value = strip_markdown_status(value);
                    if !value.is_empty() {
                        match parse_status_word(&value) {
                            Some(parsed) => row_status = Some(parsed),
                            None => violations.push(Violation::new(
                                line_number,
                                format!(
                                    "status {value:?} is invalid; expected accepted, in progress, planned, or deferred"
                                ),
                            )),
                        }
                    }
                }
            }
            check_claim_unit(trimmed, line_number, row_status, &mut violations);
            continue;
        }
        table_status_column = None;

        if trimmed.is_empty() {
            paragraph.flush(status, &mut violations);
        } else if let Some(heading_level) = markdown_heading_level(trimmed) {
            paragraph.flush(status, &mut violations);
            if status_scope_level.is_some_and(|scope_level| heading_level <= scope_level) {
                status = None;
                status_scope_level = None;
            }
            current_heading_level = Some(heading_level);
        } else {
            paragraph.push(line, line_number);
        }
    }
    paragraph.flush(status, &mut violations);
    violations
}

/// Runs only the R0.06 gate-registry and documentation checks against fixture
/// inputs.
///
/// # Errors
///
/// Returns deterministic, file-qualified diagnostics for every malformed or
/// unreadable required input, unearned claim, and dangling relative link.
pub fn check_r0_06_fixture_files(
    gate_registry: &Path,
    docs_root: &Path,
) -> Result<(), Vec<Violation>> {
    let mut violations = validate_document_tree(docs_root, docs_root.parent(), &[]);
    match read_file(gate_registry) {
        Ok(gate_source) => violations.extend(qualify_violations(
            gate_registry,
            validate_gates(&gate_source),
        )),
        Err(gate_violations) => violations.extend(gate_violations),
    }
    sort_violations(&mut violations);
    if violations.is_empty() {
        Ok(())
    } else {
        Err(violations)
    }
}

#[must_use]
pub fn validate_test_names(_source: &str) -> Vec<Violation> {
    Vec::new()
}

#[must_use]
pub fn scan_canaries(_source: &str) -> Vec<Violation> {
    Vec::new()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DeliveryStatus {
    Accepted,
    InProgress,
    Planned,
    Deferred,
}

fn parse_status_word(value: &str) -> Option<DeliveryStatus> {
    match value {
        "accepted" => Some(DeliveryStatus::Accepted),
        "in progress" => Some(DeliveryStatus::InProgress),
        "planned" => Some(DeliveryStatus::Planned),
        "deferred" => Some(DeliveryStatus::Deferred),
        _ => None,
    }
}

#[derive(Clone, Debug)]
struct GateBuilder {
    line: usize,
    status_seen: bool,
    status_line: Option<usize>,
    status: Option<DeliveryStatus>,
    section_seen: bool,
    section_line: Option<usize>,
    section: Option<String>,
    commands_seen: bool,
    commands_line: Option<usize>,
    commands: Option<Vec<String>>,
}

impl GateBuilder {
    fn new(line: usize) -> Self {
        Self {
            line,
            status_seen: false,
            status_line: None,
            status: None,
            section_seen: false,
            section_line: None,
            section: None,
            commands_seen: false,
            commands_line: None,
            commands: None,
        }
    }

    fn set(&mut self, key: &str, value: &str, line: usize, violations: &mut Vec<Violation>) {
        match key {
            "status" => {
                if std::mem::replace(&mut self.status_seen, true) {
                    violations.push(Violation::new(line, "duplicate gate registry field status"));
                    return;
                }
                self.status_line = Some(line);
                match parse_toml_string(value) {
                    Ok(value) => match parse_status_word(&value) {
                        Some(status) => self.status = Some(status),
                        None => violations.push(Violation::new(
                            line,
                            format!(
                                "gate status {value:?} is invalid; expected accepted, in progress, planned, or deferred"
                            ),
                        )),
                    },
                    Err(message) => violations.push(Violation::new(
                        line,
                        format!("gate status must be a string: {message}"),
                    )),
                }
            }
            "section" => {
                if std::mem::replace(&mut self.section_seen, true) {
                    violations.push(Violation::new(
                        line,
                        "duplicate gate registry field section",
                    ));
                    return;
                }
                self.section_line = Some(line);
                match parse_toml_string(value) {
                    Ok(value) if value.trim().is_empty() => {
                        violations.push(Violation::new(line, "gate section must not be empty"));
                    }
                    Ok(value) => self.section = Some(value),
                    Err(message) => violations.push(Violation::new(
                        line,
                        format!("gate section must be a string: {message}"),
                    )),
                }
            }
            "commands" => {
                if std::mem::replace(&mut self.commands_seen, true) {
                    violations.push(Violation::new(
                        line,
                        "duplicate gate registry field commands",
                    ));
                    return;
                }
                self.commands_line = Some(line);
                match parse_string_array(value) {
                    Ok(commands) if commands.iter().any(|command| command.trim().is_empty()) => {
                        violations.push(Violation::new(
                            line,
                            "gate command strings must not be empty",
                        ));
                    }
                    Ok(commands)
                        if commands
                            .iter()
                            .any(|command| command.contains(['\n', '\r'])) =>
                    {
                        violations.push(Violation::new(
                            line,
                            "gate command strings must not contain newlines",
                        ));
                    }
                    Ok(commands) => self.commands = Some(commands),
                    Err(message) => violations.push(Violation::new(
                        line,
                        format!("malformed gate commands: {message}"),
                    )),
                }
            }
            _ => violations.push(Violation::new(
                line,
                format!("unknown gate registry field {key}"),
            )),
        }
    }

    fn finish(self, gate_name: &str, section_number: usize, violations: &mut Vec<Violation>) {
        if !self.status_seen {
            violations.push(Violation::new(
                self.line,
                format!("gate {gate_name} is missing status"),
            ));
        }
        if !self.section_seen {
            violations.push(Violation::new(
                self.line,
                format!("gate {gate_name} is missing section"),
            ));
        }
        if !self.commands_seen {
            violations.push(Violation::new(
                self.line,
                format!("gate {gate_name} is missing commands"),
            ));
        }
        if let Some(section) = self.section {
            let expected = format!("BUILD_PLAN.md §{section_number}");
            if section != expected {
                violations.push(Violation::new(
                    self.section_line.unwrap_or(self.line),
                    format!("gate {gate_name} section must be {expected:?}, found {section:?}"),
                ));
            }
        }
        if self.status == Some(DeliveryStatus::Accepted)
            && self.commands.as_ref().is_none_or(Vec::is_empty)
        {
            violations.push(Violation::new(
                self.commands_line.unwrap_or(self.line),
                format!("accepted gate {gate_name} must have commands"),
            ));
        }
    }
}

fn is_expected_gate(name: &str) -> bool {
    name.strip_prefix('R')
        .and_then(|number| number.parse::<usize>().ok())
        .is_some_and(|number| number <= 10 && name == format!("R{number}"))
}

fn finish_gate_registry(
    schema: Option<usize>,
    mut gates: BTreeMap<String, GateBuilder>,
    violations: &mut Vec<Violation>,
) {
    if schema.is_none() {
        violations.push(Violation::new(
            1,
            "gate registry is missing required schema = 1",
        ));
    }
    validate_gate_replay_order(&gates, violations);
    validate_accepted_gate_prefix(&gates, violations);
    for number in 0..=10 {
        let gate_name = format!("R{number}");
        match gates.remove(&gate_name) {
            Some(builder) => builder.finish(&gate_name, number + 5, violations),
            None => violations.push(Violation::new(
                1,
                format!("gate registry is missing required section {gate_name}"),
            )),
        }
    }
}

fn validate_gate_replay_order(
    gates: &BTreeMap<String, GateBuilder>,
    violations: &mut Vec<Violation>,
) {
    let mut encountered: Vec<_> = gates.iter().collect();
    encountered.sort_by_key(|(_, builder)| builder.line);
    for (number, (gate_name, builder)) in encountered.into_iter().enumerate() {
        let expected = format!("R{number}");
        if gate_name != &expected {
            violations.push(Violation::new(
                builder.line,
                format!(
                    "gate sections are out of replay order: expected {expected}, found {gate_name}"
                ),
            ));
        }
    }
}

fn validate_accepted_gate_prefix(
    gates: &BTreeMap<String, GateBuilder>,
    violations: &mut Vec<Violation>,
) {
    let mut first_unaccepted = None;
    for number in 0..=10 {
        let gate_name = format!("R{number}");
        let Some(builder) = gates.get(&gate_name) else {
            continue;
        };
        if builder.status == Some(DeliveryStatus::Accepted) {
            if let Some(ref earlier_gate) = first_unaccepted {
                violations.push(Violation::new(
                    builder.status_line.unwrap_or(builder.line),
                    format!(
                        "accepted gate {gate_name} is not a prefix because earlier gate {earlier_gate} is unaccepted"
                    ),
                ));
            }
        } else if first_unaccepted.is_none() {
            first_unaccepted = Some(gate_name);
        }
    }
}

fn start_gate_table(
    line: &str,
    line_number: usize,
    gates: &mut BTreeMap<String, GateBuilder>,
    violations: &mut Vec<Violation>,
) -> Option<String> {
    let Some(table) = parse_table_header(line) else {
        violations.push(Violation::new(
            line_number,
            "malformed gate registry table header",
        ));
        return None;
    };
    let Some(gate_name) = table.strip_prefix("gate.") else {
        violations.push(Violation::new(
            line_number,
            format!("unknown gate registry table [{table}]"),
        ));
        return None;
    };
    if !is_expected_gate(gate_name) {
        violations.push(Violation::new(
            line_number,
            format!("unknown gate registry section {gate_name}"),
        ));
        return None;
    }
    if gates.contains_key(gate_name) {
        violations.push(Violation::new(
            line_number,
            format!("duplicate gate registry section {gate_name}"),
        ));
        return None;
    }
    gates.insert(gate_name.to_owned(), GateBuilder::new(line_number));
    Some(gate_name.to_owned())
}

#[derive(Clone, Debug)]
struct MarkdownLink {
    line: usize,
    target: String,
}

fn markdown_links(source: &str) -> (Vec<MarkdownLink>, Vec<Violation>) {
    let cleaned = clean_markdown_prose(source);
    let mut links = Vec::new();
    let mut violations = Vec::new();
    let mut definitions: BTreeMap<String, MarkdownLink> = BTreeMap::new();
    let mut references = Vec::new();

    for (line_index, line) in cleaned.lines().enumerate() {
        let line_number = line_index + 1;
        if let Some((label, target)) = reference_definition(line) {
            let definition = MarkdownLink {
                line: line_number,
                target,
            };
            if definitions.insert(label.clone(), definition).is_some() {
                violations.push(Violation::new(
                    line_number,
                    format!("duplicate Markdown reference definition [{label}]"),
                ));
            }
        }
        references.extend(full_reference_uses(line, line_number));

        let bytes = line.as_bytes();
        let mut index = 0;
        while index + 1 < bytes.len() {
            if bytes[index] == b']'
                && bytes[index + 1] == b'('
                && !is_escaped(bytes, index)
                && inline_link_label_start(bytes, index).is_some()
            {
                let destination_start = index + 2;
                let mut cursor = destination_start;
                let mut depth = 1_u32;
                let mut angle = false;
                while cursor < bytes.len() {
                    if bytes[cursor] == b'\\' {
                        cursor = (cursor + 2).min(bytes.len());
                        continue;
                    }
                    match bytes[cursor] {
                        b'<' if depth == 1 => angle = true,
                        b'>' if depth == 1 => angle = false,
                        b'(' if !angle => depth += 1,
                        b')' if !angle => {
                            depth -= 1;
                            if depth == 0 {
                                break;
                            }
                        }
                        _ => {}
                    }
                    cursor += 1;
                }
                if cursor == bytes.len() {
                    violations.push(Violation::new(
                        line_number,
                        "malformed Markdown link has no closing parenthesis",
                    ));
                    break;
                }
                match markdown_destination(&line[destination_start..cursor]) {
                    Some(target) => links.push(MarkdownLink {
                        line: line_number,
                        target,
                    }),
                    None => violations.push(Violation::new(
                        line_number,
                        "Markdown link destination must not be empty or malformed",
                    )),
                }
                index = cursor + 1;
            } else {
                index += 1;
            }
        }
    }
    let mut used_definitions = BTreeSet::new();
    for (_, label) in references {
        if definitions.contains_key(&label) {
            used_definitions.insert(label);
        }
    }
    for label in used_definitions {
        if let Some(definition) = definitions.remove(&label) {
            links.push(definition);
        }
    }
    (links, violations)
}

fn inline_link_label_start(bytes: &[u8], close: usize) -> Option<usize> {
    let start = bytes[..close].iter().rposition(|byte| *byte == b'[')?;
    if is_escaped(bytes, start) || bytes.get(start + 1) == Some(&b'^') {
        None
    } else {
        Some(start)
    }
}

fn reference_definition(line: &str) -> Option<(String, String)> {
    let line = strip_blockquote_prefix(line);
    let bytes = line.as_bytes();
    if bytes.first() != Some(&b'[') || bytes.get(1) == Some(&b'^') {
        return None;
    }
    let label_end =
        bytes.iter().enumerate().skip(1).find_map(|(index, byte)| {
            (*byte == b']' && !is_escaped(bytes, index)).then_some(index)
        })?;
    if bytes.get(label_end + 1) != Some(&b':') {
        return None;
    }
    let label = normalize_reference_label(&line[1..label_end]);
    if label.is_empty() {
        return None;
    }
    markdown_destination(&line[label_end + 2..]).map(|target| (label, target))
}

fn strip_blockquote_prefix(mut line: &str) -> &str {
    line = line.trim_start();
    while let Some(remainder) = line.strip_prefix('>') {
        line = remainder
            .strip_prefix(' ')
            .unwrap_or(remainder)
            .trim_start();
    }
    line
}

fn full_reference_uses(line: &str, line_number: usize) -> Vec<(usize, String)> {
    let bytes = line.as_bytes();
    let mut references = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != b'[' || is_escaped(bytes, index) || bytes.get(index + 1) == Some(&b'^') {
            index += 1;
            continue;
        }
        let Some(first_end) = find_unescaped_byte(bytes, index + 1, b']') else {
            break;
        };
        if bytes.get(first_end + 1) != Some(&b'[') {
            index = first_end + 1;
            continue;
        }
        let Some(second_end) = find_unescaped_byte(bytes, first_end + 2, b']') else {
            break;
        };
        let explicit = &line[first_end + 2..second_end];
        let label = if explicit.is_empty() {
            normalize_reference_label(&line[index + 1..first_end])
        } else {
            normalize_reference_label(explicit)
        };
        if !label.is_empty() {
            references.push((line_number, label));
        }
        index = second_end + 1;
    }
    references
}

fn find_unescaped_byte(bytes: &[u8], start: usize, expected: u8) -> Option<usize> {
    bytes
        .iter()
        .enumerate()
        .skip(start)
        .find_map(|(index, byte)| (*byte == expected && !is_escaped(bytes, index)).then_some(index))
}

fn normalize_reference_label(label: &str) -> String {
    label
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

fn markdown_destination(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    if let Some(value) = value.strip_prefix('<') {
        let end = value.find('>')?;
        let target = value[..end].trim();
        return (!target.is_empty()).then(|| target.to_owned());
    }
    let mut escaped = false;
    let mut depth = 0_u32;
    let end = value
        .char_indices()
        .find_map(|(index, character)| {
            if escaped {
                escaped = false;
                return None;
            }
            if character == '\\' {
                escaped = true;
            } else if character == '(' {
                depth += 1;
            } else if character == ')' && depth > 0 {
                depth -= 1;
            } else if character.is_whitespace() && depth == 0 {
                return Some(index);
            }
            None
        })
        .unwrap_or(value.len());
    let target = value[..end].trim();
    (!target.is_empty()).then(|| target.replace("\\(", "(").replace("\\)", ")"))
}

fn is_escaped(bytes: &[u8], index: usize) -> bool {
    let mut backslashes = 0;
    let mut cursor = index;
    while cursor > 0 && bytes[cursor - 1] == b'\\' {
        backslashes += 1;
        cursor -= 1;
    }
    backslashes % 2 == 1
}

fn relative_link_path(target: &str) -> Option<&str> {
    let target = target.trim();
    if target.is_empty()
        || target.starts_with('#')
        || target.starts_with('/')
        || target.starts_with("//")
    {
        return None;
    }
    let end = [target.find('#'), target.find('?')]
        .into_iter()
        .flatten()
        .min()
        .unwrap_or(target.len());
    let target = target[..end].trim();
    if target.is_empty() || has_uri_scheme(target) {
        None
    } else {
        Some(target)
    }
}

fn has_uri_scheme(target: &str) -> bool {
    let Some(colon) = target.find(':') else {
        return false;
    };
    let scheme = &target[..colon];
    !scheme.is_empty()
        && scheme.as_bytes()[0].is_ascii_alphabetic()
        && scheme
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'-' | b'.'))
}

fn normalize_relative_target(target: &str) -> Result<String, String> {
    if target.contains('\\') {
        return Err(format!(
            "relative documentation link {target:?} contains a non-portable backslash"
        ));
    }
    let mut components = Vec::new();
    for component in Path::new(target).components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::Normal(component) => {
                let component = component.to_str().ok_or_else(|| {
                    format!("relative documentation link {target:?} is not valid UTF-8")
                })?;
                components.push(component.to_owned());
            }
            std::path::Component::ParentDir => {
                if components.pop().is_none() {
                    return Err(format!(
                        "relative documentation link {target:?} escapes its allowed root"
                    ));
                }
            }
            std::path::Component::Prefix(_) | std::path::Component::RootDir => {
                return Err(format!(
                    "relative documentation link {target:?} must not be absolute"
                ));
            }
        }
    }
    Ok(if components.is_empty() {
        ".".to_owned()
    } else {
        components.join("/")
    })
}

fn clean_markdown_prose(source: &str) -> String {
    let bytes = source.as_bytes();
    let mut cleaned = bytes.to_vec();
    let mut in_fence: Option<(u8, usize)> = None;
    let mut in_comment = false;
    let mut line_start = 0;

    while line_start < bytes.len() {
        let line_end = bytes[line_start..]
            .iter()
            .position(|byte| *byte == b'\n')
            .map_or(bytes.len(), |offset| line_start + offset);
        let mut content_start = line_start;
        while content_start < line_end && bytes[content_start] == b' ' {
            content_start += 1;
        }
        let fence = fence_marker(&bytes[content_start..line_end]);
        if let Some((marker, length)) = in_fence {
            mask_markdown_range(&mut cleaned, line_start, line_end);
            if fence.is_some_and(|candidate| candidate.0 == marker && candidate.1 >= length) {
                in_fence = None;
            }
        } else if let Some(marker) = fence {
            mask_markdown_range(&mut cleaned, line_start, line_end);
            in_fence = Some(marker);
        } else if content_start.saturating_sub(line_start) >= 4
            || bytes.get(line_start) == Some(&b'\t')
        {
            mask_markdown_range(&mut cleaned, line_start, line_end);
        } else {
            let mut index = line_start;
            while index < line_end {
                if in_comment {
                    if bytes.get(index..index + 3) == Some(b"-->") {
                        mask_markdown_range(&mut cleaned, index, index + 3);
                        in_comment = false;
                        index += 3;
                    } else {
                        cleaned[index] = b' ';
                        index += 1;
                    }
                } else if bytes.get(index..index + 4) == Some(b"<!--") {
                    mask_markdown_range(&mut cleaned, index, index + 4);
                    in_comment = true;
                    index += 4;
                } else if bytes[index] == b'`' {
                    let run = repeated_byte(bytes, index, line_end, b'`');
                    let close = find_byte_run(bytes, index + run, line_end, b'`', run);
                    if let Some(close) = close {
                        let end = close + run;
                        mask_markdown_range(&mut cleaned, index, end);
                        index = end;
                    } else {
                        index += run;
                    }
                } else {
                    index += 1;
                }
            }
        }
        line_start = (line_end + 1).min(bytes.len());
    }
    String::from_utf8(cleaned).unwrap_or_else(|_| source.to_owned())
}

fn fence_marker(line: &[u8]) -> Option<(u8, usize)> {
    let marker = *line.first()?;
    if !matches!(marker, b'`' | b'~') {
        return None;
    }
    let length = repeated_byte(line, 0, line.len(), marker);
    (length >= 3).then_some((marker, length))
}

fn repeated_byte(bytes: &[u8], start: usize, end: usize, byte: u8) -> usize {
    let mut cursor = start;
    while cursor < end && bytes[cursor] == byte {
        cursor += 1;
    }
    cursor - start
}

fn find_byte_run(bytes: &[u8], start: usize, end: usize, byte: u8, length: usize) -> Option<usize> {
    let mut cursor = start;
    while cursor + length <= end {
        if repeated_byte(bytes, cursor, end, byte) >= length {
            return Some(cursor);
        }
        cursor += 1;
    }
    None
}

fn mask_markdown_range(cleaned: &mut [u8], start: usize, end: usize) {
    for byte in &mut cleaned[start..end] {
        if *byte != b'\n' {
            *byte = b' ';
        }
    }
}

fn explicit_status_value(line: &str) -> Option<String> {
    let mut line = line.trim();
    if let Some(rest) = line.strip_prefix("- ") {
        line = rest.trim_start();
    }
    let value = if line
        .get(.."**Status:**".len())
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("**Status:**"))
    {
        &line["**Status:**".len()..]
    } else if line
        .get(.."Status:".len())
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("Status:"))
    {
        &line["Status:".len()..]
    } else {
        return None;
    };
    let value = value.split(['.', ';']).next().unwrap_or(value).trim();
    Some(
        value
            .trim_matches(|character: char| {
                character.is_whitespace() || matches!(character, '*' | '`')
            })
            .to_owned(),
    )
}

fn markdown_heading_level(line: &str) -> Option<usize> {
    let level = line.bytes().take_while(|byte| *byte == b'#').count();
    (level > 0
        && level <= 6
        && line
            .as_bytes()
            .get(level)
            .is_some_and(u8::is_ascii_whitespace))
    .then_some(level)
}

fn markdown_table_cells(line: &str) -> Vec<String> {
    line.trim_matches('|')
        .split('|')
        .map(|cell| strip_markdown_status(cell.trim()))
        .collect()
}

fn strip_markdown_status(value: &str) -> String {
    value
        .trim()
        .trim_matches(|character: char| {
            character.is_whitespace() || matches!(character, '*' | '`' | '.')
        })
        .to_owned()
}

fn is_markdown_table_separator(value: &str) -> bool {
    let value = value.trim_matches(':');
    value.len() >= 3 && value.bytes().all(|byte| byte == b'-')
}

#[derive(Default)]
struct ProseBuffer {
    text: String,
    line_by_byte: Vec<usize>,
}

impl ProseBuffer {
    fn push(&mut self, line: &str, line_number: usize) {
        if !self.text.is_empty() {
            self.text.push(' ');
            self.line_by_byte.push(line_number);
        }
        self.text.push_str(line.trim());
        self.line_by_byte
            .extend(std::iter::repeat_n(line_number, line.trim().len()));
    }

    fn flush(&mut self, status: Option<DeliveryStatus>, violations: &mut Vec<Violation>) {
        if self.text.is_empty() {
            return;
        }
        let mut start = 0;
        for (index, character) in self.text.char_indices() {
            if matches!(character, '.' | '!' | '?') {
                self.check_range(start, index + character.len_utf8(), status, violations);
                start = index + character.len_utf8();
            }
        }
        if start < self.text.len() {
            self.check_range(start, self.text.len(), status, violations);
        }
        self.text.clear();
        self.line_by_byte.clear();
    }

    fn check_range(
        &self,
        start: usize,
        end: usize,
        status: Option<DeliveryStatus>,
        violations: &mut Vec<Violation>,
    ) {
        let sentence = &self.text[start..end];
        for (offset, verb) in unearned_claims(sentence, status) {
            let absolute = start + offset;
            let line = self.line_by_byte.get(absolute).copied().unwrap_or(1);
            violations.push(Violation::new(
                line,
                format!(
                    "claim word {verb:?} is applied to a planned deliverable without planned in the same sentence"
                ),
            ));
        }
    }
}

fn check_claim_unit(
    unit: &str,
    line: usize,
    status: Option<DeliveryStatus>,
    violations: &mut Vec<Violation>,
) {
    for (_, verb) in unearned_claims(unit, status) {
        violations.push(Violation::new(
            line,
            format!(
                "claim word {verb:?} is applied to a planned deliverable without planned in the same table row"
            ),
        ));
    }
}

fn unearned_claims(unit: &str, status: Option<DeliveryStatus>) -> Vec<(usize, &'static str)> {
    if status != Some(DeliveryStatus::Planned) {
        return Vec::new();
    }
    let lower = unit.to_ascii_lowercase();
    if contains_ascii_word(&lower, "planned") {
        return Vec::new();
    }
    let mut claims = Vec::new();
    for verb in ["supports", "guarantees", "provides"] {
        for (offset, _) in lower.match_indices(verb) {
            let end = offset + verb.len();
            if ascii_word_boundary(&lower, offset, end)
                && direct_claim_subject_before(&lower, offset)
                && !inside_quotation(unit, offset)
            {
                claims.push((offset, verb));
            }
        }
    }
    claims.sort_by_key(|claim| claim.0);
    claims
}

fn direct_claim_subject_before(value: &str, verb_offset: usize) -> bool {
    let mut words = value[..verb_offset]
        .split(|character: char| !character.is_ascii_alphanumeric() && character != '_')
        .filter(|word| !word.is_empty())
        .rev();
    let mut word = words.next();
    while word.is_some_and(|word| {
        matches!(
            word,
            "currently"
                | "now"
                | "already"
                | "also"
                | "directly"
                | "explicitly"
                | "fully"
                | "reliably"
                | "itself"
        )
    }) {
        word = words.next();
    }
    word.is_some_and(|word| matches!(word, "relay" | "service" | "product" | "system" | "server"))
}

fn contains_ascii_word(value: &str, word: &str) -> bool {
    value
        .match_indices(word)
        .any(|(start, _)| ascii_word_boundary(value, start, start + word.len()))
}

fn ascii_word_boundary(value: &str, start: usize, end: usize) -> bool {
    let before = value[..start].bytes().next_back();
    let after = value[end..].bytes().next();
    before.is_none_or(|byte| !(byte.is_ascii_alphanumeric() || byte == b'_'))
        && after.is_none_or(|byte| !(byte.is_ascii_alphanumeric() || byte == b'_'))
}

fn inside_quotation(value: &str, offset: usize) -> bool {
    value[..offset].bytes().filter(|byte| *byte == b'"').count() % 2 == 1
        || (value[..offset].matches('“').count() > value[..offset].matches('”').count())
}

fn validate_workspace_r0_06(root: &Path) -> Vec<Violation> {
    let gate_registry = root.join("ci/gates.toml");
    let mut violations =
        validate_document_tree(&root.join("docs"), Some(root), &[root.join("README.md")]);
    match read_file(&gate_registry) {
        Ok(gate_source) => violations.extend(qualify_violations(
            &gate_registry,
            validate_gates(&gate_source),
        )),
        Err(gate_violations) => violations.extend(gate_violations),
    }
    sort_violations(&mut violations);
    violations
}

fn validate_document_tree(
    docs_root: &Path,
    boundary: Option<&Path>,
    additional_documents: &[PathBuf],
) -> Vec<Violation> {
    let Some(boundary) = boundary else {
        return vec![Violation::new(
            1,
            format!(
                "{} line 1: documentation root has no containing boundary",
                docs_root.display()
            ),
        )];
    };
    let mut violations = Vec::new();
    let mut documents = Vec::new();
    collect_markdown_documents(docs_root, &mut documents, &mut violations);
    if documents.is_empty() {
        violations.push(Violation::new(
            1,
            format!(
                "{} line 1: documentation tree contains no Markdown files",
                docs_root.display()
            ),
        ));
    }
    documents.extend(additional_documents.iter().cloned());
    documents.sort();
    documents.dedup();

    for document in documents {
        let source = match read_document(&document) {
            Ok(source) => source,
            Err(violation) => {
                violations.push(violation);
                continue;
            }
        };
        violations.extend(qualify_violations(
            &document,
            validate_status_discipline(&source),
        ));
        violations.extend(validate_document_links(&document, &source, boundary));
    }
    sort_violations(&mut violations);
    violations
}

fn read_document(document: &Path) -> Result<String, Violation> {
    let metadata = fs::symlink_metadata(document).map_err(|error| {
        Violation::new(
            1,
            format!(
                "{} line 1: cannot inspect required documentation: {error}",
                document.display()
            ),
        )
    })?;
    if metadata.file_type().is_symlink() {
        return Err(Violation::new(
            1,
            format!(
                "{} line 1: required documentation must not be a symlink",
                document.display()
            ),
        ));
    }
    if !metadata.is_file() {
        return Err(Violation::new(
            1,
            format!(
                "{} line 1: required documentation is not a file",
                document.display()
            ),
        ));
    }
    let bytes = fs::read(document).map_err(|error| {
        Violation::new(
            1,
            format!(
                "{} line 1: cannot read required documentation: {error}",
                document.display()
            ),
        )
    })?;
    let source = String::from_utf8(bytes).map_err(|error| {
        Violation::new(
            1,
            format!(
                "{} line 1: documentation is not valid UTF-8: {error}",
                document.display()
            ),
        )
    })?;
    if source.trim().is_empty() {
        return Err(Violation::new(
            1,
            format!(
                "{} line 1: required documentation must not be empty",
                document.display()
            ),
        ));
    }
    Ok(source)
}

fn collect_markdown_documents(
    directory: &Path,
    documents: &mut Vec<PathBuf>,
    violations: &mut Vec<Violation>,
) {
    let metadata = match fs::symlink_metadata(directory) {
        Ok(metadata) => metadata,
        Err(error) => {
            violations.push(Violation::new(
                1,
                format!(
                    "{} line 1: cannot inspect documentation directory: {error}",
                    directory.display()
                ),
            ));
            return;
        }
    };
    if metadata.file_type().is_symlink() {
        violations.push(Violation::new(
            1,
            format!(
                "{} line 1: documentation symlink is not traversed",
                directory.display()
            ),
        ));
        return;
    }
    if !metadata.is_dir() {
        violations.push(Violation::new(
            1,
            format!(
                "{} line 1: documentation root is not a directory",
                directory.display()
            ),
        ));
        return;
    }
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) => {
            violations.push(Violation::new(
                1,
                format!(
                    "{} line 1: cannot read documentation directory: {error}",
                    directory.display()
                ),
            ));
            return;
        }
    };
    let mut entries: Vec<_> = entries.collect();
    entries.sort_by_key(|entry| {
        entry
            .as_ref()
            .map_or_else(|_| PathBuf::new(), std::fs::DirEntry::path)
    });
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                violations.push(Violation::new(
                    1,
                    format!(
                        "{} line 1: cannot read documentation entry: {error}",
                        directory.display()
                    ),
                ));
                continue;
            }
        };
        let path = entry.path();
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(error) => {
                violations.push(Violation::new(
                    1,
                    format!(
                        "{} line 1: cannot inspect documentation entry: {error}",
                        path.display()
                    ),
                ));
                continue;
            }
        };
        if file_type.is_symlink() {
            violations.push(Violation::new(
                1,
                format!(
                    "{} line 1: documentation symlink is not traversed",
                    path.display()
                ),
            ));
        } else if file_type.is_dir() {
            collect_markdown_documents(&path, documents, violations);
        } else if file_type.is_file() && path.extension().and_then(OsStr::to_str) == Some("md") {
            documents.push(path);
        }
    }
}

fn validate_document_links(document: &Path, source: &str, boundary: &Path) -> Vec<Violation> {
    let (links, extraction_violations) = markdown_links(source);
    let mut violations = qualify_violations(document, extraction_violations);
    let Some(parent) = document.parent() else {
        violations.push(Violation::new(
            1,
            format!(
                "{} line 1: documentation file has no containing directory",
                document.display()
            ),
        ));
        return violations;
    };
    let boundary = lexical_normalize(boundary);
    for link in links {
        let Some(target) = relative_link_path(&link.target) else {
            continue;
        };
        if target.contains('\\') {
            violations.push(Violation::new(
                link.line,
                format!(
                    "{} line {}: relative documentation link {target:?} contains a non-portable backslash",
                    document.display(),
                    link.line
                ),
            ));
            continue;
        }
        let candidate = lexical_normalize(&parent.join(target));
        if !candidate.starts_with(&boundary) {
            violations.push(Violation::new(
                link.line,
                format!(
                    "{} line {}: relative documentation link {target:?} escapes {}",
                    document.display(),
                    link.line,
                    boundary.display()
                ),
            ));
            continue;
        }
        match path_exists_with_exact_case(&boundary, &candidate) {
            Ok(true) => {}
            Ok(false) => violations.push(Violation::new(
                link.line,
                format!(
                    "{} line {}: relative documentation link {target:?} does not resolve",
                    document.display(),
                    link.line
                ),
            )),
            Err(message) => violations.push(Violation::new(
                link.line,
                format!("{} line {}: {message}", document.display(), link.line),
            )),
        }
    }
    violations
}

fn lexical_normalize(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            std::path::Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            std::path::Component::RootDir => normalized.push(Path::new("/")),
            std::path::Component::Normal(component) => normalized.push(component),
        }
    }
    normalized
}

fn path_exists_with_exact_case(boundary: &Path, candidate: &Path) -> Result<bool, String> {
    let relative = candidate.strip_prefix(boundary).map_err(|_| {
        format!(
            "relative documentation link target {} escapes {}",
            candidate.display(),
            boundary.display()
        )
    })?;
    let mut cursor = boundary.to_path_buf();
    if fs::symlink_metadata(&cursor)
        .map_err(|error| format!("cannot inspect documentation link boundary: {error}"))?
        .file_type()
        .is_symlink()
    {
        return Err("documentation link boundary must not be a symlink".to_owned());
    }
    for component in relative.components() {
        let std::path::Component::Normal(expected) = component else {
            return Ok(false);
        };
        let entries = fs::read_dir(&cursor).map_err(|error| {
            format!(
                "cannot inspect documentation link directory {}: {error}",
                cursor.display()
            )
        })?;
        let mut found = None;
        for entry in entries {
            let entry = entry.map_err(|error| {
                format!(
                    "cannot read documentation link directory {}: {error}",
                    cursor.display()
                )
            })?;
            if entry.file_name() == expected {
                found = Some(entry.path());
                break;
            }
        }
        let Some(path) = found else {
            return Ok(false);
        };
        let metadata = fs::symlink_metadata(&path).map_err(|error| {
            format!(
                "cannot inspect documentation link target {}: {error}",
                path.display()
            )
        })?;
        if metadata.file_type().is_symlink() {
            return Err(format!(
                "documentation link target {} traverses a symlink",
                path.display()
            ));
        }
        cursor = path;
    }
    Ok(cursor.is_file() || cursor.is_dir())
}

fn sort_violations(violations: &mut [Violation]) {
    violations.sort_by(|left, right| {
        left.message
            .cmp(&right.message)
            .then(left.line.cmp(&right.line))
    });
}

fn validate_workspace_sources(config: &ArchConfig, metadata: &WorkspaceMetadata) -> Vec<Violation> {
    let mut roots = BTreeMap::new();
    for crate_name in config.crates.keys() {
        let Some(package) = metadata.packages.get(crate_name) else {
            continue;
        };
        let Some(package_root) = package.manifest_path.parent() else {
            roots.insert(crate_name.clone(), PathBuf::new());
            continue;
        };
        roots.insert(crate_name.clone(), package_root.join("src"));
    }
    validate_source_roots(config, &roots)
}

fn validate_source_roots(config: &ArchConfig, roots: &BTreeMap<String, PathBuf>) -> Vec<Violation> {
    let mut violations = Vec::new();
    for (crate_name, policy) in &config.crates {
        let Some(source_root) = roots.get(crate_name) else {
            violations.push(Violation::new(
                1,
                format!("configured crate {crate_name} source directory is missing"),
            ));
            continue;
        };
        if !source_root.is_dir() {
            violations.push(Violation::new(
                1,
                format!(
                    "configured crate {crate_name} source directory {} is missing",
                    source_root.display()
                ),
            ));
            continue;
        }
        let mut source_files = Vec::new();
        collect_rust_sources(source_root, &mut source_files, &mut violations);
        source_files.sort();
        if source_files.is_empty() {
            violations.push(Violation::new(
                1,
                format!(
                    "configured crate {crate_name} source directory {} has no Rust source files",
                    source_root.display()
                ),
            ));
            continue;
        }
        let mut sources = BTreeMap::new();
        for source_path in &source_files {
            if let Some(source) = read_configured_source(source_path, &mut violations) {
                sources.insert(source_path.clone(), source);
            }
        }
        let exclusions = source_exclusions(&sources);
        let mut pending: BTreeSet<PathBuf> = source_files
            .into_iter()
            .filter(|path| !exclusions.contains(path))
            .collect();
        let mut visited = BTreeSet::new();
        while let Some(source_path) = pending.pop_first() {
            if !visited.insert(source_path.clone()) {
                continue;
            }
            let source = if let Some(source) = sources.get(&source_path) {
                source.clone()
            } else if let Some(source) = read_configured_source(&source_path, &mut violations) {
                sources.insert(source_path.clone(), source.clone());
                source
            } else {
                continue;
            };
            violations.extend(qualify_violations(
                &source_path,
                scan_source(&source, &policy.forbidden_tokens),
            ));
            let (includes, include_violations) =
                direct_include_sources(&source_path, source_root, &source);
            violations.extend(qualify_violations(&source_path, include_violations));
            pending.extend(includes.into_iter().filter(|path| !visited.contains(path)));
        }
    }
    violations
}

fn read_configured_source(path: &Path, violations: &mut Vec<Violation>) -> Option<String> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) => {
            violations.push(Violation::new(
                1,
                format!(
                    "{} line 1: cannot read configured source: {error}",
                    path.display()
                ),
            ));
            return None;
        }
    };
    match String::from_utf8(bytes) {
        Ok(source) => Some(source),
        Err(error) => {
            violations.push(Violation::new(
                1,
                format!(
                    "{} line 1: configured source is not valid UTF-8: {error}",
                    path.display()
                ),
            ));
            None
        }
    }
}

#[derive(Default)]
struct SourceExclusions {
    files: BTreeSet<PathBuf>,
    directories: BTreeSet<PathBuf>,
}

impl SourceExclusions {
    fn contains(&self, path: &Path) -> bool {
        self.files.contains(path)
            || self
                .directories
                .iter()
                .any(|directory| path.starts_with(directory))
    }
}

#[derive(Clone, Debug)]
struct ModuleSourceReference {
    files: [PathBuf; 2],
    directory: PathBuf,
    test_only: bool,
}

fn source_exclusions(sources: &BTreeMap<PathBuf, String>) -> SourceExclusions {
    let references: Vec<_> = sources
        .iter()
        .flat_map(|(path, source)| out_of_line_modules(path, source))
        .collect();
    let mut production_files = BTreeSet::new();
    let mut production_directories = BTreeSet::new();
    for reference in references.iter().filter(|reference| !reference.test_only) {
        production_files.extend(reference.files.iter().cloned());
        production_directories.insert(reference.directory.clone());
    }
    let mut exclusions = SourceExclusions::default();
    for reference in references.iter().filter(|reference| reference.test_only) {
        for file in &reference.files {
            if !production_files.contains(file) {
                exclusions.files.insert(file.clone());
            }
        }
        if !production_directories.contains(&reference.directory) {
            exclusions.directories.insert(reference.directory.clone());
        }
    }
    exclusions
}

fn out_of_line_modules(source_path: &Path, source: &str) -> Vec<ModuleSourceReference> {
    let (cleaned, syntax) = lex_rust(source);
    let tokens = rust_tokens(&cleaned, &syntax);
    let mut references = Vec::new();
    let mut index = 0;
    while index < tokens.len() {
        let item_start = index;
        let mut test_only = false;
        while tokens.get(index).is_some_and(|token| token.text == "#") {
            let Some(attribute_end) = token_delimiter_end(&tokens, index + 1, "[", "]") else {
                break;
            };
            if is_cfg_test_attribute(&tokens[index..=attribute_end]) {
                test_only = true;
            }
            index = attribute_end + 1;
        }
        if tokens.get(index).is_some_and(|token| token.text == "pub") {
            index += 1;
            if tokens.get(index).is_some_and(|token| token.text == "(") {
                index = token_delimiter_end(&tokens, index, "(", ")").map_or(index, |end| end + 1);
            }
        }
        let is_module = tokens.get(index).is_some_and(|token| token.text == "mod")
            && tokens
                .get(index + 1)
                .is_some_and(|token| is_rust_identifier(&token.text))
            && tokens.get(index + 2).is_some_and(|token| token.text == ";");
        if is_module {
            references.push(module_source_reference(
                source_path,
                &tokens[index + 1].text,
                test_only,
            ));
            index += 3;
        } else {
            index = item_start + 1;
        }
    }
    references
}

fn token_delimiter_end(
    tokens: &[RustToken],
    start: usize,
    open: &str,
    close: &str,
) -> Option<usize> {
    if tokens.get(start).is_none_or(|token| token.text != open) {
        return None;
    }
    let mut depth = 0_u32;
    for (index, token) in tokens.iter().enumerate().skip(start) {
        if token.text == open {
            depth += 1;
        } else if token.text == close {
            depth = depth.checked_sub(1)?;
            if depth == 0 {
                return Some(index);
            }
        }
    }
    None
}

fn is_cfg_test_attribute(tokens: &[RustToken]) -> bool {
    let expected = ["#", "[", "cfg", "(", "test", ")", "]"];
    tokens.len() == expected.len()
        && tokens
            .iter()
            .zip(expected)
            .all(|(token, expected)| token.text == expected)
}

fn module_source_reference(
    source_path: &Path,
    module: &str,
    test_only: bool,
) -> ModuleSourceReference {
    let parent = source_path.parent().unwrap_or_else(|| Path::new(""));
    let file_name = source_path.file_name().and_then(OsStr::to_str);
    let module_base = if matches!(file_name, Some("lib.rs" | "main.rs" | "mod.rs")) {
        parent.to_path_buf()
    } else {
        parent.join(source_path.file_stem().unwrap_or_default())
    };
    let directory = module_base.join(module);
    ModuleSourceReference {
        files: [
            module_base.join(format!("{module}.rs")),
            directory.join("mod.rs"),
        ],
        directory,
        test_only,
    }
}

fn direct_include_sources(
    source_path: &Path,
    source_root: &Path,
    source: &str,
) -> (Vec<PathBuf>, Vec<Violation>) {
    let (mut cleaned, syntax) = lex_rust(source);
    mask_cfg_test_items(&mut cleaned, &syntax);
    let tokens = rust_tokens(&cleaned, &syntax);
    let mut includes = Vec::new();
    let mut violations = Vec::new();
    for window in tokens.windows(3) {
        if window[0].text != "include"
            || window[1].text != "!"
            || !matches!(window[2].text.as_str(), "(" | "{" | "[")
        {
            continue;
        }
        let argument_start = window[2].offset + window[2].text.len();
        let Some(argument_start) = next_non_trivia(&cleaned, argument_start) else {
            violations.push(Violation::new(
                window[0].line,
                "include! macro is missing a statically resolvable source path",
            ));
            continue;
        };
        let Some((relative, _literal_end)) = rust_string_literal(source.as_bytes(), argument_start)
        else {
            violations.push(Violation::new(
                window[0].line,
                "include! source path must be a direct UTF-8 string literal",
            ));
            continue;
        };
        let parent = source_path.parent().unwrap_or_else(|| Path::new(""));
        let resolved = lexical_normalize(&parent.join(relative));
        let boundary = lexical_normalize(source_root);
        if !resolved.starts_with(&boundary) {
            violations.push(Violation::new(
                window[0].line,
                format!(
                    "include! source {} escapes configured source root {}",
                    resolved.display(),
                    boundary.display()
                ),
            ));
            continue;
        }
        includes.push(resolved);
    }
    includes.sort();
    includes.dedup();
    (includes, violations)
}

fn next_non_trivia(source: &[u8], mut index: usize) -> Option<usize> {
    while source.get(index).is_some_and(u8::is_ascii_whitespace) {
        index += 1;
    }
    (index < source.len()).then_some(index)
}

fn rust_string_literal(bytes: &[u8], start: usize) -> Option<(String, usize)> {
    if bytes.get(start) == Some(&b'"') {
        let end = quoted_literal_end(bytes, start, b'"');
        if end <= start + 1 || bytes.get(end - 1) != Some(&b'"') {
            return None;
        }
        return decode_rust_string(&bytes[start + 1..end - 1]).map(|value| (value, end));
    }
    if bytes.get(start) != Some(&b'r') {
        return None;
    }
    let mut quote = start + 1;
    while bytes.get(quote) == Some(&b'#') {
        quote += 1;
    }
    if bytes.get(quote) != Some(&b'"') {
        return None;
    }
    let end = rust_literal_end(bytes, start)?;
    let hashes = quote - start - 1;
    let content_end = end.checked_sub(hashes + 1)?;
    let value = std::str::from_utf8(bytes.get(quote + 1..content_end)?)
        .ok()?
        .to_owned();
    Some((value, end))
}

fn decode_rust_string(bytes: &[u8]) -> Option<String> {
    let mut value = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != b'\\' {
            value.push(bytes[index]);
            index += 1;
            continue;
        }
        let escape = *bytes.get(index + 1)?;
        match escape {
            b'\\' | b'"' => value.push(escape),
            b'n' => value.push(b'\n'),
            b'r' => value.push(b'\r'),
            b't' => value.push(b'\t'),
            b'0' => value.push(0),
            _ => return None,
        }
        index += 2;
    }
    String::from_utf8(value).ok()
}

fn collect_rust_sources(
    directory: &Path,
    sources: &mut Vec<PathBuf>,
    violations: &mut Vec<Violation>,
) {
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) => {
            violations.push(Violation::new(
                1,
                format!(
                    "{} line 1: cannot read configured source directory: {error}",
                    directory.display()
                ),
            ));
            return;
        }
    };
    let mut entries: Vec<_> = entries.collect();
    entries.sort_by_key(|entry| {
        entry
            .as_ref()
            .map_or_else(|_| PathBuf::new(), std::fs::DirEntry::path)
    });
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                violations.push(Violation::new(
                    1,
                    format!(
                        "{} line 1: cannot read configured source entry: {error}",
                        directory.display()
                    ),
                ));
                continue;
            }
        };
        let path = entry.path();
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(error) => {
                violations.push(Violation::new(
                    1,
                    format!(
                        "{} line 1: cannot inspect configured source: {error}",
                        path.display()
                    ),
                ));
                continue;
            }
        };
        if file_type.is_symlink() {
            violations.push(Violation::new(
                1,
                format!(
                    "{} line 1: configured source symlinks are not traversed",
                    path.display()
                ),
            ));
        } else if file_type.is_dir() {
            collect_rust_sources(&path, sources, violations);
        } else if file_type.is_file() && path.extension().and_then(OsStr::to_str) == Some("rs") {
            sources.push(path);
        }
    }
}

fn lex_rust(source: &str) -> (Vec<u8>, Vec<bool>) {
    let bytes = source.as_bytes();
    let mut cleaned = bytes.to_vec();
    let mut syntax = vec![true; bytes.len()];
    let mut index = 0;
    let mut block_depth = 0_u32;
    while index < bytes.len() {
        if block_depth > 0 {
            syntax[index] = false;
            if bytes.get(index..index + 2) == Some(b"/*") {
                cleaned[index] = b' ';
                cleaned[index + 1] = b' ';
                syntax[index + 1] = false;
                block_depth += 1;
                index += 2;
            } else if bytes.get(index..index + 2) == Some(b"*/") {
                cleaned[index] = b' ';
                cleaned[index + 1] = b' ';
                syntax[index + 1] = false;
                block_depth -= 1;
                index += 2;
            } else {
                if cleaned[index] != b'\n' {
                    cleaned[index] = b' ';
                }
                index += 1;
            }
            continue;
        }
        if bytes.get(index..index + 2) == Some(b"//") {
            while index < bytes.len() && bytes[index] != b'\n' {
                cleaned[index] = b' ';
                syntax[index] = false;
                index += 1;
            }
            continue;
        }
        if bytes.get(index..index + 2) == Some(b"/*") {
            cleaned[index] = b' ';
            cleaned[index + 1] = b' ';
            syntax[index] = false;
            syntax[index + 1] = false;
            block_depth = 1;
            index += 2;
            continue;
        }
        if let Some(end) = rust_literal_end(bytes, index) {
            for item in syntax.iter_mut().take(end).skip(index) {
                *item = false;
            }
            index = end;
            continue;
        }
        index += 1;
    }
    (cleaned, syntax)
}

fn rust_literal_end(bytes: &[u8], start: usize) -> Option<usize> {
    let mut quote_index = start;
    if matches!(bytes.get(start), Some(b'b' | b'c')) && bytes.get(start + 1) == Some(&b'"') {
        quote_index += 1;
    }
    if bytes.get(quote_index) == Some(&b'"') {
        return Some(quoted_literal_end(bytes, quote_index, b'"'));
    }
    if bytes.get(quote_index) == Some(&b'\'') {
        return character_literal_end(bytes, quote_index);
    }
    if bytes.get(start) == Some(&b'b') && bytes.get(start + 1) == Some(&b'\'') {
        return character_literal_end(bytes, start + 1);
    }
    let raw_start = if bytes.get(start) == Some(&b'r') {
        start
    } else if matches!(bytes.get(start), Some(b'b' | b'c')) && bytes.get(start + 1) == Some(&b'r') {
        start + 1
    } else {
        return None;
    };
    let mut index = raw_start + 1;
    let mut hashes = 0;
    while bytes.get(index) == Some(&b'#') {
        hashes += 1;
        index += 1;
    }
    if bytes.get(index) != Some(&b'"') {
        return None;
    }
    index += 1;
    while index < bytes.len() {
        if bytes[index] == b'"'
            && bytes.get(index + 1..index + 1 + hashes) == Some(&vec![b'#'; hashes])
        {
            return Some(index + 1 + hashes);
        }
        index += 1;
    }
    Some(bytes.len())
}

fn quoted_literal_end(bytes: &[u8], quote_index: usize, quote: u8) -> usize {
    let mut index = quote_index + 1;
    while index < bytes.len() {
        if bytes[index] == b'\\' {
            index = (index + 2).min(bytes.len());
        } else if bytes[index] == quote {
            return index + 1;
        } else {
            index += 1;
        }
    }
    bytes.len()
}

fn character_literal_end(bytes: &[u8], quote_index: usize) -> Option<usize> {
    let value_start = quote_index + 1;
    let value_end = if bytes.get(value_start) == Some(&b'\\') {
        escaped_character_end(bytes, value_start)?
    } else {
        let remainder = std::str::from_utf8(bytes.get(value_start..)?).ok()?;
        let character = remainder.chars().next()?;
        if matches!(character, '\'' | '\n' | '\r') {
            return None;
        }
        value_start + character.len_utf8()
    };
    (bytes.get(value_end) == Some(&b'\'')).then_some(value_end + 1)
}

fn escaped_character_end(bytes: &[u8], slash: usize) -> Option<usize> {
    let escape = *bytes.get(slash + 1)?;
    match escape {
        b'x' => {
            bytes.get(slash + 2..slash + 4)?;
            Some(slash + 4)
        }
        b'u' if bytes.get(slash + 2) == Some(&b'{') => {
            let mut index = slash + 3;
            while index < bytes.len() && bytes[index] != b'}' {
                if bytes[index] == b'\n' {
                    return None;
                }
                index += 1;
            }
            (bytes.get(index) == Some(&b'}')).then_some(index + 1)
        }
        b'\n' | b'\r' => None,
        _ => Some(slash + 2),
    }
}

fn mask_cfg_test_items(cleaned: &mut [u8], syntax: &[bool]) {
    let mut index = 0;
    while index + 2 < cleaned.len() {
        if cleaned[index] != b'#' || cleaned[index + 1] != b'[' || !syntax[index] {
            index += 1;
            continue;
        }
        let Some(attribute_end) = matching_delimiter(cleaned, syntax, index + 1, b'[', b']') else {
            index += 1;
            continue;
        };
        let canonical: String = cleaned[index + 2..attribute_end]
            .iter()
            .filter(|byte| !byte.is_ascii_whitespace())
            .map(|byte| char::from(*byte))
            .collect();
        if canonical != "cfg(test)" {
            index = attribute_end + 1;
            continue;
        }
        let mut item_start = attribute_end + 1;
        loop {
            while cleaned.get(item_start).is_some_and(u8::is_ascii_whitespace) {
                item_start += 1;
            }
            if cleaned.get(item_start..item_start + 2) != Some(b"#[") {
                break;
            }
            let Some(next_end) = matching_delimiter(cleaned, syntax, item_start + 1, b'[', b']')
            else {
                break;
            };
            item_start = next_end + 1;
        }
        let mut cursor = item_start;
        let mut end = cleaned.len();
        while cursor < cleaned.len() {
            if syntax[cursor] && cleaned[cursor] == b'{' {
                end = matching_delimiter(cleaned, syntax, cursor, b'{', b'}')
                    .map_or(cleaned.len(), |position| position + 1);
                break;
            }
            if syntax[cursor] && cleaned[cursor] == b';' {
                end = cursor + 1;
                break;
            }
            cursor += 1;
        }
        for byte in &mut cleaned[index..end] {
            if *byte != b'\n' {
                *byte = b' ';
            }
        }
        index = end;
    }
}

fn matching_delimiter(
    bytes: &[u8],
    syntax: &[bool],
    start: usize,
    open: u8,
    close: u8,
) -> Option<usize> {
    let mut depth = 0_u32;
    for index in start..bytes.len() {
        if !syntax[index] {
            continue;
        }
        if bytes[index] == open {
            depth += 1;
        } else if bytes[index] == close {
            depth = depth.checked_sub(1)?;
            if depth == 0 {
                return Some(index);
            }
        }
    }
    None
}

fn read_file(path: &Path) -> Result<String, Vec<Violation>> {
    fs::read_to_string(path).map_err(|error| {
        vec![Violation::new(
            1,
            format!(
                "{} line 1: cannot read required input: {error}",
                path.display()
            ),
        )]
    })
}

fn qualify_violations(path: &Path, violations: Vec<Violation>) -> Vec<Violation> {
    violations
        .into_iter()
        .map(|violation| {
            Violation::new(
                violation.line,
                format!(
                    "{} line {}: {}",
                    path.display(),
                    violation.line,
                    violation.message
                ),
            )
        })
        .collect()
}

#[derive(Clone, Debug)]
struct ParsedRequirement {
    version: Option<String>,
    inherited: bool,
}

#[derive(Clone, Debug)]
struct TableRequirement {
    line: usize,
    version_seen: bool,
    inherited: bool,
}

impl TableRequirement {
    fn new(line: usize) -> Self {
        Self {
            line,
            version_seen: false,
            inherited: false,
        }
    }
}

#[derive(Clone, Debug)]
enum DependencyAssignment {
    Direct(String),
    Field { dependency: String, field: String },
}

impl DependencyAssignment {
    fn dependency(&self) -> &str {
        match self {
            Self::Direct(dependency) | Self::Field { dependency, .. } => dependency,
        }
    }
}

fn process_dependency_assignment(
    assignment: DependencyAssignment,
    value: &str,
    line: usize,
    table_requirements: &mut BTreeMap<String, TableRequirement>,
    violations: &mut Vec<Violation>,
) {
    match assignment {
        DependencyAssignment::Direct(dependency) => match parse_direct_requirement(value) {
            Ok(requirement) => {
                validate_parsed_requirement(&dependency, line, &requirement, violations);
            }
            Err(message) => violations.push(Violation::new(
                line,
                format!("dependency {dependency}: {message}"),
            )),
        },
        DependencyAssignment::Field { dependency, field } => {
            let requirement = table_requirements
                .entry(dependency.clone())
                .or_insert_with(|| TableRequirement::new(line));
            match field.as_str() {
                "version" => {
                    requirement.version_seen = true;
                    match parse_toml_string(value.trim()) {
                        Ok(version) if !is_exact_version(&version) => {
                            violations.push(inexact_requirement(&dependency, line, &version));
                        }
                        Ok(_) => {}
                        Err(message) => violations.push(Violation::new(
                            line,
                            format!("dependency {dependency}: {message}"),
                        )),
                    }
                }
                "workspace" => match parse_toml_bool(value.trim()) {
                    Ok(inherited) => requirement.inherited = inherited,
                    Err(message) => violations.push(Violation::new(
                        line,
                        format!("dependency {dependency}: {message}"),
                    )),
                },
                _ => {}
            }
        }
    }
}

fn parse_direct_requirement(value: &str) -> Result<ParsedRequirement, String> {
    let value = value.trim();
    if value.starts_with('"') || value.starts_with('\'') {
        return parse_toml_string(value).map(|version| ParsedRequirement {
            version: Some(version),
            inherited: false,
        });
    }
    if value.starts_with('{') && value.ends_with('}') {
        let fields = split_top_level(&value[1..value.len() - 1], ',')?;
        let mut version = None;
        let mut inherited = false;
        for field in fields {
            if field.trim().is_empty() {
                continue;
            }
            let Some((key, field_value)) = split_assignment(field.trim()) else {
                return Err("malformed inline dependency table".to_owned());
            };
            match key.trim() {
                "version" => version = Some(parse_toml_string(field_value.trim())?),
                "workspace" => inherited = parse_toml_bool(field_value.trim())?,
                _ => {}
            }
        }
        return Ok(ParsedRequirement { version, inherited });
    }
    Err("requirement must be a string or inline table".to_owned())
}

fn parse_toml_bool(value: &str) -> Result<bool, String> {
    match value {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err("workspace must be a boolean".to_owned()),
    }
}

fn validate_parsed_requirement(
    dependency: &str,
    line: usize,
    requirement: &ParsedRequirement,
    violations: &mut Vec<Violation>,
) {
    match &requirement.version {
        Some(version) if !is_exact_version(version) => {
            violations.push(inexact_requirement(dependency, line, version));
        }
        Some(_) => {}
        None if requirement.inherited => {}
        None => violations.push(missing_requirement(dependency, line)),
    }
}

fn inexact_requirement(dependency: &str, line: usize, version: &str) -> Violation {
    Violation::new(
        line,
        format!("dependency {dependency} must use an exact =x.y.z requirement, found {version:?}"),
    )
}

fn missing_requirement(dependency: &str, line: usize) -> Violation {
    Violation::new(
        line,
        format!(
            "dependency {dependency} is missing an exact version; only workspace = true may inherit one"
        ),
    )
}

fn is_exact_version(requirement: &str) -> bool {
    let Some(version) = requirement.strip_prefix('=') else {
        return false;
    };
    if version.is_empty() || version.contains([',', '*', '^', '~', '<', '>', '=']) {
        return false;
    }
    let core = version
        .split_once(['-', '+'])
        .map_or(version, |(numeric, _)| numeric);
    let mut parts = core.split('.');
    let valid_number = |part: &str| {
        !part.is_empty()
            && part.bytes().all(|byte| byte.is_ascii_digit())
            && (part == "0" || !part.starts_with('0'))
    };
    matches!(
        (parts.next(), parts.next(), parts.next(), parts.next()),
        (Some(major), Some(minor), Some(patch), None)
            if valid_number(major) && valid_number(minor) && valid_number(patch)
    )
}

fn dependency_assignment(section: &str, key: &str) -> Option<DependencyAssignment> {
    let components = toml_table_components(section);
    let dependency_index = components.iter().position(|component| {
        matches!(
            component.as_str(),
            "dependencies" | "dev-dependencies" | "build-dependencies"
        )
    })?;
    if dependency_index + 1 < components.len() {
        Some(DependencyAssignment::Field {
            dependency: components[dependency_index + 1].clone(),
            field: unquote_toml_key(key),
        })
    } else {
        let key_components = toml_dotted_key_components(key);
        let dependency = key_components.first()?.clone();
        if key_components.len() == 1 {
            Some(DependencyAssignment::Direct(dependency))
        } else {
            Some(DependencyAssignment::Field {
                dependency,
                field: key_components[1].clone(),
            })
        }
    }
}

fn dependency_table_name(section: &str) -> Option<String> {
    let components = toml_table_components(section);
    let dependency_index = components.iter().position(|component| {
        matches!(
            component.as_str(),
            "dependencies" | "dev-dependencies" | "build-dependencies"
        )
    })?;
    components.get(dependency_index + 1).cloned()
}

fn toml_dotted_key_components(key: &str) -> Vec<String> {
    let mut components = Vec::new();
    let mut start = 0;
    let mut quote = None;
    let mut escaped = false;
    for (index, character) in key.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if quote == Some('"') && character == '\\' {
            escaped = true;
        } else if matches!(character, '\'' | '"') {
            if quote == Some(character) {
                quote = None;
            } else if quote.is_none() {
                quote = Some(character);
            }
        } else if character == '.' && quote.is_none() {
            components.push(unquote_toml_key(&key[start..index]));
            start = index + 1;
        }
    }
    components.push(unquote_toml_key(&key[start..]));
    components
}

fn toml_table_components(section: &str) -> Vec<String> {
    section
        .split('.')
        .map(|component| unquote_toml_key(component.trim()))
        .collect()
}

fn unquote_toml_key(key: &str) -> String {
    let key = key.trim();
    if key.len() >= 2
        && ((key.starts_with('"') && key.ends_with('"'))
            || (key.starts_with('\'') && key.ends_with('\'')))
    {
        key[1..key.len() - 1].to_owned()
    } else {
        key.to_owned()
    }
}

fn strip_toml_comment(line: &str) -> &str {
    let mut quote = None;
    let mut escaped = false;
    for (index, character) in line.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if quote == Some('"') && character == '\\' {
            escaped = true;
        } else if matches!(character, '\'' | '"') {
            if quote == Some(character) {
                quote = None;
            } else if quote.is_none() {
                quote = Some(character);
            }
        } else if character == '#' && quote.is_none() {
            return &line[..index];
        }
    }
    line
}

fn parse_table_header(line: &str) -> Option<String> {
    if !line.starts_with('[') || line.starts_with("[[") || line.len() < 2 || !line.ends_with(']') {
        return None;
    }
    let name = line[1..line.len() - 1].trim();
    (!name.is_empty()).then(|| name.to_owned())
}

fn split_assignment(line: &str) -> Option<(&str, String)> {
    let mut quote = None;
    let mut escaped = false;
    for (index, character) in line.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if quote == Some('"') && character == '\\' {
            escaped = true;
        } else if matches!(character, '\'' | '"') {
            if quote == Some(character) {
                quote = None;
            } else if quote.is_none() {
                quote = Some(character);
            }
        } else if character == '=' && quote.is_none() {
            return Some((&line[..index], line[index + 1..].to_owned()));
        }
    }
    None
}

fn delimiters_unbalanced(value: &str) -> bool {
    let mut square = 0_i32;
    let mut curly = 0_i32;
    let mut quote = None;
    let mut escaped = false;
    for character in value.chars() {
        if escaped {
            escaped = false;
            continue;
        }
        if quote == Some('"') && character == '\\' {
            escaped = true;
            continue;
        }
        if matches!(character, '\'' | '"') {
            if quote == Some(character) {
                quote = None;
            } else if quote.is_none() {
                quote = Some(character);
            }
        } else if quote.is_none() {
            match character {
                '[' => square += 1,
                ']' => square -= 1,
                '{' => curly += 1,
                '}' => curly -= 1,
                _ => {}
            }
        }
    }
    quote.is_some() || square != 0 || curly != 0
}

fn parse_toml_string(value: &str) -> Result<String, String> {
    let value = value.trim();
    if value.len() < 2 {
        return Err("expected a quoted string".to_owned());
    }
    let quote = value.as_bytes()[0];
    if !matches!(quote, b'\'' | b'"') || value.as_bytes()[value.len() - 1] != quote {
        return Err("expected one complete quoted string".to_owned());
    }
    let inner = &value[1..value.len() - 1];
    if inner.contains(['\n', '\r']) {
        return Err("single-line TOML strings must not contain raw newlines".to_owned());
    }
    if quote == b'\'' {
        return Ok(inner.to_owned());
    }
    let mut parsed = String::new();
    let mut characters = inner.chars();
    while let Some(character) = characters.next() {
        if character != '\\' {
            parsed.push(character);
            continue;
        }
        let escaped = characters
            .next()
            .ok_or_else(|| "unterminated string escape".to_owned())?;
        match escaped {
            '"' => parsed.push('"'),
            '\\' => parsed.push('\\'),
            'n' => parsed.push('\n'),
            'r' => parsed.push('\r'),
            't' => parsed.push('\t'),
            _ => return Err(format!("unsupported TOML escape \\{escaped}")),
        }
    }
    Ok(parsed)
}

fn parse_string_array(value: &str) -> Result<Vec<String>, String> {
    let value = value.trim();
    if !value.starts_with('[') || !value.ends_with(']') {
        return Err("policy values must be arrays of strings".to_owned());
    }
    let inner = &value[1..value.len() - 1];
    if inner.trim().is_empty() {
        return Ok(Vec::new());
    }
    let entries = split_top_level(inner, ',')?;
    let mut parsed = Vec::new();
    let mut seen = BTreeSet::new();
    let entry_count = entries.len();
    for (index, entry) in entries.into_iter().enumerate() {
        if entry.trim().is_empty() {
            if index + 1 == entry_count && inner.trim_end().ends_with(',') {
                continue;
            }
            return Err("policy array contains an empty array entry".to_owned());
        }
        let item = parse_toml_string(entry.trim())?;
        if item.is_empty() {
            return Err("policy array entries cannot be empty".to_owned());
        }
        if !seen.insert(item.clone()) {
            return Err(format!("duplicate policy array entry {item:?}"));
        }
        parsed.push(item);
    }
    Ok(parsed)
}

fn split_top_level(value: &str, delimiter: char) -> Result<Vec<&str>, String> {
    let mut entries = Vec::new();
    let mut start = 0;
    let mut quote = None;
    let mut escaped = false;
    let mut square = 0_i32;
    let mut curly = 0_i32;
    for (index, character) in value.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if quote == Some('"') && character == '\\' {
            escaped = true;
            continue;
        }
        if matches!(character, '\'' | '"') {
            if quote == Some(character) {
                quote = None;
            } else if quote.is_none() {
                quote = Some(character);
            }
            continue;
        }
        if quote.is_none() {
            match character {
                '[' => square += 1,
                ']' => square -= 1,
                '{' => curly += 1,
                '}' => curly -= 1,
                _ if character == delimiter && square == 0 && curly == 0 => {
                    entries.push(&value[start..index]);
                    start = index + character.len_utf8();
                }
                _ => {}
            }
            if square < 0 || curly < 0 {
                return Err("unbalanced delimiters".to_owned());
            }
        }
    }
    if quote.is_some() || square != 0 || curly != 0 {
        return Err("unbalanced delimiters or quotes".to_owned());
    }
    entries.push(&value[start..]);
    Ok(entries)
}

#[derive(Clone, Debug)]
struct PolicyBuilder {
    line: usize,
    allowed_deps: Option<Vec<String>>,
    allowed_dev_deps: Option<Vec<String>>,
    allowed_build_deps: Option<Vec<String>>,
    forbidden_deps: Option<Vec<String>>,
    forbidden_tokens: Option<Vec<String>>,
}

impl PolicyBuilder {
    fn new(line: usize) -> Self {
        Self {
            line,
            allowed_deps: None,
            allowed_dev_deps: None,
            allowed_build_deps: None,
            forbidden_deps: None,
            forbidden_tokens: None,
        }
    }

    fn set(
        &mut self,
        key: &str,
        values: Vec<String>,
        line: usize,
        violations: &mut Vec<Violation>,
    ) {
        let slot = match key {
            "allowed-deps" => &mut self.allowed_deps,
            "allowed-dev-deps" => &mut self.allowed_dev_deps,
            "allowed-build-deps" => &mut self.allowed_build_deps,
            "forbidden-deps" => &mut self.forbidden_deps,
            "forbidden-tokens" => &mut self.forbidden_tokens,
            _ => {
                violations.push(Violation::new(
                    line,
                    format!("unknown arch.toml policy field {key}"),
                ));
                return;
            }
        };
        if slot.replace(values).is_some() {
            violations.push(Violation::new(
                line,
                format!("duplicate arch.toml policy field {key}"),
            ));
        }
    }

    fn finish(self, name: &str, violations: &mut Vec<Violation>) -> Option<CratePolicy> {
        let Some(allowed_deps) = self.allowed_deps else {
            violations.push(Violation::new(
                self.line,
                format!("crate policy {name} is missing allowed-deps"),
            ));
            return None;
        };
        let Some(forbidden_tokens) = self.forbidden_tokens else {
            violations.push(Violation::new(
                self.line,
                format!("crate policy {name} is missing forbidden-tokens"),
            ));
            return None;
        };
        let allowed_deps: BTreeSet<String> = allowed_deps.into_iter().collect();
        let allowed_dev_deps: BTreeSet<String> = self
            .allowed_dev_deps
            .unwrap_or_default()
            .into_iter()
            .collect();
        let allowed_build_deps: BTreeSet<String> = self
            .allowed_build_deps
            .unwrap_or_default()
            .into_iter()
            .collect();
        let forbidden_deps: BTreeSet<String> = self
            .forbidden_deps
            .unwrap_or_default()
            .into_iter()
            .collect();
        for dependency in allowed_deps
            .iter()
            .chain(&allowed_dev_deps)
            .chain(&allowed_build_deps)
        {
            if forbidden_deps.contains(dependency) {
                violations.push(Violation::new(
                    self.line,
                    format!(
                        "crate policy {name} lists dependency {dependency} in both allowed and forbidden sets"
                    ),
                ));
            }
        }
        Some(CratePolicy {
            allowed_deps,
            allowed_dev_deps,
            allowed_build_deps,
            forbidden_deps,
            forbidden_tokens,
        })
    }
}

fn json_string_field<'a>(
    object: &'a BTreeMap<String, JsonValue>,
    name: &str,
) -> Result<&'a str, String> {
    object
        .get(name)
        .and_then(JsonValue::as_str)
        .ok_or_else(|| format!("cargo metadata field {name} must be a string"))
}

#[derive(Clone, Debug, PartialEq)]
enum JsonValue {
    Null,
    Bool,
    Number,
    String(String),
    Array(Vec<Self>),
    Object(BTreeMap<String, Self>),
}

impl JsonValue {
    fn as_object(&self) -> Option<&BTreeMap<String, Self>> {
        if let Self::Object(value) = self {
            Some(value)
        } else {
            None
        }
    }

    fn as_array(&self) -> Option<&[Self]> {
        if let Self::Array(value) = self {
            Some(value)
        } else {
            None
        }
    }

    fn as_str(&self) -> Option<&str> {
        if let Self::String(value) = self {
            Some(value)
        } else {
            None
        }
    }
}

struct JsonParser<'a> {
    source: &'a [u8],
    offset: usize,
}

impl<'a> JsonParser<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            source: source.as_bytes(),
            offset: 0,
        }
    }

    fn parse(mut self) -> Result<JsonValue, String> {
        let value = self.value()?;
        self.whitespace();
        if self.offset != self.source.len() {
            return Err(format!("trailing JSON at byte {}", self.offset));
        }
        Ok(value)
    }

    fn value(&mut self) -> Result<JsonValue, String> {
        self.whitespace();
        match self.source.get(self.offset) {
            Some(b'n') => self.keyword(b"null", JsonValue::Null),
            Some(b't') => self.keyword(b"true", JsonValue::Bool),
            Some(b'f') => self.keyword(b"false", JsonValue::Bool),
            Some(b'"') => self.string().map(JsonValue::String),
            Some(b'[') => self.array(),
            Some(b'{') => self.object(),
            Some(b'-' | b'0'..=b'9') => self.number(),
            Some(byte) => Err(format!(
                "unexpected JSON byte {:?} at {}",
                char::from(*byte),
                self.offset
            )),
            None => Err("unexpected end of JSON".to_owned()),
        }
    }

    fn keyword(&mut self, keyword: &[u8], value: JsonValue) -> Result<JsonValue, String> {
        if self.source.get(self.offset..self.offset + keyword.len()) == Some(keyword) {
            self.offset += keyword.len();
            Ok(value)
        } else {
            Err(format!("invalid JSON keyword at byte {}", self.offset))
        }
    }

    fn string(&mut self) -> Result<String, String> {
        self.expect(b'"')?;
        let mut value = String::new();
        while let Some(byte) = self.source.get(self.offset).copied() {
            self.offset += 1;
            match byte {
                b'"' => return Ok(value),
                b'\\' => {
                    let escape = self
                        .source
                        .get(self.offset)
                        .copied()
                        .ok_or_else(|| "unterminated JSON escape".to_owned())?;
                    self.offset += 1;
                    match escape {
                        b'"' => value.push('"'),
                        b'\\' => value.push('\\'),
                        b'/' => value.push('/'),
                        b'b' => value.push('\u{0008}'),
                        b'f' => value.push('\u{000c}'),
                        b'n' => value.push('\n'),
                        b'r' => value.push('\r'),
                        b't' => value.push('\t'),
                        b'u' => value.push(self.unicode_escape()?),
                        _ => return Err(format!("invalid JSON escape at byte {}", self.offset)),
                    }
                }
                0..=31 => return Err("control byte in JSON string".to_owned()),
                _ if byte.is_ascii() => value.push(char::from(byte)),
                _ => {
                    self.offset -= 1;
                    let remaining = std::str::from_utf8(&self.source[self.offset..])
                        .map_err(|error| format!("invalid UTF-8 in JSON string: {error}"))?;
                    let character = remaining
                        .chars()
                        .next()
                        .ok_or_else(|| "empty UTF-8 sequence".to_owned())?;
                    value.push(character);
                    self.offset += character.len_utf8();
                }
            }
        }
        Err("unterminated JSON string".to_owned())
    }

    fn unicode_escape(&mut self) -> Result<char, String> {
        let first = self.hex_quad()?;
        if (0xD800..=0xDBFF).contains(&first) {
            if self.source.get(self.offset..self.offset + 2) != Some(b"\\u") {
                return Err("high surrogate without low surrogate".to_owned());
            }
            self.offset += 2;
            let second = self.hex_quad()?;
            if !(0xDC00..=0xDFFF).contains(&second) {
                return Err("invalid low surrogate".to_owned());
            }
            let scalar = 0x1_0000 + (u32::from(first - 0xD800) << 10) + u32::from(second - 0xDC00);
            char::from_u32(scalar).ok_or_else(|| "invalid Unicode scalar".to_owned())
        } else if (0xDC00..=0xDFFF).contains(&first) {
            Err("unpaired low surrogate".to_owned())
        } else {
            char::from_u32(u32::from(first)).ok_or_else(|| "invalid Unicode scalar".to_owned())
        }
    }

    fn hex_quad(&mut self) -> Result<u16, String> {
        let bytes = self
            .source
            .get(self.offset..self.offset + 4)
            .ok_or_else(|| "short Unicode escape".to_owned())?;
        self.offset += 4;
        let mut value = 0_u16;
        for byte in bytes {
            let digit = match byte {
                b'0'..=b'9' => byte - b'0',
                b'a'..=b'f' => byte - b'a' + 10,
                b'A'..=b'F' => byte - b'A' + 10,
                _ => return Err("invalid hexadecimal Unicode escape".to_owned()),
            };
            value = value * 16 + u16::from(digit);
        }
        Ok(value)
    }

    fn array(&mut self) -> Result<JsonValue, String> {
        self.expect(b'[')?;
        let mut values = Vec::new();
        self.whitespace();
        if self.consume(b']') {
            return Ok(JsonValue::Array(values));
        }
        loop {
            values.push(self.value()?);
            self.whitespace();
            if self.consume(b']') {
                break;
            }
            self.expect(b',')?;
        }
        Ok(JsonValue::Array(values))
    }

    fn object(&mut self) -> Result<JsonValue, String> {
        self.expect(b'{')?;
        let mut values = BTreeMap::new();
        self.whitespace();
        if self.consume(b'}') {
            return Ok(JsonValue::Object(values));
        }
        loop {
            self.whitespace();
            let key = self.string()?;
            self.whitespace();
            self.expect(b':')?;
            let value = self.value()?;
            if values.insert(key.clone(), value).is_some() {
                return Err(format!("duplicate JSON object key {key:?}"));
            }
            self.whitespace();
            if self.consume(b'}') {
                break;
            }
            self.expect(b',')?;
        }
        Ok(JsonValue::Object(values))
    }

    fn number(&mut self) -> Result<JsonValue, String> {
        let start = self.offset;
        while self.source.get(self.offset).is_some_and(|byte| {
            byte.is_ascii_digit() || matches!(byte, b'-' | b'+' | b'.' | b'e' | b'E')
        }) {
            self.offset += 1;
        }
        let value = std::str::from_utf8(&self.source[start..self.offset])
            .map_err(|error| format!("invalid JSON number: {error}"))?;
        value
            .parse::<f64>()
            .map_err(|error| format!("invalid JSON number {value:?}: {error}"))?;
        Ok(JsonValue::Number)
    }

    fn whitespace(&mut self) {
        while self
            .source
            .get(self.offset)
            .is_some_and(u8::is_ascii_whitespace)
        {
            self.offset += 1;
        }
    }

    fn expect(&mut self, expected: u8) -> Result<(), String> {
        self.whitespace();
        if self.consume(expected) {
            Ok(())
        } else {
            Err(format!(
                "expected JSON byte {:?} at {}",
                char::from(expected),
                self.offset
            ))
        }
    }

    fn consume(&mut self, expected: u8) -> bool {
        if self.source.get(self.offset) == Some(&expected) {
            self.offset += 1;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::{Command, Output};
    use std::sync::Mutex;

    static FIXTURE_PROCESS_LOCK: Mutex<()> = Mutex::new(());

    fn fixture_path(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/r0_04")
            .join(name)
    }

    fn run_arch_fixture(metadata: &str, config: &Path) -> Output {
        let _guard = FIXTURE_PROCESS_LOCK
            .lock()
            .expect("fixture process lock must not be poisoned");
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

    fn run_source_fixture(config: &Path, source_root: &Path) -> Output {
        let _guard = FIXTURE_PROCESS_LOCK
            .lock()
            .expect("fixture process lock must not be poisoned");
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
                "--source-fixture-root",
            ])
            .arg(source_root)
            .arg("--config")
            .arg(config)
            .current_dir(workspace_root)
            .output()
            .expect("source-fixture invocation must start")
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
        let output = run_arch_fixture(
            "metadata-allowed.json",
            &fixture_path("arch-malformed.toml"),
        );
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
        assert!(
            !output.status.success(),
            "unreadable config input was skipped"
        );
        assert!(stderr.contains("unreadable-directory"), "{stderr}");
        assert!(stderr.contains("cannot read"), "{stderr}");
    }

    #[test]
    fn arch_r0_05_detects_every_configured_forbidden_token() {
        let config = parse_arch_config(include_str!("../arch.toml"))
            .expect("real architecture policy must parse");
        let mut checked = 0;
        for (crate_name, policy) in config.crates {
            for token in policy.forbidden_tokens {
                checked += 1;
                let source = format!(
                    "// {token} in a line comment\n/* {token} in a block comment */\n#[cfg(test)]\nmod tests {{ fn allowed() {{ {token}; }} }}\nfn violation() {{ {token}; }}\n"
                );
                let violations = scan_source(&source, std::slice::from_ref(&token));
                assert_eq!(
                    violations.len(),
                    1,
                    "crate {crate_name} token {token:?} was not detected: {violations:?}"
                );
                assert_eq!(violations[0].line, 5, "crate {crate_name} token {token:?}");
                assert!(
                    violations[0].message.contains(&token),
                    "crate {crate_name} token {token:?}: {:?}",
                    violations[0]
                );
            }
        }
        assert!(
            checked > 0,
            "the real policy must configure forbidden tokens"
        );
    }

    #[test]
    fn arch_r0_05_ignores_comments_and_nested_cfg_test_items() {
        let source = r"// SystemTime::now in a line comment
/// SystemTime::now in a doc comment
//! SystemTime::now in an inner doc comment
/* SystemTime::now in an outer block
   /* SystemTime::now in a nested block */
*/
#[cfg(test)]
mod tests {
    fn helper() { SystemTime::now(); }
    #[cfg(test)]
    fn nested_helper() { SystemTime::now(); }
}
#[cfg(test)]
fn test_only_item() { SystemTime::now(); }
fn production() {
    SystemTime::now();
}
";
        let violations = scan_source(source, &["SystemTime::now".to_owned()]);
        assert_eq!(violations.len(), 1, "{violations:?}");
        assert_eq!(violations[0].line, 16);
    }

    #[test]
    fn arch_r0_05_traverses_configured_source_tree_with_file_and_line() {
        let source_root = fixture_path("../r0_05/source-bad");
        let output = run_source_fixture(&fixture_path("../r0_05/arch-source.toml"), &source_root);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            !output.status.success(),
            "forbidden source tree was accepted"
        );
        assert!(stderr.contains("crates/relay-core/src/lib.rs"), "{stderr}");
        assert!(stderr.contains("line 4"), "{stderr}");
        assert!(stderr.contains("SystemTime::now"), "{stderr}");
    }

    #[test]
    fn arch_r0_05_missing_source_tree_fails_closed() {
        let source_root = fixture_path("../r0_05/source-missing");
        let output = run_source_fixture(&fixture_path("../r0_05/arch-source.toml"), &source_root);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            !output.status.success(),
            "missing configured source was skipped"
        );
        assert!(stderr.contains("relay-core"), "{stderr}");
        assert!(stderr.contains("source"), "{stderr}");
        assert!(stderr.contains("missing"), "{stderr}");
    }

    #[test]
    fn arch_r0_05_malformed_utf8_source_fails_closed() {
        let temp_root =
            std::env::temp_dir().join(format!("relay-arch-r0-05-utf8-{}", std::process::id()));
        let source_dir = temp_root.join("crates/relay-core/src");
        fs::create_dir_all(&source_dir).expect("temporary source tree must be created");
        fs::write(source_dir.join("lib.rs"), [0xff, 0xfe, b'\n'])
            .expect("malformed UTF-8 fixture must be written");

        let output = run_source_fixture(&fixture_path("../r0_05/arch-source.toml"), &temp_root);
        fs::remove_dir_all(&temp_root).expect("temporary source tree must be removed");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            !output.status.success(),
            "malformed UTF-8 source was skipped"
        );
        assert!(stderr.contains("lib.rs"), "{stderr}");
        assert!(stderr.contains("UTF-8"), "{stderr}");
    }

    #[test]
    fn arch_r0_05_does_not_mask_production_after_lifetime_test_item() {
        let source = r"
pub fn consume<T>() {}

#[cfg(test)]
#[allow(dead_code)]
#[rustfmt::skip]
fn helper<'a>() { consume::<&'a str>() }

pub fn production() {
    let _ = std::time::SystemTime::now();
}
";
        let violations = scan_source(
            source,
            &["std::time".to_owned(), "SystemTime::now".to_owned()],
        );
        assert_eq!(violations.len(), 2, "{violations:?}");
        assert!(violations.iter().all(|item| item.line == 10));
    }

    #[test]
    fn arch_r0_05_matches_forbidden_token_sequences_across_trivia_and_use_trees() {
        let split_path =
            "fn production() { let _ = std /* split */ :: time :: SystemTime :: now(); }\n";
        let split_violations = scan_source(
            split_path,
            &["std::time".to_owned(), "SystemTime::now".to_owned()],
        );
        assert_eq!(split_violations.len(), 2, "{split_violations:?}");
        assert!(split_violations.iter().all(|item| item.line == 1));

        let use_tree =
            "use std::{time::SystemTime as Clock};\nfn production() { let _ = Clock::now(); }\n";
        let use_violations = scan_source(use_tree, &["std::time".to_owned()]);
        assert_eq!(use_violations.len(), 1, "{use_violations:?}");
        assert_eq!(use_violations[0].line, 1);
    }

    #[test]
    fn arch_r0_05_rejects_std_aliases_and_unresolved_path_attributes() {
        let aliased = concat!(
            "use std as platform;\n",
            "use platform::time::SystemTime as Clock;\n",
            "fn production() { let _ = Clock::now(); }\n",
        );
        let alias_violations = scan_source(aliased, &["std::time".to_owned()]);
        assert_eq!(alias_violations.len(), 1, "{alias_violations:?}");
        assert_eq!(alias_violations[0].line, 1);

        let grouped_alias = concat!(
            "use std::{self as platform};\n",
            "use platform::time::SystemTime as Clock;\n",
            "fn production() { let _ = Clock::now(); }\n",
        );
        let grouped_violations = scan_source(grouped_alias, &["std::time".to_owned()]);
        assert_eq!(grouped_violations.len(), 1, "{grouped_violations:?}");
        assert_eq!(grouped_violations[0].line, 1);

        let indirect = "#[path = \"generated/production.rs\"]\nmod production;\nfn harmless() {}\n";
        let path_violations = scan_source(indirect, &["std::time".to_owned()]);
        assert_eq!(path_violations.len(), 1, "{path_violations:?}");
        assert_eq!(path_violations[0].line, 1);
        assert!(path_violations[0].message.contains("#[path]"));
    }

    #[test]
    fn arch_r0_05_scans_included_rust_sources_without_rs_extension() {
        let temp_root =
            std::env::temp_dir().join(format!("relay-arch-r0-05-include-{}", std::process::id()));
        let source_dir = temp_root.join("crates/relay-core/src");
        fs::create_dir_all(&source_dir).expect("temporary source tree must be created");
        fs::write(source_dir.join("lib.rs"), "include!(\"impl.inc\");\n")
            .expect("crate root must be written");
        fs::write(
            source_dir.join("impl.inc"),
            "fn production() { let _ = SystemTime::now(); }\n",
        )
        .expect("included Rust source must be written");

        let result =
            check_source_fixture_files(&temp_root, &fixture_path("../r0_05/arch-source.toml"));
        fs::remove_dir_all(&temp_root).expect("temporary fixture must be removed");
        let violations = result.expect_err("included source must be scanned");
        assert!(
            violations
                .iter()
                .any(|item| item.message.contains("impl.inc")
                    && item.message.contains("SystemTime::now")),
            "{violations:?}"
        );
    }

    #[test]
    fn arch_r0_05_rejects_unscanned_custom_target_paths() {
        let manifest = "[lib]\npath = \"outside/lib.rs\"\n";
        let violations = validate_source_layout(manifest);
        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(violations[0].message.contains("custom target path"));
    }

    #[test]
    fn arch_r0_05_source_layout_parser_handles_multiline_values() {
        let manifest = "[package]\nkeywords = [\n    \"relay\",\n]\n";
        let violations = validate_source_layout(manifest);
        assert!(violations.is_empty(), "{violations:?}");
    }

    #[test]
    fn arch_r0_05_rejects_dotted_custom_build_script_path() {
        let violations = validate_source_layout("package.build = \"build/custom.rs\"\n");
        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(violations[0].message.contains("build script"));
    }

    #[test]
    fn arch_r0_05_real_policy_covers_every_normative_raft_api() {
        let config = parse_arch_config(include_str!("../arch.toml"))
            .expect("real architecture policy must parse");
        let raft = config
            .crates
            .get("relay-raft")
            .expect("relay-raft policy must exist");
        for required in [
            "SystemTime::now",
            "Instant::now",
            "thread::sleep",
            "std::fs",
            "std::net",
            "rand::",
            "tokio::",
        ] {
            assert!(
                raft.forbidden_tokens.iter().any(|token| token == required),
                "relay-raft policy is missing {required:?}: {:?}",
                raft.forbidden_tokens
            );
        }
    }

    #[test]
    fn arch_r0_05_ignores_out_of_line_cfg_test_modules() {
        let temp_root = std::env::temp_dir().join(format!(
            "relay-arch-r0-05-out-of-line-test-{}",
            std::process::id()
        ));
        let source_dir = temp_root.join("crates/relay-core/src");
        fs::create_dir_all(&source_dir).expect("temporary source tree must be created");
        fs::write(
            source_dir.join("lib.rs"),
            "#[cfg(test)]\nmod tests;\npub fn production() {}\n",
        )
        .expect("crate root must be written");
        fs::write(
            source_dir.join("tests.rs"),
            "#[test]\nfn test_only() { let _ = SystemTime::now(); }\n",
        )
        .expect("test module must be written");

        let result =
            check_source_fixture_files(&temp_root, &fixture_path("../r0_05/arch-source.toml"));
        fs::remove_dir_all(&temp_root).expect("temporary fixture must be removed");
        assert!(
            result.is_ok(),
            "out-of-line cfg(test) was scanned: {result:?}"
        );
    }

    #[test]
    fn arch_r0_05_out_of_line_test_exclusion_does_not_hide_production() {
        let temp_root = std::env::temp_dir().join(format!(
            "relay-arch-r0-05-out-of-line-production-{}",
            std::process::id()
        ));
        let source_dir = temp_root.join("crates/relay-core/src");
        fs::create_dir_all(&source_dir).expect("temporary source tree must be created");
        fs::write(
            source_dir.join("lib.rs"),
            "#[cfg(test)]\nmod tests;\npub fn production() { let _ = SystemTime::now(); }\n",
        )
        .expect("crate root must be written");
        fs::write(
            source_dir.join("tests.rs"),
            "#[test]\nfn test_only() { let _ = SystemTime::now(); }\n",
        )
        .expect("test module must be written");

        let result =
            check_source_fixture_files(&temp_root, &fixture_path("../r0_05/arch-source.toml"));
        fs::remove_dir_all(&temp_root).expect("temporary fixture must be removed");
        let violations = result.expect_err("production source violation must remain visible");
        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(violations[0].message.contains("lib.rs"), "{violations:?}");
        assert!(violations[0].message.contains("line 3"), "{violations:?}");
        assert!(
            !violations[0].message.contains("tests.rs"),
            "{violations:?}"
        );
    }

    fn metadata_with_workspace_packages(package_names: &[&str]) -> String {
        let packages = package_names
            .iter()
            .map(|name| {
                format!(
                    r#"{{"name":"{name}","id":"path+file:///fixture/{name}#0.1.0","manifest_path":"/fixture/{name}/Cargo.toml","dependencies":[]}}"#
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let members = package_names
            .iter()
            .map(|name| format!(r#""path+file:///fixture/{name}#0.1.0""#))
            .collect::<Vec<_>>()
            .join(",");
        format!(r#"{{"packages":[{packages}],"workspace_members":[{members}]}}"#)
    }

    fn metadata_with_dependency(kind: &str) -> String {
        format!(
            r#"{{"packages":[{{"name":"relay-wal","id":"path+file:///fixture/relay-wal#0.1.0","manifest_path":"/fixture/relay-wal/Cargo.toml","dependencies":[{{"name":"proptest","rename":null,"kind":{kind}}}]}}],"workspace_members":["path+file:///fixture/relay-wal#0.1.0"]}}"#
        )
    }

    #[test]
    fn arch_r0_04_review_requires_exact_policy_and_workspace_coverage() {
        const PRODUCT_CRATES: [&str; 10] = [
            "relay-core",
            "relay-wal",
            "relay-raft",
            "relay-sim",
            "relay-model",
            "relay-wire",
            "relay-server",
            "relay-client",
            "relay-cli",
            "relay-bench",
        ];
        let real_policy = parse_arch_config(include_str!("../arch.toml"))
            .expect("the real architecture policy must parse");
        assert_eq!(
            real_policy
                .crates
                .keys()
                .map(String::as_str)
                .collect::<BTreeSet<_>>(),
            PRODUCT_CRATES.into_iter().collect::<BTreeSet<_>>()
        );

        let incomplete_policy =
            parse_arch_config(include_str!("../fixtures/r0_04/arch-nine-policies.toml"))
                .expect("partial policies remain valid for deterministic fixtures");
        let mut all_workspace_packages = PRODUCT_CRATES.to_vec();
        all_workspace_packages.push("arch-check");
        let full_metadata =
            parse_cargo_metadata(&metadata_with_workspace_packages(&all_workspace_packages))
                .expect("the exact real workspace shape must parse");
        let policy_errors = validate_dependency_graph(&incomplete_policy, &full_metadata);
        assert!(
            policy_errors
                .iter()
                .any(|violation| violation.message.contains("relay-bench")),
            "{policy_errors:?}"
        );

        let ten_only = parse_cargo_metadata(&metadata_with_workspace_packages(&PRODUCT_CRATES))
            .expect("structurally valid metadata remains parseable before shape validation");
        let missing_tool_errors = validate_dependency_graph(&real_policy, &ten_only);
        assert!(
            missing_tool_errors
                .iter()
                .any(|violation| violation.message.contains("arch-check")),
            "{missing_tool_errors:?}"
        );
        assert!(
            validate_dependency_graph(&real_policy, &full_metadata).is_empty(),
            "exactly ten product crates plus arch-check must be accepted"
        );

        let mut rogue_workspace = all_workspace_packages;
        rogue_workspace.push("relay-rogue");
        let rogue_metadata =
            parse_cargo_metadata(&metadata_with_workspace_packages(&rogue_workspace))
                .expect("rogue-package metadata is structurally valid");
        let rogue_errors = validate_dependency_graph(&real_policy, &rogue_metadata);
        assert!(
            rogue_errors
                .iter()
                .any(|violation| violation.message.contains("relay-rogue")),
            "{rogue_errors:?}"
        );
    }

    #[test]
    fn arch_r0_04_review_rejects_configured_rogue_workspace_crate() {
        let mut policy_source = include_str!("../arch.toml").to_owned();
        policy_source.push_str(
            "\n[crate.relay-rogue]\nallowed-deps = []\nforbidden-deps = []\nforbidden-tokens = []\n",
        );
        let policy = parse_arch_config(&policy_source)
            .expect("configured-rogue policy is syntactically valid");
        let mut workspace_packages = PRODUCT_CRATES.to_vec();
        workspace_packages.extend(["arch-check", "relay-rogue"]);
        let metadata = parse_cargo_metadata(&metadata_with_workspace_packages(&workspace_packages))
            .expect("configured-rogue metadata is structurally valid");

        let violations = validate_dependency_graph(&policy, &metadata);
        assert!(
            violations
                .iter()
                .any(|violation| violation.message.contains("relay-rogue")),
            "a configured rogue workspace crate bypassed the exact shape check: {violations:?}"
        );
    }

    #[test]
    fn arch_r0_04_review_rejects_versionless_path_and_git_but_accepts_workspace() {
        let manifest = r#"
[dependencies]
local = { path = "../local" }
remote = { git = "https://example.invalid/repo" }
inherited = { workspace = true }
"#;
        let violations = check_exact_requirements(manifest);
        assert_eq!(violations.len(), 2, "{violations:?}");
        assert!(
            violations
                .iter()
                .any(|violation| violation.message.contains("local"))
        );
        assert!(
            violations
                .iter()
                .any(|violation| violation.message.contains("remote"))
        );
        assert!(
            violations
                .iter()
                .all(|violation| !violation.message.contains("inherited"))
        );
    }

    #[test]
    fn arch_r0_04_review_retains_dependency_kind_and_separates_dev_allowlist() {
        let policy = parse_arch_config(include_str!("../fixtures/r0_04/arch-kind-policy.toml"))
            .expect("kind fixture policy must parse");
        let normal = parse_cargo_metadata(&metadata_with_dependency("null"))
            .expect("normal dependency metadata must parse");
        let development = parse_cargo_metadata(&metadata_with_dependency(r#""dev""#))
            .expect("development dependency metadata must parse");

        let normal_violations = validate_dependency_graph(&policy, &normal);
        assert!(
            normal_violations.iter().any(|violation| {
                violation.message.contains("relay-wal")
                    && violation.message.contains("proptest")
                    && violation.message.contains("normal")
            }),
            "{normal_violations:?}"
        );
        assert!(
            validate_dependency_graph(&policy, &development).is_empty(),
            "a test-only dependency should be accepted only as kind=dev"
        );

        let production = parse_arch_config(include_str!("../arch.toml"))
            .expect("real architecture policy must parse");
        assert!(
            !production.crates["relay-wal"]
                .allowed_deps
                .contains("proptest")
        );
        assert!(
            !production.crates["relay-wal"]
                .allowed_deps
                .contains("tempfile")
        );
        assert!(
            !production.crates["relay-bench"]
                .allowed_deps
                .contains("criterion")
        );
    }

    #[test]
    fn arch_r0_04_review_dependency_table_fields_and_version_are_validated() {
        let exact = r#"
[dependencies.bytes]
version = "=1.9.0"
default-features = false
features = ["std"]
"#;
        assert!(
            check_exact_requirements(exact).is_empty(),
            "non-version fields in a dependency table must not be parsed as requirements"
        );

        let missing = r#"
[dependencies.bytes]
default-features = false
features = ["std"]
"#;
        let missing_violations = check_exact_requirements(missing);
        assert_eq!(missing_violations.len(), 1, "{missing_violations:?}");
        assert!(missing_violations[0].message.contains("bytes"));
        assert!(missing_violations[0].message.contains("missing"));

        let floating = "[dependencies.bytes]\nversion = \"1.9\"\n";
        let floating_violations = check_exact_requirements(floating);
        assert_eq!(floating_violations.len(), 1, "{floating_violations:?}");
        assert!(floating_violations[0].message.contains("bytes"));
    }

    #[test]
    fn arch_r0_04_review_dotted_dependency_keys_are_checked() {
        let manifest = r#"
[dependencies]
bytes.version = "1.9"
relay-core.workspace = true
"#;
        let violations = check_exact_requirements(manifest);
        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(
            violations[0]
                .message
                .contains("dependency bytes must use an exact"),
            "{}",
            violations[0].message
        );
        assert!(!violations[0].message.contains("relay-core"));
    }

    #[test]
    fn arch_r0_04_review_rejects_allowed_forbidden_overlap() {
        let policy = r#"
[crate.relay-core]
allowed-deps = ["im", "rand"]
forbidden-deps = ["rand"]
forbidden-tokens = []
"#;
        let violations = parse_arch_config(policy)
            .expect_err("a dependency cannot be simultaneously allowed and forbidden");
        assert!(
            violations
                .iter()
                .any(|violation| violation.message.contains("rand")
                    && violation.message.contains("both")),
            "{violations:?}"
        );
    }

    #[test]
    fn arch_r0_04_review_rejects_empty_array_entries() {
        let malformed = include_str!("../fixtures/r0_04/arch-empty-array-entry.toml");
        let violations = parse_arch_config(malformed)
            .expect_err("leading or repeated commas are malformed TOML array entries");
        assert!(
            violations
                .iter()
                .any(|violation| violation.message.contains("empty array entry")),
            "{violations:?}"
        );
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

    fn valid_gate_registry() -> String {
        let mut source = "schema = 1\n".to_owned();
        for number in 0..=10 {
            let status = if number == 0 { "accepted" } else { "planned" };
            let commands = if number == 0 {
                "[\"cargo test --workspace --locked\"]"
            } else {
                "[]"
            };
            source.push_str(&format!(
                "\n[gate.R{number}]\nstatus = \"{status}\"\nsection = \"BUILD_PLAN.md §{}\"\ncommands = {commands}\n",
                number + 5
            ));
        }
        source
    }

    #[test]
    fn arch_r0_06_gate_schema_is_total_and_requires_complete_evidence() {
        assert!(validate_gates(&valid_gate_registry()).is_empty());

        let missing = valid_gate_registry().replace(
            "\n[gate.R10]\nstatus = \"planned\"\nsection = \"BUILD_PLAN.md §15\"\ncommands = []\n",
            "",
        );
        let missing_violations = validate_gates(&missing);
        assert!(
            missing_violations
                .iter()
                .any(|item| item.message.contains("R10")),
            "{missing_violations:?}"
        );

        let duplicate = format!(
            "{}\n[gate.R0]\nstatus = \"planned\"\n",
            valid_gate_registry()
        );
        let duplicate_violations = validate_gates(&duplicate);
        assert!(
            duplicate_violations.iter().any(|item| item.line > 1
                && item.message.contains("duplicate")
                && item.message.contains("R0")),
            "{duplicate_violations:?}"
        );

        let unknown_status =
            valid_gate_registry().replacen("status = \"planned\"", "status = \"complete\"", 1);
        let status_violations = validate_gates(&unknown_status);
        assert!(
            status_violations
                .iter()
                .any(|item| item.message.contains("status") && item.message.contains("complete")),
            "{status_violations:?}"
        );

        let empty_accepted = valid_gate_registry().replacen(
            "commands = [\"cargo test --workspace --locked\"]",
            "commands = []",
            1,
        );
        let command_violations = validate_gates(&empty_accepted);
        assert!(
            command_violations
                .iter()
                .any(|item| item.message.contains("accepted gate R0 must have commands")),
            "{command_violations:?}"
        );

        let malformed = valid_gate_registry().replacen("schema = 1", "schema = [", 1);
        assert!(
            !validate_gates(&malformed).is_empty(),
            "malformed registry input was silently accepted"
        );
    }

    #[test]
    fn arch_r0_06_status_scan_rejects_unearned_claim_with_line() {
        let unearned = "**Status:** planned.\nRelay provides durable delivery.\n";
        let violations = validate_status_discipline(unearned);
        assert_eq!(violations.len(), 1, "{violations:?}");
        assert_eq!(violations[0].line, 2);
        assert!(violations[0].message.contains("provides"));
        assert!(violations[0].message.contains("planned"));

        let qualified =
            "**Status:** planned.\nRelay provides durable delivery only as planned for R2.\n";
        assert!(validate_status_discipline(qualified).is_empty());
        let accepted = "**Status:** accepted.\nRelay provides durable delivery.\n";
        assert!(validate_status_discipline(accepted).is_empty());
    }

    #[test]
    fn arch_r0_06_status_scope_and_claim_words_fail_closed() {
        let planned = concat!(
            "## R1\n",
            "**Status:** planned.\n",
            "### Details\n",
            "Relay currently supports durable delivery.\n",
            "The service provides long polling.\n",
        );
        let violations = validate_status_discipline(planned);
        assert_eq!(violations.len(), 2, "{violations:?}");
        assert_eq!(
            violations.iter().map(|item| item.line).collect::<Vec<_>>(),
            [4, 5]
        );

        let unmatched = "**Status:** planned.\n`unmatched Relay guarantees persistence.\n";
        let unmatched_violations = validate_status_discipline(unmatched);
        assert_eq!(unmatched_violations.len(), 1, "{unmatched_violations:?}");
        assert_eq!(unmatched_violations[0].line, 2);

        let uppercase = validate_status_discipline("**Status:** PLANNED.\n");
        assert_eq!(uppercase.len(), 1, "{uppercase:?}");
        assert_eq!(uppercase[0].line, 1);
    }

    #[test]
    fn arch_r0_06_links_ignore_external_and_anchor_but_report_relative_line() {
        let source = concat!(
            "[external](https://example.com) [anchor](#local)\n",
            "[ok](./guide.md#section) [bad](nested/missing.md)\n",
        );
        let violations = validate_relative_links(source, &["guide.md".to_owned()]);
        assert_eq!(violations.len(), 1, "{violations:?}");
        assert_eq!(violations[0].line, 2);
        assert!(violations[0].message.contains("nested/missing.md"));
    }

    #[test]
    fn arch_r0_06_reference_links_and_malformed_inline_code_fail_closed() {
        let source = concat!(
            "> [guide][g]\n",
            ">\n",
            "> [g]: ./missing.md\n",
            "prose](./not-a-link.md)\n",
            "[^1]: explanatory prose\n",
            "`unmatched [also-missing](./also-missing.md)\n",
        );
        let violations = validate_relative_links(source, &[]);
        assert_eq!(violations.len(), 2, "{violations:?}");
        assert!(
            violations
                .iter()
                .any(|item| item.line == 3 && item.message.contains("missing.md")),
            "{violations:?}"
        );
        assert!(
            violations
                .iter()
                .any(|item| item.line == 6 && item.message.contains("also-missing.md")),
            "{violations:?}"
        );
        assert!(
            violations
                .iter()
                .all(|item| !item.message.contains("not-a-link")
                    && !item.message.contains("explanatory")),
            "{violations:?}"
        );
    }

    #[test]
    fn arch_r0_06_dangling_reference_labels_fail_closed() {
        let violations = validate_relative_links("[guide][missing]\n", &[]);
        assert_eq!(violations.len(), 1, "{violations:?}");
        assert_eq!(violations[0].line, 1);
        assert!(violations[0].message.contains("missing"));
        assert!(violations[0].message.contains("reference"));
    }

    #[test]
    fn arch_r0_06_gate_sequence_rejects_accepted_after_unaccepted() {
        let invalid_sequence = valid_gate_registry()
            .replacen("status = \"accepted\"", "status = \"in progress\"", 1)
            .replacen("status = \"planned\"", "status = \"accepted\"", 1);
        let sequence_violations = validate_gates(&invalid_sequence);
        assert!(
            sequence_violations.iter().any(|item| item.line == 9
                && item.message.contains("R1")
                && item.message.contains("R0")),
            "{sequence_violations:?}"
        );
    }

    #[test]
    fn arch_r0_06_gate_sequence_allows_only_one_in_progress_gate() {
        let invalid_sequence = valid_gate_registry()
            .replacen("status = \"accepted\"", "status = \"in progress\"", 1)
            .replacen("status = \"planned\"", "status = \"in progress\"", 1);
        let violations = validate_gates(&invalid_sequence);
        assert!(
            violations.iter().any(|item| item.line == 9
                && item.message.contains("R1")
                && item.message.contains("in progress")),
            "{violations:?}"
        );
    }

    #[test]
    fn arch_r0_06_gate_sections_must_be_in_replay_order() {
        let out_of_order = valid_gate_registry()
            .replacen("[gate.R1]", "[gate.TEMP]", 1)
            .replacen("[gate.R2]", "[gate.R1]", 1)
            .replacen("[gate.TEMP]", "[gate.R2]", 1);
        let order_violations = validate_gates(&out_of_order);
        assert!(
            order_violations
                .iter()
                .any(|item| item.message.contains("order")),
            "{order_violations:?}"
        );
    }

    #[test]
    fn arch_r0_06_gate_strings_reject_raw_newlines() {
        let multiline = valid_gate_registry().replacen(
            "commands = [\"cargo test --workspace --locked\"]",
            "commands = [\"cargo\n test --workspace --locked\"]",
            1,
        );
        let multiline_violations = validate_gates(&multiline);
        assert!(
            multiline_violations
                .iter()
                .any(|item| item.line == 6 && item.message.contains("commands")),
            "{multiline_violations:?}"
        );
    }

    #[test]
    fn arch_r0_06_gate_semantic_errors_report_field_lines() {
        let wrong_fields = valid_gate_registry()
            .replacen("section = \"BUILD_PLAN.md §5\"", "section = \"wrong\"", 1)
            .replacen(
                "commands = [\"cargo test --workspace --locked\"]",
                "commands = []",
                1,
            );
        let field_violations = validate_gates(&wrong_fields);
        assert!(
            field_violations
                .iter()
                .any(|item| item.line == 5 && item.message.contains("section")),
            "{field_violations:?}"
        );
        assert!(
            field_violations
                .iter()
                .any(|item| item.line == 6 && item.message.contains("commands")),
            "{field_violations:?}"
        );
    }

    #[test]
    fn arch_r0_06_file_pass_fails_closed_with_qualified_diagnostics() {
        let temp_root =
            std::env::temp_dir().join(format!("relay-arch-r0-06-{}", std::process::id()));
        let docs_root = temp_root.join("docs");
        fs::create_dir_all(&docs_root).expect("temporary docs tree must be created");
        let gates_path = temp_root.join("gates.toml");
        fs::write(&gates_path, valid_gate_registry()).expect("gate fixture must be written");
        fs::write(
            docs_root.join("README.md"),
            "**Status:** planned.\nRelay guarantees persistence.\n[missing](./missing.md)\n",
        )
        .expect("docs fixture must be written");

        let violations = check_r0_06_fixture_files(&gates_path, &docs_root)
            .expect_err("unearned claim and broken link must fail");
        assert!(
            violations
                .iter()
                .any(|item| item.message.contains("README.md line 2")
                    && item.message.contains("guarantees")),
            "{violations:?}"
        );
        assert!(
            violations
                .iter()
                .any(|item| item.message.contains("README.md line 3")
                    && item.message.contains("missing.md")),
            "{violations:?}"
        );

        let unreadable = fixture_path("unreadable-directory");
        let unreadable_violations = check_r0_06_fixture_files(&unreadable, &docs_root)
            .expect_err("all independent fixture violations must be aggregated");
        fs::remove_dir_all(&temp_root).expect("temporary fixture must be removed");
        assert!(
            unreadable_violations
                .iter()
                .any(|item| item.message.contains("cannot read")),
            "{unreadable_violations:?}"
        );
        assert!(
            unreadable_violations
                .iter()
                .any(|item| item.message.contains("README.md line 2")
                    && item.message.contains("guarantees")),
            "{unreadable_violations:?}"
        );
        assert!(
            unreadable_violations
                .iter()
                .any(|item| item.message.contains("README.md line 3")
                    && item.message.contains("missing.md")),
            "{unreadable_violations:?}"
        );
    }

    #[test]
    fn arch_r0_06_workspace_aggregates_independent_input_failures() {
        let temp_root =
            std::env::temp_dir().join(format!("relay-arch-r0-06-workspace-{}", std::process::id()));
        let docs_root = temp_root.join("docs");
        fs::create_dir_all(&docs_root).expect("temporary docs tree must be created");
        fs::write(
            docs_root.join("README.md"),
            "**Status:** planned.\nRelay supports persistence.\n",
        )
        .expect("docs fixture must be written");

        let violations = check_workspace_r0_04(&temp_root)
            .expect_err("independent required-input failures must aggregate");
        fs::remove_dir_all(&temp_root).expect("temporary fixture must be removed");
        assert!(
            violations
                .iter()
                .any(|item| item.message.contains("arch.toml")),
            "{violations:?}"
        );
        assert!(
            violations
                .iter()
                .any(|item| item.message.contains("README.md line 2")
                    && item.message.contains("supports")),
            "{violations:?}"
        );
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
