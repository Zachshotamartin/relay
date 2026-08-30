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
            Ok(manifest) => violations.extend(qualify_violations(
                &manifest_path,
                check_exact_requirements(&manifest),
            )),
            Err(error) => violations.push(Violation::new(
                1,
                format!(
                    "{} line 1: cannot read required input: {error}",
                    manifest_path.display()
                ),
            )),
        }
    }
    if violations.is_empty() {
        Ok(())
    } else {
        Err(violations)
    }
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
    if line.starts_with("[[") || !line.ends_with(']') {
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
    use std::path::{Path, PathBuf};
    use std::process::{Command, Output};
    use std::sync::Mutex;

    fn fixture_path(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/r0_04")
            .join(name)
    }

    fn run_arch_fixture(metadata: &str, config: &Path) -> Output {
        static FIXTURE_PROCESS_LOCK: Mutex<()> = Mutex::new(());
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
