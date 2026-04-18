use super::classification::{
    classify_default_surface_role, classify_primary_lane, dedupe_strings_preserve_order,
    FindingLeverageClass, FindingPresentationClass as PresentationClass,
    FindingTrustTier as DebtTrustTier,
};
use super::*;
use crate::metrics::v2::FindingSeverity;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
#[path = "debt_annotation.rs"]
mod debt_annotation;
#[path = "debt_sources.rs"]
mod debt_sources;

pub(crate) use self::debt_annotation::{
    annotate_debt_signal, annotate_inspection_watchpoint, classify_debt_presentation_class,
};
pub(crate) use self::debt_sources::*;
use std::path::Path;

const INSPECTION_CLONE_PRESSURE_UNIT: u32 = 900;
const INSPECTION_CLONE_PRESSURE_MAX: u32 = 1800;
const INSPECTION_HOTSPOT_PRESSURE_UNIT: u32 = 700;
const INSPECTION_HOTSPOT_PRESSURE_MAX: u32 = 1400;
const INSPECTION_COMPOUND_BONUS: u32 = 900;
const CONCEPT_DEBT_HIGH_SEVERITY_UNIT: u32 = 2200;
const CONCEPT_DEBT_HIGH_SEVERITY_MAX: u32 = 4400;
const CONCEPT_DEBT_BOUNDARY_UNIT: u32 = 1100;
const CONCEPT_DEBT_BOUNDARY_MAX: u32 = 3300;
const CONCEPT_DEBT_FINDING_UNIT: u32 = 900;
const CONCEPT_DEBT_FINDING_MAX: u32 = 2700;
const CONCEPT_DEBT_MISSING_SITE_UNIT: u32 = 700;
const CONCEPT_DEBT_MISSING_SITE_MAX: u32 = 2800;
const CONCEPT_DEBT_CONTEXT_UNIT: u32 = 80;
const CONCEPT_DEBT_CONTEXT_MAX: u32 = 1600;

#[derive(Debug, Clone, Copy, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SignalClass {
    Debt,
    Hardening,
    Watchpoint,
}

impl SignalClass {
    fn from_str(value: &str) -> Self {
        match value {
            "hardening" => Self::Hardening,
            "watchpoint" => Self::Watchpoint,
            _ => Self::Debt,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
struct ConceptDebtSummary {
    concept_id: String,
    score_0_10000: u32,
    finding_count: usize,
    high_severity_count: usize,
    boundary_pressure_count: usize,
    obligation_count: usize,
    missing_site_count: usize,
    context_burden: usize,
    dominant_kinds: Vec<String>,
    files: Vec<String>,
    summary: String,
    inspection_focus: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct DebtSignal {
    pub(crate) kind: String,
    pub(crate) trust_tier: DebtTrustTier,
    pub(crate) presentation_class: PresentationClass,
    pub(crate) leverage_class: Option<FindingLeverageClass>,
    pub(crate) primary_lane: String,
    pub(crate) default_surface_role: String,
    pub(crate) scope: String,
    pub(crate) signal_class: SignalClass,
    pub(crate) signal_families: Vec<String>,
    pub(crate) severity: FindingSeverity,
    pub(crate) score_0_10000: u32,
    pub(crate) summary: String,
    pub(crate) impact: String,
    pub(crate) files: Vec<String>,
    pub(crate) role_tags: Vec<String>,
    pub(crate) leverage_reasons: Vec<String>,
    pub(crate) evidence: Vec<String>,
    pub(crate) inspection_focus: Vec<String>,
    pub(crate) candidate_split_axes: Vec<String>,
    pub(crate) related_surfaces: Vec<String>,
    pub(crate) metrics: DebtSignalMetrics,
}

#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct InspectionWatchpoint {
    pub(crate) kind: String,
    pub(crate) trust_tier: DebtTrustTier,
    pub(crate) presentation_class: PresentationClass,
    pub(crate) leverage_class: Option<FindingLeverageClass>,
    pub(crate) primary_lane: String,
    pub(crate) default_surface_role: String,
    pub(crate) scope: String,
    pub(crate) severity: FindingSeverity,
    pub(crate) score_0_10000: u32,
    pub(crate) summary: String,
    pub(crate) impact: String,
    pub(crate) files: Vec<String>,
    pub(crate) role_tags: Vec<String>,
    pub(crate) leverage_reasons: Vec<String>,
    pub(crate) evidence: Vec<String>,
    pub(crate) inspection_focus: Vec<String>,
    pub(crate) candidate_split_axes: Vec<String>,
    pub(crate) related_surfaces: Vec<String>,
    pub(crate) signal_families: Vec<String>,
    pub(crate) clone_family_count: usize,
    pub(crate) hotspot_count: usize,
    pub(crate) missing_site_count: usize,
    pub(crate) boundary_pressure_count: usize,
}

#[derive(Debug, Clone, serde::Serialize)]
struct DebtCluster {
    trust_tier: DebtTrustTier,
    presentation_class: PresentationClass,
    leverage_class: FindingLeverageClass,
    primary_lane: String,
    default_surface_role: String,
    scope: String,
    severity: FindingSeverity,
    score_0_10000: u32,
    summary: String,
    impact: String,
    files: Vec<String>,
    role_tags: Vec<String>,
    leverage_reasons: Vec<String>,
    evidence: Vec<String>,
    inspection_focus: Vec<String>,
    signal_families: Vec<String>,
    signal_kinds: Vec<String>,
    metrics: DebtClusterMetrics,
}

#[derive(Default)]
struct ConceptDebtAggregate {
    finding_count: usize,
    high_severity_count: usize,
    boundary_pressure_count: usize,
    obligation_count: usize,
    missing_site_count: usize,
    context_burden: usize,
    kinds: BTreeMap<String, usize>,
    files: BTreeSet<String>,
}

#[derive(Default)]
pub(crate) struct DebtReportOutputs {
    concept_summaries: Vec<ConceptDebtSummary>,
    debt_signals: Vec<DebtSignal>,
    experimental_debt_signals: Vec<DebtSignal>,
    debt_clusters: Vec<DebtCluster>,
    watchpoints: Vec<InspectionWatchpoint>,
    context_error: Option<String>,
}

impl DebtReportOutputs {
    pub(crate) fn context_error(&self) -> Option<String> {
        self.context_error.clone()
    }

    pub(crate) fn serialized_watchpoints(&self) -> Vec<Value> {
        serialized_values(&self.watchpoints)
    }
}

#[derive(Debug, Clone, serde::Serialize, Default)]
pub(crate) struct DebtSignalMetrics {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) finding_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) high_severity_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) boundary_pressure_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) obligation_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) missing_site_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) context_burden: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) file_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) line_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) function_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) member_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) recently_touched_file_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) fan_in: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) fan_out: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) instability_0_10000: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) dead_symbol_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) dead_line_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) cycle_size: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) cut_candidate_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) largest_cycle_after_best_cut: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) inbound_reference_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) public_surface_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) reachable_from_tests: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) guardrail_test_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) role_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) divergence_score: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) family_score_0_10000: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) authority_breadth: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) side_effect_breadth: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) timer_retry_weight: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) async_branch_weight: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) max_complexity: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) churn_commits: Option<u32>,
}

#[derive(Debug, Clone, serde::Serialize, Default)]
struct DebtClusterMetrics {
    signal_count: usize,
    file_count: usize,
    concept_count: usize,
    clone_family_count: usize,
    hotspot_count: usize,
    structural_signal_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn annotate_debt_signal_preserves_explicit_leverage_metadata() {
        let signal = annotate_debt_signal(DebtSignal {
            kind: "dependency_sprawl".to_string(),
            trust_tier: DebtTrustTier::Trusted,
            presentation_class: PresentationClass::StructuralDebt,
            leverage_class: Some(FindingLeverageClass::LocalRefactorTarget),
            primary_lane: String::new(),
            default_surface_role: String::new(),
            scope: "src/components/terminal-session.ts".to_string(),
            signal_class: SignalClass::Debt,
            signal_families: vec!["coordination".to_string()],
            severity: FindingSeverity::Medium,
            score_0_10000: 7_200,
            summary: "Facade has broad dependency pressure".to_string(),
            impact: "Coordination is still too centralized.".to_string(),
            files: vec!["src/components/terminal-session.ts".to_string()],
            role_tags: vec!["facade_with_extracted_owners".to_string()],
            leverage_reasons: vec!["extracted_owner_shell_pressure".to_string()],
            evidence: vec!["fan-out: 12".to_string()],
            inspection_focus: vec!["inspect extracted owners".to_string()],
            candidate_split_axes: vec!["facade owner boundary".to_string()],
            related_surfaces: vec!["src/components/terminal-session.ts".to_string()],
            metrics: DebtSignalMetrics {
                fan_in: Some(30),
                fan_out: Some(12),
                ..DebtSignalMetrics::default()
            },
        });

        assert_eq!(
            signal.leverage_class,
            Some(FindingLeverageClass::LocalRefactorTarget)
        );
        assert_eq!(
            signal.leverage_reasons,
            vec!["extracted_owner_shell_pressure".to_string()]
        );
        assert_eq!(signal.primary_lane, "maintainer_watchpoint");
        assert_eq!(signal.default_surface_role, "supporting_watchpoint");
    }

    #[test]
    fn annotate_inspection_watchpoint_preserves_explicit_leverage_metadata() {
        let watchpoint = annotate_inspection_watchpoint(InspectionWatchpoint {
            kind: "cycle_cluster".to_string(),
            trust_tier: DebtTrustTier::Watchpoint,
            presentation_class: PresentationClass::Watchpoint,
            leverage_class: Some(FindingLeverageClass::ArchitectureSignal),
            primary_lane: String::new(),
            default_surface_role: String::new(),
            scope: "src/store/store.ts".to_string(),
            severity: FindingSeverity::High,
            score_0_10000: 8_900,
            summary: "Shared barrel sits inside a mixed cycle".to_string(),
            impact: "Layer boundaries stay ambiguous.".to_string(),
            files: vec!["src/store/store.ts".to_string()],
            role_tags: vec!["component_barrel".to_string()],
            leverage_reasons: vec!["shared_barrel_boundary_hub".to_string()],
            evidence: vec!["cycle size: 14".to_string()],
            inspection_focus: vec!["inspect the cut candidate".to_string()],
            candidate_split_axes: vec!["contract extraction".to_string()],
            related_surfaces: vec!["src/store/store.ts".to_string()],
            signal_families: vec!["dependency".to_string()],
            clone_family_count: 0,
            hotspot_count: 0,
            missing_site_count: 0,
            boundary_pressure_count: 0,
        });

        assert_eq!(
            watchpoint.leverage_class,
            Some(FindingLeverageClass::ArchitectureSignal)
        );
        assert_eq!(
            watchpoint.leverage_reasons,
            vec!["shared_barrel_boundary_hub".to_string()]
        );
        assert_eq!(watchpoint.primary_lane, "maintainer_watchpoint");
        assert_eq!(watchpoint.default_surface_role, "supporting_watchpoint");
    }

    #[test]
    fn insert_debt_report_fields_emits_only_canonical_fields() {
        let mut result = serde_json::Map::new();
        let debt_outputs = DebtReportOutputs {
            concept_summaries: Vec::new(),
            debt_signals: vec![annotate_debt_signal(DebtSignal {
                kind: "concept".to_string(),
                trust_tier: DebtTrustTier::Trusted,
                presentation_class: PresentationClass::StructuralDebt,
                leverage_class: Some(FindingLeverageClass::LocalRefactorTarget),
                primary_lane: String::new(),
                default_surface_role: String::new(),
                scope: "concept:task_state".to_string(),
                signal_class: SignalClass::Debt,
                signal_families: vec!["boundary".to_string()],
                severity: FindingSeverity::Medium,
                score_0_10000: 6_200,
                summary: "Concept debt".to_string(),
                impact: "Updates still cross multiple seams.".to_string(),
                files: vec!["src/domain/task-state.ts".to_string()],
                role_tags: Vec::new(),
                leverage_reasons: Vec::new(),
                evidence: vec!["finding count: 3".to_string()],
                inspection_focus: vec!["inspect the concept boundary".to_string()],
                candidate_split_axes: vec!["concept boundary".to_string()],
                related_surfaces: vec!["src/domain/task-state.ts".to_string()],
                metrics: DebtSignalMetrics::default(),
            })],
            experimental_debt_signals: Vec::new(),
            debt_clusters: Vec::new(),
            watchpoints: vec![annotate_inspection_watchpoint(InspectionWatchpoint {
                kind: "concept_watchpoint".to_string(),
                trust_tier: DebtTrustTier::Watchpoint,
                presentation_class: PresentationClass::Watchpoint,
                leverage_class: Some(FindingLeverageClass::RegrowthWatchpoint),
                primary_lane: String::new(),
                default_surface_role: String::new(),
                scope: "concept:task_state".to_string(),
                severity: FindingSeverity::Low,
                score_0_10000: 3_800,
                summary: "Watchpoint".to_string(),
                impact: "Still worth checking before larger edits.".to_string(),
                files: vec!["src/domain/task-state.ts".to_string()],
                role_tags: Vec::new(),
                leverage_reasons: Vec::new(),
                evidence: vec!["missing site count: 1".to_string()],
                inspection_focus: vec!["inspect propagation coverage".to_string()],
                candidate_split_axes: vec!["propagation surface".to_string()],
                related_surfaces: vec!["src/domain/task-state.ts".to_string()],
                signal_families: vec!["propagation".to_string()],
                clone_family_count: 0,
                hotspot_count: 0,
                missing_site_count: 1,
                boundary_pressure_count: 0,
            })],
            context_error: Some("context unavailable".to_string()),
        };

        let context_error = insert_debt_report_fields(&mut result, debt_outputs);
        assert_eq!(context_error.as_deref(), Some("context unavailable"));
        assert_eq!(result["debt_signal_count"], 1);
        assert_eq!(result["watchpoint_count"], 1);
        assert!(result.get("quality_opportunity_count").is_none());
        assert!(result.get("quality_opportunities").is_none());
        assert!(result.get("optimization_priority_count").is_none());
        assert!(result.get("optimization_priorities").is_none());
        assert!(result.get("debt_context_error").is_none());
        assert!(result.get("opportunity_context_error").is_none());
        assert_eq!(
            result["debt_signals"][0]["primary_lane"],
            json!("maintainer_watchpoint")
        );
        assert_eq!(
            result["watchpoints"][0]["default_surface_role"],
            json!("supporting_watchpoint")
        );
    }

    #[test]
    fn debt_classifications_serialize_to_canonical_strings() {
        let signal = annotate_debt_signal(DebtSignal {
            kind: "concept".to_string(),
            trust_tier: DebtTrustTier::Watchpoint,
            presentation_class: PresentationClass::Watchpoint,
            leverage_class: Some(FindingLeverageClass::BoundaryDiscipline),
            primary_lane: String::new(),
            default_surface_role: String::new(),
            scope: "concept:task_state".to_string(),
            signal_class: SignalClass::Watchpoint,
            signal_families: vec!["propagation".to_string()],
            severity: FindingSeverity::Medium,
            score_0_10000: 5_500,
            summary: "Concept debt".to_string(),
            impact: "Updates still cross multiple seams.".to_string(),
            files: vec!["src/domain/task-state.ts".to_string()],
            role_tags: Vec::new(),
            leverage_reasons: vec!["boundary_or_facade_seam_pressure".to_string()],
            evidence: vec!["finding count: 3".to_string()],
            inspection_focus: vec!["inspect the concept boundary".to_string()],
            candidate_split_axes: vec!["concept boundary".to_string()],
            related_surfaces: vec!["src/domain/task-state.ts".to_string()],
            metrics: DebtSignalMetrics::default(),
        });
        let watchpoint = annotate_inspection_watchpoint(InspectionWatchpoint {
            kind: "concept_watchpoint".to_string(),
            trust_tier: DebtTrustTier::Experimental,
            presentation_class: PresentationClass::Experimental,
            leverage_class: Some(FindingLeverageClass::RegrowthWatchpoint),
            primary_lane: String::new(),
            default_surface_role: String::new(),
            scope: "concept:task_state".to_string(),
            severity: FindingSeverity::High,
            score_0_10000: 8_800,
            summary: "Watchpoint".to_string(),
            impact: "Still worth checking before larger edits.".to_string(),
            files: vec!["src/domain/task-state.ts".to_string()],
            role_tags: Vec::new(),
            leverage_reasons: vec!["regrowth_watchpoint".to_string()],
            evidence: vec!["missing site count: 1".to_string()],
            inspection_focus: vec!["inspect propagation coverage".to_string()],
            candidate_split_axes: vec!["propagation surface".to_string()],
            related_surfaces: vec!["src/domain/task-state.ts".to_string()],
            signal_families: vec!["propagation".to_string()],
            clone_family_count: 0,
            hotspot_count: 0,
            missing_site_count: 1,
            boundary_pressure_count: 0,
        });

        let signal_json = serde_json::to_value(&signal).expect("serialize signal");
        let watchpoint_json = serde_json::to_value(&watchpoint).expect("serialize watchpoint");

        assert_eq!(signal_json["trust_tier"], json!("watchpoint"));
        assert_eq!(signal_json["presentation_class"], json!("watchpoint"));
        assert_eq!(signal_json["leverage_class"], json!("boundary_discipline"));
        assert_eq!(signal_json["primary_lane"], json!("maintainer_watchpoint"));
        assert_eq!(
            signal_json["default_surface_role"],
            json!("supporting_watchpoint")
        );
        assert_eq!(watchpoint_json["trust_tier"], json!("experimental"));
        assert_eq!(watchpoint_json["presentation_class"], json!("experimental"));
        assert_eq!(
            watchpoint_json["leverage_class"],
            json!("regrowth_watchpoint")
        );
        assert_eq!(watchpoint_json["primary_lane"], json!("experimental"));
        assert_eq!(
            watchpoint_json["default_surface_role"],
            json!("experimental")
        );
    }
}
