//! Conservative contract parity analyzer driven by explicit contract rules.

use super::SemanticFinding;
use crate::analysis::semantic::SemanticSnapshot;
use crate::metrics::rules::RulesConfig;
use std::collections::BTreeSet;
use std::path::Path;

use super::parity_support::{
    build_contract_parity_report, contract_in_scope, finding_kind_and_severity,
};

pub(crate) const VERSIONING_MARKER_KIND: &str = "versioning";
pub(crate) const SATISFIED_STATUS: &str = "satisfied";
pub(crate) const MISSING_STATUS: &str = "missing";

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum ParityFindingKind {
    MissingBrowserPath,
    MissingElectronPath,
    ParityVersionGap,
    SnapshotWithoutLiveUpdate,
    MissingLiveUpdatePath,
    MissingContractSymbol,
}

impl ParityFindingKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::MissingBrowserPath => "missing_browser_path",
            Self::MissingElectronPath => "missing_electron_path",
            Self::ParityVersionGap => "parity_version_gap",
            Self::SnapshotWithoutLiveUpdate => "snapshot_without_live_update",
            Self::MissingLiveUpdatePath => "missing_live_update_path",
            Self::MissingContractSymbol => "missing_contract_symbol",
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum ParityCellKind {
    CategoriesSymbol,
    PayloadMapSymbol,
    RegistrySymbol,
    BrowserEntry,
    ElectronEntry,
    BrowserLiveUpdateBinding,
    ElectronLiveUpdateBinding,
    VersioningMarker,
    SnapshotWithoutLiveUpdate,
}

impl ParityCellKind {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::CategoriesSymbol => "categories_symbol",
            Self::PayloadMapSymbol => "payload_map_symbol",
            Self::RegistrySymbol => "registry_symbol",
            Self::BrowserEntry => "browser_entry",
            Self::ElectronEntry => "electron_entry",
            Self::BrowserLiveUpdateBinding => "browser_live_update_binding",
            Self::ElectronLiveUpdateBinding => "electron_live_update_binding",
            Self::VersioningMarker => VERSIONING_MARKER_KIND,
            Self::SnapshotWithoutLiveUpdate => "snapshot_without_live_update",
        }
    }

    pub(crate) fn from_str(value: &str) -> Option<Self> {
        match value {
            "categories_symbol" => Some(Self::CategoriesSymbol),
            "payload_map_symbol" => Some(Self::PayloadMapSymbol),
            "registry_symbol" => Some(Self::RegistrySymbol),
            "browser_entry" => Some(Self::BrowserEntry),
            "electron_entry" => Some(Self::ElectronEntry),
            "browser_live_update_binding" => Some(Self::BrowserLiveUpdateBinding),
            "electron_live_update_binding" => Some(Self::ElectronLiveUpdateBinding),
            VERSIONING_MARKER_KIND => Some(Self::VersioningMarker),
            "snapshot_without_live_update" => Some(Self::SnapshotWithoutLiveUpdate),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ParityScope {
    All,
    Changed,
}

#[derive(Debug, Clone, serde::Serialize, Eq, PartialEq, Ord, PartialOrd)]
pub struct ParityCell {
    pub kind: String,
    pub status: String,
    pub detail: String,
    pub path: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ContractParityReport {
    pub id: String,
    pub kind: String,
    pub priority: Option<String>,
    pub files: Vec<String>,
    pub satisfied_cells: Vec<ParityCell>,
    pub missing_cells: Vec<ParityCell>,
    pub score_0_10000: u32,
    pub summary: String,
}

#[derive(Debug, Clone, Default)]
pub struct ParityBuildResult {
    pub reports: Vec<ContractParityReport>,
    pub read_warnings: Vec<String>,
}

pub fn build_parity_reports(
    config: &RulesConfig,
    semantic: &SemanticSnapshot,
    root: &Path,
    scope: ParityScope,
    changed_files: &BTreeSet<String>,
) -> ParityBuildResult {
    let mut read_warnings = Vec::new();
    let mut reports = config
        .contract
        .iter()
        .filter(|contract| contract_in_scope(contract, scope, changed_files))
        .map(|contract| {
            build_contract_parity_report(config, contract, semantic, root, &mut read_warnings)
        })
        .collect::<Vec<_>>();
    reports.sort_by(|left, right| left.id.cmp(&right.id));
    ParityBuildResult {
        reports,
        read_warnings,
    }
}

pub fn build_parity_findings(reports: &[ContractParityReport]) -> Vec<SemanticFinding> {
    let mut findings = Vec::new();

    for report in reports {
        for cell in &report.missing_cells {
            let (kind, severity) = finding_kind_and_severity(cell);
            findings.push(SemanticFinding {
                kind: kind.as_str().to_string(),
                severity,
                concept_id: report.id.clone(),
                summary: format!("Contract '{}' is missing {}", report.id, cell.kind),
                files: cell.path.iter().cloned().collect(),
                evidence: vec![cell.detail.clone()],
            });
        }
    }

    findings
}

pub fn parity_score_0_10000(reports: &[ContractParityReport]) -> u32 {
    let mut satisfied = 0usize;
    let mut total = 0usize;

    for report in reports {
        satisfied += report.satisfied_cells.len();
        total += report.satisfied_cells.len() + report.missing_cells.len();
    }

    if total == 0 {
        return 10000;
    }

    ((satisfied as f64 / total as f64) * 10000.0).round() as u32
}

#[cfg(test)]
mod tests {
    use super::{build_parity_reports, parity_score_0_10000, ContractParityReport, ParityScope};
    use crate::analysis::semantic::{
        ProjectModel, SemanticCapability, SemanticFileFact, SemanticSnapshot, SymbolFact,
    };
    use crate::metrics::rules::RulesConfig;
    use crate::test_support::temp_root;
    use std::collections::BTreeSet;

    fn report_by_id<'a>(reports: &'a [ContractParityReport], id: &str) -> &'a ContractParityReport {
        reports
            .iter()
            .find(|report| report.id == id)
            .expect("report")
    }

    #[test]
    fn reports_missing_runtime_and_versioning_cells() {
        let root = temp_root(
            "sentrux-parity",
            "missing-runtime",
            &["src/domain", "src/app", "src/runtime"],
        );
        std::fs::write(
            root.join("src/domain/bootstrap.ts"),
            "export const SERVER_STATE_BOOTSTRAP_CATEGORIES = ['task'];\n",
        )
        .expect("write categories");
        std::fs::write(
            root.join("src/app/registry.ts"),
            "export const SERVER_STATE_BOOTSTRAP_REGISTRY = {};\n",
        )
        .expect("write registry");

        let config: RulesConfig = toml::from_str(
            r#"
                [[contract]]
                id = "server_state_bootstrap"
                categories_symbol = "src/domain/bootstrap.ts::SERVER_STATE_BOOTSTRAP_CATEGORIES"
                registry_symbol = "src/app/registry.ts::SERVER_STATE_BOOTSTRAP_REGISTRY"
                browser_entry = "src/runtime/browser.ts"
                required_capabilities = ["snapshot", "live_updates", "versioning"]
            "#,
        )
        .expect("rules config");
        let semantic = SemanticSnapshot {
            project: ProjectModel::default(),
            analyzed_files: 2,
            capabilities: vec![SemanticCapability::Symbols],
            files: vec![SemanticFileFact::default()],
            symbols: vec![
                SymbolFact {
                    id: "categories".to_string(),
                    path: "src/domain/bootstrap.ts".to_string(),
                    name: "SERVER_STATE_BOOTSTRAP_CATEGORIES".to_string(),
                    kind: "const".to_string(),
                    line: 1,
                },
                SymbolFact {
                    id: "registry".to_string(),
                    path: "src/app/registry.ts".to_string(),
                    name: "SERVER_STATE_BOOTSTRAP_REGISTRY".to_string(),
                    kind: "const".to_string(),
                    line: 1,
                },
            ],
            reads: Vec::new(),
            writes: Vec::new(),
            closed_domains: Vec::new(),
            closed_domain_sites: Vec::new(),
            transition_sites: Vec::new(),
        };

        let reports = build_parity_reports(
            &config,
            &semantic,
            &root,
            ParityScope::All,
            &BTreeSet::new(),
        )
        .reports;
        let report = report_by_id(&reports, "server_state_bootstrap");

        assert!(report
            .missing_cells
            .iter()
            .any(|cell| cell.kind == "browser_entry"));
        assert!(!report
            .missing_cells
            .iter()
            .any(|cell| cell.kind == "browser_live_update_binding"));
        assert!(report
            .missing_cells
            .iter()
            .any(|cell| cell.kind == "versioning"));
        assert!(parity_score_0_10000(&reports) < 10000);

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn changed_scope_filters_untouched_contracts() {
        let root = temp_root(
            "sentrux-parity",
            "changed-scope",
            &["src/domain", "src/app", "src/runtime"],
        );
        let config: RulesConfig = toml::from_str(
            r#"
                [[contract]]
                id = "bootstrap"
                categories_symbol = "src/domain/bootstrap.ts::SERVER_STATE_BOOTSTRAP_CATEGORIES"
                registry_symbol = "src/app/registry.ts::SERVER_STATE_BOOTSTRAP_REGISTRY"
                browser_entry = "src/runtime/browser.ts"
            "#,
        )
        .expect("rules config");
        let semantic = SemanticSnapshot::default();
        let changed_files = BTreeSet::from(["src/runtime/browser.ts".to_string()]);

        let reports = build_parity_reports(
            &config,
            &semantic,
            &root,
            ParityScope::Changed,
            &changed_files,
        )
        .reports;

        assert_eq!(reports.len(), 1);

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn runtime_binding_requires_exact_identifier_match() {
        let root = temp_root(
            "sentrux-parity",
            "binding-token",
            &["src/domain", "src/app", "src/runtime"],
        );
        std::fs::write(
            root.join("src/domain/bootstrap.ts"),
            "export const SERVER_STATE_BOOTSTRAP_CATEGORIES = ['task'];\n",
        )
        .expect("write categories");
        std::fs::write(
            root.join("src/app/registry.ts"),
            "export const SERVER_STATE_BOOTSTRAP_REGISTRY = {};\n",
        )
        .expect("write registry");
        std::fs::create_dir_all(root.join("src/runtime")).expect("create runtime dir");
        std::fs::write(
            root.join("src/runtime/browser.ts"),
            "const SERVER_STATE_BOOTSTRAP_CATEGORIES_WRAPPER = true;\n",
        )
        .expect("write browser");

        let config: RulesConfig = toml::from_str(
            r#"
                [[contract]]
                id = "bootstrap"
                categories_symbol = "src/domain/bootstrap.ts::SERVER_STATE_BOOTSTRAP_CATEGORIES"
                registry_symbol = "src/app/registry.ts::SERVER_STATE_BOOTSTRAP_REGISTRY"
                browser_entry = "src/runtime/browser.ts"
                required_capabilities = ["live_updates"]
            "#,
        )
        .expect("rules config");
        let semantic = SemanticSnapshot {
            project: ProjectModel::default(),
            analyzed_files: 3,
            capabilities: vec![SemanticCapability::Symbols],
            files: vec![SemanticFileFact::default()],
            symbols: vec![
                SymbolFact {
                    id: "categories".to_string(),
                    path: "src/domain/bootstrap.ts".to_string(),
                    name: "SERVER_STATE_BOOTSTRAP_CATEGORIES".to_string(),
                    kind: "const".to_string(),
                    line: 1,
                },
                SymbolFact {
                    id: "registry".to_string(),
                    path: "src/app/registry.ts".to_string(),
                    name: "SERVER_STATE_BOOTSTRAP_REGISTRY".to_string(),
                    kind: "const".to_string(),
                    line: 1,
                },
            ],
            reads: Vec::new(),
            writes: Vec::new(),
            closed_domains: Vec::new(),
            closed_domain_sites: Vec::new(),
            transition_sites: Vec::new(),
        };

        let reports = build_parity_reports(
            &config,
            &semantic,
            &root,
            ParityScope::All,
            &BTreeSet::new(),
        )
        .reports;
        let report = report_by_id(&reports, "bootstrap");

        assert!(report
            .missing_cells
            .iter()
            .any(|cell| cell.kind == "browser_live_update_binding"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn runtime_binding_accepts_related_contract_family_symbols() {
        let root = temp_root(
            "sentrux-parity",
            "binding-family",
            &["src/domain", "src/app", "src/runtime"],
        );
        std::fs::write(
            root.join("src/domain/bootstrap.ts"),
            r#"
                export const SERVER_STATE_BOOTSTRAP_CATEGORIES = ['task'];
                export type AnyServerStateBootstrapSnapshot = { category: 'task' };
            "#,
        )
        .expect("write categories");
        std::fs::write(
            root.join("src/app/registry.ts"),
            "export const SERVER_STATE_BOOTSTRAP_REGISTRY = {};\n",
        )
        .expect("write registry");
        std::fs::write(
            root.join("src/app/bootstrap.ts"),
            "export function replaceServerStateBootstrap(): void {}\n",
        )
        .expect("write bootstrap");
        std::fs::write(
            root.join("src/runtime/browser.ts"),
            "import type { AnyServerStateBootstrapSnapshot } from '../domain/bootstrap';\nconst apply = (_snapshots: AnyServerStateBootstrapSnapshot[]) => {};\n",
        )
        .expect("write browser");
        std::fs::write(
            root.join("src/runtime/electron.ts"),
            "import { replaceServerStateBootstrap } from '../app/bootstrap';\nreplaceServerStateBootstrap();\n",
        )
        .expect("write electron");

        let config: RulesConfig = toml::from_str(
            r#"
                [[concept]]
                id = "bootstrap"
                anchors = ["src/app/bootstrap.ts::replaceServerStateBootstrap"]

                [[contract]]
                id = "bootstrap"
                categories_symbol = "src/domain/bootstrap.ts::SERVER_STATE_BOOTSTRAP_CATEGORIES"
                registry_symbol = "src/app/registry.ts::SERVER_STATE_BOOTSTRAP_REGISTRY"
                browser_entry = "src/runtime/browser.ts"
                electron_entry = "src/runtime/electron.ts"
                required_capabilities = ["snapshot", "live_updates"]
            "#,
        )
        .expect("rules config");
        let semantic = SemanticSnapshot {
            project: ProjectModel::default(),
            analyzed_files: 5,
            capabilities: vec![SemanticCapability::Symbols],
            files: vec![SemanticFileFact::default()],
            symbols: vec![
                SymbolFact {
                    id: "categories".to_string(),
                    path: "src/domain/bootstrap.ts".to_string(),
                    name: "SERVER_STATE_BOOTSTRAP_CATEGORIES".to_string(),
                    kind: "const".to_string(),
                    line: 2,
                },
                SymbolFact {
                    id: "bootstrap-type".to_string(),
                    path: "src/domain/bootstrap.ts".to_string(),
                    name: "AnyServerStateBootstrapSnapshot".to_string(),
                    kind: "type".to_string(),
                    line: 3,
                },
                SymbolFact {
                    id: "registry".to_string(),
                    path: "src/app/registry.ts".to_string(),
                    name: "SERVER_STATE_BOOTSTRAP_REGISTRY".to_string(),
                    kind: "const".to_string(),
                    line: 1,
                },
                SymbolFact {
                    id: "replace".to_string(),
                    path: "src/app/bootstrap.ts".to_string(),
                    name: "replaceServerStateBootstrap".to_string(),
                    kind: "function".to_string(),
                    line: 1,
                },
            ],
            reads: Vec::new(),
            writes: Vec::new(),
            closed_domains: Vec::new(),
            closed_domain_sites: Vec::new(),
            transition_sites: Vec::new(),
        };

        let reports = build_parity_reports(
            &config,
            &semantic,
            &root,
            ParityScope::All,
            &BTreeSet::new(),
        )
        .reports;
        let report = report_by_id(&reports, "bootstrap");

        assert!(report
            .satisfied_cells
            .iter()
            .any(|cell| cell.kind == "browser_live_update_binding"));
        assert!(report
            .satisfied_cells
            .iter()
            .any(|cell| cell.kind == "electron_live_update_binding"));
        assert!(!report
            .missing_cells
            .iter()
            .any(|cell| cell.kind == "snapshot_without_live_update"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn report_without_assessable_cells_scores_zero() {
        let root = temp_root(
            "sentrux-parity",
            "no-cells",
            &["src/domain", "src/app", "src/runtime"],
        );
        let config: RulesConfig = toml::from_str(
            r#"
                [[contract]]
                id = "empty_contract"
            "#,
        )
        .expect("rules config");

        let reports = build_parity_reports(
            &config,
            &SemanticSnapshot::default(),
            &root,
            ParityScope::All,
            &BTreeSet::new(),
        )
        .reports;
        let report = report_by_id(&reports, "empty_contract");

        assert_eq!(report.score_0_10000, 0);
        assert!(report.summary.contains("no assessable parity cells"));

        let _ = std::fs::remove_dir_all(root);
    }
}
