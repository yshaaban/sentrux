use super::obligations_domain::concept_rule_paths;
use super::{
    obligation_report_confidence, obligation_report_origin, obligation_report_score,
    obligation_report_severity, obligation_report_trust_tier, ObligationReport, ObligationScope,
    ObligationSite,
};
use crate::analysis::semantic::SemanticSnapshot;
use crate::metrics::rules::{self, ContractRule, RulesConfig};
use crate::metrics::testgap::is_test_file;
use std::collections::BTreeSet;

#[derive(Clone, Copy)]
struct ContractSymbolTarget<'a> {
    scoped_symbol: &'a str,
    kind: &'static str,
    detail: &'static str,
}

#[derive(Clone, Copy)]
struct ContractFileTarget<'a> {
    path: &'a str,
    kind: &'static str,
    detail: &'static str,
}

pub(super) fn build_contract_obligation(
    config: &RulesConfig,
    contract: &ContractRule,
    semantic: &SemanticSnapshot,
    scope: ObligationScope,
    changed_files: &BTreeSet<String>,
) -> Option<ObligationReport> {
    let trigger_paths = contract_trigger_paths(config, contract, semantic);
    let triggered = contract_is_triggered(&trigger_paths, changed_files);
    let required_sites = contract_required_sites(contract, semantic);
    if required_sites.is_empty() {
        return None;
    }

    let mut files = required_sites
        .iter()
        .map(|site| site.path.clone())
        .collect::<BTreeSet<_>>();
    files.extend(trigger_paths.iter().cloned());
    let missing_structural_sites = required_sites
        .iter()
        .filter(|site| is_missing_contract_site(site))
        .cloned()
        .collect::<Vec<_>>();

    if scope == ObligationScope::Changed && !triggered && missing_structural_sites.is_empty() {
        return None;
    }
    if scope == ObligationScope::All && missing_structural_sites.is_empty() {
        return None;
    }

    let mut satisfied_sites = Vec::new();
    let mut missing_sites = Vec::new();
    for site in &required_sites {
        if is_missing_contract_site(site) {
            missing_sites.push(site.clone());
            continue;
        }

        if scope == ObligationScope::Changed && triggered {
            if changed_files.contains(&site.path) {
                satisfied_sites.push(site.clone());
            } else {
                missing_sites.push(site.clone());
            }
        } else {
            satisfied_sites.push(site.clone());
        }
    }

    let summary =
        contract_obligation_summary(contract, &trigger_paths, changed_files, &missing_sites);
    let origin = obligation_report_origin(true);
    let severity =
        obligation_report_severity("contract_surface_completeness", origin, missing_sites.len());

    Some(ObligationReport {
        id: format!("contract::{}", contract.id),
        kind: "contract_surface_completeness".to_string(),
        concept_id: Some(contract.id.clone()),
        domain_symbol_name: None,
        origin,
        trust_tier: obligation_report_trust_tier(origin),
        confidence: obligation_report_confidence(origin),
        severity,
        score_0_10000: obligation_report_score(severity, origin, missing_sites.len()),
        summary,
        files: files.into_iter().collect(),
        required_sites: required_sites.clone(),
        satisfied_sites,
        missing_sites,
        missing_variants: Vec::new(),
        context_burden: required_sites.len(),
    })
}

fn contract_required_sites(
    contract: &ContractRule,
    semantic: &SemanticSnapshot,
) -> Vec<ObligationSite> {
    let mut sites = BTreeSet::new();

    for target in contract_required_symbol_targets(contract) {
        push_contract_symbol_site(
            &mut sites,
            semantic,
            Some(target.scoped_symbol),
            target.kind,
            target.detail,
        );
    }
    for target in contract_required_file_targets(contract) {
        push_contract_file_site(
            &mut sites,
            semantic,
            Some(target.path),
            target.kind,
            target.detail,
        );
    }

    sort_contract_sites(sites)
}

fn contract_required_symbol_targets(contract: &ContractRule) -> Vec<ContractSymbolTarget<'_>> {
    let mut targets = Vec::new();

    if let Some(scoped_symbol) = contract.categories_symbol.as_deref() {
        targets.push(ContractSymbolTarget {
            scoped_symbol,
            kind: "categories_symbol",
            detail: "update categories symbol",
        });
    }
    if let Some(scoped_symbol) = contract.payload_map_symbol.as_deref() {
        targets.push(ContractSymbolTarget {
            scoped_symbol,
            kind: "payload_map_symbol",
            detail: "update payload map symbol",
        });
    }
    if let Some(scoped_symbol) = contract.registry_symbol.as_deref() {
        targets.push(ContractSymbolTarget {
            scoped_symbol,
            kind: "registry_symbol",
            detail: "update registry symbol",
        });
    }
    targets.extend(
        contract
            .required_symbols
            .iter()
            .map(|scoped_symbol| ContractSymbolTarget {
                scoped_symbol,
                kind: "required_symbol",
                detail: "update required contract symbol",
            }),
    );

    targets
}

fn contract_required_file_targets(contract: &ContractRule) -> Vec<ContractFileTarget<'_>> {
    let mut targets = Vec::new();

    if let Some(path) = contract.browser_entry.as_deref() {
        targets.push(ContractFileTarget {
            path,
            kind: "browser_entry",
            detail: "update browser runtime entry",
        });
    }
    if let Some(path) = contract.electron_entry.as_deref() {
        targets.push(ContractFileTarget {
            path,
            kind: "electron_entry",
            detail: "update electron runtime entry",
        });
    }
    targets.extend(
        contract
            .required_files
            .iter()
            .map(|path| ContractFileTarget {
                path,
                kind: "required_file",
                detail: "update required contract file",
            }),
    );

    targets
}

fn contract_trigger_symbol_targets(contract: &ContractRule) -> Vec<&str> {
    let mut targets = contract_required_symbol_targets(contract)
        .into_iter()
        .map(|target| target.scoped_symbol)
        .collect::<Vec<_>>();
    targets.extend(contract.trigger_symbols.iter().map(String::as_str));
    targets
}

fn contract_trigger_file_paths(contract: &ContractRule) -> Vec<&str> {
    let mut paths = contract_required_file_targets(contract)
        .into_iter()
        .map(|target| target.path)
        .collect::<Vec<_>>();
    paths.extend(contract.trigger_files.iter().map(String::as_str));
    paths
}

fn push_contract_symbol_site(
    sites: &mut BTreeSet<ObligationSite>,
    semantic: &SemanticSnapshot,
    scoped_symbol: Option<&str>,
    kind: &str,
    detail: &str,
) {
    let Some((path, symbol_name)) = scoped_symbol.and_then(|value| value.split_once("::")) else {
        return;
    };

    let symbol = semantic
        .symbols
        .iter()
        .find(|symbol| symbol.path == path && symbol.name == symbol_name);
    let site_detail = if symbol.is_some() {
        detail.to_string()
    } else {
        missing_contract_site_detail(symbol_name)
    };
    let line = symbol.map(|symbol| symbol.line);

    sites.insert(ObligationSite {
        path: path.to_string(),
        kind: kind.to_string(),
        line,
        detail: site_detail,
    });
}

fn push_contract_file_site(
    sites: &mut BTreeSet<ObligationSite>,
    semantic: &SemanticSnapshot,
    path: Option<&str>,
    kind: &str,
    detail: &str,
) {
    let Some(path) = path else {
        return;
    };
    let present = semantic.files.iter().any(|file| file.path == path);
    sites.insert(ObligationSite {
        path: path.to_string(),
        kind: kind.to_string(),
        line: None,
        detail: if present {
            detail.to_string()
        } else {
            missing_contract_site_detail(path)
        },
    });
}

fn contract_trigger_paths(
    config: &RulesConfig,
    contract: &ContractRule,
    semantic: &SemanticSnapshot,
) -> Vec<String> {
    let mut paths = BTreeSet::new();

    for scoped_symbol in contract_trigger_symbol_targets(contract) {
        if let Some(path) = scoped_symbol_path(scoped_symbol) {
            paths.insert(path);
        }
    }
    for path in contract_trigger_file_paths(contract) {
        paths.insert(path.to_string());
    }
    paths.extend(contract_related_concept_paths(config, contract));
    paths.extend(contract_related_semantic_paths(contract, semantic));

    paths.into_iter().collect()
}

fn contract_is_triggered(trigger_paths: &[String], changed_files: &BTreeSet<String>) -> bool {
    if changed_files.is_empty() {
        return false;
    }

    trigger_paths
        .iter()
        .any(|pattern| changed_files.iter().any(|path| path_matches(pattern, path)))
}

fn contract_obligation_summary(
    contract: &ContractRule,
    trigger_paths: &[String],
    changed_files: &BTreeSet<String>,
    missing_sites: &[ObligationSite],
) -> String {
    if missing_sites.is_empty() {
        return format!(
            "Contract '{}' is fully updated across all required surfaces",
            contract.id
        );
    }

    let missing_summary = summarize_contract_missing_sites(missing_sites);
    let changed_triggers = changed_contract_trigger_paths(trigger_paths, changed_files);
    if !changed_triggers.is_empty() {
        return format!(
            "Contract '{}' changed in {} but {} were not updated",
            contract.id,
            changed_triggers.join(", "),
            missing_summary
        );
    }

    format!("Contract '{}' is missing {}", contract.id, missing_summary)
}

fn contract_related_concept_paths(
    config: &RulesConfig,
    contract: &ContractRule,
) -> BTreeSet<String> {
    config
        .concept
        .iter()
        .filter(|concept| concept.id == contract.id)
        .flat_map(concept_rule_paths)
        .filter(|path| !is_test_file(path))
        .collect()
}

fn contract_related_semantic_paths(
    contract: &ContractRule,
    semantic: &SemanticSnapshot,
) -> BTreeSet<String> {
    let symbol_names = contract_trigger_symbol_names(contract);
    if symbol_names.is_empty() {
        return BTreeSet::new();
    }

    let mut paths = BTreeSet::new();
    for read in &semantic.reads {
        if !is_test_file(&read.path) && contract_symbol_match_any(&symbol_names, &read.symbol_name)
        {
            paths.insert(read.path.clone());
        }
    }
    for write in &semantic.writes {
        if !is_test_file(&write.path)
            && contract_symbol_match_any(&symbol_names, &write.symbol_name)
        {
            paths.insert(write.path.clone());
        }
    }
    for symbol in &semantic.symbols {
        if !is_test_file(&symbol.path) && contract_symbol_match_any(&symbol_names, &symbol.name) {
            paths.insert(symbol.path.clone());
        }
    }

    paths
}

fn contract_trigger_symbol_names(contract: &ContractRule) -> BTreeSet<String> {
    let mut symbol_names = BTreeSet::new();

    for scoped_symbol in contract_trigger_symbol_targets(contract) {
        if let Some(symbol_name) = scoped_symbol_name(scoped_symbol) {
            symbol_names.insert(symbol_name.to_string());
        }
    }

    symbol_names
}

fn changed_contract_trigger_paths(
    trigger_paths: &[String],
    changed_files: &BTreeSet<String>,
) -> Vec<String> {
    trigger_paths
        .iter()
        .filter(|pattern| changed_files.iter().any(|path| path_matches(pattern, path)))
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn is_missing_contract_site(site: &ObligationSite) -> bool {
    site.detail.starts_with("declared contract site is missing")
}

fn missing_contract_site_detail(target: &str) -> String {
    format!("declared contract site is missing from semantic snapshot: {target}")
}

fn scoped_symbol_path(value: &str) -> Option<String> {
    value.split_once("::").map(|(path, _)| path.to_string())
}

fn scoped_symbol_name(value: &str) -> Option<&str> {
    value.split_once("::").map(|(_, symbol_name)| symbol_name)
}

fn contract_symbol_match_any(symbol_names: &BTreeSet<String>, candidate: &str) -> bool {
    symbol_names
        .iter()
        .any(|symbol_name| contract_symbol_matches(symbol_name, candidate))
}

fn contract_symbol_matches(symbol_name: &str, candidate: &str) -> bool {
    candidate == symbol_name
        || candidate
            .strip_prefix(symbol_name)
            .map(|suffix| suffix.starts_with('.'))
            .unwrap_or(false)
}

fn sort_contract_sites(sites: BTreeSet<ObligationSite>) -> Vec<ObligationSite> {
    let mut sorted = sites.into_iter().collect::<Vec<_>>();
    sorted.sort_by(|left, right| {
        contract_site_priority(left)
            .cmp(&contract_site_priority(right))
            .then(left.path.cmp(&right.path))
            .then(left.kind.cmp(&right.kind))
            .then(left.line.cmp(&right.line))
    });
    sorted
}

fn contract_site_priority(site: &ObligationSite) -> u8 {
    match site.kind.as_str() {
        "browser_entry" | "electron_entry" => 0,
        "registry_symbol" => 1,
        "payload_map_symbol" | "categories_symbol" => 2,
        "required_symbol" | "required_file" => {
            if contract_site_has_boundary_risk(site) {
                3
            } else {
                4
            }
        }
        _ => 5,
    }
}

fn contract_site_has_boundary_risk(site: &ObligationSite) -> bool {
    let lowered_path = site.path.to_ascii_lowercase();
    let lowered_detail = site.detail.to_ascii_lowercase();
    let risky_markers = [
        "adapter",
        "browser",
        "client",
        "desktop",
        "electron",
        "hydrate",
        "ipc",
        "persist",
        "restore",
        "rpc",
        "serialize",
        "server",
        "session",
        "transport",
        "websocket",
        "ws",
    ];

    risky_markers
        .iter()
        .any(|marker| lowered_path.contains(marker) || lowered_detail.contains(marker))
}

pub(super) fn summarize_contract_missing_sites(missing_sites: &[ObligationSite]) -> String {
    let labels = dedup_labels_preserving_order(
        missing_sites
            .iter()
            .map(contract_site_summary_label)
            .collect::<Vec<_>>(),
    );
    if labels.len() == 1 {
        return format!("the {} surface", labels[0]);
    }
    if labels.len() == 2 {
        return format!("the {} and {} surfaces", labels[0], labels[1]);
    }

    format!(
        "the {}, {} and {} other required surfaces",
        labels[0],
        labels[1],
        labels.len() - 2
    )
}

fn contract_site_summary_label(site: &ObligationSite) -> &'static str {
    match site.kind.as_str() {
        "browser_entry" => "browser runtime entry",
        "electron_entry" => "electron runtime entry",
        "registry_symbol" => "registry",
        "payload_map_symbol" => "payload map",
        "categories_symbol" => "categories",
        "required_symbol" => "required symbol",
        "required_file" => "required file",
        _ => "declared contract",
    }
}

fn dedup_labels_preserving_order(labels: Vec<&'static str>) -> Vec<&'static str> {
    let mut seen = BTreeSet::new();
    let mut deduped = Vec::new();
    for label in labels {
        if seen.insert(label) {
            deduped.push(label);
        }
    }
    deduped
}

pub(super) fn path_matches(pattern: &str, path: &str) -> bool {
    if rules::glob_match(pattern, path) {
        return true;
    }
    pattern == path
}
