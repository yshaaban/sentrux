//! Conservative state-integrity analysis built on closed-domain and obligation facts.

use super::{state_support, ObligationReport, ObligationSite, SemanticFinding};
use crate::analysis::semantic::TransitionSite;
use crate::metrics::rules::RulesConfig;
use std::collections::BTreeSet;

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
    semantic: &crate::analysis::semantic::SemanticSnapshot,
    obligations: &[ObligationReport],
    scope: StateScope,
    changed_files: &BTreeSet<String>,
) -> Vec<StateIntegrityReport> {
    let mut reports = Vec::new();

    for state_model in &config.state_model {
        let report =
            state_support::build_state_integrity_report(state_model, semantic, obligations);
        if scope == StateScope::Changed
            && !state_support::state_model_in_changed_scope(state_model, &report, changed_files)
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
    state_support::build_state_integrity_findings(reports)
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
            .any(|path| state_support::roots_match_path(&state_model.roots, path))
        {
            ids.insert(state_model.id.clone());
        }
    }
    ids.into_iter().collect()
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
                ..ExhaustivenessSite::default()
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
                ..ExhaustivenessSite::default()
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
                ..ExhaustivenessSite::default()
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
                ..ExhaustivenessSite::default()
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
                ..ExhaustivenessSite::default()
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
                ..ExhaustivenessSite::default()
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
