//! Conservative obligation engine for closed-domain completeness.

use super::{concept_targets, symbol_matches_targets, SemanticFinding};
use crate::analysis::semantic::{ClosedDomain, ExhaustivenessSite, SemanticSnapshot};
use crate::metrics::rules::{self, ConceptRule, RulesConfig};
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

fn path_matches(pattern: &str, path: &str) -> bool {
    if rules::glob_match(pattern, path) {
        return true;
    }
    pattern == path
}

#[cfg(test)]
mod tests {
    use super::{build_obligations, obligation_score_0_10000, ObligationScope};
    use crate::analysis::semantic::{
        ClosedDomain, ExhaustivenessSite, ProjectModel, SemanticCapability, SemanticFileFact,
        SemanticSnapshot,
    };
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
}
