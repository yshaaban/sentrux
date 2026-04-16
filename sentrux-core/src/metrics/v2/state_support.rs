//! Internal helpers for state-integrity analysis.

use super::{FindingSeverity, ObligationReport, ObligationSite, SemanticFinding};
use crate::analysis::semantic::{
    ClosedDomain, ExhaustivenessProofKind, ExhaustivenessSite, ExhaustivenessSiteKind,
    SemanticCapability, SemanticSnapshot, TransitionSite,
};
use crate::metrics::rules::{self, StateModelRule};
use std::collections::{BTreeMap, BTreeSet};

struct RelevantStateInputs<'a> {
    sites: Vec<&'a ExhaustivenessSite>,
    domains: Vec<&'a ClosedDomain>,
    domain_symbol_names: BTreeSet<String>,
}

struct ObligationCoverageSummary {
    files: BTreeSet<String>,
    required_sites: BTreeSet<ObligationSite>,
    satisfied_sites: BTreeSet<ObligationSite>,
    missing_sites: BTreeSet<ObligationSite>,
    missing_variants: BTreeSet<String>,
    context_burden: usize,
}

struct TransitionCoverageSummary {
    transition_sites_supported: bool,
    files: BTreeSet<String>,
    sites: Vec<TransitionSite>,
    gap_sites: Vec<TransitionSite>,
    missing_variants: BTreeSet<String>,
}

pub(super) fn build_state_integrity_findings(
    reports: &[super::StateIntegrityReport],
) -> Vec<SemanticFinding> {
    let mut findings = Vec::new();

    for report in reports {
        if let Some(finding) = build_unmapped_state_model_finding(report) {
            findings.push(finding);
            continue;
        }

        for finding in [
            build_missing_exhaustive_switch_finding(report),
            build_missing_assert_never_finding(report),
            build_missing_variant_coverage_finding(report),
            build_missing_transition_sites_finding(report),
            build_transition_coverage_gap_finding(report),
            build_high_context_burden_finding(report),
        ]
        .into_iter()
        .flatten()
        {
            findings.push(finding);
        }
    }

    findings
}

pub(super) fn build_state_integrity_report(
    state_model: &StateModelRule,
    semantic: &SemanticSnapshot,
    obligations: &[ObligationReport],
) -> super::StateIntegrityReport {
    let relevant = collect_relevant_state_inputs(state_model, semantic);
    let related_obligations =
        collect_related_obligations(obligations, &relevant.domain_symbol_names);
    let mut files = collect_relevant_report_files(&relevant);
    let obligation_summary = summarize_obligation_coverage(&related_obligations);
    files.extend(obligation_summary.files.iter().cloned());
    let transition_summary = summarize_transition_coverage(
        state_model,
        semantic,
        &relevant.domain_symbol_names,
        &relevant.domains,
    );
    files.extend(transition_summary.files.iter().cloned());

    let missing_exhaustive_switch_domains =
        missing_switch_domains(state_model, &relevant.domain_symbol_names, &relevant.sites);
    let missing_assert_never_domains =
        missing_assert_never_domains(state_model, &relevant.domain_symbol_names, &relevant.sites);
    let explicitness_score_0_10000 = explicitness_score_0_10000(
        state_model,
        relevant.domain_symbol_names.len(),
        relevant.sites.len(),
        obligation_summary.required_sites.len(),
        obligation_summary.satisfied_sites.len(),
        missing_exhaustive_switch_domains.len(),
        missing_assert_never_domains.len(),
        transition_summary.transition_sites_supported,
        transition_summary.sites.len(),
        transition_summary.gap_sites.len(),
    );
    let context_burden = obligation_summary.context_burden + transition_summary.sites.len();

    super::StateIntegrityReport {
        id: state_model.id.clone(),
        kind: state_model.kind.clone(),
        roots: state_model.roots.clone(),
        domain_symbol_names: relevant.domain_symbol_names.into_iter().collect(),
        files: files.into_iter().collect(),
        required_sites: obligation_summary.required_sites.into_iter().collect(),
        satisfied_sites: obligation_summary.satisfied_sites.into_iter().collect(),
        missing_sites: obligation_summary.missing_sites.into_iter().collect(),
        missing_variants: obligation_summary.missing_variants.into_iter().collect(),
        missing_exhaustive_switch_domains,
        missing_assert_never_domains,
        transition_sites_supported: transition_summary.transition_sites_supported,
        transition_sites: transition_summary.sites,
        transition_gap_sites: transition_summary.gap_sites,
        missing_transition_variants: transition_summary.missing_variants.into_iter().collect(),
        explicitness_score_0_10000,
        context_burden,
    }
}

fn build_unmapped_state_model_finding(
    report: &super::StateIntegrityReport,
) -> Option<SemanticFinding> {
    if !report.domain_symbol_names.is_empty() {
        return None;
    }

    Some(SemanticFinding {
        kind: "state_model_unmapped".to_string(),
        severity: FindingSeverity::Medium,
        concept_id: report.id.clone(),
        summary: format!(
            "State model '{}' has no matching closed-domain coverage under its configured roots",
            report.id
        ),
        files: report.files.clone(),
        evidence: report.roots.clone(),
    })
}

fn build_missing_exhaustive_switch_finding(
    report: &super::StateIntegrityReport,
) -> Option<SemanticFinding> {
    if report.missing_exhaustive_switch_domains.is_empty() {
        return None;
    }

    Some(SemanticFinding {
        kind: "state_model_missing_exhaustive_switch".to_string(),
        severity: FindingSeverity::High,
        concept_id: report.id.clone(),
        summary: format!(
            "State model '{}' is missing switch-based coverage for {} domain(s)",
            report.id,
            report.missing_exhaustive_switch_domains.len()
        ),
        files: report.files.clone(),
        evidence: report.missing_exhaustive_switch_domains.clone(),
    })
}

fn build_missing_assert_never_finding(
    report: &super::StateIntegrityReport,
) -> Option<SemanticFinding> {
    if report.missing_assert_never_domains.is_empty() {
        return None;
    }

    Some(SemanticFinding {
        kind: "state_model_missing_assert_never".to_string(),
        severity: FindingSeverity::Medium,
        concept_id: report.id.clone(),
        summary: format!(
            "State model '{}' is missing assertNever-backed default handling for {} domain(s)",
            report.id,
            report.missing_assert_never_domains.len()
        ),
        files: report.files.clone(),
        evidence: report.missing_assert_never_domains.clone(),
    })
}

fn build_missing_variant_coverage_finding(
    report: &super::StateIntegrityReport,
) -> Option<SemanticFinding> {
    if report.missing_sites.is_empty() && report.missing_variants.is_empty() {
        return None;
    }

    Some(SemanticFinding {
        kind: "state_model_missing_variant_coverage".to_string(),
        severity: if report.missing_variants.is_empty() {
            FindingSeverity::Medium
        } else {
            FindingSeverity::High
        },
        concept_id: report.id.clone(),
        summary: state_variant_coverage_summary(report),
        files: report.files.clone(),
        evidence: build_missing_variant_coverage_evidence(report),
    })
}

fn build_missing_variant_coverage_evidence(report: &super::StateIntegrityReport) -> Vec<String> {
    report
        .missing_sites
        .iter()
        .map(|site| format!("{} [{}]", site.path, site.detail))
        .chain(
            report
                .missing_variants
                .iter()
                .map(|variant| format!("missing variant '{variant}'")),
        )
        .collect()
}

fn build_missing_transition_sites_finding(
    report: &super::StateIntegrityReport,
) -> Option<SemanticFinding> {
    if !report.transition_sites_supported
        || !report.transition_sites.is_empty()
        || report.domain_symbol_names.is_empty()
    {
        return None;
    }

    Some(SemanticFinding {
        kind: "state_model_missing_transition_sites".to_string(),
        severity: FindingSeverity::Medium,
        concept_id: report.id.clone(),
        summary: format!(
            "State model '{}' has no explicit transition sites under its configured roots",
            report.id
        ),
        files: report.files.clone(),
        evidence: report.roots.clone(),
    })
}

fn build_transition_coverage_gap_finding(
    report: &super::StateIntegrityReport,
) -> Option<SemanticFinding> {
    if report.transition_gap_sites.is_empty() && report.missing_transition_variants.is_empty() {
        return None;
    }

    Some(SemanticFinding {
        kind: "state_model_transition_coverage_gap".to_string(),
        severity: FindingSeverity::High,
        concept_id: report.id.clone(),
        summary: state_transition_coverage_summary(report),
        files: report.files.clone(),
        evidence: build_transition_coverage_evidence(report),
    })
}

fn build_transition_coverage_evidence(report: &super::StateIntegrityReport) -> Vec<String> {
    report
        .transition_gap_sites
        .iter()
        .map(|site| {
            let source_variant = site.source_variant.as_deref().unwrap_or("unknown");
            format!("{}:{} [{}]", site.path, site.line, source_variant)
        })
        .chain(
            report
                .missing_transition_variants
                .iter()
                .map(|variant| format!("missing transition source '{variant}'")),
        )
        .collect()
}

fn build_high_context_burden_finding(
    report: &super::StateIntegrityReport,
) -> Option<SemanticFinding> {
    if report.context_burden < 8 {
        return None;
    }

    Some(SemanticFinding {
        kind: "state_model_high_context_burden".to_string(),
        severity: FindingSeverity::Medium,
        concept_id: report.id.clone(),
        summary: format!(
            "State model '{}' has high static context burden ({})",
            report.id, report.context_burden
        ),
        files: report.files.clone(),
        evidence: report.domain_symbol_names.clone(),
    })
}

fn collect_relevant_state_inputs<'a>(
    state_model: &StateModelRule,
    semantic: &'a SemanticSnapshot,
) -> RelevantStateInputs<'a> {
    let sites = semantic
        .closed_domain_sites
        .iter()
        .filter(|site| roots_match_path(&state_model.roots, &site.path))
        .collect::<Vec<_>>();
    let site_domains = sites
        .iter()
        .map(|site| site.domain_symbol_name.clone())
        .collect::<BTreeSet<_>>();
    let domains = semantic
        .closed_domains
        .iter()
        .filter(|domain| {
            site_domains.contains(&domain.symbol_name)
                || roots_match_path(&state_model.roots, &domain.path)
        })
        .collect::<Vec<_>>();
    let mut domain_symbol_names = domains
        .iter()
        .map(|domain| domain.symbol_name.clone())
        .collect::<BTreeSet<_>>();
    domain_symbol_names.extend(site_domains);

    RelevantStateInputs {
        sites,
        domains,
        domain_symbol_names,
    }
}

fn collect_related_obligations<'a>(
    obligations: &'a [ObligationReport],
    domain_symbol_names: &BTreeSet<String>,
) -> Vec<&'a ObligationReport> {
    obligations
        .iter()
        .filter(|obligation| {
            obligation
                .domain_symbol_name
                .as_ref()
                .map(|symbol_name| domain_symbol_names.contains(symbol_name))
                .unwrap_or(false)
        })
        .collect()
}

fn collect_relevant_report_files(relevant: &RelevantStateInputs<'_>) -> BTreeSet<String> {
    let mut files = BTreeSet::new();
    for domain in &relevant.domains {
        files.insert(domain.path.clone());
    }
    for site in &relevant.sites {
        files.insert(site.path.clone());
    }
    files
}

fn summarize_obligation_coverage(
    related_obligations: &[&ObligationReport],
) -> ObligationCoverageSummary {
    let mut files = BTreeSet::new();
    let mut required_sites = BTreeSet::new();
    let mut satisfied_sites = BTreeSet::new();
    let mut missing_sites = BTreeSet::new();
    let mut missing_variants = BTreeSet::new();
    let mut context_burden = 0usize;

    for obligation in related_obligations {
        files.extend(obligation.files.iter().cloned());
        required_sites.extend(obligation.required_sites.iter().cloned());
        satisfied_sites.extend(obligation.satisfied_sites.iter().cloned());
        missing_sites.extend(obligation.missing_sites.iter().cloned());
        missing_variants.extend(obligation.missing_variants.iter().cloned());
        context_burden += obligation.context_burden;
    }

    ObligationCoverageSummary {
        files,
        required_sites,
        satisfied_sites,
        missing_sites,
        missing_variants,
        context_burden,
    }
}

fn summarize_transition_coverage(
    state_model: &StateModelRule,
    semantic: &SemanticSnapshot,
    domain_symbol_names: &BTreeSet<String>,
    relevant_domains: &[&ClosedDomain],
) -> TransitionCoverageSummary {
    let transition_sites_supported = semantic
        .capabilities
        .contains(&SemanticCapability::TransitionSites);
    let sites = semantic
        .transition_sites
        .iter()
        .filter(|site| {
            roots_match_path(&state_model.roots, &site.path)
                && domain_symbol_names.contains(&site.domain_symbol_name)
        })
        .cloned()
        .collect::<Vec<_>>();
    let files = sites
        .iter()
        .map(|site| site.path.clone())
        .collect::<BTreeSet<_>>();
    let gap_sites = transition_gap_sites(&sites);
    let missing_variants = missing_transition_variants(relevant_domains, &sites, &gap_sites);

    TransitionCoverageSummary {
        transition_sites_supported,
        files,
        sites,
        gap_sites,
        missing_variants,
    }
}

pub(super) fn state_model_in_changed_scope(
    state_model: &StateModelRule,
    report: &super::StateIntegrityReport,
    changed_files: &BTreeSet<String>,
) -> bool {
    if changed_files.is_empty() {
        return false;
    }

    changed_files
        .iter()
        .any(|path| roots_match_path(&state_model.roots, path) || report.files.contains(path))
}

pub(super) fn missing_switch_domains(
    state_model: &StateModelRule,
    domain_symbol_names: &BTreeSet<String>,
    relevant_sites: &[&ExhaustivenessSite],
) -> Vec<String> {
    if !state_model.require_exhaustive_switch {
        return Vec::new();
    }

    domain_symbol_names
        .iter()
        .filter(|domain| {
            !relevant_sites.iter().any(|site| {
                site.domain_symbol_name == **domain
                    && site.site_kind == ExhaustivenessSiteKind::Switch
            })
        })
        .cloned()
        .collect()
}

pub(super) fn missing_assert_never_domains(
    state_model: &StateModelRule,
    domain_symbol_names: &BTreeSet<String>,
    relevant_sites: &[&ExhaustivenessSite],
) -> Vec<String> {
    if !state_model.require_assert_never {
        return Vec::new();
    }

    domain_symbol_names
        .iter()
        .filter(|domain| {
            !relevant_sites.iter().any(|site| {
                site.domain_symbol_name == **domain
                    && site.proof_kind == ExhaustivenessProofKind::AssertNever
            })
        })
        .cloned()
        .collect()
}

pub(super) fn explicitness_score_0_10000(
    state_model: &StateModelRule,
    domain_count: usize,
    site_count: usize,
    required_site_count: usize,
    satisfied_site_count: usize,
    missing_switch_count: usize,
    missing_assert_never_count: usize,
    transition_sites_supported: bool,
    transition_site_count: usize,
    transition_gap_count: usize,
) -> u32 {
    if domain_count == 0 {
        return 0;
    }

    let mut components = vec![1.0];
    let coverage = if required_site_count == 0 {
        if site_count > 0 {
            1.0
        } else {
            0.0
        }
    } else {
        satisfied_site_count as f64 / required_site_count as f64
    };
    components.push(coverage.clamp(0.0, 1.0));

    if state_model.require_exhaustive_switch {
        components
            .push((1.0 - (missing_switch_count as f64 / domain_count as f64)).clamp(0.0, 1.0));
    }
    if state_model.require_assert_never {
        components.push(
            (1.0 - (missing_assert_never_count as f64 / domain_count as f64)).clamp(0.0, 1.0),
        );
    }
    if transition_sites_supported {
        let transition_component = if transition_site_count == 0 {
            0.0
        } else {
            (1.0 - (transition_gap_count as f64 / transition_site_count as f64)).clamp(0.0, 1.0)
        };
        components.push(transition_component);
    }

    ((components.iter().sum::<f64>() / components.len() as f64) * 10000.0).round() as u32
}

fn state_variant_coverage_summary(report: &super::StateIntegrityReport) -> String {
    match (report.missing_sites.len(), report.missing_variants.len()) {
        (0, missing_variant_count) => format!(
            "State model '{}' is missing coverage for {} variant(s)",
            report.id, missing_variant_count
        ),
        (missing_site_count, 0) => format!(
            "State model '{}' is missing {} required state update site(s)",
            report.id, missing_site_count
        ),
        (missing_site_count, missing_variant_count) => format!(
            "State model '{}' is missing {} required state update site(s) and {} variant(s)",
            report.id, missing_site_count, missing_variant_count
        ),
    }
}

fn state_transition_coverage_summary(report: &super::StateIntegrityReport) -> String {
    match (
        report.transition_gap_sites.len(),
        report.missing_transition_variants.len(),
    ) {
        (0, missing_variant_count) => format!(
            "State model '{}' is missing explicit transition coverage for {} source variant(s)",
            report.id, missing_variant_count
        ),
        (gap_count, 0) => format!(
            "State model '{}' has {} transition branch(es) without an explicit next-state mapping",
            report.id, gap_count
        ),
        (gap_count, missing_variant_count) => format!(
            "State model '{}' has {} transition branch(es) without an explicit next-state mapping and {} uncovered source variant(s)",
            report.id, gap_count, missing_variant_count
        ),
    }
}

fn transition_gap_sites(transition_sites: &[TransitionSite]) -> Vec<TransitionSite> {
    let mut groups = BTreeMap::<&str, Vec<&TransitionSite>>::new();
    for site in transition_sites {
        groups.entry(&site.group_id).or_default().push(site);
    }

    let mut gaps = Vec::new();
    for sites in groups.into_values() {
        gaps.extend(
            sites
                .into_iter()
                .filter(|site| site.target_variants.is_empty())
                .cloned(),
        );
    }
    gaps
}

fn missing_transition_variants(
    relevant_domains: &[&crate::analysis::semantic::ClosedDomain],
    transition_sites: &[TransitionSite],
    transition_gap_sites: &[TransitionSite],
) -> BTreeSet<String> {
    let mut missing_variants = transition_gap_sites
        .iter()
        .filter_map(|site| site.source_variant.clone())
        .collect::<BTreeSet<_>>();
    if transition_sites.is_empty() {
        return missing_variants;
    }

    let expected_variants_by_domain = relevant_domains
        .iter()
        .map(|domain| {
            (
                domain.symbol_name.as_str(),
                domain
                    .variants
                    .iter()
                    .map(String::as_str)
                    .collect::<BTreeSet<_>>(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let covered_variants_by_domain = transition_sites
        .iter()
        .filter_map(|site| {
            site.source_variant
                .as_deref()
                .map(|variant| (site.domain_symbol_name.as_str(), variant))
        })
        .fold(
            BTreeMap::<&str, BTreeSet<&str>>::new(),
            |mut groups, (domain, variant)| {
                groups.entry(domain).or_default().insert(variant);
                groups
            },
        );

    for (domain, expected_variants) in expected_variants_by_domain {
        let covered_variants = covered_variants_by_domain.get(domain);
        for variant in expected_variants {
            if covered_variants
                .map(|variants| variants.contains(variant))
                .unwrap_or(false)
            {
                continue;
            }
            missing_variants.insert(variant.to_string());
        }
    }

    missing_variants
}

pub(super) fn roots_match_path(roots: &[String], path: &str) -> bool {
    roots.iter().any(|root| path_matches(root, path))
}

fn path_matches(pattern: &str, path: &str) -> bool {
    rules::glob_match(pattern, path) || pattern == path
}
