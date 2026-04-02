//! MCP tool handler implementations — core tools.
//!
//! Each handler has the uniform signature: `fn(&Value, &Tier, &mut McpState) -> Result<Value, String>`
//! Each tool also has a `_def()` function returning its `ToolDef` (schema + tier + handler co-located).
//!
//! Tier-aware truncation: detail lists are limited to `tier.detail_limit()` items.
//! Free users see top-3 + total counts. Pro users see everything.

use super::registry::ToolDef;
use super::{
    agent_brief::{build_agent_brief, AgentBriefInput, AgentBriefMode},
    response::{
        extend_diagnostics, extend_diagnostics_availability, insert_diagnostics,
        insert_error_diagnostics, insert_rules_semantic_context_diagnostics,
        insert_rules_semantic_diagnostics, insert_rules_semantic_evolution_diagnostics,
        insert_semantic_diagnostics, DiagnosticEntry,
    },
    semantic_cache::{
        current_semantic_cache_identity, load_persisted_semantic_snapshot,
        save_persisted_semantic_snapshot, SemanticCacheIdentity, SemanticCacheSource,
    },
    session_v2_schema_supported, McpState, PatchSafetyAnalysisCache, RulesCacheIdentity,
    ScanCacheIdentity, SessionV2Baseline, SessionV2ConfidenceSnapshot, SESSION_V2_SCHEMA_VERSION,
};
use crate::analysis::scanner;
use crate::analysis::scanner::common::ScanMetadata;
use crate::analysis::semantic::SemanticSnapshot;
use crate::core::snapshot::Snapshot;
use crate::license::Tier;
use crate::metrics;
use crate::metrics::arch;
use crate::metrics::rules::RulesConfig;
use serde_json::{json, Value};
use std::collections::{hash_map::DefaultHasher, BTreeSet, HashSet};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::Arc;

const FINDINGS_LIMIT_MAX: usize = 50;
const FINDINGS_CLONE_SUPPORT_LIMIT: usize = 10;
const FINDINGS_DEBT_SUPPORT_LIMIT: usize = 5;

mod agent_format;
mod brief;
mod check;
#[path = "checkpoint.rs"]
mod checkpoint;
mod classification;
mod clone_support;
mod concepts;
#[path = "context.rs"]
mod context;
mod debt;
mod findings;
mod health;
mod scan;
mod semantic_batch;
mod session;
#[cfg(test)]
pub(crate) mod test_support;
mod view_support;

pub(crate) use self::agent_format::{
    actions_from_findings_and_obligations, AgentAction, AgentGate, CheckDiagnostics,
};
pub(crate) use self::brief::agent_brief_def;
pub use self::brief::cli_agent_brief;
pub(crate) use self::check::check_def;
pub(crate) use self::checkpoint::{
    build_v2_confidence_report, changed_scope_from_session_context,
    compatible_session_baseline_status, current_git_head, current_scan_identity,
    current_session_v2_baseline, current_session_v2_baseline_with_status, load_persisted_baseline,
    load_session_v2_baseline_status, overall_confidence_0_10000, project_fingerprint,
    ratio_score_0_10000, save_baseline, save_session_v2_baseline, scan_confidence_0_10000,
    snapshot_file_hashes, working_tree_changed_files,
};
pub(crate) use self::classification::{
    backfill_leverage_fields, build_finding_details, classify_presentation_class,
    combined_other_finding_values, decorate_finding_with_classification, finding_concept_id,
    finding_files, finding_kind, finding_payload_map, finding_values, is_experimental_finding,
    merge_findings, partition_experimental_findings, serialized_values, severity_of_value,
};
pub(crate) use self::clone_support::{
    build_session_introduced_clone_findings, clone_findings_for_health,
    filter_clone_values_by_visible_clone_ids, merge_session_introduced_clone_findings,
    visible_clone_ids, SESSION_INTRODUCED_CLONE_KIND,
};
pub(crate) use self::concepts::{
    check_rules_def, concepts_def, explain_concept_def, project_shape_def, trace_symbol_def,
};
pub(crate) use self::context::{
    apply_root_suppressions, apply_suppressions, invalidate_rules_cache, load_v2_rules_config,
    semantic_cache_status_json, semantic_rules_loaded, suppression_match_count,
    SuppressionApplication,
};
pub(crate) use self::debt::{
    build_debt_report_outputs, clone_family_inspection_focus, insert_debt_report_fields,
};
pub(crate) use self::findings::{
    concentration_def, findings_def, obligations_def, parity_def, state_def,
};
pub(crate) use self::health::health_def;
pub(crate) use self::scan::{rescan_def, scan_def};
pub(crate) use self::semantic_batch::{
    analyze_changed_patch_scope, build_semantic_analysis_batch, semantic_findings_and_obligations,
};
pub(crate) use self::session::prepare_patch_check_context;
pub use self::session::{cli_evaluate_v2_gate, cli_save_v2_session};
pub(crate) use self::session::{gate_def, session_end_def, session_start_def};
pub(crate) use self::view_support::{
    legacy_baseline_delta_json, optional_project_shape_json, project_shape_json_cached,
    project_shape_report_cached, scan_trust_json,
};

#[cfg(test)]
pub(crate) use self::check::handle_check;
#[cfg(test)]
pub(crate) use self::clone_support::build_clone_drift_finding_values;
#[cfg(test)]
pub(crate) use self::concepts::{
    handle_concepts, handle_explain_concept, handle_project_shape, handle_trace_symbol,
};
#[cfg(test)]
pub(crate) use self::findings::handle_concentration;
#[cfg(test)]
pub(crate) use self::findings::handle_findings;
pub(crate) use self::scan::handle_scan;
#[cfg(test)]
pub(crate) use self::session::handle_session_end;
#[cfg(test)]
pub(crate) use self::session::handle_session_start;

// ── Scan helper (shared by scan, rescan, session_end) ──

pub(crate) struct ScanBundle {
    pub snapshot: Snapshot,
    pub metadata: ScanMetadata,
    pub health: metrics::HealthReport,
    pub arch_report: arch::ArchReport,
}

pub(crate) fn do_scan(root: &Path) -> Result<ScanBundle, String> {
    let root_str = root.to_str().ok_or("Invalid path encoding")?;
    let s = crate::core::settings::Settings::default();
    let result = scanner::scan_directory(
        root_str,
        None,
        None,
        &scanner::common::ScanLimits {
            max_file_size_kb: s.max_file_size_kb,
            max_parse_size_kb: s.max_parse_size_kb,
            max_call_targets: s.max_call_targets,
        },
        None, // MCP scans are not cancellable
    )
    .map_err(|e| format!("Scan failed: {e}"))?;
    let arch_report = arch::compute_arch(&result.snapshot);
    let health = metrics::compute_health(&result.snapshot);
    Ok(ScanBundle {
        snapshot: result.snapshot,
        metadata: result.metadata,
        health,
        arch_report,
    })
}

fn do_scan_with_identity(root: &Path) -> Result<(ScanBundle, Option<ScanCacheIdentity>), String> {
    let identity_before = current_scan_identity(root);
    let bundle = do_scan(root)?;
    let identity_after = current_scan_identity(root);
    let scan_identity = match (identity_before, identity_after) {
        (Some(before), Some(after)) if before == after => Some(after),
        _ => None,
    };
    Ok((bundle, scan_identity))
}

fn load_rules_config(root: &Path) -> Result<crate::metrics::rules::RulesConfig, String> {
    let rules_path = root.join(".sentrux").join("rules.toml");
    if !rules_path.exists() {
        return Err(format!(
            "No rules file found at {}/.sentrux/rules.toml. Create one to define architectural constraints.",
            root.display()
        ));
    }
    crate::metrics::rules::RulesConfig::load(&rules_path)
}

fn fresh_mcp_state() -> McpState {
    McpState {
        tier: crate::license::current_tier(),
        scan_root: None,
        cached_snapshot: None,
        cached_scan_metadata: None,
        cached_semantic: None,
        cached_semantic_identity: None,
        cached_semantic_source: None,
        cached_health: None,
        cached_arch: None,
        cached_project_shape: None,
        cached_project_shape_identity: None,
        baseline: None,
        session_v2: None,
        cached_evolution: None,
        cached_scan_identity: None,
        cached_rules_identity: None,
        cached_rules_config: None,
        cached_rules_error: None,
        cached_patch_safety: None,
        semantic_bridge: None,
        agent_session: super::session_telemetry::AgentSessionState::new(),
    }
}

fn analyze_semantic_snapshot(
    state: &mut McpState,
    root: &Path,
) -> Result<Option<SemanticSnapshot>, String> {
    if let Some(semantic) = &state.cached_semantic {
        if state.scan_root.as_deref() == Some(root) && state.cached_semantic_identity.is_none() {
            state.cached_semantic_source = Some(SemanticCacheSource::Memory);
            return Ok(Some(semantic.clone()));
        }
    }

    let project = crate::analysis::semantic::discover_project(root)
        .map_err(|error| format!("Semantic project discovery failed: {error}"))?;
    if project.primary_language.as_deref() != Some("typescript")
        || project.tsconfig_paths.is_empty()
    {
        return Ok(None);
    }
    let cache_identity = current_semantic_identity(root, &project);

    if let Some(semantic) = &state.cached_semantic {
        if state.cached_semantic_identity.as_ref() == cache_identity.as_ref() {
            state.cached_semantic_source = Some(SemanticCacheSource::Memory);
            return Ok(Some(semantic.clone()));
        }
    }

    if let Some(identity) = cache_identity.as_ref() {
        if let Ok(Some(snapshot)) = load_persisted_semantic_snapshot(root, identity) {
            state.cached_semantic = Some(snapshot.clone());
            state.cached_semantic_identity = Some(identity.clone());
            state.cached_semantic_source = Some(SemanticCacheSource::Disk);
            return Ok(Some(snapshot));
        }
    }

    let bridge = state
        .semantic_bridge
        .get_or_insert_with(crate::app::bridge::TypeScriptBridgeSupervisor::with_default_config);
    let semantic = bridge
        .analyze_project(&project)
        .map_err(|error| format!("Semantic analysis unavailable: {error}"))?;
    state.cached_semantic = Some(semantic.clone());
    state.cached_semantic_identity = cache_identity.clone();
    state.cached_semantic_source = Some(SemanticCacheSource::Bridge);
    if let Some(identity) = cache_identity.as_ref() {
        let _ = save_persisted_semantic_snapshot(root, identity, &semantic);
    }

    Ok(Some(semantic))
}

fn current_semantic_identity(
    root: &Path,
    project: &crate::analysis::semantic::ProjectModel,
) -> Option<SemanticCacheIdentity> {
    let scan_identity = current_scan_identity(root)?;
    Some(current_semantic_cache_identity(
        project,
        scan_identity.git_head,
        scan_identity.working_tree_paths,
        scan_identity.working_tree_hashes,
    ))
}

fn concentration_history(
    state: &mut McpState,
    root: &Path,
    lookback_days: Option<u32>,
    allow_compute: bool,
) -> (
    Option<crate::metrics::v2::ConcentrationHistory>,
    Option<String>,
) {
    if lookback_days.is_none() {
        if let Some(report) = &state.cached_evolution {
            return (
                Some(crate::metrics::v2::ConcentrationHistory::from(report)),
                None,
            );
        }
    }

    if !allow_compute {
        return (
            None,
            Some("Evolution context unavailable on fast path.".to_string()),
        );
    }

    let (known_files, complexity_map) = match state.cached_snapshot.as_ref() {
        Some(snapshot) => (
            crate::app::mcp_server::handlers_evo::build_known_files(snapshot),
            crate::app::mcp_server::handlers_evo::build_complexity_map(snapshot),
        ),
        None => return (None, Some("No scan data. Call 'scan' first.".to_string())),
    };

    match crate::metrics::evolution::compute_evolution(
        root,
        &known_files,
        &complexity_map,
        lookback_days,
    ) {
        Ok(report) => {
            let history = crate::metrics::v2::ConcentrationHistory::from(&report);
            if lookback_days.is_none() {
                state.cached_evolution = Some(report);
            }
            (Some(history), None)
        }
        Err(error) => (
            None,
            Some(format!("Evolution context unavailable: {error}")),
        ),
    }
}

fn evolution_report_for_snapshot(
    state: &mut McpState,
    root: &Path,
    snapshot: &Snapshot,
    allow_compute: bool,
) -> (Option<crate::metrics::evo::EvolutionReport>, Option<String>) {
    if let Some(report) = &state.cached_evolution {
        return (Some(report.clone()), None);
    }

    if !allow_compute {
        return (
            None,
            Some("Evolution context unavailable on fast path.".to_string()),
        );
    }

    let known_files = crate::app::mcp_server::handlers_evo::build_known_files(snapshot);
    let complexity_map = crate::app::mcp_server::handlers_evo::build_complexity_map(snapshot);

    match crate::metrics::evolution::compute_evolution(root, &known_files, &complexity_map, None) {
        Ok(report) => {
            state.cached_evolution = Some(report.clone());
            (Some(report), None)
        }
        Err(error) => (
            None,
            Some(format!("Clone drift context unavailable: {error}")),
        ),
    }
}

fn update_scan_cache(
    state: &mut McpState,
    root: PathBuf,
    bundle: ScanBundle,
    baseline: Option<arch::ArchBaseline>,
    identity: Option<ScanCacheIdentity>,
) {
    let root_changed = state
        .scan_root
        .as_ref()
        .map(|existing_root| existing_root != &root)
        .unwrap_or(false);
    if root_changed {
        state.session_v2 = None;
        invalidate_rules_cache(state);
    }
    state.baseline = baseline;
    state.scan_root = Some(root);
    state.cached_snapshot = Some(Arc::new(bundle.snapshot));
    state.cached_scan_metadata = Some(bundle.metadata);
    state.cached_semantic = None;
    state.cached_semantic_identity = None;
    state.cached_semantic_source = None;
    state.cached_health = Some(bundle.health);
    state.cached_arch = Some(bundle.arch_report);
    state.cached_project_shape = None;
    state.cached_project_shape_identity = None;
    state.cached_evolution = None;
    state.cached_scan_identity = identity;
    state.cached_patch_safety = None;
}

fn cached_scan_bundle(state: &McpState, root: &Path) -> Option<ScanBundle> {
    if state.scan_root.as_deref() != Some(root) {
        return None;
    }

    Some(ScanBundle {
        snapshot: (*state.cached_snapshot.as_ref()?).as_ref().clone(),
        metadata: state.cached_scan_metadata.clone()?,
        health: state.cached_health.clone()?,
        arch_report: state.cached_arch.clone()?,
    })
}

fn scan_cache_matches_identity(state: &McpState, identity: Option<&ScanCacheIdentity>) -> bool {
    state.cached_scan_identity.as_ref() == identity
}
fn merge_optional_errors(left: Option<String>, right: Option<String>) -> Option<String> {
    match (left, right) {
        (Some(left), Some(right)) => Some(format!("{left}; {right}")),
        (Some(left), None) => Some(left),
        (None, Some(right)) => Some(right),
        (None, None) => None,
    }
}

pub(crate) fn session_v2_analysis_signature(session_v2: Option<&SessionV2Baseline>) -> Option<u64> {
    let session_v2 = session_v2?;
    let mut hasher = DefaultHasher::new();
    session_v2.schema_version.hash(&mut hasher);
    session_v2.project_fingerprint.hash(&mut hasher);
    session_v2.sentrux_version.hash(&mut hasher);
    session_v2.git_head.hash(&mut hasher);
    for path in &session_v2.working_tree_paths {
        path.hash(&mut hasher);
    }
    for (path, file_hash) in &session_v2.file_hashes {
        path.hash(&mut hasher);
        file_hash.hash(&mut hasher);
    }
    Some(hasher.finish())
}

fn build_session_v2_baseline(
    state: &mut McpState,
    root: &Path,
    snapshot: &Snapshot,
    health: &metrics::HealthReport,
    metadata: &ScanMetadata,
) -> (SessionV2Baseline, SuppressionApplication, Option<String>) {
    let file_hashes = snapshot_file_hashes(root, snapshot);
    let git_head = current_git_head(root);
    let working_tree_paths = working_tree_changed_files(root).unwrap_or_default();
    let (clone_payload, clone_error) = clone_findings_for_health(
        state,
        root,
        snapshot,
        health,
        health.duplicate_groups.len(),
        true,
    );
    let (semantic_findings, _, semantic_error) = semantic_findings_and_obligations(
        state,
        root,
        Some(snapshot),
        crate::metrics::v2::ObligationScope::All,
        &BTreeSet::new(),
    );
    let structural_reports =
        crate::metrics::v2::build_structural_debt_reports_with_root(root, snapshot, health);
    let all_finding_values = combined_other_finding_values(&semantic_findings, &structural_reports);
    let (config, _) = load_v2_rules_config(state, root);
    let suppression_application = apply_suppressions(
        &config,
        finding_values(&clone_payload.exact_findings, &all_finding_values),
    );
    let finding_payloads = finding_payload_map(&suppression_application.visible_findings);
    let confidence = SessionV2ConfidenceSnapshot {
        scan_confidence_0_10000: Some(scan_confidence_0_10000(metadata)),
        rule_coverage_0_10000: Some(config.v2_rule_coverage().coverage_0_10000),
    };

    (
        SessionV2Baseline {
            schema_version: SESSION_V2_SCHEMA_VERSION,
            project_fingerprint: Some(project_fingerprint(root)),
            sentrux_version: Some(env!("CARGO_PKG_VERSION").to_string()),
            file_hashes,
            finding_payloads,
            git_head,
            working_tree_paths,
            confidence,
        },
        suppression_application,
        merge_optional_errors(semantic_error, clone_error),
    )
}

fn is_internal_sentrux_path(path: &str) -> bool {
    path == ".sentrux"
        || path.starts_with(".sentrux/")
        || path == ".sentrux\\"
        || path.starts_with(".sentrux\\")
}

fn debt_signal_concentration_reports(
    state: &mut McpState,
    root: &Path,
    snapshot: &Snapshot,
    candidate_files: &BTreeSet<String>,
    allow_cold_evolution: bool,
) -> (Vec<crate::metrics::v2::ConcentrationReport>, Option<String>) {
    if candidate_files.is_empty() {
        return (Vec::new(), None);
    }

    let complexity_map = crate::app::mcp_server::handlers_evo::build_complexity_map(snapshot);
    let (config, rules_error) = load_v2_rules_config(state, root);
    let (semantic, semantic_error) = match analyze_semantic_snapshot(state, root) {
        Ok(semantic) => (semantic, None),
        Err(error) => (None, Some(error)),
    };
    let (history, evolution_error) = concentration_history(state, root, None, allow_cold_evolution);
    let concentration_result = crate::metrics::v2::build_concentration_reports(
        root,
        candidate_files,
        &complexity_map,
        &config,
        semantic.as_ref(),
        history.as_ref(),
    );
    let reports = concentration_result.reports;
    let findings = crate::metrics::v2::build_concentration_findings(&reports, reports.len());
    let suppression_application = apply_suppressions(&config, serialized_values(&findings));
    let visible_paths = suppression_application
        .visible_findings
        .iter()
        .flat_map(finding_files)
        .collect::<BTreeSet<_>>();
    let visible_reports = reports
        .into_iter()
        .filter(|report| visible_paths.contains(&report.path))
        .collect::<Vec<_>>();

    (
        visible_reports,
        merge_optional_errors(
            merge_optional_errors(rules_error, semantic_error),
            merge_optional_errors(
                evolution_error,
                (!concentration_result.read_warnings.is_empty())
                    .then(|| concentration_result.read_warnings.join(" | ")),
            ),
        ),
    )
}

#[cfg(test)]
fn build_exact_clone_findings(
    groups: &[crate::metrics::DuplicateGroup],
    limit: usize,
) -> Vec<Value> {
    build_clone_drift_finding_values(groups, None, limit)
}

#[cfg(test)]
fn distinct_file_count(group: &crate::metrics::DuplicateGroup) -> usize {
    use std::collections::HashSet;

    group
        .instances
        .iter()
        .map(|(file, _, _)| file.as_str())
        .collect::<HashSet<_>>()
        .len()
}

#[cfg(test)]
mod brief_tests;
#[cfg(test)]
mod check_tests;
#[cfg(test)]
mod concepts_tests;
#[cfg(test)]
mod findings_tests;
#[cfg(test)]
mod session_tests;
