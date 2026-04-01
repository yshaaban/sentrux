//! Structural debt reports built from existing health metrics and snapshot facts.

use super::FindingSeverity;
mod cycles;
mod graph;
mod guardrails;
mod path_roles;
mod reports;
mod scoring;
mod utils;

use crate::core::snapshot::{flatten_files_ref, Snapshot};
use crate::core::types::FileNode;
use crate::metrics::testgap::is_test_file;
use crate::metrics::{is_package_index_for_path, HealthReport};
use graph::build_structural_graph;
use guardrails::{detect_architecture_guardrails, GuardrailFileEvidence};
use path_roles::path_role_tags;
use reports::{
    build_dead_island_reports, build_dead_private_code_cluster_reports,
    build_dependency_sprawl_reports, build_large_file_reports, build_unstable_hotspot_reports,
};
use scoring::severity_priority;
use std::collections::BTreeMap;
use std::path::Path;
use utils::dedupe_strings_preserve_order;

pub use cycles::CycleCutCandidate;

#[derive(Debug, Clone, Copy, Eq, PartialEq, serde::Serialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum StructuralTrustTier {
    #[default]
    Trusted,
    Watchpoint,
    Experimental,
}

impl StructuralTrustTier {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Trusted => "trusted",
            Self::Watchpoint => "watchpoint",
            Self::Experimental => "experimental",
        }
    }
}

impl PartialEq<&str> for StructuralTrustTier {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, serde::Serialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum StructuralPresentationClass {
    #[default]
    StructuralDebt,
    GuardedFacade,
    ToolingDebt,
    HardeningNote,
    Watchpoint,
    Experimental,
}

impl StructuralPresentationClass {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::StructuralDebt => "structural_debt",
            Self::GuardedFacade => "guarded_facade",
            Self::ToolingDebt => "tooling_debt",
            Self::HardeningNote => "hardening_note",
            Self::Watchpoint => "watchpoint",
            Self::Experimental => "experimental",
        }
    }
}

impl PartialEq<&str> for StructuralPresentationClass {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, serde::Serialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum StructuralLeverageClass {
    #[default]
    SecondaryCleanup,
    LocalRefactorTarget,
    ArchitectureSignal,
    RegrowthWatchpoint,
    ToolingDebt,
    BoundaryDiscipline,
    HardeningNote,
    Experimental,
}

impl StructuralLeverageClass {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SecondaryCleanup => "secondary_cleanup",
            Self::LocalRefactorTarget => "local_refactor_target",
            Self::ArchitectureSignal => "architecture_signal",
            Self::RegrowthWatchpoint => "regrowth_watchpoint",
            Self::ToolingDebt => "tooling_debt",
            Self::BoundaryDiscipline => "boundary_discipline",
            Self::HardeningNote => "hardening_note",
            Self::Experimental => "experimental",
        }
    }
}

impl PartialEq<&str> for StructuralLeverageClass {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, serde::Serialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum StructuralSignalClass {
    #[default]
    Debt,
    Watchpoint,
}

impl StructuralSignalClass {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Debt => "debt",
            Self::Watchpoint => "watchpoint",
        }
    }
}

impl PartialEq<&str> for StructuralSignalClass {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

#[derive(Debug, Clone, serde::Serialize, Default)]
pub struct StructuralDebtMetrics {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fan_in: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fan_out: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instability_0_10000: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dead_symbol_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dead_line_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cycle_size: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_complexity: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inbound_reference_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub public_surface_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reachable_from_tests: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cut_candidate_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub largest_cycle_after_best_cut: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub guardrail_test_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role_count: Option<usize>,
}

#[derive(Debug, Clone, serde::Serialize, Default)]
pub struct StructuralDebtReport {
    pub kind: String,
    pub trust_tier: StructuralTrustTier,
    pub presentation_class: StructuralPresentationClass,
    pub leverage_class: StructuralLeverageClass,
    pub scope: String,
    pub signal_class: StructuralSignalClass,
    pub signal_families: Vec<String>,
    pub severity: FindingSeverity,
    pub score_0_10000: u32,
    pub summary: String,
    pub impact: String,
    pub files: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub role_tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub leverage_reasons: Vec<String>,
    pub evidence: Vec<String>,
    pub inspection_focus: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub candidate_split_axes: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub related_surfaces: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cut_candidates: Vec<CycleCutCandidate>,
    pub metrics: StructuralDebtMetrics,
}

#[derive(Debug, Clone, Default)]
pub(super) struct FileFacts {
    pub(super) lang: String,
    pub(super) lines: usize,
    pub(super) function_count: u32,
    pub(super) max_complexity: u32,
    pub(super) is_test: bool,
    pub(super) is_package_index: bool,
    pub(super) has_entry_tag: bool,
    pub(super) public_function_count: usize,
    pub(super) role_tags: Vec<String>,
    pub(super) guardrail_tests: Vec<String>,
    pub(super) facade_owner_factories: Vec<String>,
    pub(super) boundary_guard_literals: Vec<String>,
}

pub fn build_structural_debt_reports(
    snapshot: &Snapshot,
    health: &HealthReport,
) -> Vec<StructuralDebtReport> {
    build_structural_debt_reports_internal(snapshot, health, None)
}

pub fn build_structural_debt_reports_with_root(
    root: &Path,
    snapshot: &Snapshot,
    health: &HealthReport,
) -> Vec<StructuralDebtReport> {
    build_structural_debt_reports_internal(snapshot, health, Some(root))
}

fn build_structural_debt_reports_internal(
    snapshot: &Snapshot,
    health: &HealthReport,
    root: Option<&Path>,
) -> Vec<StructuralDebtReport> {
    let file_facts = build_file_facts(snapshot, root);
    let graph = build_structural_graph(snapshot);
    let mut reports = Vec::new();

    reports.extend(build_large_file_reports(health, &file_facts, &graph));
    reports.extend(build_dependency_sprawl_reports(health, &file_facts, &graph));
    reports.extend(build_unstable_hotspot_reports(health, &file_facts, &graph));
    reports.extend(cycles::build_cycle_cluster_reports(
        health,
        &file_facts,
        &graph,
    ));
    reports.extend(build_dead_private_code_cluster_reports(health, &file_facts));
    reports.extend(build_dead_island_reports(
        snapshot,
        health,
        &file_facts,
        &graph,
    ));

    reports.sort_by(|left, right| {
        severity_priority(right.severity)
            .cmp(&severity_priority(left.severity))
            .then_with(|| right.score_0_10000.cmp(&left.score_0_10000))
            .then_with(|| left.scope.cmp(&right.scope))
    });
    reports
}

fn build_file_facts(snapshot: &Snapshot, root: Option<&Path>) -> BTreeMap<String, FileFacts> {
    let entry_surface_paths = snapshot
        .entry_points
        .iter()
        .map(|entry| entry.file.clone())
        .collect::<std::collections::BTreeSet<_>>();
    let file_paths = flatten_files_ref(&snapshot.root)
        .into_iter()
        .filter(|file| !file.lang.is_empty() && file.lang != "unknown")
        .map(|file| file.path.clone())
        .collect::<std::collections::BTreeSet<_>>();
    let guardrail_evidence = root
        .map(|root| detect_architecture_guardrails(root, &file_paths))
        .unwrap_or_default();

    flatten_files_ref(&snapshot.root)
        .into_iter()
        .filter(|file| !file.lang.is_empty() && file.lang != "unknown")
        .map(|file| {
            let evidence = guardrail_evidence.get(&file.path);
            (
                file.path.clone(),
                file_facts(file, entry_surface_paths.contains(&file.path), evidence),
            )
        })
        .collect()
}

fn file_facts(
    file: &FileNode,
    is_entry_surface: bool,
    guardrail_evidence: Option<&GuardrailFileEvidence>,
) -> FileFacts {
    let max_complexity = file
        .sa
        .as_ref()
        .and_then(|analysis| analysis.functions.as_ref())
        .map(|functions| {
            functions
                .iter()
                .map(|function| function.cc.unwrap_or(0).max(function.cog.unwrap_or(0)))
                .max()
                .unwrap_or(0)
        })
        .unwrap_or(0);

    let mut role_tags = Vec::new();
    if is_entry_surface {
        role_tags.push("entry_surface".to_string());
    }
    if let Some(evidence) = guardrail_evidence {
        if !evidence.tests.is_empty() {
            role_tags.push("guarded_seam".to_string());
        }
        if evidence.facade_owner_factories.len() >= 2 {
            role_tags.push("facade_with_extracted_owners".to_string());
        }
        if !evidence.boundary_guard_literals.is_empty() {
            role_tags.push("guarded_boundary".to_string());
        }
    }
    if file.path.contains("store/store.") && role_tags.iter().any(|tag| tag == "guarded_boundary") {
        role_tags.push("component_barrel".to_string());
    }
    role_tags.extend(path_role_tags(&file.path));

    FileFacts {
        lang: file.lang.clone(),
        lines: file.lines as usize,
        function_count: file.funcs,
        max_complexity,
        is_test: file
            .sa
            .as_ref()
            .and_then(|analysis| analysis.tags.as_ref())
            .is_some_and(|tags| tags.iter().any(|tag| tag.contains("test")))
            || is_test_file(&file.path),
        is_package_index: is_package_index_for_path(&file.path),
        has_entry_tag: file
            .sa
            .as_ref()
            .and_then(|analysis| analysis.tags.as_ref())
            .is_some_and(|tags| tags.iter().any(|tag| tag == "entry")),
        public_function_count: file
            .sa
            .as_ref()
            .and_then(|analysis| analysis.functions.as_ref())
            .map(|functions| {
                functions
                    .iter()
                    .filter(|function| function.is_public)
                    .count()
            })
            .unwrap_or(0),
        role_tags: dedupe_strings_preserve_order(role_tags),
        guardrail_tests: guardrail_evidence
            .map(|evidence| evidence.tests.clone())
            .unwrap_or_default(),
        facade_owner_factories: guardrail_evidence
            .map(|evidence| evidence.facade_owner_factories.clone())
            .unwrap_or_default(),
        boundary_guard_literals: guardrail_evidence
            .map(|evidence| evidence.boundary_guard_literals.clone())
            .unwrap_or_default(),
    }
}

#[cfg(test)]
fn has_dead_island_report(reports: &[StructuralDebtReport], expected_files: &[&str]) -> bool {
    let expected_files = expected_files
        .iter()
        .map(|path| path.to_string())
        .collect::<Vec<_>>();
    reports
        .iter()
        .any(|report| report.kind == "dead_island" && report.files == expected_files)
}

#[cfg(test)]
mod tests;
