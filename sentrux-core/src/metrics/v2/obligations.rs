//! Conservative obligation engine for closed-domain completeness.

use super::{concept_targets, symbol_matches_targets, SemanticFinding};
use crate::analysis::semantic::{ClosedDomain, ExhaustivenessSite, SemanticSnapshot};
use crate::metrics::rules::{self, ConceptRule, ContractRule, RulesConfig};
use crate::metrics::testgap::is_test_file;
use std::collections::{BTreeSet, HashSet};

const MAX_ZERO_CONFIG_DOMAIN_VARIANTS: usize = 16;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ObligationScope {
    All,
    Changed,
}

#[derive(Debug, Clone, serde::Serialize, Eq, PartialEq, Ord, PartialOrd)]
pub struct ObligationSite {
    pub path: String,
    pub kind: String,
    pub line: Option<u32>,
    pub detail: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ObligationReport {
    pub id: String,
    pub kind: String,
    pub concept_id: Option<String>,
    pub domain_symbol_name: Option<String>,
    pub summary: String,
    pub files: Vec<String>,
    pub required_sites: Vec<ObligationSite>,
    pub satisfied_sites: Vec<ObligationSite>,
    pub missing_sites: Vec<ObligationSite>,
    pub missing_variants: Vec<String>,
    pub context_burden: usize,
}

#[derive(Default)]
struct DomainSiteCoverage {
    files: BTreeSet<String>,
    required_sites: BTreeSet<ObligationSite>,
    satisfied_sites: BTreeSet<ObligationSite>,
    missing_sites: BTreeSet<ObligationSite>,
    missing_variants: BTreeSet<String>,
}

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

pub fn build_obligations(
    config: &RulesConfig,
    semantic: &SemanticSnapshot,
    scope: ObligationScope,
    changed_files: &BTreeSet<String>,
) -> Vec<ObligationReport> {
    let mut obligations = Vec::new();
    let mut covered_domains = HashSet::new();

    for concept in &config.concept {
        let concept_domains = relevant_domains(concept, semantic);
        if concept_domains.is_empty() {
            continue;
        }

        for domain in concept_domains {
            covered_domains.insert(domain.symbol_name.clone());
            if !domain_is_in_scope(concept, domain, semantic, scope, changed_files) {
                continue;
            }

            let report = build_concept_domain_obligation(concept, domain, semantic, changed_files);
            if report.context_burden > 0 {
                obligations.push(report);
            }
        }
    }

    for contract in &config.contract {
        if let Some(report) =
            build_contract_obligation(config, contract, semantic, scope, changed_files)
        {
            obligations.push(report);
        }
    }

    for domain in &semantic.closed_domains {
        if covered_domains.contains(&domain.symbol_name) {
            continue;
        }
        if !zero_config_domain_is_actionable(domain, semantic) {
            continue;
        }

        if scope == ObligationScope::Changed
            && !changed_files.contains(&domain.path)
            && !relevant_production_exhaustiveness_sites(domain, semantic)
                .iter()
                .any(|site| changed_files.contains(&site.path))
        {
            continue;
        }

        let report = build_zero_config_domain_obligation(domain, semantic);
        if !report.missing_sites.is_empty() {
            obligations.push(report);
        }
    }

    obligations.sort_by(|left, right| {
        left.concept_id
            .cmp(&right.concept_id)
            .then(left.domain_symbol_name.cmp(&right.domain_symbol_name))
            .then(left.id.cmp(&right.id))
    });
    obligations
}

fn build_contract_obligation(
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

    Some(ObligationReport {
        id: format!("contract::{}", contract.id),
        kind: "contract_surface_completeness".to_string(),
        concept_id: Some(contract.id.clone()),
        domain_symbol_name: None,
        summary,
        files: files.into_iter().collect(),
        required_sites: required_sites.clone(),
        satisfied_sites,
        missing_sites,
        missing_variants: Vec::new(),
        context_burden: required_sites.len(),
    })
}

pub fn build_obligation_findings(obligations: &[ObligationReport]) -> Vec<SemanticFinding> {
    obligations
        .iter()
        .filter(|obligation| !obligation.missing_sites.is_empty())
        .map(|obligation| {
            let severity = if obligation.kind == "closed_domain_exhaustiveness"
                && !obligation.missing_variants.is_empty()
            {
                "high"
            } else {
                "medium"
            };
            SemanticFinding {
                kind: obligation.kind.clone(),
                severity: severity.to_string(),
                concept_id: obligation
                    .concept_id
                    .clone()
                    .unwrap_or_else(|| obligation.domain_symbol_name.clone().unwrap_or_default()),
                summary: obligation.summary.clone(),
                files: obligation.files.clone(),
                evidence: obligation
                    .missing_sites
                    .iter()
                    .map(|site| format!("{} [{}]", site.path, site.detail))
                    .collect(),
            }
        })
        .collect()
}

pub fn changed_concepts_from_obligations(obligations: &[ObligationReport]) -> Vec<String> {
    let mut concepts = BTreeSet::new();
    for obligation in obligations {
        if let Some(concept_id) = &obligation.concept_id {
            concepts.insert(concept_id.clone());
        } else if let Some(domain_symbol_name) = &obligation.domain_symbol_name {
            concepts.insert(domain_symbol_name.clone());
        }
    }
    concepts.into_iter().collect()
}

pub fn changed_concept_ids_from_files(
    config: &RulesConfig,
    changed_files: &BTreeSet<String>,
) -> Vec<String> {
    let mut concepts = BTreeSet::new();

    for concept in &config.concept {
        if concept_rule_paths(concept)
            .iter()
            .any(|pattern| changed_files.iter().any(|path| path_matches(pattern, path)))
        {
            concepts.insert(concept.id.clone());
        }
    }

    concepts.into_iter().collect()
}

pub fn obligation_score_0_10000(obligations: &[ObligationReport]) -> u32 {
    let total_sites: usize = obligations
        .iter()
        .map(|obligation| obligation.required_sites.len())
        .sum();
    if total_sites == 0 {
        return 10000;
    }

    let satisfied_sites: usize = obligations
        .iter()
        .map(|obligation| obligation.satisfied_sites.len())
        .sum();
    ((satisfied_sites as f64 / total_sites as f64) * 10000.0).round() as u32
}

fn relevant_domains<'a>(
    concept: &ConceptRule,
    semantic: &'a SemanticSnapshot,
) -> Vec<&'a ClosedDomain> {
    let targets = concept_targets(concept);
    semantic
        .closed_domains
        .iter()
        .filter(|domain| symbol_matches_targets(&domain.symbol_name, &targets))
        .collect()
}

fn domain_is_in_scope(
    concept: &ConceptRule,
    domain: &ClosedDomain,
    semantic: &SemanticSnapshot,
    scope: ObligationScope,
    changed_files: &BTreeSet<String>,
) -> bool {
    if scope == ObligationScope::All {
        return true;
    }
    if changed_files.is_empty() {
        return false;
    }
    if changed_files.contains(&domain.path) {
        return true;
    }

    if concept_rule_paths(concept)
        .iter()
        .any(|pattern| changed_files.iter().any(|path| path_matches(pattern, path)))
    {
        return true;
    }

    semantic
        .closed_domain_sites
        .iter()
        .filter(|site| site.domain_symbol_name == domain.symbol_name)
        .any(|site| changed_files.contains(&site.path))
}

fn build_concept_domain_obligation(
    concept: &ConceptRule,
    domain: &ClosedDomain,
    semantic: &SemanticSnapshot,
    changed_files: &BTreeSet<String>,
) -> ObligationReport {
    let sites = relevant_exhaustiveness_sites(domain, semantic);
    let mut coverage = evaluate_domain_site_coverage(domain, &sites);
    coverage.files.insert(domain.path.clone());

    if sites.is_empty() {
        let missing_site = ObligationSite {
            path: domain.path.clone(),
            kind: "closed_domain".to_string(),
            line: Some(domain.line),
            detail: format!(
                "no exhaustive mapping or switch site found for '{}'",
                domain.symbol_name
            ),
        };
        coverage.required_sites.insert(missing_site.clone());
        coverage.missing_sites.insert(missing_site);
    }

    if !concept.related_tests.is_empty() && !changed_files.is_empty() {
        for pattern in &concept.related_tests {
            let test_site = ObligationSite {
                path: pattern.clone(),
                kind: "related_test".to_string(),
                line: None,
                detail: "related test coverage for changed concept".to_string(),
            };
            coverage.required_sites.insert(test_site.clone());
            if changed_files.iter().any(|path| path_matches(pattern, path)) {
                coverage.satisfied_sites.insert(test_site);
            } else {
                coverage.missing_sites.insert(test_site);
            }
        }
    }

    let required_sites = coverage.required_sites.into_iter().collect::<Vec<_>>();
    let satisfied_sites = coverage.satisfied_sites.into_iter().collect::<Vec<_>>();
    let missing_sites = coverage.missing_sites.into_iter().collect::<Vec<_>>();
    let missing_variants = coverage.missing_variants.into_iter().collect::<Vec<_>>();
    let files = coverage.files.into_iter().collect::<Vec<_>>();
    let context_burden = required_sites.len();

    ObligationReport {
        id: format!("{}::{}", concept.id, domain.symbol_name),
        kind: "closed_domain_exhaustiveness".to_string(),
        concept_id: Some(concept.id.clone()),
        domain_symbol_name: Some(domain.symbol_name.clone()),
        summary: obligation_summary(
            concept.id.as_str(),
            domain,
            &missing_sites,
            &missing_variants,
        ),
        files,
        required_sites,
        satisfied_sites,
        missing_sites,
        missing_variants,
        context_burden,
    }
}

fn build_zero_config_domain_obligation(
    domain: &ClosedDomain,
    semantic: &SemanticSnapshot,
) -> ObligationReport {
    let sites = relevant_production_exhaustiveness_sites(domain, semantic);
    let mut coverage = evaluate_domain_site_coverage(domain, &sites);
    coverage.files.insert(domain.path.clone());

    let required_sites = coverage.required_sites.into_iter().collect::<Vec<_>>();
    let satisfied_sites = coverage.satisfied_sites.into_iter().collect::<Vec<_>>();
    let missing_sites = coverage.missing_sites.into_iter().collect::<Vec<_>>();
    let missing_variants = coverage.missing_variants.into_iter().collect::<Vec<_>>();
    let files = coverage.files.into_iter().collect::<Vec<_>>();
    let context_burden = required_sites.len();

    ObligationReport {
        id: format!("closed_domain::{}", domain.symbol_name),
        kind: "closed_domain_exhaustiveness".to_string(),
        concept_id: None,
        domain_symbol_name: Some(domain.symbol_name.clone()),
        summary: obligation_summary(
            &domain.symbol_name,
            domain,
            &missing_sites,
            &missing_variants,
        ),
        files,
        required_sites,
        satisfied_sites,
        missing_sites,
        missing_variants,
        context_burden,
    }
}

fn obligation_summary(
    label: &str,
    domain: &ClosedDomain,
    missing_sites: &[ObligationSite],
    missing_variants: &[String],
) -> String {
    if missing_sites.is_empty() {
        return format!(
            "Closed domain '{}' is fully covered across {} required sites",
            label,
            domain.variants.len()
        );
    }

    if missing_variants.is_empty() {
        return format!("Closed domain '{}' is missing required update sites", label);
    }

    format!(
        "Closed domain '{}' is missing coverage for variants: {}",
        label,
        missing_variants.join(", ")
    )
}

fn relevant_exhaustiveness_sites<'a>(
    domain: &ClosedDomain,
    semantic: &'a SemanticSnapshot,
) -> Vec<&'a ExhaustivenessSite> {
    semantic
        .closed_domain_sites
        .iter()
        .filter(|site| site.domain_symbol_name == domain.symbol_name)
        .collect()
}

fn relevant_production_exhaustiveness_sites<'a>(
    domain: &ClosedDomain,
    semantic: &'a SemanticSnapshot,
) -> Vec<&'a ExhaustivenessSite> {
    relevant_exhaustiveness_sites(domain, semantic)
        .into_iter()
        .filter(|site| !is_test_file(&site.path))
        .collect()
}

fn zero_config_domain_is_actionable(domain: &ClosedDomain, semantic: &SemanticSnapshot) -> bool {
    if domain.variants.len() > MAX_ZERO_CONFIG_DOMAIN_VARIANTS {
        return false;
    }

    !relevant_production_exhaustiveness_sites(domain, semantic).is_empty()
}

fn evaluate_domain_site_coverage(
    domain: &ClosedDomain,
    sites: &[&ExhaustivenessSite],
) -> DomainSiteCoverage {
    let mut coverage = DomainSiteCoverage::default();

    for site in sites {
        coverage.files.insert(site.path.clone());
        let site_variants = missing_site_variants(domain, site);
        let detail = if site_variants.is_empty() {
            format!(
                "covers {} variants via {}",
                domain.variants.len(),
                site.site_kind
            )
        } else {
            format!(
                "missing variants: {}",
                site_variants.iter().cloned().collect::<Vec<_>>().join(", ")
            )
        };
        let obligation_site = ObligationSite {
            path: site.path.clone(),
            kind: site.site_kind.clone(),
            line: Some(site.line),
            detail,
        };
        coverage.required_sites.insert(obligation_site.clone());
        if site_variants.is_empty() {
            coverage.satisfied_sites.insert(obligation_site);
        } else {
            coverage.missing_variants.extend(site_variants);
            coverage.missing_sites.insert(obligation_site);
        }
    }

    coverage
}

fn missing_site_variants(domain: &ClosedDomain, site: &ExhaustivenessSite) -> BTreeSet<String> {
    let covered = site
        .covered_variants
        .iter()
        .cloned()
        .collect::<HashSet<_>>();
    domain
        .variants
        .iter()
        .filter(|variant| !covered.contains(*variant))
        .cloned()
        .collect()
}

fn concept_rule_paths(concept: &ConceptRule) -> Vec<String> {
    let mut paths = Vec::new();
    for scoped_path in concept
        .anchors
        .iter()
        .chain(concept.authoritative_inputs.iter())
        .chain(concept.allowed_writers.iter())
        .chain(concept.forbid_writers.iter())
        .chain(concept.canonical_accessors.iter())
        .chain(concept.forbid_raw_reads.iter())
    {
        if let Some((path, _)) = scoped_path.split_once("::") {
            paths.push(path.to_string());
        }
    }
    paths.extend(concept.related_tests.iter().cloned());
    paths
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

fn summarize_contract_missing_sites(missing_sites: &[ObligationSite]) -> String {
    let labels = missing_sites
        .iter()
        .map(contract_site_summary_label)
        .collect::<Vec<_>>();
    let labels = stable_dedup_labels(labels);
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

fn stable_dedup_labels(labels: Vec<&'static str>) -> Vec<&'static str> {
    let mut seen = BTreeSet::new();
    let mut deduped = Vec::new();
    for label in labels {
        if seen.insert(label) {
            deduped.push(label);
        }
    }
    deduped
}

fn path_matches(pattern: &str, path: &str) -> bool {
    if rules::glob_match(pattern, path) {
        return true;
    }
    pattern == path
}

#[cfg(test)]
mod tests {
    use super::{
        build_obligations, obligation_score_0_10000, summarize_contract_missing_sites,
        ObligationScope,
    };
    use crate::analysis::semantic::{
        ClosedDomain, ExhaustivenessSite, ProjectModel, ReadFact, SemanticCapability,
        SemanticFileFact, SemanticSnapshot, SymbolFact,
    };
    use crate::metrics::v2::ObligationSite;
    use crate::metrics::rules::RulesConfig;
    use std::collections::BTreeSet;

    #[test]
    fn computes_missing_variants_and_related_test_obligations() {
        let config: RulesConfig = toml::from_str(
            r#"
                [[concept]]
                id = "task_presentation_status"
                anchors = ["src/app/task-presentation-status.ts::TaskDotStatus"]
                related_tests = ["src/app/task-presentation-status.test.ts"]
            "#,
        )
        .expect("rules config");
        let semantic = SemanticSnapshot {
            project: ProjectModel::default(),
            analyzed_files: 1,
            capabilities: vec![
                SemanticCapability::ClosedDomains,
                SemanticCapability::ClosedDomainSites,
            ],
            files: vec![SemanticFileFact::default()],
            symbols: Vec::new(),
            reads: Vec::new(),
            writes: Vec::new(),
            closed_domains: vec![ClosedDomain {
                path: "src/app/task-presentation-status.ts".to_string(),
                symbol_name: "TaskDotStatus".to_string(),
                variants: vec!["idle".to_string(), "busy".to_string(), "error".to_string()],
                line: 4,
            }],
            closed_domain_sites: vec![ExhaustivenessSite {
                path: "src/components/Sidebar.tsx".to_string(),
                domain_symbol_name: "TaskDotStatus".to_string(),
                site_kind: "switch".to_string(),
                proof_kind: "assertNever".to_string(),
                covered_variants: vec!["idle".to_string(), "busy".to_string()],
                line: 20,
            }],
        };
        let changed_files = BTreeSet::from([
            "src/app/task-presentation-status.ts".to_string(),
            "src/components/Sidebar.tsx".to_string(),
        ]);

        let obligations =
            build_obligations(&config, &semantic, ObligationScope::Changed, &changed_files);

        assert_eq!(obligations.len(), 1);
        assert_eq!(obligations[0].missing_variants, vec!["error".to_string()]);
        assert_eq!(obligations[0].missing_sites.len(), 2);
        assert!(obligations[0]
            .missing_sites
            .iter()
            .any(|site| site.kind == "related_test"));
        assert!(obligation_score_0_10000(&obligations) < 10000);
    }

    #[test]
    fn changed_scope_includes_allowed_writer_paths() {
        let config: RulesConfig = toml::from_str(
            r#"
                [[concept]]
                id = "task_state"
                anchors = ["src/domain/task-state.ts::TaskState"]
                allowed_writers = ["src/app/task-state-writer.ts::*"]
            "#,
        )
        .expect("rules config");
        let semantic = SemanticSnapshot {
            project: ProjectModel::default(),
            analyzed_files: 1,
            capabilities: vec![
                SemanticCapability::ClosedDomains,
                SemanticCapability::ClosedDomainSites,
            ],
            files: vec![SemanticFileFact::default()],
            symbols: Vec::new(),
            reads: Vec::new(),
            writes: Vec::new(),
            closed_domains: vec![ClosedDomain {
                path: "src/domain/task-state.ts".to_string(),
                symbol_name: "TaskState".to_string(),
                variants: vec!["idle".to_string(), "running".to_string()],
                line: 1,
            }],
            closed_domain_sites: vec![ExhaustivenessSite {
                path: "src/app/presenter.ts".to_string(),
                domain_symbol_name: "TaskState".to_string(),
                site_kind: "switch".to_string(),
                proof_kind: "assertNever".to_string(),
                covered_variants: vec!["idle".to_string()],
                line: 10,
            }],
        };
        let changed_files = BTreeSet::from(["src/app/task-state-writer.ts".to_string()]);

        let obligations =
            build_obligations(&config, &semantic, ObligationScope::Changed, &changed_files);

        assert_eq!(obligations.len(), 1);
        assert_eq!(obligations[0].concept_id.as_deref(), Some("task_state"));
    }

    #[test]
    fn zero_config_domains_ignore_test_only_sites() {
        let config: RulesConfig = toml::from_str("").expect("empty rules config");
        let semantic = SemanticSnapshot {
            project: ProjectModel::default(),
            analyzed_files: 1,
            capabilities: vec![
                SemanticCapability::ClosedDomains,
                SemanticCapability::ClosedDomainSites,
            ],
            files: vec![SemanticFileFact::default()],
            symbols: Vec::new(),
            reads: Vec::new(),
            writes: Vec::new(),
            closed_domains: vec![ClosedDomain {
                path: "src/domain/task-state.ts".to_string(),
                symbol_name: "TaskState".to_string(),
                variants: vec!["idle".to_string(), "running".to_string()],
                line: 1,
            }],
            closed_domain_sites: vec![ExhaustivenessSite {
                path: "src/domain/task-state.test.ts".to_string(),
                domain_symbol_name: "TaskState".to_string(),
                site_kind: "switch".to_string(),
                proof_kind: "assertNever".to_string(),
                covered_variants: vec!["idle".to_string()],
                line: 10,
            }],
        };

        let obligations =
            build_obligations(&config, &semantic, ObligationScope::All, &BTreeSet::new());

        assert!(obligations.is_empty());
    }

    #[test]
    fn zero_config_domains_ignore_large_variant_sets() {
        let config: RulesConfig = toml::from_str("").expect("empty rules config");
        let semantic = SemanticSnapshot {
            project: ProjectModel::default(),
            analyzed_files: 1,
            capabilities: vec![
                SemanticCapability::ClosedDomains,
                SemanticCapability::ClosedDomainSites,
            ],
            files: vec![SemanticFileFact::default()],
            symbols: Vec::new(),
            reads: Vec::new(),
            writes: Vec::new(),
            closed_domains: vec![ClosedDomain {
                path: "src/domain/ipc.ts".to_string(),
                symbol_name: "IPC".to_string(),
                variants: (0..20).map(|index| format!("Variant{index}")).collect(),
                line: 1,
            }],
            closed_domain_sites: vec![ExhaustivenessSite {
                path: "src/app/ipc-switch.ts".to_string(),
                domain_symbol_name: "IPC".to_string(),
                site_kind: "switch".to_string(),
                proof_kind: "switch".to_string(),
                covered_variants: vec!["Variant0".to_string()],
                line: 10,
            }],
        };

        let obligations =
            build_obligations(&config, &semantic, ObligationScope::All, &BTreeSet::new());

        assert!(obligations.is_empty());
    }

    #[test]
    fn changed_scope_requires_related_contract_surfaces() {
        let config: RulesConfig = toml::from_str(
            r#"
                [[contract]]
                id = "server_state_bootstrap"
                categories_symbol = "src/domain/server-state-bootstrap.ts::SERVER_STATE_BOOTSTRAP_CATEGORIES"
                payload_map_symbol = "src/domain/server-state-bootstrap.ts::ServerStateBootstrapPayloadMap"
                registry_symbol = "src/app/server-state-bootstrap-registry.ts::SERVER_STATE_BOOTSTRAP_REGISTRY"
                browser_entry = "src/runtime/browser-session.ts"
                electron_entry = "src/app/desktop-session.ts"
            "#,
        )
        .expect("rules config");
        let semantic = SemanticSnapshot {
            project: ProjectModel::default(),
            analyzed_files: 4,
            capabilities: vec![SemanticCapability::Symbols],
            files: vec![
                SemanticFileFact {
                    path: "src/domain/server-state-bootstrap.ts".to_string(),
                    ..SemanticFileFact::default()
                },
                SemanticFileFact {
                    path: "src/app/server-state-bootstrap-registry.ts".to_string(),
                    ..SemanticFileFact::default()
                },
                SemanticFileFact {
                    path: "src/runtime/browser-session.ts".to_string(),
                    ..SemanticFileFact::default()
                },
                SemanticFileFact {
                    path: "src/app/desktop-session.ts".to_string(),
                    ..SemanticFileFact::default()
                },
            ],
            symbols: vec![
                SymbolFact {
                    id: "cats".to_string(),
                    path: "src/domain/server-state-bootstrap.ts".to_string(),
                    name: "SERVER_STATE_BOOTSTRAP_CATEGORIES".to_string(),
                    kind: "const".to_string(),
                    line: 3,
                },
                SymbolFact {
                    id: "payload".to_string(),
                    path: "src/domain/server-state-bootstrap.ts".to_string(),
                    name: "ServerStateBootstrapPayloadMap".to_string(),
                    kind: "type_alias".to_string(),
                    line: 8,
                },
                SymbolFact {
                    id: "registry".to_string(),
                    path: "src/app/server-state-bootstrap-registry.ts".to_string(),
                    name: "SERVER_STATE_BOOTSTRAP_REGISTRY".to_string(),
                    kind: "const".to_string(),
                    line: 5,
                },
            ],
            reads: Vec::new(),
            writes: Vec::new(),
            closed_domains: Vec::new(),
            closed_domain_sites: Vec::new(),
        };
        let changed_files = BTreeSet::from(["src/domain/server-state-bootstrap.ts".to_string()]);

        let obligations =
            build_obligations(&config, &semantic, ObligationScope::Changed, &changed_files);

        let contract = obligations
            .iter()
            .find(|obligation| obligation.kind == "contract_surface_completeness")
            .expect("contract obligation");
        assert_eq!(
            contract.concept_id.as_deref(),
            Some("server_state_bootstrap")
        );
        assert!(contract
            .missing_sites
            .iter()
            .any(|site| site.kind == "registry_symbol"));
        assert!(contract
            .missing_sites
            .iter()
            .any(|site| site.kind == "browser_entry"));
        assert!(contract
            .missing_sites
            .iter()
            .any(|site| site.kind == "electron_entry"));
    }

    #[test]
    fn changed_scope_triggers_contract_when_registry_surface_changes() {
        let config: RulesConfig = toml::from_str(
            r#"
                [[contract]]
                id = "server_state_bootstrap"
                categories_symbol = "src/domain/server-state-bootstrap.ts::SERVER_STATE_BOOTSTRAP_CATEGORIES"
                payload_map_symbol = "src/domain/server-state-bootstrap.ts::ServerStateBootstrapPayloadMap"
                registry_symbol = "src/app/server-state-bootstrap-registry.ts::SERVER_STATE_BOOTSTRAP_REGISTRY"
                browser_entry = "src/runtime/browser-session.ts"
                electron_entry = "src/app/desktop-session.ts"
            "#,
        )
        .expect("rules config");
        let semantic = SemanticSnapshot {
            project: ProjectModel::default(),
            analyzed_files: 4,
            capabilities: vec![SemanticCapability::Symbols],
            files: vec![
                SemanticFileFact {
                    path: "src/domain/server-state-bootstrap.ts".to_string(),
                    ..SemanticFileFact::default()
                },
                SemanticFileFact {
                    path: "src/app/server-state-bootstrap-registry.ts".to_string(),
                    ..SemanticFileFact::default()
                },
                SemanticFileFact {
                    path: "src/runtime/browser-session.ts".to_string(),
                    ..SemanticFileFact::default()
                },
                SemanticFileFact {
                    path: "src/app/desktop-session.ts".to_string(),
                    ..SemanticFileFact::default()
                },
            ],
            symbols: vec![
                SymbolFact {
                    id: "cats".to_string(),
                    path: "src/domain/server-state-bootstrap.ts".to_string(),
                    name: "SERVER_STATE_BOOTSTRAP_CATEGORIES".to_string(),
                    kind: "const".to_string(),
                    line: 3,
                },
                SymbolFact {
                    id: "payload".to_string(),
                    path: "src/domain/server-state-bootstrap.ts".to_string(),
                    name: "ServerStateBootstrapPayloadMap".to_string(),
                    kind: "type_alias".to_string(),
                    line: 8,
                },
                SymbolFact {
                    id: "registry".to_string(),
                    path: "src/app/server-state-bootstrap-registry.ts".to_string(),
                    name: "SERVER_STATE_BOOTSTRAP_REGISTRY".to_string(),
                    kind: "const".to_string(),
                    line: 5,
                },
            ],
            reads: Vec::new(),
            writes: Vec::new(),
            closed_domains: Vec::new(),
            closed_domain_sites: Vec::new(),
        };
        let changed_files =
            BTreeSet::from(["src/app/server-state-bootstrap-registry.ts".to_string()]);

        let obligations =
            build_obligations(&config, &semantic, ObligationScope::Changed, &changed_files);

        let contract = obligations
            .iter()
            .find(|obligation| obligation.kind == "contract_surface_completeness")
            .expect("contract obligation");
        assert!(contract
            .satisfied_sites
            .iter()
            .any(|site| site.kind == "registry_symbol"));
        assert!(contract
            .missing_sites
            .iter()
            .any(|site| site.kind == "categories_symbol"));
        assert!(contract
            .missing_sites
            .iter()
            .any(|site| site.kind == "browser_entry"));
    }

    #[test]
    fn changed_scope_triggers_from_semantically_related_contract_reader() {
        let config: RulesConfig = toml::from_str(
            r#"
                [[contract]]
                id = "server_state_bootstrap"
                categories_symbol = "src/domain/server-state-bootstrap.ts::SERVER_STATE_BOOTSTRAP_CATEGORIES"
                payload_map_symbol = "src/domain/server-state-bootstrap.ts::ServerStateBootstrapPayloadMap"
                registry_symbol = "src/app/server-state-bootstrap-registry.ts::SERVER_STATE_BOOTSTRAP_REGISTRY"
                browser_entry = "src/runtime/browser-session.ts"
                required_symbols = ["src/app/bootstrap-persist.ts::serializeBootstrapPayload"]
            "#,
        )
        .expect("rules config");
        let semantic = SemanticSnapshot {
            project: ProjectModel::default(),
            analyzed_files: 5,
            capabilities: vec![SemanticCapability::Symbols, SemanticCapability::Reads],
            files: vec![
                SemanticFileFact {
                    path: "src/domain/server-state-bootstrap.ts".to_string(),
                    ..SemanticFileFact::default()
                },
                SemanticFileFact {
                    path: "src/app/server-state-bootstrap-registry.ts".to_string(),
                    ..SemanticFileFact::default()
                },
                SemanticFileFact {
                    path: "src/runtime/browser-session.ts".to_string(),
                    ..SemanticFileFact::default()
                },
                SemanticFileFact {
                    path: "src/app/bootstrap-persist.ts".to_string(),
                    ..SemanticFileFact::default()
                },
                SemanticFileFact {
                    path: "src/app/bootstrap-adapter.ts".to_string(),
                    ..SemanticFileFact::default()
                },
            ],
            symbols: vec![
                SymbolFact {
                    id: "cats".to_string(),
                    path: "src/domain/server-state-bootstrap.ts".to_string(),
                    name: "SERVER_STATE_BOOTSTRAP_CATEGORIES".to_string(),
                    kind: "const".to_string(),
                    line: 3,
                },
                SymbolFact {
                    id: "payload".to_string(),
                    path: "src/domain/server-state-bootstrap.ts".to_string(),
                    name: "ServerStateBootstrapPayloadMap".to_string(),
                    kind: "type_alias".to_string(),
                    line: 8,
                },
                SymbolFact {
                    id: "registry".to_string(),
                    path: "src/app/server-state-bootstrap-registry.ts".to_string(),
                    name: "SERVER_STATE_BOOTSTRAP_REGISTRY".to_string(),
                    kind: "const".to_string(),
                    line: 5,
                },
                SymbolFact {
                    id: "persist".to_string(),
                    path: "src/app/bootstrap-persist.ts".to_string(),
                    name: "serializeBootstrapPayload".to_string(),
                    kind: "function".to_string(),
                    line: 12,
                },
            ],
            reads: vec![ReadFact {
                path: "src/app/bootstrap-adapter.ts".to_string(),
                symbol_name: "ServerStateBootstrapPayloadMap".to_string(),
                read_kind: "type_reference".to_string(),
                line: 21,
            }],
            writes: Vec::new(),
            closed_domains: Vec::new(),
            closed_domain_sites: Vec::new(),
        };
        let changed_files = BTreeSet::from(["src/app/bootstrap-adapter.ts".to_string()]);

        let obligations =
            build_obligations(&config, &semantic, ObligationScope::Changed, &changed_files);

        let contract = obligations
            .iter()
            .find(|obligation| obligation.kind == "contract_surface_completeness")
            .expect("contract obligation");
        assert!(contract
            .files
            .contains(&"src/app/bootstrap-adapter.ts".to_string()));
        assert!(contract.summary.contains("bootstrap-adapter.ts"));
        assert!(contract
            .missing_sites
            .iter()
            .any(|site| site.kind == "required_symbol"));
    }

    #[test]
    fn changed_scope_triggers_from_semantically_related_contract_symbol_declaration() {
        let config: RulesConfig = toml::from_str(
            r#"
                [[contract]]
                id = "server_state_bootstrap"
                payload_map_symbol = "src/domain/server-state-bootstrap.ts::ServerStateBootstrapPayloadMap"
                required_symbols = ["src/app/bootstrap-persist.ts::serializeBootstrapPayload"]
            "#,
        )
        .expect("rules config");
        let semantic = SemanticSnapshot {
            project: ProjectModel::default(),
            analyzed_files: 3,
            capabilities: vec![SemanticCapability::Symbols],
            files: vec![
                SemanticFileFact {
                    path: "src/domain/server-state-bootstrap.ts".to_string(),
                    ..SemanticFileFact::default()
                },
                SemanticFileFact {
                    path: "src/app/bootstrap-persist.ts".to_string(),
                    ..SemanticFileFact::default()
                },
                SemanticFileFact {
                    path: "src/app/bootstrap-field-adapter.ts".to_string(),
                    ..SemanticFileFact::default()
                },
            ],
            symbols: vec![
                SymbolFact {
                    id: "payload".to_string(),
                    path: "src/domain/server-state-bootstrap.ts".to_string(),
                    name: "ServerStateBootstrapPayloadMap".to_string(),
                    kind: "type_alias".to_string(),
                    line: 8,
                },
                SymbolFact {
                    id: "persist".to_string(),
                    path: "src/app/bootstrap-persist.ts".to_string(),
                    name: "serializeBootstrapPayload".to_string(),
                    kind: "function".to_string(),
                    line: 12,
                },
                SymbolFact {
                    id: "field".to_string(),
                    path: "src/app/bootstrap-field-adapter.ts".to_string(),
                    name: "ServerStateBootstrapPayloadMap.snapshot".to_string(),
                    kind: "property_signature".to_string(),
                    line: 4,
                },
            ],
            reads: Vec::new(),
            writes: Vec::new(),
            closed_domains: Vec::new(),
            closed_domain_sites: Vec::new(),
        };
        let changed_files = BTreeSet::from(["src/app/bootstrap-field-adapter.ts".to_string()]);

        let obligations =
            build_obligations(&config, &semantic, ObligationScope::Changed, &changed_files);

        let contract = obligations
            .iter()
            .find(|obligation| obligation.kind == "contract_surface_completeness")
            .expect("contract obligation");
        assert!(contract
            .files
            .contains(&"src/app/bootstrap-field-adapter.ts".to_string()));
        assert!(contract.summary.contains("bootstrap-field-adapter.ts"));
    }

    #[test]
    fn changed_scope_ignores_test_only_contract_readers() {
        let config: RulesConfig = toml::from_str(
            r#"
                [[contract]]
                id = "server_state_bootstrap"
                categories_symbol = "src/domain/server-state-bootstrap.ts::SERVER_STATE_BOOTSTRAP_CATEGORIES"
                registry_symbol = "src/app/server-state-bootstrap-registry.ts::SERVER_STATE_BOOTSTRAP_REGISTRY"
                browser_entry = "src/runtime/browser-session.ts"
            "#,
        )
        .expect("rules config");
        let semantic = SemanticSnapshot {
            project: ProjectModel::default(),
            analyzed_files: 4,
            capabilities: vec![SemanticCapability::Symbols, SemanticCapability::Reads],
            files: vec![
                SemanticFileFact {
                    path: "src/domain/server-state-bootstrap.ts".to_string(),
                    ..SemanticFileFact::default()
                },
                SemanticFileFact {
                    path: "src/app/server-state-bootstrap-registry.ts".to_string(),
                    ..SemanticFileFact::default()
                },
                SemanticFileFact {
                    path: "src/runtime/browser-session.ts".to_string(),
                    ..SemanticFileFact::default()
                },
                SemanticFileFact {
                    path: "src/app/server-state-bootstrap.test.ts".to_string(),
                    ..SemanticFileFact::default()
                },
            ],
            symbols: vec![
                SymbolFact {
                    id: "cats".to_string(),
                    path: "src/domain/server-state-bootstrap.ts".to_string(),
                    name: "SERVER_STATE_BOOTSTRAP_CATEGORIES".to_string(),
                    kind: "const".to_string(),
                    line: 3,
                },
                SymbolFact {
                    id: "registry".to_string(),
                    path: "src/app/server-state-bootstrap-registry.ts".to_string(),
                    name: "SERVER_STATE_BOOTSTRAP_REGISTRY".to_string(),
                    kind: "const".to_string(),
                    line: 5,
                },
            ],
            reads: vec![ReadFact {
                path: "src/app/server-state-bootstrap.test.ts".to_string(),
                symbol_name: "SERVER_STATE_BOOTSTRAP_CATEGORIES".to_string(),
                read_kind: "property_access".to_string(),
                line: 12,
            }],
            writes: Vec::new(),
            closed_domains: Vec::new(),
            closed_domain_sites: Vec::new(),
        };
        let changed_files = BTreeSet::from(["src/app/server-state-bootstrap.test.ts".to_string()]);

        let obligations =
            build_obligations(&config, &semantic, ObligationScope::Changed, &changed_files);

        assert!(obligations.is_empty());
    }

    #[test]
    fn all_scope_reports_missing_declared_contract_sites() {
        let config: RulesConfig = toml::from_str(
            r#"
                [[contract]]
                id = "server_state_bootstrap"
                categories_symbol = "src/domain/server-state-bootstrap.ts::SERVER_STATE_BOOTSTRAP_CATEGORIES"
                browser_entry = "src/runtime/browser-session.ts"
            "#,
        )
        .expect("rules config");
        let semantic = SemanticSnapshot {
            project: ProjectModel::default(),
            analyzed_files: 0,
            capabilities: vec![SemanticCapability::Symbols],
            files: Vec::new(),
            symbols: Vec::new(),
            reads: Vec::new(),
            writes: Vec::new(),
            closed_domains: Vec::new(),
            closed_domain_sites: Vec::new(),
        };

        let obligations =
            build_obligations(&config, &semantic, ObligationScope::All, &BTreeSet::new());

        let contract = obligations
            .iter()
            .find(|obligation| obligation.kind == "contract_surface_completeness")
            .expect("contract obligation");
        assert_eq!(contract.missing_sites.len(), 2);
        assert!(contract
            .missing_sites
            .iter()
            .all(|site| site.detail.contains("declared contract site is missing")));
    }

    #[test]
    fn all_scope_reports_required_contract_extensions() {
        let config: RulesConfig = toml::from_str(
            r#"
                [[contract]]
                id = "server_state_bootstrap"
                categories_symbol = "src/domain/server-state-bootstrap.ts::SERVER_STATE_BOOTSTRAP_CATEGORIES"
                registry_symbol = "src/app/server-state-bootstrap-registry.ts::SERVER_STATE_BOOTSTRAP_REGISTRY"
                required_symbols = ["src/app/bootstrap-persist.ts::serializeBootstrapPayload"]
                required_files = ["src/runtime/server-state-bootstrap.ts"]
            "#,
        )
        .expect("rules config");
        let semantic = SemanticSnapshot {
            project: ProjectModel::default(),
            analyzed_files: 0,
            capabilities: vec![SemanticCapability::Symbols],
            files: Vec::new(),
            symbols: Vec::new(),
            reads: Vec::new(),
            writes: Vec::new(),
            closed_domains: Vec::new(),
            closed_domain_sites: Vec::new(),
        };

        let obligations =
            build_obligations(&config, &semantic, ObligationScope::All, &BTreeSet::new());

        let contract = obligations
            .iter()
            .find(|obligation| obligation.kind == "contract_surface_completeness")
            .expect("contract obligation");
        assert!(contract
            .missing_sites
            .iter()
            .any(|site| site.kind == "required_symbol"));
        assert!(contract
            .missing_sites
            .iter()
            .any(|site| site.kind == "required_file"));
    }

    #[test]
    fn contract_missing_sites_prioritize_runtime_and_registry_surfaces() {
        let config: RulesConfig = toml::from_str(
            r#"
                [[contract]]
                id = "server_state_bootstrap"
                registry_symbol = "src/app/server-state-bootstrap-registry.ts::SERVER_STATE_BOOTSTRAP_REGISTRY"
                browser_entry = "src/runtime/browser-session.ts"
                required_files = ["src/app/bootstrap-persist.ts"]
            "#,
        )
        .expect("rules config");
        let semantic = SemanticSnapshot {
            project: ProjectModel::default(),
            analyzed_files: 0,
            capabilities: vec![SemanticCapability::Symbols],
            files: Vec::new(),
            symbols: Vec::new(),
            reads: Vec::new(),
            writes: Vec::new(),
            closed_domains: Vec::new(),
            closed_domain_sites: Vec::new(),
        };

        let obligations =
            build_obligations(&config, &semantic, ObligationScope::All, &BTreeSet::new());

        let contract = obligations
            .iter()
            .find(|obligation| obligation.kind == "contract_surface_completeness")
            .expect("contract obligation");
        assert_eq!(
            contract
                .missing_sites
                .first()
                .map(|site| site.kind.as_str()),
            Some("browser_entry")
        );
        assert_eq!(
            contract.missing_sites.get(1).map(|site| site.kind.as_str()),
            Some("registry_symbol")
        );
        assert_eq!(
            contract.missing_sites.get(2).map(|site| site.kind.as_str()),
            Some("required_file")
        );
    }

    #[test]
    fn contract_missing_site_summary_dedupes_non_adjacent_labels() {
        let summary = summarize_contract_missing_sites(&[
            ObligationSite {
                path: "src/app/bootstrap-adapter.ts".to_string(),
                kind: "required_symbol".to_string(),
                line: None,
                detail: "update adapter".to_string(),
            },
            ObligationSite {
                path: "src/runtime/browser-session.ts".to_string(),
                kind: "browser_entry".to_string(),
                line: None,
                detail: "update browser runtime entry".to_string(),
            },
            ObligationSite {
                path: "src/app/bootstrap-persist.ts".to_string(),
                kind: "required_symbol".to_string(),
                line: None,
                detail: "update required contract symbol".to_string(),
            },
        ]);

        assert_eq!(
            summary,
            "the required symbol and browser runtime entry surfaces"
        );
    }
}
