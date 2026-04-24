use crate::analysis::concepts::infer_concepts;
use crate::analysis::project_shape::ProjectShapeReport;
use crate::analysis::semantic::SemanticSnapshot;
use crate::metrics::rules::{ConceptRule, ModuleContractRule, RulesConfig, StateModelRule};
use std::collections::BTreeSet;

const MIN_INFERRED_CONCEPT_CONFIDENCE: u32 = 7000;

#[derive(Debug, Clone, Default, serde::Serialize, PartialEq, Eq)]
pub struct InferredRulesSummary {
    pub concepts: usize,
    pub state_models: usize,
    pub module_contracts: usize,
}

pub fn merge_inferred_rules(
    config: &RulesConfig,
    shape: &ProjectShapeReport,
    semantic: Option<&SemanticSnapshot>,
) -> (RulesConfig, InferredRulesSummary) {
    let mut merged = config.clone();
    let mut summary = InferredRulesSummary::default();

    let inferred_module_contracts = infer_module_contract_rules(config, shape);
    summary.module_contracts = inferred_module_contracts.len();
    merged.module_contract.extend(inferred_module_contracts);

    if let Some(semantic) = semantic {
        let inferred_concepts = infer_concept_rules(config, semantic);
        summary.concepts = inferred_concepts.len();
        merged.concept.extend(inferred_concepts);

        let inferred_state_models = infer_state_model_rules(config, shape, semantic);
        summary.state_models = inferred_state_models.len();
        merged.state_model.extend(inferred_state_models);
    }

    (merged, summary)
}

fn infer_module_contract_rules(
    config: &RulesConfig,
    shape: &ProjectShapeReport,
) -> Vec<ModuleContractRule> {
    let existing_module_contracts = config
        .module_contract
        .iter()
        .map(|rule| (rule.id.clone(), rule.root.clone()))
        .collect::<BTreeSet<_>>();

    shape
        .module_contracts
        .iter()
        .filter(|suggestion| suggestion.confidence == "high")
        .filter(|suggestion| {
            !existing_module_contracts.contains(&(suggestion.id.clone(), suggestion.root.clone()))
        })
        .map(|suggestion| ModuleContractRule {
            id: suggestion.id.clone(),
            root: suggestion.root.clone(),
            public_api: suggestion.public_api.clone(),
            forbid_cross_module_deep_imports: true,
        })
        .collect()
}

fn infer_concept_rules(config: &RulesConfig, semantic: &SemanticSnapshot) -> Vec<ConceptRule> {
    let existing_ids = config
        .concept
        .iter()
        .map(|concept| concept.id.clone())
        .collect::<BTreeSet<_>>();

    infer_concepts(config, semantic)
        .into_iter()
        .filter(|suggestion| suggestion.confidence_0_10000 >= MIN_INFERRED_CONCEPT_CONFIDENCE)
        .filter(|suggestion| !existing_ids.contains(&suggestion.id))
        .map(|suggestion| {
            let (kind, authoritative_inputs, canonical_accessors) =
                inferred_concept_surfaces(&suggestion.kind, &suggestion.anchors);

            ConceptRule {
                id: suggestion.id,
                kind,
                priority: Some("inferred".to_string()),
                anchors: suggestion.anchors,
                authoritative_inputs,
                canonical_accessors,
                ..ConceptRule::default()
            }
        })
        .collect()
}

fn inferred_concept_surfaces(kind: &str, anchors: &[String]) -> (String, Vec<String>, Vec<String>) {
    match kind {
        "store_like_symbol" => ("projection".to_string(), anchors.to_vec(), anchors.to_vec()),
        _ => ("closed_domain".to_string(), Vec::new(), anchors.to_vec()),
    }
}

fn infer_state_model_rules(
    config: &RulesConfig,
    shape: &ProjectShapeReport,
    semantic: &SemanticSnapshot,
) -> Vec<StateModelRule> {
    let existing_state_model_ids = config
        .state_model
        .iter()
        .map(|state_model| state_model.id.clone())
        .collect::<BTreeSet<_>>();
    let existing_state_model_roots = config
        .state_model
        .iter()
        .flat_map(|state_model| state_model.roots.iter().cloned())
        .collect::<BTreeSet<_>>();

    shape
        .boundary_roots
        .iter()
        .filter(|boundary| boundary.kind == "client_state")
        .filter(|boundary| !existing_state_model_roots.contains(&boundary.root))
        .filter(|boundary| boundary_root_has_state_semantics(&boundary.root, semantic))
        .map(|boundary| StateModelRule {
            id: inferred_state_model_id(&boundary.root),
            roots: vec![boundary.root.clone()],
            kind: boundary.kind.clone(),
            require_exhaustive_switch: true,
            require_assert_never: semantic
                .transition_sites
                .iter()
                .any(|site| site.path.starts_with(&boundary.root)),
        })
        .filter(|state_model| !existing_state_model_ids.contains(&state_model.id))
        .collect()
}

fn boundary_root_has_state_semantics(root: &str, semantic: &SemanticSnapshot) -> bool {
    semantic
        .closed_domains
        .iter()
        .any(|domain| domain.path.starts_with(root))
        || semantic
            .closed_domain_sites
            .iter()
            .any(|site| site.path.starts_with(root))
        || semantic
            .transition_sites
            .iter()
            .any(|site| site.path.starts_with(root))
}

fn inferred_state_model_id(root: &str) -> String {
    let mut id = String::from("inferred_state_model");
    for character in root.chars() {
        if character.is_ascii_alphanumeric() {
            id.push('_');
            id.push(character.to_ascii_lowercase());
        } else if !id.ends_with('_') {
            id.push('_');
        }
    }
    id.trim_end_matches('_').to_string()
}

#[cfg(test)]
mod tests {
    use super::{merge_inferred_rules, InferredRulesSummary};
    use crate::analysis::project_shape::{
        BoundaryRootSuggestion, ModuleContractSuggestion, ProjectShapeReport,
    };
    use crate::analysis::semantic::{
        ClosedDomain, ExhaustivenessProofKind, ExhaustivenessSite, ExhaustivenessSiteKind,
        ProjectModel, SemanticCapability, SemanticSnapshot, SymbolFact,
    };
    use crate::metrics::rules::RulesConfig;

    #[test]
    fn merges_high_confidence_inferred_rules_into_empty_config() {
        let config: RulesConfig = toml::from_str("").expect("rules config");
        let shape = ProjectShapeReport {
            boundary_roots: vec![BoundaryRootSuggestion {
                kind: "client_state".to_string(),
                root: "src/store".to_string(),
                evidence: vec!["client state layer".to_string()],
            }],
            module_contracts: vec![ModuleContractSuggestion {
                id: "feature_modules".to_string(),
                root: "src/modules".to_string(),
                public_api: vec!["index.ts".to_string()],
                nested_public_api: vec!["components/index.ts".to_string()],
                confidence: "high".to_string(),
                evidence: vec!["feature module barrels detected".to_string()],
            }],
            ..ProjectShapeReport::default()
        };
        let semantic = SemanticSnapshot {
            project: ProjectModel::default(),
            capabilities: vec![
                SemanticCapability::ClosedDomains,
                SemanticCapability::ClosedDomainSites,
                SemanticCapability::TransitionSites,
            ],
            symbols: vec![SymbolFact {
                path: "src/store/state.ts".to_string(),
                name: "TaskState".to_string(),
                kind: "type_alias".to_string(),
                line: 1,
                id: "symbol:task-state".to_string(),
            }],
            closed_domains: vec![ClosedDomain {
                path: "src/store/state.ts".to_string(),
                symbol_name: "TaskState".to_string(),
                variants: vec!["idle".to_string(), "busy".to_string()],
                line: 1,
                defining_file: Some("src/store/state.ts".to_string()),
            }],
            closed_domain_sites: vec![ExhaustivenessSite {
                path: "src/store/reducer.ts".to_string(),
                domain_symbol_name: "TaskState".to_string(),
                defining_file: Some("src/store/state.ts".to_string()),
                site_kind: ExhaustivenessSiteKind::Switch,
                proof_kind: ExhaustivenessProofKind::Switch,
                covered_variants: vec!["idle".to_string()],
                line: 8,
                ..ExhaustivenessSite::default()
            }],
            transition_sites: vec![crate::analysis::semantic::TransitionSite {
                path: "src/store/reducer.ts".to_string(),
                domain_symbol_name: "TaskState".to_string(),
                group_id: "task-state".to_string(),
                transition_kind: crate::analysis::semantic::TransitionKind::SwitchCase,
                source_variant: Some("idle".to_string()),
                target_variants: vec!["busy".to_string()],
                line: 12,
            }],
            ..SemanticSnapshot::default()
        };

        let (merged, summary) = merge_inferred_rules(&config, &shape, Some(&semantic));

        assert_eq!(
            summary,
            InferredRulesSummary {
                concepts: 1,
                state_models: 1,
                module_contracts: 1,
            }
        );
        assert_eq!(merged.concept.len(), 1);
        assert_eq!(merged.state_model.len(), 1);
        assert_eq!(merged.module_contract.len(), 1);
        assert_eq!(merged.concept[0].canonical_accessors.len(), 1);
        assert_eq!(merged.state_model[0].roots, vec!["src/store".to_string()]);
    }

    #[test]
    fn preserves_explicit_rules_without_duplication() {
        let config: RulesConfig = toml::from_str(
            r#"
                [[concept]]
                id = "task_state"
                anchors = ["src/store/state.ts::TaskState"]
                canonical_accessors = ["src/store/state.ts::TaskState"]

                [[state_model]]
                id = "inferred_state_model_src_store"
                roots = ["src/store"]
                require_exhaustive_switch = true

                [[module_contract]]
                id = "feature_modules"
                root = "src/modules"
                public_api = ["index.ts"]
                forbid_cross_module_deep_imports = true
            "#,
        )
        .expect("rules config");
        let shape = ProjectShapeReport {
            boundary_roots: vec![BoundaryRootSuggestion {
                kind: "client_state".to_string(),
                root: "src/store".to_string(),
                evidence: vec![],
            }],
            module_contracts: vec![ModuleContractSuggestion {
                id: "feature_modules".to_string(),
                root: "src/modules".to_string(),
                public_api: vec!["index.ts".to_string()],
                nested_public_api: Vec::new(),
                confidence: "high".to_string(),
                evidence: vec![],
            }],
            ..ProjectShapeReport::default()
        };
        let semantic = SemanticSnapshot {
            closed_domains: vec![ClosedDomain {
                path: "src/store/state.ts".to_string(),
                symbol_name: "TaskState".to_string(),
                variants: vec!["idle".to_string()],
                line: 1,
                defining_file: Some("src/store/state.ts".to_string()),
            }],
            closed_domain_sites: vec![ExhaustivenessSite {
                path: "src/store/reducer.ts".to_string(),
                domain_symbol_name: "TaskState".to_string(),
                defining_file: Some("src/store/state.ts".to_string()),
                site_kind: ExhaustivenessSiteKind::Switch,
                proof_kind: ExhaustivenessProofKind::Switch,
                covered_variants: vec![],
                line: 4,
                ..ExhaustivenessSite::default()
            }],
            ..SemanticSnapshot::default()
        };

        let (merged, summary) = merge_inferred_rules(&config, &shape, Some(&semantic));

        assert_eq!(summary.concepts, 0);
        assert_eq!(summary.state_models, 0);
        assert_eq!(summary.module_contracts, 0);
        assert_eq!(merged.concept.len(), 1);
        assert_eq!(merged.state_model.len(), 1);
        assert_eq!(merged.module_contract.len(), 1);
    }
}
