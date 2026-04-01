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
use self::obligations_contract::summarize_contract_missing_sites;
use self::obligations_contract::{build_contract_obligation, path_matches};
use self::obligations_domain::{
    build_domain_obligation, concept_rule_paths, domain_is_in_scope, relevant_domains,
    relevant_production_exhaustiveness_sites, zero_config_domain_is_actionable,
    DomainObligationPlan,
};

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
            && !changed_files.contains(&domain.path)
            && !relevant_production_exhaustiveness_sites(domain, semantic)
                .iter()
                .any(|site| changed_files.contains(&site.path))
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
        .map(|obligation| {
            let severity = if obligation.kind == "closed_domain_exhaustiveness"
                && !obligation.missing_variants.is_empty()
            {
                FindingSeverity::High
            } else {
                FindingSeverity::Medium
            };
            SemanticFinding {
                kind: obligation.kind.clone(),
                severity,
                concept_id: obligation_concept_id(obligation).to_owned(),
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

#[cfg(test)]
mod tests {
    use super::{
        build_obligations, obligation_score_0_10000, summarize_contract_missing_sites,
        ObligationScope,
    };
    use crate::analysis::semantic::{
        ClosedDomain, ExhaustivenessProofKind, ExhaustivenessSite, ExhaustivenessSiteKind,
        ProjectModel, ReadFact, SemanticCapability, SemanticFileFact, SemanticSnapshot, SymbolFact,
    };
    use crate::metrics::rules::RulesConfig;
    use crate::metrics::v2::ObligationSite;
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
                site_kind: ExhaustivenessSiteKind::Switch,
                proof_kind: ExhaustivenessProofKind::AssertNever,
                covered_variants: vec!["idle".to_string(), "busy".to_string()],
                line: 20,
            }],
            transition_sites: Vec::new(),
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
                site_kind: ExhaustivenessSiteKind::Switch,
                proof_kind: ExhaustivenessProofKind::AssertNever,
                covered_variants: vec!["idle".to_string()],
                line: 10,
            }],
            transition_sites: Vec::new(),
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
                site_kind: ExhaustivenessSiteKind::Switch,
                proof_kind: ExhaustivenessProofKind::AssertNever,
                covered_variants: vec!["idle".to_string()],
                line: 10,
            }],
            transition_sites: Vec::new(),
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
                site_kind: ExhaustivenessSiteKind::Switch,
                proof_kind: ExhaustivenessProofKind::Switch,
                covered_variants: vec!["Variant0".to_string()],
                line: 10,
            }],
            transition_sites: Vec::new(),
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
            transition_sites: Vec::new(),
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
            transition_sites: Vec::new(),
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
            transition_sites: Vec::new(),
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
            transition_sites: Vec::new(),
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
            transition_sites: Vec::new(),
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
            transition_sites: Vec::new(),
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
            transition_sites: Vec::new(),
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
            transition_sites: Vec::new(),
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
