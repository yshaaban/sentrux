use super::parity::{
    ContractParityReport, ParityCell, ParityCellKind, ParityFindingKind, ParityScope,
    MISSING_STATUS, SATISFIED_STATUS,
};
use super::FindingSeverity;
use crate::analysis::semantic::SemanticSnapshot;
use crate::metrics::rules::{self, ConceptRule, ContractRule, RulesConfig};
use std::collections::{BTreeSet, HashSet};
use std::path::Path;

pub(crate) fn build_contract_parity_report(
    config: &RulesConfig,
    contract: &ContractRule,
    semantic: &SemanticSnapshot,
    root: &Path,
    read_warnings: &mut Vec<String>,
) -> ContractParityReport {
    let mut satisfied = BTreeSet::new();
    let mut missing = BTreeSet::new();
    let mut files = BTreeSet::new();

    let (browser_entry_present, electron_entry_present) = push_contract_presence_cells(
        &mut satisfied,
        &mut missing,
        &mut files,
        semantic,
        contract,
        root,
    );

    let contract_symbol_names = contract_symbol_names(config, contract, semantic);
    let (browser_has_binding, electron_has_binding) = runtime_binding_status(
        root,
        contract,
        &contract_symbol_names,
        read_warnings,
        browser_entry_present,
        electron_entry_present,
    );

    if requires_capability(contract, "live_updates") {
        if browser_entry_present {
            push_runtime_binding_cell(
                &mut satisfied,
                &mut missing,
                contract.browser_entry.as_deref(),
                browser_has_binding,
                ParityCellKind::BrowserLiveUpdateBinding,
            );
        }
        if electron_entry_present {
            push_runtime_binding_cell(
                &mut satisfied,
                &mut missing,
                contract.electron_entry.as_deref(),
                electron_has_binding,
                ParityCellKind::ElectronLiveUpdateBinding,
            );
        }
    }

    push_snapshot_without_live_update_cell(
        &mut satisfied,
        &mut missing,
        contract,
        browser_entry_present,
        electron_entry_present,
        browser_has_binding,
        electron_has_binding,
    );
    push_versioning_cell(&mut satisfied, &mut missing, root, contract);

    build_contract_report(contract, satisfied, missing, files)
}

fn push_contract_presence_cells(
    satisfied: &mut BTreeSet<ParityCell>,
    missing: &mut BTreeSet<ParityCell>,
    files: &mut BTreeSet<String>,
    semantic: &SemanticSnapshot,
    contract: &ContractRule,
    root: &Path,
) -> (bool, bool) {
    push_symbol_cell(
        satisfied,
        missing,
        files,
        semantic,
        contract.categories_symbol.as_deref(),
        ParityCellKind::CategoriesSymbol,
        "categories symbol declared",
    );
    push_symbol_cell(
        satisfied,
        missing,
        files,
        semantic,
        contract.payload_map_symbol.as_deref(),
        ParityCellKind::PayloadMapSymbol,
        "payload map symbol declared",
    );
    push_symbol_cell(
        satisfied,
        missing,
        files,
        semantic,
        contract.registry_symbol.as_deref(),
        ParityCellKind::RegistrySymbol,
        "registry symbol declared",
    );

    let browser_entry_present = push_file_presence_cell(
        satisfied,
        missing,
        files,
        root,
        contract.browser_entry.as_deref(),
        ParityCellKind::BrowserEntry,
        "browser entry present",
    );
    let electron_entry_present = push_file_presence_cell(
        satisfied,
        missing,
        files,
        root,
        contract.electron_entry.as_deref(),
        ParityCellKind::ElectronEntry,
        "electron entry present",
    );

    (browser_entry_present, electron_entry_present)
}

fn runtime_binding_status(
    root: &Path,
    contract: &ContractRule,
    contract_symbol_names: &HashSet<String>,
    read_warnings: &mut Vec<String>,
    browser_entry_present: bool,
    electron_entry_present: bool,
) -> (bool, bool) {
    let browser_has_binding = runtime_binding_present(
        root,
        contract.browser_entry.as_deref(),
        contract_symbol_names,
        read_warnings,
        browser_entry_present,
    );
    let electron_has_binding = runtime_binding_present(
        root,
        contract.electron_entry.as_deref(),
        contract_symbol_names,
        read_warnings,
        electron_entry_present,
    );

    (browser_has_binding, electron_has_binding)
}

pub(crate) fn contract_in_scope(
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

pub(crate) fn finding_kind_and_severity(cell: &ParityCell) -> (ParityFindingKind, FindingSeverity) {
    match ParityCellKind::from_str(cell.kind.as_str()) {
        Some(ParityCellKind::BrowserEntry) => {
            (ParityFindingKind::MissingBrowserPath, FindingSeverity::High)
        }
        Some(ParityCellKind::ElectronEntry) => (
            ParityFindingKind::MissingElectronPath,
            FindingSeverity::High,
        ),
        Some(ParityCellKind::VersioningMarker) => {
            (ParityFindingKind::ParityVersionGap, FindingSeverity::Medium)
        }
        Some(ParityCellKind::SnapshotWithoutLiveUpdate) => (
            ParityFindingKind::SnapshotWithoutLiveUpdate,
            FindingSeverity::High,
        ),
        Some(ParityCellKind::BrowserLiveUpdateBinding)
        | Some(ParityCellKind::ElectronLiveUpdateBinding) => (
            ParityFindingKind::MissingLiveUpdatePath,
            FindingSeverity::High,
        ),
        _ => (
            ParityFindingKind::MissingContractSymbol,
            FindingSeverity::High,
        ),
    }
}

fn push_symbol_cell(
    satisfied: &mut BTreeSet<ParityCell>,
    missing: &mut BTreeSet<ParityCell>,
    files: &mut BTreeSet<String>,
    semantic: &SemanticSnapshot,
    target: Option<&str>,
    kind: ParityCellKind,
    label: &str,
) {
    let Some(target) = target else {
        return;
    };
    let Some((path, symbol_name)) = target.split_once("::") else {
        return;
    };

    files.insert(path.to_string());
    let present = semantic
        .symbols
        .iter()
        .any(|symbol| symbol.path == path && symbol.name == symbol_name);
    push_presence_cell(
        satisfied,
        missing,
        ParityCellInput {
            files: Some(files),
            path,
            kind,
            present,
            detail: format_symbol_detail(label, target, present),
        },
    );
}

fn push_file_presence_cell(
    satisfied: &mut BTreeSet<ParityCell>,
    missing: &mut BTreeSet<ParityCell>,
    files: &mut BTreeSet<String>,
    root: &Path,
    path: Option<&str>,
    kind: ParityCellKind,
    label: &str,
) -> bool {
    let Some(path) = path else {
        return false;
    };
    let present = root.join(path).exists();
    push_presence_cell(
        satisfied,
        missing,
        ParityCellInput {
            files: Some(files),
            path,
            kind,
            present,
            detail: format_symbol_detail(label, path, present),
        },
    );
    present
}

fn format_symbol_detail(label: &str, value: &str, present: bool) -> String {
    if present {
        format!("{label}: {value}")
    } else {
        format!("{label} missing: {value}")
    }
}

fn runtime_binding_present(
    root: &Path,
    path: Option<&str>,
    symbol_names: &HashSet<String>,
    read_warnings: &mut Vec<String>,
    entry_present: bool,
) -> bool {
    entry_present
        && path
            .map(|entry_path| {
                file_mentions_any_symbol(root, entry_path, symbol_names).unwrap_or_else(|error| {
                    read_warnings.push(error);
                    false
                })
            })
            .unwrap_or(false)
}

fn requires_capability(contract: &ContractRule, capability: &str) -> bool {
    contract
        .required_capabilities
        .iter()
        .any(|value| value == capability)
}

fn push_snapshot_without_live_update_cell(
    satisfied: &mut BTreeSet<ParityCell>,
    missing: &mut BTreeSet<ParityCell>,
    contract: &ContractRule,
    browser_entry_present: bool,
    electron_entry_present: bool,
    browser_has_binding: bool,
    electron_has_binding: bool,
) {
    if !requires_capability(contract, "snapshot")
        || (!browser_entry_present && !electron_entry_present)
        || browser_has_binding
        || electron_has_binding
    {
        return;
    }

    if let Some(path) = contract
        .browser_entry
        .as_deref()
        .or(contract.electron_entry.as_deref())
    {
        push_presence_cell(
            satisfied,
            missing,
            ParityCellInput {
                files: None,
                path,
                kind: ParityCellKind::SnapshotWithoutLiveUpdate,
                present: false,
                detail: "snapshot contract exists without any runtime binding".to_string(),
            },
        );
    }
}

fn push_versioning_cell(
    satisfied: &mut BTreeSet<ParityCell>,
    missing: &mut BTreeSet<ParityCell>,
    root: &Path,
    contract: &ContractRule,
) {
    if !requires_capability(contract, "versioning") {
        return;
    }

    let versioning_paths = contract_rule_paths(contract);
    let has_versioning = versioning_paths
        .iter()
        .any(|path| file_has_version_marker(root, path));
    let cell = ParityCell {
        kind: ParityCellKind::VersioningMarker.as_str().to_string(),
        status: if has_versioning {
            SATISFIED_STATUS
        } else {
            MISSING_STATUS
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

fn build_contract_report(
    contract: &ContractRule,
    satisfied: BTreeSet<ParityCell>,
    missing: BTreeSet<ParityCell>,
    files: BTreeSet<String>,
) -> ContractParityReport {
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

fn push_runtime_binding_cell(
    satisfied: &mut BTreeSet<ParityCell>,
    missing: &mut BTreeSet<ParityCell>,
    path: Option<&str>,
    present: bool,
    kind: ParityCellKind,
) {
    let Some(path) = path else {
        return;
    };

    push_presence_cell(
        satisfied,
        missing,
        ParityCellInput {
            files: None,
            path,
            kind,
            present,
            detail: if present {
                format!("runtime entry references contract-family symbols: {path}")
            } else {
                format!("runtime entry does not reference contract-family symbols: {path}")
            },
        },
    );
}

struct ParityCellInput<'a> {
    files: Option<&'a mut BTreeSet<String>>,
    path: &'a str,
    kind: ParityCellKind,
    present: bool,
    detail: String,
}

fn push_presence_cell(
    satisfied: &mut BTreeSet<ParityCell>,
    missing: &mut BTreeSet<ParityCell>,
    input: ParityCellInput<'_>,
) {
    let ParityCellInput {
        files,
        path,
        kind,
        present,
        detail,
    } = input;

    if let Some(files) = files {
        files.insert(path.to_string());
    }

    let cell = ParityCell {
        kind: kind.as_str().to_string(),
        status: if present {
            SATISFIED_STATUS
        } else {
            MISSING_STATUS
        }
        .to_string(),
        detail,
        path: Some(path.to_string()),
    };
    if present {
        satisfied.insert(cell);
    } else {
        missing.insert(cell);
    }
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

fn file_mentions_any_symbol(
    root: &Path,
    path: &str,
    symbol_names: &HashSet<String>,
) -> Result<bool, String> {
    if symbol_names.is_empty() {
        return Ok(false);
    }

    let content = std::fs::read_to_string(root.join(path))
        .map_err(|error| format!("Failed to read parity source '{path}': {error}"))?;
    let tokens = identifier_tokens(&content);
    Ok(symbol_names
        .iter()
        .any(|symbol_name| tokens.contains(symbol_name)))
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

fn path_matches(pattern: &str, path: &str) -> bool {
    rules::glob_match(pattern, path) || pattern == path
}
