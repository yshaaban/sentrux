//! Conservative obligation engine for closed-domain completeness.

use super::{FindingSeverity, SemanticFinding};
use crate::analysis::semantic::SemanticSnapshot;
use crate::metrics::rules::RulesConfig;
use std::collections::{BTreeSet, HashSet};
#[path = "obligations_contract.rs"]
mod obligations_contract;
#[path = "obligations_domain.rs"]
mod obligations_domain;

#[cfg(test)]
#[path = "obligations_tests.rs"]
mod obligations_tests;

use self::obligations_contract::{build_contract_obligation, path_matches};
use self::obligations_domain::{
    build_domain_obligation, concept_rule_paths, domain_is_in_scope, relevant_domains,
    zero_config_domain_is_actionable, zero_config_domain_is_related_to_changed_files,
    DomainObligationPlan,
};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ObligationScope {
    All,
    Changed,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObligationOrigin {
    Explicit,
    ZeroConfig,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ObligationTrustTier {
    Trusted,
    Watchpoint,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ObligationConfidence {
    High,
    Medium,
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
    pub origin: ObligationOrigin,
    pub trust_tier: ObligationTrustTier,
    pub confidence: ObligationConfidence,
    pub severity: FindingSeverity,
    pub score_0_10000: u32,
    pub summary: String,
    pub files: Vec<String>,
    pub required_sites: Vec<ObligationSite>,
    pub satisfied_sites: Vec<ObligationSite>,
    pub missing_sites: Vec<ObligationSite>,
    pub missing_variants: Vec<String>,
    pub context_burden: usize,
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

            let report = build_domain_obligation(
                domain,
                DomainObligationPlan::for_concept(concept, domain, semantic, changed_files),
            );
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
            && !zero_config_domain_is_related_to_changed_files(domain, semantic, changed_files)
        {
            continue;
        }

        let report = build_domain_obligation(
            domain,
            DomainObligationPlan::for_zero_config(domain, semantic),
        );
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

pub fn build_obligation_findings(obligations: &[ObligationReport]) -> Vec<SemanticFinding> {
    obligations
        .iter()
        .filter(|obligation| !obligation.missing_sites.is_empty())
        .map(|obligation| SemanticFinding {
            kind: obligation.kind.clone(),
            severity: obligation.severity,
            concept_id: obligation_concept_id(obligation).to_owned(),
            summary: obligation.summary.clone(),
            files: obligation.files.clone(),
            evidence: obligation
                .missing_sites
                .iter()
                .map(|site| format!("{} [{}]", site.path, site.detail))
                .collect(),
        })
        .collect()
}

pub fn changed_concepts_from_obligations(obligations: &[ObligationReport]) -> Vec<String> {
    let mut concepts = BTreeSet::new();
    for obligation in obligations {
        let concept_id = obligation_concept_id(obligation);
        if !concept_id.is_empty() {
            concepts.insert(concept_id.to_owned());
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

fn obligation_concept_id(obligation: &ObligationReport) -> &str {
    obligation
        .concept_id
        .as_deref()
        .or(obligation.domain_symbol_name.as_deref())
        .unwrap_or_default()
}

pub(crate) fn obligation_report_origin(has_explicit_owner: bool) -> ObligationOrigin {
    if has_explicit_owner {
        ObligationOrigin::Explicit
    } else {
        ObligationOrigin::ZeroConfig
    }
}

pub(crate) fn obligation_report_trust_tier(origin: ObligationOrigin) -> ObligationTrustTier {
    match origin {
        ObligationOrigin::Explicit => ObligationTrustTier::Trusted,
        ObligationOrigin::ZeroConfig => ObligationTrustTier::Watchpoint,
    }
}

pub(crate) fn obligation_report_confidence(origin: ObligationOrigin) -> ObligationConfidence {
    match origin {
        ObligationOrigin::Explicit => ObligationConfidence::High,
        ObligationOrigin::ZeroConfig => ObligationConfidence::Medium,
    }
}

pub(crate) fn obligation_report_severity(
    kind: &str,
    origin: ObligationOrigin,
    missing_variants: usize,
) -> FindingSeverity {
    if kind == "closed_domain_exhaustiveness" || missing_variants > 0 {
        FindingSeverity::High
    } else if origin == ObligationOrigin::Explicit {
        FindingSeverity::High
    } else {
        FindingSeverity::Medium
    }
}

pub(crate) fn obligation_report_score(
    severity: FindingSeverity,
    origin: ObligationOrigin,
    missing_site_count: usize,
) -> u32 {
    let severity_bonus = match severity {
        FindingSeverity::High => 1800,
        FindingSeverity::Medium => 1000,
        FindingSeverity::Low => 200,
    };
    let origin_bonus = match origin {
        ObligationOrigin::Explicit => 1600,
        ObligationOrigin::ZeroConfig => 600,
    };
    let site_bonus = (missing_site_count.min(3) as u32) * 500;
    (6000 + severity_bonus + origin_bonus + site_bonus).min(10_000)
}
