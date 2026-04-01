//! Conservative state-integrity analysis built on closed-domain and obligation facts.

use super::{FindingSeverity, ObligationReport, ObligationSite, SemanticFinding};
use crate::analysis::semantic::{
    ExhaustivenessProofKind, ExhaustivenessSite, ExhaustivenessSiteKind, SemanticCapability,
    SemanticSnapshot, TransitionSite,
};
use crate::metrics::rules::{self, RulesConfig, StateModelRule};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum StateScope {
    All,
    Changed,
}

#[derive(Debug, Clone, serde::Serialize, Default)]
pub struct StateIntegrityReport {
    pub id: String,
    pub kind: String,
    pub roots: Vec<String>,
    pub domain_symbol_names: Vec<String>,
    pub files: Vec<String>,
    pub required_sites: Vec<ObligationSite>,
    pub satisfied_sites: Vec<ObligationSite>,
    pub missing_sites: Vec<ObligationSite>,
    pub missing_variants: Vec<String>,
    pub missing_exhaustive_switch_domains: Vec<String>,
    pub missing_assert_never_domains: Vec<String>,
    pub transition_sites_supported: bool,
    pub transition_sites: Vec<TransitionSite>,
    pub transition_gap_sites: Vec<TransitionSite>,
    pub missing_transition_variants: Vec<String>,
    pub explicitness_score_0_10000: u32,
    pub context_burden: usize,
}

pub fn build_state_integrity_reports(
    config: &RulesConfig,
    semantic: &SemanticSnapshot,
    obligations: &[ObligationReport],
    scope: StateScope,
    changed_files: &BTreeSet<String>,
) -> Vec<StateIntegrityReport> {
    let mut reports = Vec::new();

    for state_model in &config.state_model {
        let report = build_state_integrity_report(state_model, semantic, obligations);
        if scope == StateScope::Changed
            && !state_model_in_changed_scope(state_model, &report, changed_files)
        {
            continue;
        }
        reports.push(report);
    }

    reports.sort_by(|left, right| {
        left.explicitness_score_0_10000
            .cmp(&right.explicitness_score_0_10000)
            .then_with(|| right.context_burden.cmp(&left.context_burden))
            .then_with(|| left.id.cmp(&right.id))
    });
    reports
}

pub fn build_state_integrity_findings(reports: &[StateIntegrityReport]) -> Vec<SemanticFinding> {
    let mut findings = Vec::new();

    for report in reports {
        if report.domain_symbol_names.is_empty() {
            findings.push(SemanticFinding {
                kind: "state_model_unmapped".to_string(),
                severity: FindingSeverity::Medium,
                concept_id: report.id.clone(),
                summary: format!(
                    "State model '{}' has no matching closed-domain coverage under its configured roots",
                    report.id
                ),
                files: report.files.clone(),
                evidence: report.roots.clone(),
            });
            continue;
        }

        if !report.missing_exhaustive_switch_domains.is_empty() {
            findings.push(SemanticFinding {
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
            });
        }

        if !report.missing_assert_never_domains.is_empty() {
            findings.push(SemanticFinding {
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
            });
        }

        if !report.missing_sites.is_empty() || !report.missing_variants.is_empty() {
            let summary = match (
                report.missing_sites.len(),
                report.missing_variants.len(),
            ) {
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
            };
            findings.push(SemanticFinding {
                kind: "state_model_missing_variant_coverage".to_string(),
                severity: if report.missing_variants.is_empty() {
                    FindingSeverity::Medium
                } else {
                    FindingSeverity::High
                },
                concept_id: report.id.clone(),
                summary,
                files: report.files.clone(),
                evidence: report
                    .missing_sites
                    .iter()
                    .map(|site| format!("{} [{}]", site.path, site.detail))
                    .chain(
                        report
                            .missing_variants
                            .iter()
                            .map(|variant| format!("missing variant '{variant}'")),
                    )
                    .collect(),
            });
        }

        if report.transition_sites_supported
            && report.transition_sites.is_empty()
            && !report.domain_symbol_names.is_empty()
        {
            findings.push(SemanticFinding {
                kind: "state_model_missing_transition_sites".to_string(),
                severity: FindingSeverity::Medium,
                concept_id: report.id.clone(),
                summary: format!(
                    "State model '{}' has no explicit transition sites under its configured roots",
                    report.id
                ),
                files: report.files.clone(),
                evidence: report.roots.clone(),
            });
        }

        if !report.transition_gap_sites.is_empty() || !report.missing_transition_variants.is_empty()
        {
            let summary = match (
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
            };
            findings.push(SemanticFinding {
                kind: "state_model_transition_coverage_gap".to_string(),
                severity: FindingSeverity::High,
                concept_id: report.id.clone(),
                summary,
                files: report.files.clone(),
                evidence: report
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
                    .collect(),
            });
        }

        if report.context_burden >= 8 {
            findings.push(SemanticFinding {
                kind: "state_model_high_context_burden".to_string(),
                severity: FindingSeverity::Medium,
                concept_id: report.id.clone(),
                summary: format!(
                    "State model '{}' has high static context burden ({})",
                    report.id, report.context_burden
                ),
                files: report.files.clone(),
                evidence: report.domain_symbol_names.clone(),
            });
        }
    }

    findings
}

pub fn state_integrity_score_0_10000(reports: &[StateIntegrityReport]) -> u32 {
    if reports.is_empty() {
        return 10000;
    }

    let total = reports
        .iter()
        .map(|report| report.explicitness_score_0_10000 as u64)
        .sum::<u64>();
    (total / reports.len() as u64) as u32
}

pub fn changed_state_model_ids_from_files(
    config: &RulesConfig,
    changed_files: &BTreeSet<String>,
) -> Vec<String> {
    let mut ids = BTreeSet::new();
    for state_model in &config.state_model {
        if changed_files
            .iter()
            .any(|path| roots_match_path(&state_model.roots, path))
        {
            ids.insert(state_model.id.clone());
        }
    }
    ids.into_iter().collect()
}

fn build_state_integrity_report(
    state_model: &StateModelRule,
    semantic: &SemanticSnapshot,
    obligations: &[ObligationReport],
) -> StateIntegrityReport {
    let relevant_sites = semantic
        .closed_domain_sites
        .iter()
        .filter(|site| roots_match_path(&state_model.roots, &site.path))
        .collect::<Vec<_>>();
    let site_domains = relevant_sites
        .iter()
        .map(|site| site.domain_symbol_name.clone())
        .collect::<BTreeSet<_>>();
    let relevant_domains = semantic
        .closed_domains
        .iter()
        .filter(|domain| {
            site_domains.contains(&domain.symbol_name)
                || roots_match_path(&state_model.roots, &domain.path)
        })
        .collect::<Vec<_>>();

    let mut domain_symbol_names = relevant_domains
        .iter()
        .map(|domain| domain.symbol_name.clone())
        .collect::<BTreeSet<_>>();
    domain_symbol_names.extend(site_domains);

    let related_obligations = obligations
        .iter()
        .filter(|obligation| {
            obligation
                .domain_symbol_name
                .as_ref()
                .map(|symbol_name| domain_symbol_names.contains(symbol_name))
                .unwrap_or(false)
        })
        .collect::<Vec<_>>();

    let mut files = BTreeSet::new();
    for domain in &relevant_domains {
        files.insert(domain.path.clone());
    }
    for site in &relevant_sites {
        files.insert(site.path.clone());
    }

    let mut required_sites = BTreeSet::new();
    let mut satisfied_sites = BTreeSet::new();
    let mut missing_sites = BTreeSet::new();
    let mut missing_variants = BTreeSet::new();
    let transition_sites_supported = semantic
        .capabilities
        .contains(&SemanticCapability::TransitionSites);
    let transition_sites = semantic
        .transition_sites
        .iter()
        .filter(|site| {
            roots_match_path(&state_model.roots, &site.path)
                && domain_symbol_names.contains(&site.domain_symbol_name)
        })
        .cloned()
        .collect::<Vec<_>>();
    for site in &transition_sites {
        files.insert(site.path.clone());
    }
    let transition_gap_sites = transition_gap_sites(&transition_sites);
    let missing_transition_variants =
        missing_transition_variants(&relevant_domains, &transition_sites, &transition_gap_sites);
    let mut context_burden = 0usize;
    for obligation in related_obligations {
        files.extend(obligation.files.iter().cloned());
        required_sites.extend(obligation.required_sites.iter().cloned());
        satisfied_sites.extend(obligation.satisfied_sites.iter().cloned());
        missing_sites.extend(obligation.missing_sites.iter().cloned());
        missing_variants.extend(obligation.missing_variants.iter().cloned());
        context_burden += obligation.context_burden;
    }

    let missing_exhaustive_switch_domains =
        missing_switch_domains(state_model, &domain_symbol_names, &relevant_sites);
    let missing_assert_never_domains =
        missing_assert_never_domains(state_model, &domain_symbol_names, &relevant_sites);
    let explicitness_score_0_10000 = explicitness_score_0_10000(
        state_model,
        domain_symbol_names.len(),
        relevant_sites.len(),
        required_sites.len(),
        satisfied_sites.len(),
        missing_exhaustive_switch_domains.len(),
        missing_assert_never_domains.len(),
        transition_sites_supported,
        transition_sites.len(),
        transition_gap_sites.len(),
    );
    context_burden += transition_sites.len();

    StateIntegrityReport {
        id: state_model.id.clone(),
        kind: state_model.kind.clone(),
        roots: state_model.roots.clone(),
        domain_symbol_names: domain_symbol_names.into_iter().collect(),
        files: files.into_iter().collect(),
        required_sites: required_sites.into_iter().collect(),
        satisfied_sites: satisfied_sites.into_iter().collect(),
        missing_sites: missing_sites.into_iter().collect(),
        missing_variants: missing_variants.into_iter().collect(),
        missing_exhaustive_switch_domains,
        missing_assert_never_domains,
        transition_sites_supported,
        transition_sites,
        transition_gap_sites,
        missing_transition_variants: missing_transition_variants.into_iter().collect(),
        explicitness_score_0_10000,
        context_burden,
    }
}

fn state_model_in_changed_scope(
    state_model: &StateModelRule,
    report: &StateIntegrityReport,
    changed_files: &BTreeSet<String>,
) -> bool {
    if changed_files.is_empty() {
        return false;
    }

    changed_files
        .iter()
        .any(|path| roots_match_path(&state_model.roots, path) || report.files.contains(path))
}

fn missing_switch_domains(
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

fn missing_assert_never_domains(
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

fn explicitness_score_0_10000(
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

fn roots_match_path(roots: &[String], path: &str) -> bool {
    roots.iter().any(|root| path_matches(root, path))
}

fn path_matches(pattern: &str, path: &str) -> bool {
    rules::glob_match(pattern, path) || pattern == path
}

#[cfg(test)]
mod tests {
    use super::{
        build_state_integrity_findings, build_state_integrity_reports,
        changed_state_model_ids_from_files, state_integrity_score_0_10000, StateScope,
    };
    use crate::analysis::semantic::{
        ClosedDomain, ExhaustivenessProofKind, ExhaustivenessSite, ExhaustivenessSiteKind,
        ProjectModel, SemanticCapability, SemanticSnapshot, TransitionKind, TransitionSite,
    };
    use crate::metrics::rules::RulesConfig;
    use crate::metrics::v2::{build_obligations, ObligationScope};
    use std::collections::BTreeSet;

    #[test]
    fn state_reports_capture_missing_assert_never_and_variants() {
        let config: RulesConfig = toml::from_str(
            r#"
                [[concept]]
                id = "browser_sync_state"
                anchors = ["src/domain/browser-sync-state.ts::BrowserSyncState"]

                [[state_model]]
                id = "browser_state_sync"
                roots = ["src/runtime/browser-state-sync-controller.ts"]
                require_exhaustive_switch = true
                require_assert_never = true
            "#,
        )
        .expect("rules config");
        let semantic = SemanticSnapshot {
            project: ProjectModel::default(),
            analyzed_files: 1,
            capabilities: vec![
                SemanticCapability::ClosedDomains,
                SemanticCapability::ClosedDomainSites,
                SemanticCapability::TransitionSites,
            ],
            files: Vec::new(),
            symbols: Vec::new(),
            reads: Vec::new(),
            writes: Vec::new(),
            closed_domains: vec![ClosedDomain {
                path: "src/domain/browser-sync-state.ts".to_string(),
                symbol_name: "BrowserSyncState".to_string(),
                variants: vec![
                    "idle".to_string(),
                    "running".to_string(),
                    "error".to_string(),
                ],
                line: 1,
                defining_file: Some("src/domain/browser-sync-state.ts".to_string()),
            }],
            closed_domain_sites: vec![ExhaustivenessSite {
                path: "src/runtime/browser-state-sync-controller.ts".to_string(),
                domain_symbol_name: "BrowserSyncState".to_string(),
                defining_file: Some("src/domain/browser-sync-state.ts".to_string()),
                site_kind: ExhaustivenessSiteKind::Switch,
                proof_kind: ExhaustivenessProofKind::Switch,
                covered_variants: vec!["idle".to_string(), "running".to_string()],
                line: 12,
            }],
            transition_sites: Vec::new(),
        };
        let obligations =
            build_obligations(&config, &semantic, ObligationScope::All, &BTreeSet::new());

        let reports = build_state_integrity_reports(
            &config,
            &semantic,
            &obligations,
            StateScope::All,
            &BTreeSet::new(),
        );
        let findings = build_state_integrity_findings(&reports);

        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].missing_variants, vec!["error".to_string()]);
        assert_eq!(
            reports[0].missing_assert_never_domains,
            vec!["BrowserSyncState".to_string()]
        );
        assert!(reports[0].explicitness_score_0_10000 < 10000);
        assert!(findings
            .iter()
            .any(|finding| finding.kind == "state_model_missing_assert_never"));
        assert!(findings
            .iter()
            .any(|finding| finding.kind == "state_model_missing_variant_coverage"));
        assert!(state_integrity_score_0_10000(&reports) < 10000);
    }

    #[test]
    fn state_reports_flag_unmapped_roots_and_changed_models() {
        let config: RulesConfig = toml::from_str(
            r#"
                [[state_model]]
                id = "orphan_state_model"
                roots = ["src/runtime/orphan-controller.ts"]
                require_exhaustive_switch = true
            "#,
        )
        .expect("rules config");
        let semantic = SemanticSnapshot {
            project: ProjectModel::default(),
            analyzed_files: 0,
            capabilities: Vec::new(),
            files: Vec::new(),
            symbols: Vec::new(),
            reads: Vec::new(),
            writes: Vec::new(),
            closed_domains: Vec::new(),
            closed_domain_sites: Vec::new(),
            transition_sites: Vec::new(),
        };

        let changed_files = BTreeSet::from(["src/runtime/orphan-controller.ts".to_string()]);
        let reports = build_state_integrity_reports(
            &config,
            &semantic,
            &[],
            StateScope::Changed,
            &changed_files,
        );
        let findings = build_state_integrity_findings(&reports);

        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].domain_symbol_names.len(), 0);
        assert!(findings
            .iter()
            .any(|finding| finding.kind == "state_model_unmapped"));
        assert_eq!(
            changed_state_model_ids_from_files(&config, &changed_files),
            vec!["orphan_state_model".to_string()]
        );
    }

    #[test]
    fn state_reports_capture_transition_gaps() {
        let config: RulesConfig = toml::from_str(
            r#"
                [[state_model]]
                id = "browser_state_sync"
                roots = ["src/runtime/browser-state-sync-controller.ts"]
                require_exhaustive_switch = true
                require_assert_never = true
            "#,
        )
        .expect("rules config");
        let semantic = SemanticSnapshot {
            project: ProjectModel::default(),
            analyzed_files: 1,
            capabilities: vec![
                SemanticCapability::ClosedDomains,
                SemanticCapability::ClosedDomainSites,
                SemanticCapability::TransitionSites,
            ],
            files: Vec::new(),
            symbols: Vec::new(),
            reads: Vec::new(),
            writes: Vec::new(),
            closed_domains: vec![ClosedDomain {
                path: "src/domain/browser-sync-state.ts".to_string(),
                symbol_name: "BrowserSyncState".to_string(),
                variants: vec![
                    "idle".to_string(),
                    "running".to_string(),
                    "error".to_string(),
                ],
                line: 1,
                defining_file: Some("src/domain/browser-sync-state.ts".to_string()),
            }],
            closed_domain_sites: vec![ExhaustivenessSite {
                path: "src/runtime/browser-state-sync-controller.ts".to_string(),
                domain_symbol_name: "BrowserSyncState".to_string(),
                defining_file: Some("src/domain/browser-sync-state.ts".to_string()),
                site_kind: ExhaustivenessSiteKind::Switch,
                proof_kind: ExhaustivenessProofKind::AssertNever,
                covered_variants: vec![
                    "idle".to_string(),
                    "running".to_string(),
                    "error".to_string(),
                ],
                line: 12,
            }],
            transition_sites: vec![
                TransitionSite {
                    path: "src/runtime/browser-state-sync-controller.ts".to_string(),
                    domain_symbol_name: "BrowserSyncState".to_string(),
                    group_id: "src/runtime/browser-state-sync-controller.ts:12:BrowserSyncState"
                        .to_string(),
                    transition_kind: TransitionKind::SwitchCase,
                    source_variant: Some("idle".to_string()),
                    target_variants: vec!["running".to_string()],
                    line: 13,
                },
                TransitionSite {
                    path: "src/runtime/browser-state-sync-controller.ts".to_string(),
                    domain_symbol_name: "BrowserSyncState".to_string(),
                    group_id: "src/runtime/browser-state-sync-controller.ts:12:BrowserSyncState"
                        .to_string(),
                    transition_kind: TransitionKind::SwitchCase,
                    source_variant: Some("running".to_string()),
                    target_variants: vec!["error".to_string()],
                    line: 17,
                },
                TransitionSite {
                    path: "src/runtime/browser-state-sync-controller.ts".to_string(),
                    domain_symbol_name: "BrowserSyncState".to_string(),
                    group_id: "src/runtime/browser-state-sync-controller.ts:12:BrowserSyncState"
                        .to_string(),
                    transition_kind: TransitionKind::SwitchCase,
                    source_variant: Some("error".to_string()),
                    target_variants: Vec::new(),
                    line: 21,
                },
            ],
        };

        let reports = build_state_integrity_reports(
            &config,
            &semantic,
            &[],
            StateScope::All,
            &BTreeSet::new(),
        );
        let findings = build_state_integrity_findings(&reports);

        assert_eq!(reports.len(), 1);
        assert_eq!(
            reports[0].missing_transition_variants,
            vec!["error".to_string()]
        );
        assert_eq!(reports[0].transition_gap_sites.len(), 1);
        assert!(findings
            .iter()
            .any(|finding| finding.kind == "state_model_transition_coverage_gap"));
    }

    #[test]
    fn state_reports_flag_missing_transition_source_variants() {
        let config: RulesConfig = toml::from_str(
            r#"
                [[state_model]]
                id = "browser_state_sync"
                roots = ["src/runtime/browser-state-sync-controller.ts"]
            "#,
        )
        .expect("rules config");
        let semantic = SemanticSnapshot {
            project: ProjectModel::default(),
            analyzed_files: 1,
            capabilities: vec![
                SemanticCapability::ClosedDomains,
                SemanticCapability::ClosedDomainSites,
                SemanticCapability::TransitionSites,
            ],
            files: Vec::new(),
            symbols: Vec::new(),
            reads: Vec::new(),
            writes: Vec::new(),
            closed_domains: vec![ClosedDomain {
                path: "src/domain/browser-sync-state.ts".to_string(),
                symbol_name: "BrowserSyncState".to_string(),
                variants: vec![
                    "idle".to_string(),
                    "running".to_string(),
                    "error".to_string(),
                ],
                line: 1,
                defining_file: Some("src/domain/browser-sync-state.ts".to_string()),
            }],
            closed_domain_sites: vec![ExhaustivenessSite {
                path: "src/runtime/browser-state-sync-controller.ts".to_string(),
                domain_symbol_name: "BrowserSyncState".to_string(),
                defining_file: Some("src/domain/browser-sync-state.ts".to_string()),
                site_kind: ExhaustivenessSiteKind::Switch,
                proof_kind: ExhaustivenessProofKind::Switch,
                covered_variants: vec!["idle".to_string(), "running".to_string()],
                line: 8,
            }],
            transition_sites: vec![
                TransitionSite {
                    path: "src/runtime/browser-state-sync-controller.ts".to_string(),
                    domain_symbol_name: "BrowserSyncState".to_string(),
                    group_id: "src/runtime/browser-state-sync-controller.ts:8:BrowserSyncState"
                        .to_string(),
                    transition_kind: TransitionKind::SwitchCase,
                    source_variant: Some("idle".to_string()),
                    target_variants: vec!["running".to_string()],
                    line: 9,
                },
                TransitionSite {
                    path: "src/runtime/browser-state-sync-controller.ts".to_string(),
                    domain_symbol_name: "BrowserSyncState".to_string(),
                    group_id: "src/runtime/browser-state-sync-controller.ts:8:BrowserSyncState"
                        .to_string(),
                    transition_kind: TransitionKind::SwitchCase,
                    source_variant: Some("running".to_string()),
                    target_variants: vec!["error".to_string()],
                    line: 13,
                },
            ],
        };

        let reports = build_state_integrity_reports(
            &config,
            &semantic,
            &[],
            StateScope::All,
            &BTreeSet::new(),
        );
        let findings = build_state_integrity_findings(&reports);

        assert_eq!(reports.len(), 1);
        assert!(reports[0].transition_gap_sites.is_empty());
        assert_eq!(
            reports[0].missing_transition_variants,
            vec!["error".to_string()]
        );
        assert!(findings
            .iter()
            .any(|finding| finding.kind == "state_model_transition_coverage_gap"));
    }

    #[test]
    fn state_reports_treat_all_empty_transition_groups_as_gaps() {
        let config: RulesConfig = toml::from_str(
            r#"
                [[state_model]]
                id = "browser_state_sync"
                roots = ["src/runtime/browser-state-sync-controller.ts"]
            "#,
        )
        .expect("rules config");
        let semantic = SemanticSnapshot {
            project: ProjectModel::default(),
            analyzed_files: 1,
            capabilities: vec![
                SemanticCapability::ClosedDomains,
                SemanticCapability::ClosedDomainSites,
                SemanticCapability::TransitionSites,
            ],
            files: Vec::new(),
            symbols: Vec::new(),
            reads: Vec::new(),
            writes: Vec::new(),
            closed_domains: vec![ClosedDomain {
                path: "src/domain/browser-sync-state.ts".to_string(),
                symbol_name: "BrowserSyncState".to_string(),
                variants: vec![
                    "idle".to_string(),
                    "running".to_string(),
                    "error".to_string(),
                ],
                line: 1,
                defining_file: Some("src/domain/browser-sync-state.ts".to_string()),
            }],
            closed_domain_sites: vec![ExhaustivenessSite {
                path: "src/runtime/browser-state-sync-controller.ts".to_string(),
                domain_symbol_name: "BrowserSyncState".to_string(),
                defining_file: Some("src/domain/browser-sync-state.ts".to_string()),
                site_kind: ExhaustivenessSiteKind::Switch,
                proof_kind: ExhaustivenessProofKind::Switch,
                covered_variants: vec![
                    "idle".to_string(),
                    "running".to_string(),
                    "error".to_string(),
                ],
                line: 8,
            }],
            transition_sites: vec![
                TransitionSite {
                    path: "src/runtime/browser-state-sync-controller.ts".to_string(),
                    domain_symbol_name: "BrowserSyncState".to_string(),
                    group_id: "src/runtime/browser-state-sync-controller.ts:8:BrowserSyncState"
                        .to_string(),
                    transition_kind: TransitionKind::SwitchCase,
                    source_variant: Some("idle".to_string()),
                    target_variants: Vec::new(),
                    line: 9,
                },
                TransitionSite {
                    path: "src/runtime/browser-state-sync-controller.ts".to_string(),
                    domain_symbol_name: "BrowserSyncState".to_string(),
                    group_id: "src/runtime/browser-state-sync-controller.ts:8:BrowserSyncState"
                        .to_string(),
                    transition_kind: TransitionKind::SwitchCase,
                    source_variant: Some("running".to_string()),
                    target_variants: Vec::new(),
                    line: 13,
                },
            ],
        };

        let reports = build_state_integrity_reports(
            &config,
            &semantic,
            &[],
            StateScope::All,
            &BTreeSet::new(),
        );

        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].transition_gap_sites.len(), 2);
        assert!(reports[0].explicitness_score_0_10000 < 10000);
    }

    #[test]
    fn state_reports_flag_missing_transition_sites() {
        let config: RulesConfig = toml::from_str(
            r#"
                [[state_model]]
                id = "browser_state_sync"
                roots = ["src/runtime/browser-state-sync-controller.ts"]
                require_exhaustive_switch = true
            "#,
        )
        .expect("rules config");
        let semantic = SemanticSnapshot {
            project: ProjectModel::default(),
            analyzed_files: 1,
            capabilities: vec![
                SemanticCapability::ClosedDomains,
                SemanticCapability::ClosedDomainSites,
                SemanticCapability::TransitionSites,
            ],
            files: Vec::new(),
            symbols: Vec::new(),
            reads: Vec::new(),
            writes: Vec::new(),
            closed_domains: vec![ClosedDomain {
                path: "src/domain/browser-sync-state.ts".to_string(),
                symbol_name: "BrowserSyncState".to_string(),
                variants: vec!["idle".to_string(), "running".to_string()],
                line: 1,
                defining_file: Some("src/domain/browser-sync-state.ts".to_string()),
            }],
            closed_domain_sites: vec![ExhaustivenessSite {
                path: "src/runtime/browser-state-sync-controller.ts".to_string(),
                domain_symbol_name: "BrowserSyncState".to_string(),
                defining_file: Some("src/domain/browser-sync-state.ts".to_string()),
                site_kind: ExhaustivenessSiteKind::Switch,
                proof_kind: ExhaustivenessProofKind::Switch,
                covered_variants: vec!["idle".to_string(), "running".to_string()],
                line: 8,
            }],
            transition_sites: Vec::new(),
        };

        let reports = build_state_integrity_reports(
            &config,
            &semantic,
            &[],
            StateScope::All,
            &BTreeSet::new(),
        );
        let findings = build_state_integrity_findings(&reports);

        assert!(findings
            .iter()
            .any(|finding| finding.kind == "state_model_missing_transition_sites"));
    }

    #[test]
    fn state_reports_do_not_require_transition_sites_without_support() {
        let config: RulesConfig = toml::from_str(
            r#"
                [[state_model]]
                id = "browser_state_sync"
                roots = ["src/runtime/browser-state-sync-controller.ts"]
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
            files: Vec::new(),
            symbols: Vec::new(),
            reads: Vec::new(),
            writes: Vec::new(),
            closed_domains: vec![ClosedDomain {
                path: "src/domain/browser-sync-state.ts".to_string(),
                symbol_name: "BrowserSyncState".to_string(),
                variants: vec!["idle".to_string(), "running".to_string()],
                line: 1,
                defining_file: Some("src/domain/browser-sync-state.ts".to_string()),
            }],
            closed_domain_sites: vec![ExhaustivenessSite {
                path: "src/runtime/browser-state-sync-controller.ts".to_string(),
                domain_symbol_name: "BrowserSyncState".to_string(),
                defining_file: Some("src/domain/browser-sync-state.ts".to_string()),
                site_kind: ExhaustivenessSiteKind::Switch,
                proof_kind: ExhaustivenessProofKind::Switch,
                covered_variants: vec!["idle".to_string(), "running".to_string()],
                line: 8,
            }],
            transition_sites: Vec::new(),
        };

        let reports = build_state_integrity_reports(
            &config,
            &semantic,
            &[],
            StateScope::All,
            &BTreeSet::new(),
        );
        let findings = build_state_integrity_findings(&reports);

        assert_eq!(reports.len(), 1);
        assert!(!findings
            .iter()
            .any(|finding| finding.kind == "state_model_missing_transition_sites"));
    }
}
