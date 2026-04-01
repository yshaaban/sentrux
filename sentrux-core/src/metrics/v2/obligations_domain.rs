use super::obligations_contract::path_matches;
use crate::analysis::semantic::{ClosedDomain, ExhaustivenessSite, SemanticSnapshot};
use crate::metrics::rules::ConceptRule;
use crate::metrics::testgap::is_test_file;
use crate::metrics::v2::{
    concept_targets, symbol_matches_targets, ObligationReport, ObligationScope, ObligationSite,
};
use std::collections::{BTreeSet, HashSet};

const MAX_ZERO_CONFIG_DOMAIN_VARIANTS: usize = 16;

#[derive(Default)]
struct DomainSiteCoverage {
    files: BTreeSet<String>,
    required_sites: BTreeSet<ObligationSite>,
    satisfied_sites: BTreeSet<ObligationSite>,
    missing_sites: BTreeSet<ObligationSite>,
    missing_variants: BTreeSet<String>,
}

pub(super) struct DomainObligationPlan<'a> {
    concept: Option<&'a ConceptRule>,
    changed_files: Option<&'a BTreeSet<String>>,
    require_declared_site_when_missing: bool,
    sites: Vec<&'a ExhaustivenessSite>,
}

impl<'a> DomainObligationPlan<'a> {
    pub(super) fn for_concept(
        concept: &'a ConceptRule,
        domain: &ClosedDomain,
        semantic: &'a SemanticSnapshot,
        changed_files: &'a BTreeSet<String>,
    ) -> Self {
        Self {
            concept: Some(concept),
            changed_files: Some(changed_files),
            require_declared_site_when_missing: true,
            sites: relevant_exhaustiveness_sites(domain, semantic),
        }
    }

    pub(super) fn for_zero_config(domain: &ClosedDomain, semantic: &'a SemanticSnapshot) -> Self {
        Self {
            concept: None,
            changed_files: None,
            require_declared_site_when_missing: false,
            sites: relevant_production_exhaustiveness_sites(domain, semantic),
        }
    }
}

pub(super) fn relevant_domains<'a>(
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

pub(super) fn domain_is_in_scope(
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

pub(super) fn build_domain_obligation(
    domain: &ClosedDomain,
    plan: DomainObligationPlan<'_>,
) -> ObligationReport {
    let DomainObligationPlan {
        concept,
        changed_files,
        require_declared_site_when_missing,
        sites,
    } = plan;
    let mut coverage = evaluate_domain_site_coverage(domain, &sites);
    coverage.files.insert(domain.path.clone());

    if require_declared_site_when_missing && sites.is_empty() {
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

    if let (Some(concept), Some(changed_files)) = (concept, changed_files) {
        apply_related_test_coverage(&mut coverage, concept, changed_files);
    }

    let required_sites = coverage.required_sites.into_iter().collect::<Vec<_>>();
    let satisfied_sites = coverage.satisfied_sites.into_iter().collect::<Vec<_>>();
    let missing_sites = coverage.missing_sites.into_iter().collect::<Vec<_>>();
    let missing_variants = coverage.missing_variants.into_iter().collect::<Vec<_>>();
    let files = coverage.files.into_iter().collect::<Vec<_>>();
    let summary_label = concept
        .map(|concept| concept.id.as_str())
        .unwrap_or(domain.symbol_name.as_str());

    ObligationReport {
        id: concept
            .map(|concept| format!("{}::{}", concept.id, domain.symbol_name))
            .unwrap_or_else(|| format!("closed_domain::{}", domain.symbol_name)),
        kind: "closed_domain_exhaustiveness".to_string(),
        concept_id: concept.map(|concept| concept.id.clone()),
        domain_symbol_name: Some(domain.symbol_name.clone()),
        summary: obligation_summary(summary_label, domain, &missing_sites, &missing_variants),
        context_burden: required_sites.len(),
        files,
        required_sites,
        satisfied_sites,
        missing_sites,
        missing_variants,
    }
}

fn apply_related_test_coverage(
    coverage: &mut DomainSiteCoverage,
    concept: &ConceptRule,
    changed_files: &BTreeSet<String>,
) {
    if concept.related_tests.is_empty() || changed_files.is_empty() {
        return;
    }

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

pub(super) fn relevant_production_exhaustiveness_sites<'a>(
    domain: &ClosedDomain,
    semantic: &'a SemanticSnapshot,
) -> Vec<&'a ExhaustivenessSite> {
    relevant_exhaustiveness_sites(domain, semantic)
        .into_iter()
        .filter(|site| !is_test_file(&site.path))
        .collect()
}

pub(super) fn zero_config_domain_is_actionable(
    domain: &ClosedDomain,
    semantic: &SemanticSnapshot,
) -> bool {
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
                site.site_kind.as_str()
            )
        } else {
            format!(
                "missing variants: {}",
                site_variants.iter().cloned().collect::<Vec<_>>().join(", ")
            )
        };
        let obligation_site = ObligationSite {
            path: site.path.clone(),
            kind: site.site_kind.as_str().to_string(),
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

pub(super) fn concept_rule_paths(concept: &ConceptRule) -> Vec<String> {
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
