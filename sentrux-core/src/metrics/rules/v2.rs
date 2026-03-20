//! V2 rules model extensions for explicit concepts, contracts, and suppressions.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct ProjectRuleConfig {
    #[serde(default)]
    pub primary_language: Option<String>,
    #[serde(default)]
    pub exclude: Vec<String>,
    #[serde(default)]
    pub archetypes: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct ConceptRule {
    pub id: String,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub priority: Option<String>,
    #[serde(default)]
    pub anchors: Vec<String>,
    #[serde(default)]
    pub authoritative_inputs: Vec<String>,
    #[serde(default)]
    pub allowed_writers: Vec<String>,
    #[serde(default)]
    pub forbid_writers: Vec<String>,
    #[serde(default)]
    pub canonical_accessors: Vec<String>,
    #[serde(default)]
    pub forbid_raw_reads: Vec<String>,
    #[serde(default)]
    pub related_tests: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct ContractRule {
    pub id: String,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub priority: Option<String>,
    #[serde(default)]
    pub categories_symbol: Option<String>,
    #[serde(default)]
    pub payload_map_symbol: Option<String>,
    #[serde(default)]
    pub registry_symbol: Option<String>,
    #[serde(default)]
    pub browser_entry: Option<String>,
    #[serde(default)]
    pub electron_entry: Option<String>,
    #[serde(default)]
    pub required_capabilities: Vec<String>,
    #[serde(default)]
    pub trigger_symbols: Vec<String>,
    #[serde(default)]
    pub trigger_files: Vec<String>,
    #[serde(default)]
    pub required_symbols: Vec<String>,
    #[serde(default)]
    pub required_files: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct StateModelRule {
    pub id: String,
    #[serde(default)]
    pub roots: Vec<String>,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub require_exhaustive_switch: bool,
    #[serde(default)]
    pub require_assert_never: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct ModuleContractRule {
    pub id: String,
    pub root: String,
    #[serde(default = "default_module_contract_public_api")]
    pub public_api: Vec<String>,
    #[serde(default = "default_true")]
    pub forbid_cross_module_deep_imports: bool,
}

fn default_module_contract_public_api() -> Vec<String> {
    vec!["index.ts".to_string(), "index.tsx".to_string()]
}

const fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct SuppressionRule {
    pub kind: String,
    #[serde(default)]
    pub concept: Option<String>,
    #[serde(default)]
    pub file: Option<String>,
    #[serde(default)]
    pub reason: String,
    #[serde(default)]
    pub expires: Option<String>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct RuleCoverage {
    pub concepts_declared: usize,
    pub concepts_machine_checkable: usize,
    pub contracts_declared: usize,
    pub contracts_machine_checkable: usize,
    pub state_models_declared: usize,
    pub state_models_machine_checkable: usize,
    pub module_contracts_declared: usize,
    pub module_contracts_machine_checkable: usize,
    pub suppressions_declared: usize,
    pub suppressions_expiring: usize,
    pub coverage_0_10000: u32,
}

pub fn compute_rule_coverage(
    concepts: &[ConceptRule],
    contracts: &[ContractRule],
    state_models: &[StateModelRule],
    module_contracts: &[ModuleContractRule],
    suppressions: &[SuppressionRule],
) -> RuleCoverage {
    let concepts_machine_checkable = concepts
        .iter()
        .filter(|concept| {
            !concept.anchors.is_empty()
                && (!concept.allowed_writers.is_empty()
                    || !concept.forbid_writers.is_empty()
                    || !concept.canonical_accessors.is_empty()
                    || !concept.forbid_raw_reads.is_empty()
                    || !concept.authoritative_inputs.is_empty())
        })
        .count();
    let contracts_machine_checkable = contracts
        .iter()
        .filter(|contract| {
            let core_declared =
                contract.registry_symbol.is_some() && contract.categories_symbol.is_some();
            let additional_surfaces = contract.browser_entry.is_some()
                || contract.electron_entry.is_some()
                || contract.payload_map_symbol.is_some()
                || !contract.required_symbols.is_empty()
                || !contract.required_files.is_empty();

            core_declared && additional_surfaces
        })
        .count();
    let state_models_machine_checkable = state_models
        .iter()
        .filter(|state_model| {
            !state_model.roots.is_empty()
                && (state_model.require_assert_never || state_model.require_exhaustive_switch)
        })
        .count();
    let module_contracts_machine_checkable = module_contracts
        .iter()
        .filter(|contract| {
            !contract.id.is_empty()
                && !contract.root.is_empty()
                && !contract.public_api.is_empty()
                && contract.forbid_cross_module_deep_imports
        })
        .count();
    let suppressions_expiring = suppressions
        .iter()
        .filter(|suppression| suppression.expires.is_some())
        .count();

    let declared_total = concepts.len() + contracts.len() + state_models.len() + module_contracts.len();
    let machine_checkable_total = concepts_machine_checkable
        + contracts_machine_checkable
        + state_models_machine_checkable
        + module_contracts_machine_checkable;
    let coverage_0_10000 = if declared_total == 0 {
        0
    } else {
        ((machine_checkable_total as f64 / declared_total as f64) * 10000.0).round() as u32
    };

    RuleCoverage {
        concepts_declared: concepts.len(),
        concepts_machine_checkable,
        contracts_declared: contracts.len(),
        contracts_machine_checkable,
        state_models_declared: state_models.len(),
        state_models_machine_checkable,
        module_contracts_declared: module_contracts.len(),
        module_contracts_machine_checkable,
        suppressions_declared: suppressions.len(),
        suppressions_expiring,
        coverage_0_10000,
    }
}

#[cfg(test)]
mod tests {
    use super::compute_rule_coverage;
    use crate::metrics::rules::RulesConfig;

    #[test]
    fn parses_v2_rules_sections() {
        let config: RulesConfig = toml::from_str(
            r#"
                [project]
                primary_language = "typescript"
                exclude = ["dist/**"]
                archetypes = ["modular_nextjs_frontend"]

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
                trigger_symbols = ["src/domain/server-state-bootstrap.ts::buildBootstrapPayload"]
                required_files = ["src/runtime/server-state-bootstrap.ts"]

                [[state_model]]
                id = "browser_state_sync"
                roots = ["src/runtime/browser-state-sync-controller.ts"]
                require_exhaustive_switch = true

                [[module_contract]]
                id = "feature_modules"
                root = "src/modules"
                public_api = ["index.ts", "index.tsx"]
                forbid_cross_module_deep_imports = true

                [[suppress]]
                kind = "multi_writer_concept"
                concept = "legacy_browser_state"
                reason = "Temporary migration bridge"
                expires = "2026-06-30"
            "#,
        )
        .expect("rules config");

        assert_eq!(
            config.project.primary_language.as_deref(),
            Some("typescript")
        );
        assert_eq!(config.concept.len(), 1);
        assert_eq!(config.contract.len(), 1);
        assert_eq!(config.state_model.len(), 1);
        assert_eq!(config.module_contract.len(), 1);
        assert_eq!(config.suppress.len(), 1);
        assert_eq!(config.contract[0].trigger_symbols.len(), 1);
        assert_eq!(config.contract[0].required_files.len(), 1);
    }

    #[test]
    fn rule_coverage_counts_machine_checkable_sections() {
        let config: RulesConfig = toml::from_str(
            r#"
                [[concept]]
                id = "task_git_status"
                anchors = ["src/store/core.ts::store.taskGitStatus"]
                allowed_writers = ["src/app/git-status-sync.ts::*"]

                [[concept]]
                id = "weak_concept"

                [[contract]]
                id = "server_state_bootstrap"
                registry_symbol = "src/app/server-state-bootstrap-registry.ts::SERVER_STATE_BOOTSTRAP_REGISTRY"
                categories_symbol = "src/domain/server-state-bootstrap.ts::SERVER_STATE_BOOTSTRAP_CATEGORIES"
                browser_entry = "src/runtime/browser-session.ts"

                [[state_model]]
                id = "browser_state_sync"
                roots = ["src/runtime/browser-state-sync-controller.ts"]
                require_exhaustive_switch = true

                [[module_contract]]
                id = "feature_modules"
                root = "src/modules"
            "#,
        )
        .expect("rules config");

        let coverage = compute_rule_coverage(
            &config.concept,
            &config.contract,
            &config.state_model,
            &config.module_contract,
            &config.suppress,
        );

        assert_eq!(coverage.concepts_declared, 2);
        assert_eq!(coverage.concepts_machine_checkable, 1);
        assert_eq!(coverage.contracts_machine_checkable, 1);
        assert_eq!(coverage.state_models_machine_checkable, 1);
        assert_eq!(coverage.module_contracts_machine_checkable, 1);
        assert_eq!(coverage.coverage_0_10000, 8000);
    }
}
