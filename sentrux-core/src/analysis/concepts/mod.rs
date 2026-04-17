//! Explicit concept graph extraction from v2 rules.

use crate::analysis::guardrail_tests::walk_guardrail_test_sources;
use crate::analysis::semantic::SemanticSnapshot;
use crate::metrics::rules::{ConceptRule, ContractRule, RulesConfig, StateModelRule};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::Path;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct ConceptGraph {
    pub concepts: Vec<ConceptNode>,
    pub contracts: Vec<ContractNode>,
    pub state_models: Vec<StateModelNode>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct ConceptNode {
    pub id: String,
    pub kind: String,
    pub priority: Option<String>,
    pub anchors: Vec<String>,
    pub authoritative_inputs: Vec<String>,
    pub allowed_writers: Vec<String>,
    pub forbid_writers: Vec<String>,
    pub canonical_accessors: Vec<String>,
    pub forbid_raw_reads: Vec<String>,
    pub related_tests: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct ContractNode {
    pub id: String,
    pub kind: String,
    pub priority: Option<String>,
    pub categories_symbol: Option<String>,
    pub payload_map_symbol: Option<String>,
    pub registry_symbol: Option<String>,
    pub browser_entry: Option<String>,
    pub electron_entry: Option<String>,
    pub required_capabilities: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct StateModelNode {
    pub id: String,
    pub kind: String,
    pub roots: Vec<String>,
    pub require_exhaustive_switch: bool,
    pub require_assert_never: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct GuardrailTestEvidence {
    pub path: String,
    pub matched_concepts: Vec<String>,
    pub matched_symbols: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct InferredConceptSuggestion {
    pub id: String,
    pub kind: String,
    pub anchors: Vec<String>,
    pub evidence: Vec<String>,
    pub confidence_0_10000: u32,
}

pub fn extract_concept_graph(config: &RulesConfig) -> ConceptGraph {
    ConceptGraph {
        concepts: config.concept.iter().map(concept_node_from_rule).collect(),
        contracts: config
            .contract
            .iter()
            .map(contract_node_from_rule)
            .collect(),
        state_models: config
            .state_model
            .iter()
            .map(state_model_node_from_rule)
            .collect(),
    }
}

pub fn detect_guardrail_tests(root: &Path, config: &RulesConfig) -> Vec<GuardrailTestEvidence> {
    let concept_symbols = config
        .concept
        .iter()
        .map(|concept| {
            let symbols = concept_guardrail_symbols(config, concept);
            (concept.id.as_str(), symbols, concept.related_tests.clone())
        })
        .collect::<Vec<_>>();
    let mut evidence = walk_guardrail_test_sources(root)
        .into_iter()
        .map(|(relative_path, contents)| {
            let mut matched_concepts = BTreeSet::new();
            let mut matched_symbols = BTreeSet::new();
            for (concept_id, symbols, related_tests) in &concept_symbols {
                for pattern in related_tests {
                    if crate::metrics::rules::glob_match(pattern, &relative_path) {
                        matched_concepts.insert((*concept_id).to_string());
                        matched_symbols.insert(format!("related_test:{pattern}"));
                    }
                }
                for symbol in symbols {
                    if contents.contains(symbol) {
                        matched_concepts.insert((*concept_id).to_string());
                        matched_symbols.insert(symbol.clone());
                    }
                }
            }

            Some(GuardrailTestEvidence {
                path: relative_path,
                matched_concepts: matched_concepts.into_iter().collect(),
                matched_symbols: matched_symbols.into_iter().collect(),
            })
        })
        .flatten()
        .collect::<Vec<_>>();
    evidence.sort_by(|left, right| left.path.cmp(&right.path));
    evidence
}

pub fn infer_concepts(
    config: &RulesConfig,
    semantic: &SemanticSnapshot,
) -> Vec<InferredConceptSuggestion> {
    let configured_symbols = config
        .concept
        .iter()
        .flat_map(concept_suppression_symbols)
        .collect::<HashSet<_>>();
    let mut suggestions = Vec::new();
    let mut seen_ids = HashSet::new();

    push_closed_domain_suggestions(
        &mut suggestions,
        &mut seen_ids,
        &configured_symbols,
        semantic,
    );
    push_store_like_symbol_suggestions(
        &mut suggestions,
        &mut seen_ids,
        &configured_symbols,
        semantic,
    );
    suggestions.sort_by(|left, right| {
        right
            .confidence_0_10000
            .cmp(&left.confidence_0_10000)
            .then_with(|| left.id.cmp(&right.id))
    });
    suggestions
}

fn push_closed_domain_suggestions(
    suggestions: &mut Vec<InferredConceptSuggestion>,
    seen_ids: &mut HashSet<String>,
    configured_symbols: &HashSet<String>,
    semantic: &SemanticSnapshot,
) {
    for domain in &semantic.closed_domains {
        if configured_symbols.contains(&domain.symbol_name) {
            continue;
        }

        let site_count = closed_domain_site_count(semantic, &domain.symbol_name);
        if site_count == 0 {
            continue;
        }

        let id = infer_concept_id(&domain.symbol_name);
        if !seen_ids.insert(id.clone()) {
            continue;
        }

        suggestions.push(InferredConceptSuggestion {
            id,
            kind: "closed_domain".to_string(),
            anchors: vec![format!("{}::{}", domain.path, domain.symbol_name)],
            evidence: vec![format!(
                "closed domain '{}' has {} exhaustiveness site(s)",
                domain.symbol_name, site_count
            )],
            confidence_0_10000: (6500 + (site_count.min(4) as u32 * 500)).min(9000),
        });
    }
}

fn closed_domain_site_count(semantic: &SemanticSnapshot, domain_symbol_name: &str) -> usize {
    semantic
        .closed_domain_sites
        .iter()
        .filter(|site| site.domain_symbol_name == domain_symbol_name)
        .count()
}

fn push_store_like_symbol_suggestions(
    suggestions: &mut Vec<InferredConceptSuggestion>,
    seen_ids: &mut HashSet<String>,
    configured_symbols: &HashSet<String>,
    semantic: &SemanticSnapshot,
) {
    for (symbol_name, files) in sorted_accessed_store_symbols(semantic) {
        if configured_symbols.contains(&symbol_name) || files.len() < 2 {
            continue;
        }

        let id = infer_concept_id(&symbol_name);
        if !seen_ids.insert(id.clone()) {
            continue;
        }

        suggestions.push(InferredConceptSuggestion {
            id,
            kind: "store_like_symbol".to_string(),
            anchors: store_like_suggestion_anchors(semantic, &symbol_name, &files),
            evidence: vec![format!(
                "symbol '{}' is touched from {} file(s)",
                symbol_name,
                files.len()
            )],
            confidence_0_10000: (5500 + (files.len().min(4) as u32 * 500)).min(8000),
        });
    }
}

fn sorted_accessed_store_symbols(semantic: &SemanticSnapshot) -> Vec<(String, BTreeSet<String>)> {
    let mut accessed_symbols: HashMap<String, BTreeSet<String>> = HashMap::new();
    record_store_like_symbol_accesses(&mut accessed_symbols, &semantic.reads);
    record_store_like_symbol_accesses(&mut accessed_symbols, &semantic.writes);

    let mut accessed_entries = accessed_symbols.into_iter().collect::<Vec<_>>();
    accessed_entries.sort_by(|(left_symbol, left_files), (right_symbol, right_files)| {
        right_files
            .len()
            .cmp(&left_files.len())
            .then_with(|| left_symbol.cmp(right_symbol))
    });
    accessed_entries
}

fn record_store_like_symbol_accesses<T>(
    accessed_symbols: &mut HashMap<String, BTreeSet<String>>,
    accesses: &[T],
) where
    T: StoreLikeSymbolAccess,
{
    for access in accesses {
        if !is_store_like_symbol(access.symbol_name()) {
            continue;
        }

        accessed_symbols
            .entry(access.symbol_name().to_string())
            .or_default()
            .insert(access.path().to_string());
    }
}

trait StoreLikeSymbolAccess {
    fn path(&self) -> &str;
    fn symbol_name(&self) -> &str;
}

impl StoreLikeSymbolAccess for crate::analysis::semantic::ReadFact {
    fn path(&self) -> &str {
        &self.path
    }

    fn symbol_name(&self) -> &str {
        &self.symbol_name
    }
}

impl StoreLikeSymbolAccess for crate::analysis::semantic::WriteFact {
    fn path(&self) -> &str {
        &self.path
    }

    fn symbol_name(&self) -> &str {
        &self.symbol_name
    }
}

fn store_like_suggestion_anchors(
    semantic: &SemanticSnapshot,
    symbol_name: &str,
    files: &BTreeSet<String>,
) -> Vec<String> {
    let anchors = semantic
        .symbols
        .iter()
        .filter(|symbol| symbol.name == symbol_name)
        .map(|symbol| format!("{}::{}", symbol.path, symbol.name))
        .collect::<Vec<_>>();
    let mut anchors = if anchors.is_empty() {
        files
            .iter()
            .next()
            .map(|path| vec![format!("{path}::{symbol_name}")])
            .unwrap_or_default()
    } else {
        anchors
    };
    anchors.sort();
    anchors.dedup();
    anchors
}

fn concept_node_from_rule(rule: &ConceptRule) -> ConceptNode {
    ConceptNode {
        id: rule.id.clone(),
        kind: rule.kind.clone(),
        priority: rule.priority.clone(),
        anchors: rule.anchors.clone(),
        authoritative_inputs: rule.authoritative_inputs.clone(),
        allowed_writers: rule.allowed_writers.clone(),
        forbid_writers: rule.forbid_writers.clone(),
        canonical_accessors: rule.canonical_accessors.clone(),
        forbid_raw_reads: rule.forbid_raw_reads.clone(),
        related_tests: rule.related_tests.clone(),
    }
}

fn contract_node_from_rule(rule: &ContractRule) -> ContractNode {
    ContractNode {
        id: rule.id.clone(),
        kind: rule.kind.clone(),
        priority: rule.priority.clone(),
        categories_symbol: rule.categories_symbol.clone(),
        payload_map_symbol: rule.payload_map_symbol.clone(),
        registry_symbol: rule.registry_symbol.clone(),
        browser_entry: rule.browser_entry.clone(),
        electron_entry: rule.electron_entry.clone(),
        required_capabilities: rule.required_capabilities.clone(),
    }
}

fn state_model_node_from_rule(rule: &StateModelRule) -> StateModelNode {
    StateModelNode {
        id: rule.id.clone(),
        kind: rule.kind.clone(),
        roots: rule.roots.clone(),
        require_exhaustive_switch: rule.require_exhaustive_switch,
        require_assert_never: rule.require_assert_never,
    }
}

fn concept_suppression_symbols(concept: &ConceptRule) -> HashSet<String> {
    scoped_symbol_targets(concept_scoped_fields(concept))
}

fn concept_guardrail_symbols(config: &RulesConfig, concept: &ConceptRule) -> HashSet<String> {
    let mut symbols = concept_suppression_symbols(concept);
    for contract in &config.contract {
        if !contract_relates_to_concept(contract, concept) {
            continue;
        }
        symbols.extend(contract_targets(contract));
    }
    symbols
}

fn infer_concept_id(symbol_name: &str) -> String {
    let mut id = String::new();
    let mut previous_was_separator = false;
    let mut previous_was_lowercase = false;

    for character in symbol_name.chars() {
        if character.is_ascii_alphanumeric() {
            if character.is_ascii_uppercase() && previous_was_lowercase && !id.is_empty() {
                id.push('_');
            }
            id.push(character.to_ascii_lowercase());
            previous_was_separator = false;
            previous_was_lowercase = character.is_ascii_lowercase();
        } else if !previous_was_separator && !id.is_empty() {
            id.push('_');
            previous_was_separator = true;
            previous_was_lowercase = false;
        }
    }

    id.trim_matches('_').to_string()
}

fn is_store_like_symbol(symbol_name: &str) -> bool {
    symbol_name.starts_with("store.")
}

fn contract_targets(contract: &ContractRule) -> HashSet<String> {
    scoped_symbol_targets(contract_scoped_fields(contract))
}

fn contract_paths(contract: &ContractRule) -> HashSet<String> {
    let mut paths = scoped_path_targets(contract_scoped_fields(contract));
    if let Some(path) = &contract.browser_entry {
        paths.insert(path.clone());
    }
    if let Some(path) = &contract.electron_entry {
        paths.insert(path.clone());
    }
    paths
}

fn concept_paths(concept: &ConceptRule) -> HashSet<String> {
    scoped_path_targets(concept_scoped_fields(concept))
}

fn contract_relates_to_concept(contract: &ContractRule, concept: &ConceptRule) -> bool {
    let concept_symbols = concept_suppression_symbols(concept);
    let contract_symbols = contract_targets(contract);
    if !concept_symbols.is_disjoint(&contract_symbols) {
        return true;
    }

    let concept_paths = concept_paths(concept);
    let contract_paths = contract_paths(contract);
    !concept_paths.is_disjoint(&contract_paths)
}

fn concept_scoped_fields(concept: &ConceptRule) -> Vec<&str> {
    concept
        .anchors
        .iter()
        .chain(concept.authoritative_inputs.iter())
        .chain(concept.allowed_writers.iter())
        .chain(concept.forbid_writers.iter())
        .chain(concept.canonical_accessors.iter())
        .chain(concept.forbid_raw_reads.iter())
        .map(String::as_str)
        .collect()
}

fn contract_scoped_fields(contract: &ContractRule) -> Vec<&str> {
    [
        contract.categories_symbol.as_deref(),
        contract.payload_map_symbol.as_deref(),
        contract.registry_symbol.as_deref(),
    ]
    .into_iter()
    .flatten()
    .collect()
}

fn scoped_symbol_targets<'a>(values: impl IntoIterator<Item = &'a str>) -> HashSet<String> {
    values
        .into_iter()
        .filter_map(|value| value.split_once("::").map(|(_, symbol)| symbol.to_string()))
        .collect()
}

fn scoped_path_targets<'a>(values: impl IntoIterator<Item = &'a str>) -> HashSet<String> {
    values
        .into_iter()
        .filter_map(|value| value.split_once("::").map(|(path, _)| path.to_string()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{detect_guardrail_tests, extract_concept_graph, infer_concepts};
    use crate::analysis::semantic::{
        ClosedDomain, ExhaustivenessProofKind, ExhaustivenessSite, ExhaustivenessSiteKind,
        ProjectModel, ReadFact, SemanticSnapshot, SymbolFact, WriteFact,
    };
    use crate::metrics::rules::RulesConfig;
    use crate::test_support::temp_root;

    #[test]
    fn extracts_v2_concepts_from_rules() {
        let config: RulesConfig = toml::from_str(
            r#"
                [[concept]]
                id = "task_git_status"
                kind = "authoritative_state"
                anchors = ["src/store/core.ts::store.taskGitStatus"]
                allowed_writers = ["src/app/git-status-sync.ts::*"]

                [[contract]]
                id = "server_state_bootstrap"
                registry_symbol = "src/app/server-state-bootstrap-registry.ts::SERVER_STATE_BOOTSTRAP_REGISTRY"
                categories_symbol = "src/domain/server-state-bootstrap.ts::SERVER_STATE_BOOTSTRAP_CATEGORIES"
                browser_entry = "src/runtime/browser-session.ts"

                [[state_model]]
                id = "browser_state_sync"
                roots = ["src/runtime/browser-state-sync-controller.ts"]
                require_exhaustive_switch = true
            "#,
        )
        .expect("rules config");

        let graph = extract_concept_graph(&config);

        assert_eq!(graph.concepts.len(), 1);
        assert_eq!(graph.contracts.len(), 1);
        assert_eq!(graph.state_models.len(), 1);
        assert_eq!(graph.concepts[0].id, "task_git_status");
    }

    #[test]
    fn detects_guardrail_tests_with_matching_concepts() {
        let root = temp_root("sentrux-concepts", "guardrails", &[]);
        let tests_dir = root.join("src/components");
        std::fs::create_dir_all(&tests_dir).expect("create test dir");
        std::fs::write(
            tests_dir.join("Sidebar.architecture.test.ts"),
            "expect(getTaskStatus()).toBeDefined();\nexpect(TaskStateRegistry).toBeDefined();\n",
        )
        .expect("write guardrail");
        let config: RulesConfig = toml::from_str(
            r#"
                [[concept]]
                id = "task_git_status"
                anchors = ["src/store/core.ts::store.taskGitStatus"]
                canonical_accessors = ["src/app/task-presentation.ts::getTaskStatus"]
                related_tests = ["src/components/Sidebar.architecture.test.ts"]

                [[contract]]
                id = "task_bootstrap"
                registry_symbol = "src/app/task-presentation.ts::TaskStateRegistry"
            "#,
        )
        .expect("rules config");

        let evidence = detect_guardrail_tests(&root, &config);

        assert_eq!(evidence.len(), 1);
        assert_eq!(evidence[0].matched_concepts, vec!["task_git_status"]);
        assert!(evidence[0]
            .matched_symbols
            .iter()
            .any(|symbol| symbol == "getTaskStatus"));
        assert!(evidence[0]
            .matched_symbols
            .iter()
            .any(|symbol| symbol == "TaskStateRegistry"));
        assert!(evidence[0].matched_symbols.iter().any(|symbol| {
            symbol == "related_test:src/components/Sidebar.architecture.test.ts"
        }));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn infers_closed_domain_and_store_like_concepts_conservatively() {
        let config: RulesConfig = toml::from_str(
            r#"
                [[concept]]
                id = "configured_concept"
                anchors = ["src/store/core.ts::store.configuredState"]
            "#,
        )
        .expect("rules config");
        let semantic = SemanticSnapshot {
            project: ProjectModel::default(),
            analyzed_files: 2,
            capabilities: Vec::new(),
            files: Vec::new(),
            symbols: vec![
                SymbolFact {
                    id: "task-state".to_string(),
                    path: "src/domain/task-state.ts".to_string(),
                    name: "TaskState".to_string(),
                    kind: "type_alias".to_string(),
                    line: 1,
                },
                SymbolFact {
                    id: "git-status".to_string(),
                    path: "src/store/core.ts".to_string(),
                    name: "store.taskGitStatus".to_string(),
                    kind: "property".to_string(),
                    line: 1,
                },
            ],
            reads: vec![
                ReadFact {
                    path: "src/components/TaskRow.tsx".to_string(),
                    symbol_name: "store.taskGitStatus".to_string(),
                    read_kind: "property_access".to_string(),
                    line: 8,
                },
                ReadFact {
                    path: "src/components/Theme.tsx".to_string(),
                    symbol_name: "ThemeState".to_string(),
                    read_kind: "identifier".to_string(),
                    line: 10,
                },
            ],
            writes: vec![
                WriteFact {
                    path: "src/app/task-sync.ts".to_string(),
                    symbol_name: "store.taskGitStatus".to_string(),
                    write_kind: "store_call".to_string(),
                    line: 4,
                },
                WriteFact {
                    path: "src/app/theme-sync.ts".to_string(),
                    symbol_name: "ThemeState".to_string(),
                    write_kind: "assignment".to_string(),
                    line: 7,
                },
            ],
            closed_domains: vec![ClosedDomain {
                path: "src/domain/task-state.ts".to_string(),
                symbol_name: "TaskState".to_string(),
                variants: vec!["idle".to_string(), "running".to_string()],
                line: 1,
                defining_file: Some("src/domain/task-state.ts".to_string()),
            }],
            closed_domain_sites: vec![ExhaustivenessSite {
                path: "src/app/task-view.ts".to_string(),
                domain_symbol_name: "TaskState".to_string(),
                defining_file: Some("src/domain/task-state.ts".to_string()),
                site_kind: ExhaustivenessSiteKind::Switch,
                proof_kind: ExhaustivenessProofKind::Switch,
                covered_variants: vec!["idle".to_string()],
                line: 10,
            }],
            transition_sites: Vec::new(),
        };

        let suggestions = infer_concepts(&config, &semantic);

        assert!(suggestions
            .iter()
            .any(|suggestion| suggestion.id == "task_state"));
        assert!(suggestions
            .iter()
            .any(|suggestion| suggestion.id == "store_task_git_status"));
        assert!(!suggestions
            .iter()
            .any(|suggestion| suggestion.id == "theme_state"));
    }

    #[test]
    fn inferred_concept_fallback_anchor_is_deterministic() {
        let config: RulesConfig = toml::from_str("").expect("empty rules config");
        let semantic = SemanticSnapshot {
            project: ProjectModel::default(),
            analyzed_files: 2,
            capabilities: Vec::new(),
            files: Vec::new(),
            symbols: Vec::new(),
            reads: vec![
                ReadFact {
                    path: "src/z.ts".to_string(),
                    symbol_name: "store.taskLease".to_string(),
                    read_kind: "property_access".to_string(),
                    line: 1,
                },
                ReadFact {
                    path: "src/a.ts".to_string(),
                    symbol_name: "store.taskLease".to_string(),
                    read_kind: "property_access".to_string(),
                    line: 2,
                },
            ],
            writes: Vec::new(),
            closed_domains: Vec::new(),
            closed_domain_sites: Vec::new(),
            transition_sites: Vec::new(),
        };

        let suggestions = infer_concepts(&config, &semantic);
        let task_lease = suggestions
            .iter()
            .find(|suggestion| suggestion.id == "store_task_lease")
            .expect("store task lease suggestion");

        assert_eq!(task_lease.anchors, vec!["src/a.ts::store.taskLease"]);
    }

    #[test]
    fn inferred_store_like_concept_prefers_broadest_symbol_for_shared_id() {
        let config: RulesConfig = toml::from_str("").expect("empty rules config");
        let semantic = SemanticSnapshot {
            project: ProjectModel::default(),
            analyzed_files: 4,
            capabilities: Vec::new(),
            files: Vec::new(),
            symbols: Vec::new(),
            reads: vec![
                ReadFact {
                    path: "src/a.ts".to_string(),
                    symbol_name: "store.focusedPanel".to_string(),
                    read_kind: "property_access".to_string(),
                    line: 1,
                },
                ReadFact {
                    path: "src/b.ts".to_string(),
                    symbol_name: "store.focusedPanel".to_string(),
                    read_kind: "property_access".to_string(),
                    line: 2,
                },
                ReadFact {
                    path: "src/c.ts".to_string(),
                    symbol_name: "store.focusedPanel".to_string(),
                    read_kind: "property_access".to_string(),
                    line: 3,
                },
                ReadFact {
                    path: "src/d.ts".to_string(),
                    symbol_name: "store.focusedPanel.*".to_string(),
                    read_kind: "property_access".to_string(),
                    line: 4,
                },
                ReadFact {
                    path: "src/e.ts".to_string(),
                    symbol_name: "store.focusedPanel.*".to_string(),
                    read_kind: "property_access".to_string(),
                    line: 5,
                },
            ],
            writes: Vec::new(),
            closed_domains: Vec::new(),
            closed_domain_sites: Vec::new(),
            transition_sites: Vec::new(),
        };

        let suggestions = infer_concepts(&config, &semantic);
        let focused_panel = suggestions
            .iter()
            .find(|suggestion| suggestion.id == "store_focused_panel")
            .expect("store focused panel suggestion");

        assert_eq!(focused_panel.anchors, vec!["src/a.ts::store.focusedPanel"]);
        assert_eq!(
            focused_panel.evidence,
            vec!["symbol 'store.focusedPanel' is touched from 3 file(s)"]
        );
    }
}
