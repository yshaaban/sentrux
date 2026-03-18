//! Conservative contract parity analyzer driven by explicit contract rules.

use super::SemanticFinding;
use crate::analysis::semantic::SemanticSnapshot;
use crate::metrics::rules::{self, ConceptRule, ContractRule, RulesConfig};
use std::collections::{BTreeSet, HashSet};
use std::path::Path;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ParityScope {
    All,
    Changed,
}

#[derive(Debug, Clone, serde::Serialize, Eq, PartialEq, Ord, PartialOrd)]
pub struct ParityCell {
    pub kind: String,
    pub status: String,
    pub detail: String,
    pub path: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ContractParityReport {
    pub id: String,
    pub kind: String,
    pub priority: Option<String>,
    pub files: Vec<String>,
    pub satisfied_cells: Vec<ParityCell>,
    pub missing_cells: Vec<ParityCell>,
    pub score_0_10000: u32,
    pub summary: String,
}

pub fn build_parity_reports(
    config: &RulesConfig,
    semantic: &SemanticSnapshot,
    root: &Path,
    scope: ParityScope,
    changed_files: &BTreeSet<String>,
) -> Vec<ContractParityReport> {
    let mut reports = config
        .contract
        .iter()
        .filter(|contract| contract_in_scope(contract, scope, changed_files))
        .map(|contract| build_contract_parity_report(config, contract, semantic, root))
        .collect::<Vec<_>>();
    reports.sort_by(|left, right| left.id.cmp(&right.id));
    reports
}

pub fn build_parity_findings(reports: &[ContractParityReport]) -> Vec<SemanticFinding> {
    let mut findings = Vec::new();

    for report in reports {
        for cell in &report.missing_cells {
            let (kind, severity) = finding_kind_and_severity(cell);
            findings.push(SemanticFinding {
                kind: kind.to_string(),
                severity: severity.to_string(),
                concept_id: report.id.clone(),
                summary: format!("Contract '{}' is missing {}", report.id, cell.kind),
                files: cell.path.iter().cloned().collect(),
                evidence: vec![cell.detail.clone()],
            });
        }
    }

    findings
}

pub fn parity_score_0_10000(reports: &[ContractParityReport]) -> u32 {
    let mut satisfied = 0usize;
    let mut total = 0usize;

    for report in reports {
        satisfied += report.satisfied_cells.len();
        total += report.satisfied_cells.len() + report.missing_cells.len();
    }

    if total == 0 {
        return 10000;
    }

    ((satisfied as f64 / total as f64) * 10000.0).round() as u32
}

fn build_contract_parity_report(
    config: &RulesConfig,
    contract: &ContractRule,
    semantic: &SemanticSnapshot,
    root: &Path,
) -> ContractParityReport {
    let mut satisfied = BTreeSet::new();
    let mut missing = BTreeSet::new();
    let mut files = BTreeSet::new();

    push_symbol_cell(
        &mut satisfied,
        &mut missing,
        &mut files,
        semantic,
        contract.categories_symbol.as_deref(),
        "categories_symbol",
        "categories symbol declared",
    );
    push_symbol_cell(
        &mut satisfied,
        &mut missing,
        &mut files,
        semantic,
        contract.payload_map_symbol.as_deref(),
        "payload_map_symbol",
        "payload map symbol declared",
    );
    push_symbol_cell(
        &mut satisfied,
        &mut missing,
        &mut files,
        semantic,
        contract.registry_symbol.as_deref(),
        "registry_symbol",
        "registry symbol declared",
    );
    push_file_cell(
        &mut satisfied,
        &mut missing,
        &mut files,
        root,
        contract.browser_entry.as_deref(),
        "browser_entry",
        "browser entry present",
    );
    let browser_entry_present = contract
        .browser_entry
        .as_deref()
        .map(|path| file_exists(root, path))
        .unwrap_or(false);
    push_file_cell(
        &mut satisfied,
        &mut missing,
        &mut files,
        root,
        contract.electron_entry.as_deref(),
        "electron_entry",
        "electron entry present",
    );
    let electron_entry_present = contract
        .electron_entry
        .as_deref()
        .map(|path| file_exists(root, path))
        .unwrap_or(false);

    let contract_symbol_names = contract_symbol_names(config, contract, semantic);
    let browser_has_binding = browser_entry_present
        && contract
            .browser_entry
            .as_deref()
            .map(|path| file_mentions_any_symbol(root, path, &contract_symbol_names))
            .unwrap_or(false);
    let electron_has_binding = electron_entry_present
        && contract
            .electron_entry
            .as_deref()
            .map(|path| file_mentions_any_symbol(root, path, &contract_symbol_names))
            .unwrap_or(false);

    if contract
        .required_capabilities
        .iter()
        .any(|value| value == "live_updates")
    {
        if browser_entry_present {
            push_runtime_binding_cell(
                &mut satisfied,
                &mut missing,
                contract.browser_entry.as_deref(),
                browser_has_binding,
                "browser_live_update_binding",
            );
        }
        if electron_entry_present {
            push_runtime_binding_cell(
                &mut satisfied,
                &mut missing,
                contract.electron_entry.as_deref(),
                electron_has_binding,
                "electron_live_update_binding",
            );
        }
    }

    if contract
        .required_capabilities
        .iter()
        .any(|value| value == "snapshot")
        && (browser_entry_present || electron_entry_present)
        && !browser_has_binding
        && !electron_has_binding
    {
        missing.insert(ParityCell {
            kind: "snapshot_without_live_update".to_string(),
            status: "missing".to_string(),
            detail: "snapshot contract exists without any runtime binding".to_string(),
            path: contract
                .browser_entry
                .clone()
                .or_else(|| contract.electron_entry.clone()),
        });
    }

    if contract
        .required_capabilities
        .iter()
        .any(|value| value == "versioning")
    {
        let versioning_paths = contract_rule_paths(contract);
        let has_versioning = versioning_paths
            .iter()
            .any(|path| file_has_version_marker(root, path));
        let cell = ParityCell {
            kind: "versioning".to_string(),
            status: if has_versioning {
                "satisfied"
            } else {
                "missing"
            }
            .to_string(),
            detail: if has_versioning {
                "version marker present in contract paths".to_string()
            } else {
                "no version marker found in configured contract paths".to_string()
            },
            path: versioning_paths.into_iter().next(),
        };
        if has_versioning {
            satisfied.insert(cell);
        } else {
            missing.insert(cell);
        }
    }

    let satisfied_cells = satisfied.into_iter().collect::<Vec<_>>();
    let missing_cells = missing.into_iter().collect::<Vec<_>>();
    let files = files.into_iter().collect::<Vec<_>>();
    let total_cells = satisfied_cells.len() + missing_cells.len();
    let score_0_10000 = if total_cells == 0 {
        0
    } else {
        ((satisfied_cells.len() as f64 / total_cells as f64) * 10000.0).round() as u32
    };
    let summary = if total_cells == 0 {
        format!("Contract '{}' has no assessable parity cells", contract.id)
    } else if missing_cells.is_empty() {
        format!(
            "Contract '{}' satisfied {} parity cells",
            contract.id,
            satisfied_cells.len()
        )
    } else {
        format!(
            "Contract '{}' is missing parity cells: {}",
            contract.id,
            missing_cells
                .iter()
                .map(|cell| cell.kind.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )
    };

    ContractParityReport {
        id: contract.id.clone(),
        kind: contract.kind.clone(),
        priority: contract.priority.clone(),
        files,
        satisfied_cells,
        missing_cells,
        score_0_10000,
        summary,
    }
}

fn push_symbol_cell(
    satisfied: &mut BTreeSet<ParityCell>,
    missing: &mut BTreeSet<ParityCell>,
    files: &mut BTreeSet<String>,
    semantic: &SemanticSnapshot,
    target: Option<&str>,
    kind: &str,
    label: &str,
) {
    let Some(target) = target else {
        return;
    };
    let (path, symbol_name) = match target.split_once("::") {
        Some(parts) => parts,
        None => return,
    };
    files.insert(path.to_string());
    let present = semantic
        .symbols
        .iter()
        .any(|symbol| symbol.path == path && symbol.name == symbol_name);
    let cell = ParityCell {
        kind: kind.to_string(),
        status: if present { "satisfied" } else { "missing" }.to_string(),
        detail: if present {
            format!("{label}: {target}")
        } else {
            format!("{label} missing: {target}")
        },
        path: Some(path.to_string()),
    };
    if present {
        satisfied.insert(cell);
    } else {
        missing.insert(cell);
    }
}

fn push_file_cell(
    satisfied: &mut BTreeSet<ParityCell>,
    missing: &mut BTreeSet<ParityCell>,
    files: &mut BTreeSet<String>,
    root: &Path,
    path: Option<&str>,
    kind: &str,
    label: &str,
) {
    let Some(path) = path else {
        return;
    };
    files.insert(path.to_string());
    let present = root.join(path).exists();
    let cell = ParityCell {
        kind: kind.to_string(),
        status: if present { "satisfied" } else { "missing" }.to_string(),
        detail: if present {
            format!("{label}: {path}")
        } else {
            format!("{label} missing: {path}")
        },
        path: Some(path.to_string()),
    };
    if present {
        satisfied.insert(cell);
    } else {
        missing.insert(cell);
    }
}

fn push_runtime_binding_cell(
    satisfied: &mut BTreeSet<ParityCell>,
    missing: &mut BTreeSet<ParityCell>,
    path: Option<&str>,
    present: bool,
    kind: &str,
) {
    let Some(path) = path else {
        return;
    };
    let cell = ParityCell {
        kind: kind.to_string(),
        status: if present { "satisfied" } else { "missing" }.to_string(),
        detail: if present {
            format!("runtime entry references contract-family symbols: {path}")
        } else {
            format!("runtime entry does not reference contract-family symbols: {path}")
        },
        path: Some(path.to_string()),
    };
    if present {
        satisfied.insert(cell);
    } else {
        missing.insert(cell);
    }
}

fn contract_in_scope(
    contract: &ContractRule,
    scope: ParityScope,
    changed_files: &BTreeSet<String>,
) -> bool {
    if scope == ParityScope::All {
        return true;
    }
    if changed_files.is_empty() {
        return false;
    }
    contract_rule_paths(contract)
        .iter()
        .any(|pattern| changed_files.iter().any(|path| path_matches(pattern, path)))
}

fn contract_rule_paths(contract: &ContractRule) -> Vec<String> {
    let mut paths = Vec::new();
    for scoped_path in [
        contract.categories_symbol.as_deref(),
        contract.payload_map_symbol.as_deref(),
        contract.registry_symbol.as_deref(),
    ]
    .into_iter()
    .flatten()
    {
        if let Some((path, _)) = scoped_path.split_once("::") {
            paths.push(path.to_string());
        }
    }
    paths.extend(contract.browser_entry.iter().cloned());
    paths.extend(contract.electron_entry.iter().cloned());
    paths
}

fn contract_symbol_names(
    config: &RulesConfig,
    contract: &ContractRule,
    semantic: &SemanticSnapshot,
) -> HashSet<String> {
    let mut symbol_names = HashSet::new();
    let related_paths = contract_related_paths(config, contract);

    for scoped_symbol in contract_scoped_symbols(contract) {
        if let Some((_, symbol_name)) = scoped_symbol.split_once("::") {
            symbol_names.insert(symbol_name.to_string());
        }
    }

    for symbol in &semantic.symbols {
        if related_paths.contains(&symbol.path) {
            symbol_names.insert(symbol.name.clone());
        }
    }

    symbol_names
}

fn contract_related_paths(config: &RulesConfig, contract: &ContractRule) -> HashSet<String> {
    let mut paths = contract_rule_paths(contract)
        .into_iter()
        .collect::<HashSet<_>>();

    if let Some(concept) = config
        .concept
        .iter()
        .find(|concept| concept.id == contract.id)
    {
        paths.extend(concept_binding_paths(concept));
    }

    paths
}

fn concept_binding_paths(concept: &ConceptRule) -> HashSet<String> {
    concept
        .anchors
        .iter()
        .chain(concept.canonical_accessors.iter())
        .filter_map(|value| value.split_once("::").map(|(path, _)| path.to_string()))
        .collect()
}

fn contract_scoped_symbols(contract: &ContractRule) -> impl Iterator<Item = &str> {
    [
        contract.categories_symbol.as_deref(),
        contract.payload_map_symbol.as_deref(),
        contract.registry_symbol.as_deref(),
    ]
    .into_iter()
    .flatten()
}

fn file_mentions_any_symbol(root: &Path, path: &str, symbol_names: &HashSet<String>) -> bool {
    if symbol_names.is_empty() {
        return false;
    }
    let Ok(content) = std::fs::read_to_string(root.join(path)) else {
        return false;
    };
    let tokens = identifier_tokens(&content);
    symbol_names
        .iter()
        .any(|symbol_name| tokens.contains(symbol_name))
}

fn file_exists(root: &Path, path: &str) -> bool {
    root.join(path).exists()
}

fn file_has_version_marker(root: &Path, path: &str) -> bool {
    let Ok(content) = std::fs::read_to_string(root.join(path)) else {
        return false;
    };
    let lowered_tokens = identifier_tokens(&content)
        .into_iter()
        .map(|token| token.to_ascii_lowercase())
        .collect::<HashSet<_>>();
    lowered_tokens.contains("version")
        || lowered_tokens.contains("schemaversion")
        || lowered_tokens.contains("schema_version")
}

fn identifier_tokens(content: &str) -> HashSet<String> {
    content
        .split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
        .filter(|token| !token.is_empty())
        .map(|token| token.to_string())
        .collect()
}

fn finding_kind_and_severity(cell: &ParityCell) -> (&'static str, &'static str) {
    match cell.kind.as_str() {
        "browser_entry" => ("missing_browser_path", "high"),
        "electron_entry" => ("missing_electron_path", "high"),
        "versioning" => ("parity_version_gap", "medium"),
        "snapshot_without_live_update" => ("snapshot_without_live_update", "high"),
        "browser_live_update_binding" | "electron_live_update_binding" => {
            ("missing_live_update_path", "high")
        }
        _ => ("missing_contract_symbol", "high"),
    }
}

fn path_matches(pattern: &str, path: &str) -> bool {
    rules::glob_match(pattern, path) || pattern == path
}

#[cfg(test)]
mod tests {
    use super::{build_parity_reports, parity_score_0_10000, ContractParityReport, ParityScope};
    use crate::analysis::semantic::{
        ProjectModel, SemanticCapability, SemanticFileFact, SemanticSnapshot, SymbolFact,
    };
    use crate::metrics::rules::RulesConfig;
    use std::collections::BTreeSet;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root(label: &str) -> std::path::PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "sentrux-parity-{label}-{}-{unique}",
            std::process::id()
        ));
        std::fs::create_dir_all(root.join("src/domain")).expect("create domain dir");
        std::fs::create_dir_all(root.join("src/app")).expect("create app dir");
        std::fs::create_dir_all(root.join("src/runtime")).expect("create runtime dir");
        root
    }

    fn report_by_id<'a>(reports: &'a [ContractParityReport], id: &str) -> &'a ContractParityReport {
        reports
            .iter()
            .find(|report| report.id == id)
            .expect("report")
    }

    #[test]
    fn reports_missing_runtime_and_versioning_cells() {
        let root = temp_root("missing-runtime");
        std::fs::write(
            root.join("src/domain/bootstrap.ts"),
            "export const SERVER_STATE_BOOTSTRAP_CATEGORIES = ['task'];\n",
        )
        .expect("write categories");
        std::fs::write(
            root.join("src/app/registry.ts"),
            "export const SERVER_STATE_BOOTSTRAP_REGISTRY = {};\n",
        )
        .expect("write registry");

        let config: RulesConfig = toml::from_str(
            r#"
                [[contract]]
                id = "server_state_bootstrap"
                categories_symbol = "src/domain/bootstrap.ts::SERVER_STATE_BOOTSTRAP_CATEGORIES"
                registry_symbol = "src/app/registry.ts::SERVER_STATE_BOOTSTRAP_REGISTRY"
                browser_entry = "src/runtime/browser.ts"
                required_capabilities = ["snapshot", "live_updates", "versioning"]
            "#,
        )
        .expect("rules config");
        let semantic = SemanticSnapshot {
            project: ProjectModel::default(),
            analyzed_files: 2,
            capabilities: vec![SemanticCapability::Symbols],
            files: vec![SemanticFileFact::default()],
            symbols: vec![
                SymbolFact {
                    id: "categories".to_string(),
                    path: "src/domain/bootstrap.ts".to_string(),
                    name: "SERVER_STATE_BOOTSTRAP_CATEGORIES".to_string(),
                    kind: "const".to_string(),
                    line: 1,
                },
                SymbolFact {
                    id: "registry".to_string(),
                    path: "src/app/registry.ts".to_string(),
                    name: "SERVER_STATE_BOOTSTRAP_REGISTRY".to_string(),
                    kind: "const".to_string(),
                    line: 1,
                },
            ],
            reads: Vec::new(),
            writes: Vec::new(),
            closed_domains: Vec::new(),
            closed_domain_sites: Vec::new(),
        };

        let reports = build_parity_reports(
            &config,
            &semantic,
            &root,
            ParityScope::All,
            &BTreeSet::new(),
        );
        let report = report_by_id(&reports, "server_state_bootstrap");

        assert!(report
            .missing_cells
            .iter()
            .any(|cell| cell.kind == "browser_entry"));
        assert!(!report
            .missing_cells
            .iter()
            .any(|cell| cell.kind == "browser_live_update_binding"));
        assert!(report
            .missing_cells
            .iter()
            .any(|cell| cell.kind == "versioning"));
        assert!(parity_score_0_10000(&reports) < 10000);

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn changed_scope_filters_untouched_contracts() {
        let root = temp_root("changed-scope");
        let config: RulesConfig = toml::from_str(
            r#"
                [[contract]]
                id = "bootstrap"
                categories_symbol = "src/domain/bootstrap.ts::SERVER_STATE_BOOTSTRAP_CATEGORIES"
                registry_symbol = "src/app/registry.ts::SERVER_STATE_BOOTSTRAP_REGISTRY"
                browser_entry = "src/runtime/browser.ts"
            "#,
        )
        .expect("rules config");
        let semantic = SemanticSnapshot::default();
        let changed_files = BTreeSet::from(["src/runtime/browser.ts".to_string()]);

        let reports = build_parity_reports(
            &config,
            &semantic,
            &root,
            ParityScope::Changed,
            &changed_files,
        );

        assert_eq!(reports.len(), 1);

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn runtime_binding_requires_exact_identifier_match() {
        let root = temp_root("binding-token");
        std::fs::write(
            root.join("src/domain/bootstrap.ts"),
            "export const SERVER_STATE_BOOTSTRAP_CATEGORIES = ['task'];\n",
        )
        .expect("write categories");
        std::fs::write(
            root.join("src/app/registry.ts"),
            "export const SERVER_STATE_BOOTSTRAP_REGISTRY = {};\n",
        )
        .expect("write registry");
        std::fs::create_dir_all(root.join("src/runtime")).expect("create runtime dir");
        std::fs::write(
            root.join("src/runtime/browser.ts"),
            "const SERVER_STATE_BOOTSTRAP_CATEGORIES_WRAPPER = true;\n",
        )
        .expect("write browser");

        let config: RulesConfig = toml::from_str(
            r#"
                [[contract]]
                id = "bootstrap"
                categories_symbol = "src/domain/bootstrap.ts::SERVER_STATE_BOOTSTRAP_CATEGORIES"
                registry_symbol = "src/app/registry.ts::SERVER_STATE_BOOTSTRAP_REGISTRY"
                browser_entry = "src/runtime/browser.ts"
                required_capabilities = ["live_updates"]
            "#,
        )
        .expect("rules config");
        let semantic = SemanticSnapshot {
            project: ProjectModel::default(),
            analyzed_files: 3,
            capabilities: vec![SemanticCapability::Symbols],
            files: vec![SemanticFileFact::default()],
            symbols: vec![
                SymbolFact {
                    id: "categories".to_string(),
                    path: "src/domain/bootstrap.ts".to_string(),
                    name: "SERVER_STATE_BOOTSTRAP_CATEGORIES".to_string(),
                    kind: "const".to_string(),
                    line: 1,
                },
                SymbolFact {
                    id: "registry".to_string(),
                    path: "src/app/registry.ts".to_string(),
                    name: "SERVER_STATE_BOOTSTRAP_REGISTRY".to_string(),
                    kind: "const".to_string(),
                    line: 1,
                },
            ],
            reads: Vec::new(),
            writes: Vec::new(),
            closed_domains: Vec::new(),
            closed_domain_sites: Vec::new(),
        };

        let reports = build_parity_reports(
            &config,
            &semantic,
            &root,
            ParityScope::All,
            &BTreeSet::new(),
        );
        let report = report_by_id(&reports, "bootstrap");

        assert!(report
            .missing_cells
            .iter()
            .any(|cell| cell.kind == "browser_live_update_binding"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn runtime_binding_accepts_related_contract_family_symbols() {
        let root = temp_root("binding-family");
        std::fs::write(
            root.join("src/domain/bootstrap.ts"),
            r#"
                export const SERVER_STATE_BOOTSTRAP_CATEGORIES = ['task'];
                export type AnyServerStateBootstrapSnapshot = { category: 'task' };
            "#,
        )
        .expect("write categories");
        std::fs::write(
            root.join("src/app/registry.ts"),
            "export const SERVER_STATE_BOOTSTRAP_REGISTRY = {};\n",
        )
        .expect("write registry");
        std::fs::write(
            root.join("src/app/bootstrap.ts"),
            "export function replaceServerStateBootstrap(): void {}\n",
        )
        .expect("write bootstrap");
        std::fs::write(
            root.join("src/runtime/browser.ts"),
            "import type { AnyServerStateBootstrapSnapshot } from '../domain/bootstrap';\nconst apply = (_snapshots: AnyServerStateBootstrapSnapshot[]) => {};\n",
        )
        .expect("write browser");
        std::fs::write(
            root.join("src/runtime/electron.ts"),
            "import { replaceServerStateBootstrap } from '../app/bootstrap';\nreplaceServerStateBootstrap();\n",
        )
        .expect("write electron");

        let config: RulesConfig = toml::from_str(
            r#"
                [[concept]]
                id = "bootstrap"
                anchors = ["src/app/bootstrap.ts::replaceServerStateBootstrap"]

                [[contract]]
                id = "bootstrap"
                categories_symbol = "src/domain/bootstrap.ts::SERVER_STATE_BOOTSTRAP_CATEGORIES"
                registry_symbol = "src/app/registry.ts::SERVER_STATE_BOOTSTRAP_REGISTRY"
                browser_entry = "src/runtime/browser.ts"
                electron_entry = "src/runtime/electron.ts"
                required_capabilities = ["snapshot", "live_updates"]
            "#,
        )
        .expect("rules config");
        let semantic = SemanticSnapshot {
            project: ProjectModel::default(),
            analyzed_files: 5,
            capabilities: vec![SemanticCapability::Symbols],
            files: vec![SemanticFileFact::default()],
            symbols: vec![
                SymbolFact {
                    id: "categories".to_string(),
                    path: "src/domain/bootstrap.ts".to_string(),
                    name: "SERVER_STATE_BOOTSTRAP_CATEGORIES".to_string(),
                    kind: "const".to_string(),
                    line: 2,
                },
                SymbolFact {
                    id: "bootstrap-type".to_string(),
                    path: "src/domain/bootstrap.ts".to_string(),
                    name: "AnyServerStateBootstrapSnapshot".to_string(),
                    kind: "type".to_string(),
                    line: 3,
                },
                SymbolFact {
                    id: "registry".to_string(),
                    path: "src/app/registry.ts".to_string(),
                    name: "SERVER_STATE_BOOTSTRAP_REGISTRY".to_string(),
                    kind: "const".to_string(),
                    line: 1,
                },
                SymbolFact {
                    id: "replace".to_string(),
                    path: "src/app/bootstrap.ts".to_string(),
                    name: "replaceServerStateBootstrap".to_string(),
                    kind: "function".to_string(),
                    line: 1,
                },
            ],
            reads: Vec::new(),
            writes: Vec::new(),
            closed_domains: Vec::new(),
            closed_domain_sites: Vec::new(),
        };

        let reports = build_parity_reports(
            &config,
            &semantic,
            &root,
            ParityScope::All,
            &BTreeSet::new(),
        );
        let report = report_by_id(&reports, "bootstrap");

        assert!(report
            .satisfied_cells
            .iter()
            .any(|cell| cell.kind == "browser_live_update_binding"));
        assert!(report
            .satisfied_cells
            .iter()
            .any(|cell| cell.kind == "electron_live_update_binding"));
        assert!(!report
            .missing_cells
            .iter()
            .any(|cell| cell.kind == "snapshot_without_live_update"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn report_without_assessable_cells_scores_zero() {
        let root = temp_root("no-cells");
        let config: RulesConfig = toml::from_str(
            r#"
                [[contract]]
                id = "empty_contract"
            "#,
        )
        .expect("rules config");

        let reports = build_parity_reports(
            &config,
            &SemanticSnapshot::default(),
            &root,
            ParityScope::All,
            &BTreeSet::new(),
        );
        let report = report_by_id(&reports, "empty_contract");

        assert_eq!(report.score_0_10000, 0);
        assert!(report.summary.contains("no assessable parity cells"));

        let _ = std::fs::remove_dir_all(root);
    }
}
