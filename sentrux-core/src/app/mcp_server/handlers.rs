//! MCP tool handler implementations — core tools.
//!
//! Each handler has the uniform signature: `fn(&Value, &Tier, &mut McpState) -> Result<Value, String>`
//! Each tool also has a `_def()` function returning its `ToolDef` (schema + tier + handler co-located).
//!
//! Tier-aware truncation: detail lists are limited to `tier.detail_limit()` items.
//! Free users see top-3 + total counts. Pro users see everything.

use super::registry::ToolDef;
use super::{
    session_v2_schema_supported, McpState, PatchSafetyAnalysisCache, RulesCacheIdentity,
    ScanCacheIdentity, SessionV2Baseline, SessionV2ConfidenceSnapshot, SESSION_V2_SCHEMA_VERSION,
};
use crate::analysis::project_shape::{
    detect_project_shape, render_starter_rules, ProjectShapeReport,
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
use std::collections::{hash_map::DefaultHasher, BTreeMap, BTreeSet, HashSet};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use time::format_description::FormatItem;
use time::macros::format_description;
use time::{Date, OffsetDateTime};

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

fn scan_trust_json(metadata: &ScanMetadata) -> Value {
    let scope_coverage = ratio_score_0_10000(metadata.kept_files, metadata.candidate_files);
    let resolution_confidence = ratio_score_0_10000(
        metadata.resolution.resolved,
        metadata.resolution.resolved + metadata.resolution.unresolved_internal,
    );
    let overall_confidence =
        overall_confidence_0_10000(metadata, scope_coverage, resolution_confidence);
    json!({
        "mode": metadata.mode.as_str(),
        "fallback_reason": metadata.fallback_reason,
        "candidate_files": metadata.candidate_files,
        "tracked_candidates": metadata.tracked_candidates,
        "untracked_candidates": metadata.untracked_candidates,
        "kept_files": metadata.kept_files,
        "scope_coverage_0_10000": scope_coverage,
        "overall_confidence_0_10000": overall_confidence,
        "partial": metadata.partial,
        "truncated": metadata.truncated,
        "exclusions": {
            "total": metadata.exclusions.total(),
            "bucketed": {
                "vendor": metadata.exclusions.bucketed.vendor,
                "generated": metadata.exclusions.bucketed.generated,
                "build": metadata.exclusions.bucketed.build,
                "fixture": metadata.exclusions.bucketed.fixture,
                "cache": metadata.exclusions.bucketed.cache,
            },
            "ignored_extension": metadata.exclusions.ignored_extension,
            "too_large": metadata.exclusions.too_large,
            "metadata_error": metadata.exclusions.metadata_error,
        },
        "resolution": {
            "resolved": metadata.resolution.resolved,
            "unresolved_internal": metadata.resolution.unresolved_internal,
            "unresolved_external": metadata.resolution.unresolved_external,
            "unresolved_unknown": metadata.resolution.unresolved_unknown,
            "internal_confidence_0_10000": resolution_confidence,
        },
    })
}

fn snapshot_file_paths(snapshot: &Snapshot) -> Vec<String> {
    crate::core::snapshot::flatten_files_ref(snapshot.root.as_ref())
        .into_iter()
        .filter(|file| !file.is_dir)
        .map(|file| file.path.clone())
        .collect()
}

fn project_shape_report(root: &Path, snapshot: &Snapshot, config: &RulesConfig) -> ProjectShapeReport {
    let workspace_files = crate::analysis::semantic::discover_project(root)
        .map(|project| project.workspace_files)
        .unwrap_or_default();
    let file_paths = snapshot_file_paths(snapshot);
    detect_project_shape(
        Some(root),
        &file_paths,
        &workspace_files,
        &config.project.archetypes,
    )
}

fn project_shape_json(root: &Path, snapshot: &Snapshot, config: &RulesConfig) -> Value {
    let shape = project_shape_report(root, snapshot, config);
    json!({
        "primary_archetype": shape.primary_archetype,
        "configured_archetypes": shape.configured_archetypes,
        "detected_archetypes": shape.detected_archetypes,
        "effective_archetypes": shape.effective_archetypes,
        "capabilities": shape.capabilities,
        "boundary_roots": shape.boundary_roots,
        "module_contracts": shape.module_contracts,
        "starter_rules_toml": render_starter_rules(
            &shape,
            config.project.primary_language.as_deref(),
            &config.project.exclude,
        ),
    })
}

fn optional_project_shape_json(root: &Path, snapshot: Option<&Snapshot>, config: &RulesConfig) -> Value {
    let Some(snapshot) = snapshot else {
        return json!({
            "available": false,
            "error": "No scan data. Call 'scan' first.",
        });
    };
    project_shape_json(root, snapshot, config)
}

#[derive(Debug, Clone, serde::Serialize)]
struct SessionBaselineStatus {
    loaded: bool,
    compatible: bool,
    schema_version: Option<u32>,
    error: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
struct V2ConfidenceReport {
    scan_confidence_0_10000: u32,
    rule_coverage_0_10000: u32,
    semantic_rules_loaded: bool,
    session_baseline: SessionBaselineStatus,
}

fn missing_session_baseline_status() -> SessionBaselineStatus {
    SessionBaselineStatus {
        loaded: false,
        compatible: false,
        schema_version: None,
        error: None,
    }
}

fn compatible_session_baseline_status(schema_version: u32) -> SessionBaselineStatus {
    SessionBaselineStatus {
        loaded: true,
        compatible: true,
        schema_version: Some(schema_version),
        error: None,
    }
}

fn incompatible_session_baseline_status(
    schema_version: Option<u32>,
    error: String,
) -> SessionBaselineStatus {
    SessionBaselineStatus {
        loaded: true,
        compatible: false,
        schema_version,
        error: Some(error),
    }
}

fn project_mismatch_session_baseline_status(
    schema_version: u32,
    baseline_fingerprint: &str,
    current_fingerprint: &str,
) -> SessionBaselineStatus {
    incompatible_session_baseline_status(
        Some(schema_version),
        format!(
            "Session baseline project fingerprint {baseline_fingerprint} does not match current project fingerprint {current_fingerprint}"
        ),
    )
}

fn project_mismatch_status(
    baseline: &SessionV2Baseline,
    current_fingerprint: &str,
) -> Option<SessionBaselineStatus> {
    let baseline_fingerprint = baseline.project_fingerprint.as_deref()?;
    (baseline_fingerprint != current_fingerprint).then(|| {
        project_mismatch_session_baseline_status(
            baseline.schema_version,
            baseline_fingerprint,
            current_fingerprint,
        )
    })
}

fn ratio_score_0_10000(numerator: usize, denominator: usize) -> u32 {
    if denominator == 0 {
        return 10000;
    }
    ((numerator as f64 / denominator as f64) * 10000.0).round() as u32
}

fn overall_confidence_0_10000(
    metadata: &ScanMetadata,
    scope_coverage: u32,
    resolution_confidence: u32,
) -> u32 {
    let mut score = scope_coverage.min(resolution_confidence);
    if metadata.partial {
        score = score.saturating_mul(8) / 10;
    }
    if metadata.truncated {
        score = score.saturating_mul(7) / 10;
    }
    if metadata.fallback_reason.is_some() {
        score = score.saturating_mul(9) / 10;
    }
    score
}

fn scan_confidence_0_10000(metadata: &ScanMetadata) -> u32 {
    let scope_coverage = ratio_score_0_10000(metadata.kept_files, metadata.candidate_files);
    let resolution_confidence = ratio_score_0_10000(
        metadata.resolution.resolved,
        metadata.resolution.resolved + metadata.resolution.unresolved_internal,
    );
    overall_confidence_0_10000(metadata, scope_coverage, resolution_confidence)
}

fn build_v2_confidence_report(
    metadata: &ScanMetadata,
    config: &RulesConfig,
    session_baseline: SessionBaselineStatus,
) -> V2ConfidenceReport {
    V2ConfidenceReport {
        scan_confidence_0_10000: scan_confidence_0_10000(metadata),
        rule_coverage_0_10000: config.v2_rule_coverage().coverage_0_10000,
        semantic_rules_loaded: semantic_rules_loaded(config),
        session_baseline,
    }
}

fn legacy_baseline_delta_json(diff: Option<&arch::ArchDiff>) -> Value {
    match diff {
        Some(diff) => json!({
            "available": true,
            "signal_before": ((diff.signal_before * 10000.0).round() as i32),
            "signal_after": ((diff.signal_after * 10000.0).round() as i32),
            "signal_delta": (((diff.signal_after - diff.signal_before) * 10000.0).round() as i32),
            "cycles_before": diff.cycles_before,
            "cycles_after": diff.cycles_after,
            "coupling_before": diff.coupling_before,
            "coupling_after": diff.coupling_after,
            "degraded": diff.degraded,
        }),
        None => json!({
            "available": false,
        }),
    }
}

fn load_persisted_baseline(root: &Path) -> Result<Option<arch::ArchBaseline>, String> {
    let baseline_path = arch::baseline_path(root);
    if !baseline_path.exists() {
        return Ok(None);
    }
    arch::ArchBaseline::load(&baseline_path).map(Some)
}

fn session_v2_baseline_path(root: &Path) -> PathBuf {
    root.join(".sentrux").join("session-v2.json")
}

fn load_session_v2_baseline_status(
    root: &Path,
) -> (Option<SessionV2Baseline>, SessionBaselineStatus) {
    let baseline_path = session_v2_baseline_path(root);
    if !baseline_path.exists() {
        return (None, missing_session_baseline_status());
    }
    let bytes = match std::fs::read(&baseline_path) {
        Ok(bytes) => bytes,
        Err(error) => {
            return (
                None,
                incompatible_session_baseline_status(
                    None,
                    format!("Failed to read {}: {error}", baseline_path.display()),
                ),
            )
        }
    };
    let baseline: SessionV2Baseline = match serde_json::from_slice(&bytes) {
        Ok(baseline) => baseline,
        Err(error) => {
            return (
                None,
                incompatible_session_baseline_status(
                    None,
                    format!("Failed to parse {}: {error}", baseline_path.display()),
                ),
            )
        }
    };
    let schema_version = baseline.schema_version;
    if !session_v2_schema_supported(schema_version) {
        return (
            None,
            incompatible_session_baseline_status(
                Some(schema_version),
                format!(
                    "Unsupported v2 session baseline schema version {schema_version}; supported range is {}-{}",
                    super::MIN_SUPPORTED_SESSION_V2_SCHEMA_VERSION,
                    SESSION_V2_SCHEMA_VERSION
                ),
            ),
        );
    }

    (
        Some(baseline),
        compatible_session_baseline_status(schema_version),
    )
}

#[cfg(test)]
fn load_persisted_session_v2(root: &Path) -> Result<Option<SessionV2Baseline>, String> {
    Ok(load_session_v2_baseline_status(root).0)
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

fn empty_rules_config() -> crate::metrics::rules::RulesConfig {
    crate::metrics::rules::RulesConfig {
        project: Default::default(),
        constraints: Default::default(),
        language: Default::default(),
        layers: Vec::new(),
        boundaries: Vec::new(),
        concept: Vec::new(),
        contract: Vec::new(),
        state_model: Vec::new(),
        module_contract: Vec::new(),
        suppress: Vec::new(),
    }
}

fn current_rules_cache_identity(root: &Path) -> RulesCacheIdentity {
    let rules_path = root.join(".sentrux").join("rules.toml");
    let metadata = std::fs::metadata(&rules_path).ok();

    RulesCacheIdentity {
        rules_path,
        exists: metadata.is_some(),
        len: metadata.as_ref().map(std::fs::Metadata::len),
        modified_unix_nanos: metadata
            .and_then(|metadata| metadata.modified().ok())
            .and_then(|modified| modified.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|duration| duration.as_nanos()),
    }
}

fn load_v2_rules_config(
    state: &mut McpState,
    root: &Path,
) -> (crate::metrics::rules::RulesConfig, Option<String>) {
    let identity = current_rules_cache_identity(root);
    if state.cached_rules_identity.as_ref() == Some(&identity) {
        return (
            state
                .cached_rules_config
                .clone()
                .unwrap_or_else(empty_rules_config),
            state.cached_rules_error.clone(),
        );
    }

    let (config, error) = if !identity.exists {
        (empty_rules_config(), None)
    } else {
        match crate::metrics::rules::RulesConfig::load(&identity.rules_path) {
            Ok(config) => (config, None),
            Err(error) => (empty_rules_config(), Some(error)),
        }
    };

    state.cached_rules_identity = Some(identity);
    state.cached_rules_config = Some(config.clone());
    state.cached_rules_error = error.clone();

    (config, error)
}

fn invalidate_rules_cache(state: &mut McpState) {
    if state.cached_rules_identity.is_none()
        && state.cached_rules_config.is_none()
        && state.cached_rules_error.is_none()
    {
        return;
    }

    state.cached_rules_identity = None;
    state.cached_rules_config = None;
    state.cached_rules_error = None;
}

fn semantic_rules_loaded(config: &RulesConfig) -> bool {
    !config.concept.is_empty() || !config.contract.is_empty() || !config.state_model.is_empty()
}

fn save_baseline(root: &Path, baseline: &arch::ArchBaseline) -> Result<std::path::PathBuf, String> {
    let baseline_path = arch::baseline_path(root);
    if let Some(parent) = baseline_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            format!(
                "Failed to create baseline directory {}: {e}",
                parent.display()
            )
        })?;
    }
    baseline.save(&baseline_path)?;
    Ok(baseline_path)
}

fn save_session_v2_baseline(
    root: &Path,
    baseline: &SessionV2Baseline,
) -> Result<std::path::PathBuf, String> {
    let baseline_path = session_v2_baseline_path(root);
    if let Some(parent) = baseline_path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| {
            format!(
                "Failed to create session baseline directory {}: {error}",
                parent.display()
            )
        })?;
    }
    let payload = serde_json::to_vec_pretty(baseline)
        .map_err(|error| format!("Failed to serialize session baseline: {error}"))?;
    std::fs::write(&baseline_path, payload)
        .map_err(|error| format!("Failed to write {}: {error}", baseline_path.display()))?;
    Ok(baseline_path)
}

fn current_session_v2_baseline(
    state: &mut McpState,
    root: &Path,
) -> Result<Option<SessionV2Baseline>, String> {
    current_session_v2_baseline_with_status(state, root).map(|(baseline, _)| baseline)
}

fn current_session_v2_baseline_with_status(
    state: &mut McpState,
    root: &Path,
) -> Result<(Option<SessionV2Baseline>, SessionBaselineStatus), String> {
    let current_fingerprint = project_fingerprint(root);
    if let Some(session_v2) = &state.session_v2 {
        if let Some(status) = project_mismatch_status(session_v2, &current_fingerprint) {
            return Ok((None, status));
        }
        return Ok((
            Some(session_v2.clone()),
            compatible_session_baseline_status(session_v2.schema_version),
        ));
    }

    let (session_v2, status) = load_session_v2_baseline_status(root);
    if let Some(session_v2) = &session_v2 {
        if let Some(status) = project_mismatch_status(session_v2, &current_fingerprint) {
            return Ok((None, status));
        }
        state.session_v2 = Some(session_v2.clone());
    }
    Ok((session_v2, status))
}

fn fresh_mcp_state() -> McpState {
    McpState {
        tier: crate::license::current_tier(),
        scan_root: None,
        cached_snapshot: None,
        cached_scan_metadata: None,
        cached_semantic: None,
        cached_health: None,
        cached_arch: None,
        baseline: None,
        session_v2: None,
        cached_evolution: None,
        cached_scan_identity: None,
        cached_rules_identity: None,
        cached_rules_config: None,
        cached_rules_error: None,
        cached_patch_safety: None,
        semantic_bridge: None,
    }
}

fn refresh_changed_scope(state: &mut McpState, root: &Path) -> Result<BTreeSet<String>, String> {
    let session_v2 = current_session_v2_baseline(state, root)?;
    let context = prepare_patch_check_context(state, root, session_v2.as_ref())?;
    let changed_files = context.changed_files.clone();
    let persisted_baseline = load_persisted_baseline(root).ok().flatten();
    if !context.reused_cached_scan {
        update_scan_cache(
            state,
            root.to_path_buf(),
            context.bundle,
            persisted_baseline.or(state.baseline.clone()),
            context.scan_identity,
        );
    } else if persisted_baseline.is_some() {
        state.baseline = persisted_baseline;
    }
    Ok(changed_files)
}

fn analyze_semantic_snapshot(
    state: &mut McpState,
    root: &Path,
) -> Result<Option<SemanticSnapshot>, String> {
    if let Some(semantic) = &state.cached_semantic {
        return Ok(Some(semantic.clone()));
    }

    let project = crate::analysis::semantic::discover_project(root)
        .map_err(|error| format!("Semantic project discovery failed: {error}"))?;
    if project.primary_language.as_deref() != Some("typescript")
        || project.tsconfig_paths.is_empty()
    {
        return Ok(None);
    }

    let bridge = state
        .semantic_bridge
        .get_or_insert_with(crate::app::bridge::TypeScriptBridgeSupervisor::with_default_config);
    let semantic = bridge
        .analyze_project(&project)
        .map_err(|error| format!("Semantic analysis unavailable: {error}"))?;
    state.cached_semantic = Some(semantic.clone());

    Ok(Some(semantic))
}

fn concentration_history(
    state: &mut McpState,
    root: &Path,
    lookback_days: Option<u32>,
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
) -> (Option<crate::metrics::evo::EvolutionReport>, Option<String>) {
    if let Some(report) = &state.cached_evolution {
        return (Some(report.clone()), None);
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
    state.cached_health = Some(bundle.health);
    state.cached_arch = Some(bundle.arch_report);
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

struct PatchCheckContext {
    bundle: ScanBundle,
    changed_files: BTreeSet<String>,
    reused_cached_scan: bool,
    scan_identity: Option<ScanCacheIdentity>,
}

fn prepare_patch_check_context(
    state: &McpState,
    root: &Path,
    session_v2: Option<&SessionV2Baseline>,
) -> Result<PatchCheckContext, String> {
    if let Some(bundle) = cached_scan_bundle(state, root) {
        let current_identity = current_scan_identity(root);
        let changed_files = changed_files_from_session_context(
            root,
            &bundle.snapshot,
            session_v2,
            current_identity.as_ref(),
        );
        if changed_files.is_empty() || scan_cache_matches_identity(state, current_identity.as_ref())
        {
            return Ok(PatchCheckContext {
                bundle,
                changed_files,
                reused_cached_scan: true,
                scan_identity: None,
            });
        }
    }

    let (bundle, scan_identity) = do_scan_with_identity(root)?;
    let changed_files =
        changed_files_from_session_context(root, &bundle.snapshot, session_v2, None);

    Ok(PatchCheckContext {
        bundle,
        changed_files,
        reused_cached_scan: false,
        scan_identity,
    })
}

fn scan_cache_matches_identity(state: &McpState, identity: Option<&ScanCacheIdentity>) -> bool {
    state.cached_scan_identity.as_ref() == identity
}

fn current_scan_identity(root: &Path) -> Option<ScanCacheIdentity> {
    let working_tree_paths = working_tree_changed_files(root)?;
    Some(ScanCacheIdentity {
        git_head: current_git_head(root),
        working_tree_hashes: file_hashes_for_paths(root, &working_tree_paths),
        working_tree_paths,
    })
}

fn git_output(root: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    String::from_utf8(output.stdout).ok()
}

fn trimmed_git_output(root: &Path, args: &[&str]) -> Option<String> {
    let output = git_output(root, args)?;
    let output = output.trim();
    if output.is_empty() {
        return None;
    }
    Some(output.to_string())
}

fn working_tree_changed_files(root: &Path) -> Option<BTreeSet<String>> {
    let stdout = git_output(root, &["status", "--porcelain", "--untracked-files=all"])?;
    Some(
        stdout
            .lines()
            .flat_map(parse_porcelain_paths)
            .filter(|path| !is_internal_sentrux_path(path))
            .map(|path| path.replace('\\', "/"))
            .collect(),
    )
}

fn parse_porcelain_paths(line: &str) -> Vec<String> {
    if line.len() < 4 {
        return Vec::new();
    }
    let Some(path) = line.get(3..) else {
        return Vec::new();
    };
    let path = path.trim();
    if path.is_empty() {
        return Vec::new();
    }
    if let Some((old_path, renamed_to)) = path.split_once(" -> ") {
        return vec![old_path.to_string(), renamed_to.to_string()];
    }
    vec![path.to_string()]
}

fn current_git_head(root: &Path) -> Option<String> {
    trimmed_git_output(root, &["rev-parse", "HEAD"])
}

fn git_root_commit(root: &Path) -> Option<String> {
    let roots = git_output(root, &["rev-list", "--max-parents=0", "HEAD"])?;
    roots
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(str::to_string)
}

fn git_origin_url(root: &Path) -> Option<String> {
    trimmed_git_output(root, &["config", "--get", "remote.origin.url"])
        .map(|origin| origin.replace('\\', "/"))
}

fn project_fingerprint(root: &Path) -> String {
    let fingerprint_source = if let Some(root_commit) = git_root_commit(root) {
        format!("git-root:{root_commit}")
    } else if let Some(origin_url) = git_origin_url(root) {
        format!("git-origin:{origin_url}")
    } else {
        let normalized_root = root
            .canonicalize()
            .unwrap_or_else(|_| root.to_path_buf())
            .to_string_lossy()
            .replace('\\', "/");
        format!("path:{normalized_root}")
    };
    let mut hasher = DefaultHasher::new();
    fingerprint_source.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn diff_paths_between_heads(
    root: &Path,
    baseline_head: &str,
    current_head: &str,
) -> Option<BTreeSet<String>> {
    let range = format!("{baseline_head}..{current_head}");
    let stdout = git_output(
        root,
        &[
            "diff",
            "--name-status",
            "--find-renames",
            "--find-copies",
            &range,
        ],
    )?;
    Some(
        stdout
            .lines()
            .flat_map(parse_name_status_paths)
            .filter(|path| !is_internal_sentrux_path(path))
            .map(|path| path.replace('\\', "/"))
            .collect(),
    )
}

fn parse_name_status_paths(line: &str) -> Vec<String> {
    let mut parts = line.split('\t');
    let Some(status) = parts.next().map(str::trim) else {
        return Vec::new();
    };
    if status.is_empty() {
        return Vec::new();
    }

    if status.starts_with('R') || status.starts_with('C') {
        let old_path = parts.next().unwrap_or_default().trim();
        let new_path = parts.next().unwrap_or_default().trim();
        return [old_path, new_path]
            .into_iter()
            .filter(|path| !path.is_empty())
            .map(str::to_string)
            .collect();
    }

    parts
        .next()
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .map(|path| vec![path.to_string()])
        .unwrap_or_default()
}

fn snapshot_file_hashes(root: &Path, snapshot: &Snapshot) -> BTreeMap<String, u64> {
    snapshot_file_hashes_for_paths(root, snapshot, &scanned_file_paths(snapshot))
}

fn snapshot_file_hashes_for_paths(
    root: &Path,
    snapshot: &Snapshot,
    paths: &BTreeSet<String>,
) -> BTreeMap<String, u64> {
    let scanned_paths = scanned_file_paths(snapshot);
    let eligible_paths = paths
        .iter()
        .filter(|path| scanned_paths.contains(*path))
        .filter(|path| !is_internal_sentrux_path(path))
        .cloned()
        .collect::<BTreeSet<_>>();
    file_hashes_for_paths(root, &eligible_paths)
}

fn file_hashes_for_paths(root: &Path, paths: &BTreeSet<String>) -> BTreeMap<String, u64> {
    paths
        .iter()
        .filter(|path| !is_internal_sentrux_path(path))
        .filter_map(|path| {
            let absolute_path = root.join(path);
            let bytes = std::fs::read(&absolute_path).ok()?;
            Some((path.clone(), stable_hash(&bytes)))
        })
        .collect()
}

fn diff_file_hashes(
    baseline_hashes: &BTreeMap<String, u64>,
    current_hashes: &BTreeMap<String, u64>,
) -> BTreeSet<String> {
    let candidate_paths = baseline_hashes
        .keys()
        .chain(current_hashes.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    diff_file_hashes_for_paths(baseline_hashes, current_hashes, &candidate_paths)
}

fn diff_file_hashes_for_paths(
    baseline_hashes: &BTreeMap<String, u64>,
    current_hashes: &BTreeMap<String, u64>,
    candidate_paths: &BTreeSet<String>,
) -> BTreeSet<String> {
    candidate_paths
        .iter()
        .filter(|path| baseline_hashes.get(*path) != current_hashes.get(*path))
        .cloned()
        .collect()
}

fn scanned_file_paths(snapshot: &Snapshot) -> BTreeSet<String> {
    crate::core::snapshot::flatten_files_ref(snapshot.root.as_ref())
        .into_iter()
        .map(|file| file.path.clone())
        .collect()
}

fn filter_changed_files_to_snapshot(
    changed_files: BTreeSet<String>,
    snapshot: &Snapshot,
) -> BTreeSet<String> {
    let scanned_paths = scanned_file_paths(snapshot);
    changed_files
        .into_iter()
        .filter(|path| scanned_paths.contains(path))
        .collect()
}

fn changed_files_from_session_context(
    root: &Path,
    snapshot: &Snapshot,
    session_v2: Option<&SessionV2Baseline>,
    current_identity: Option<&ScanCacheIdentity>,
) -> BTreeSet<String> {
    match session_v2 {
        Some(session_v2) => {
            if let Some(candidate_paths) =
                changed_session_candidate_paths(root, snapshot, session_v2, current_identity)
            {
                if candidate_paths.is_empty() {
                    return BTreeSet::new();
                }
                let current_file_hashes =
                    snapshot_file_hashes_for_paths(root, snapshot, &candidate_paths);
                return diff_file_hashes_for_paths(
                    &session_v2.file_hashes,
                    &current_file_hashes,
                    &candidate_paths,
                );
            }

            let current_file_hashes = snapshot_file_hashes(root, snapshot);
            diff_file_hashes(&session_v2.file_hashes, &current_file_hashes)
        }
        None => current_identity
            .map(|identity| identity.working_tree_paths.clone())
            .or_else(|| working_tree_changed_files(root))
            .map(|changed_files| filter_changed_files_to_snapshot(changed_files, snapshot))
            .unwrap_or_default(),
    }
}

fn changed_session_candidate_paths(
    root: &Path,
    snapshot: &Snapshot,
    session_v2: &SessionV2Baseline,
    current_identity: Option<&ScanCacheIdentity>,
) -> Option<BTreeSet<String>> {
    let current_working_tree_paths = current_identity
        .map(|identity| identity.working_tree_paths.clone())
        .or_else(|| working_tree_changed_files(root))?;
    let mut candidate_paths = session_v2
        .working_tree_paths
        .union(&current_working_tree_paths)
        .cloned()
        .collect::<BTreeSet<_>>();

    let current_head = current_identity.and_then(|identity| identity.git_head.clone());
    match (session_v2.git_head.as_deref(), current_head.as_deref()) {
        (Some(baseline_head), Some(current_head)) if baseline_head != current_head => {
            let committed_paths = diff_paths_between_heads(root, baseline_head, current_head)?;
            candidate_paths.extend(committed_paths);
        }
        (Some(_), None) | (None, Some(_)) => return None,
        _ => {}
    }

    Some(filter_changed_files_to_session_scope(
        candidate_paths,
        snapshot,
        session_v2,
    ))
}

fn filter_changed_files_to_session_scope(
    changed_files: BTreeSet<String>,
    snapshot: &Snapshot,
    session_v2: &SessionV2Baseline,
) -> BTreeSet<String> {
    let scanned_paths = scanned_file_paths(snapshot);
    changed_files
        .into_iter()
        .filter(|path| scanned_paths.contains(path) || session_v2.file_hashes.contains_key(path))
        .collect()
}

fn stable_hash(bytes: &[u8]) -> u64 {
    let mut hasher = DefaultHasher::new();
    bytes.hash(&mut hasher);
    hasher.finish()
}

fn merge_optional_errors(left: Option<String>, right: Option<String>) -> Option<String> {
    match (left, right) {
        (Some(left), Some(right)) => Some(format!("{left}; {right}")),
        (Some(left), None) => Some(left),
        (None, Some(right)) => Some(right),
        (None, None) => None,
    }
}

#[cfg(test)]
fn build_clone_drift_finding_values(
    groups: &[crate::metrics::DuplicateGroup],
    evolution: Option<&crate::metrics::evo::EvolutionReport>,
    limit: usize,
) -> Vec<Value> {
    serialized_values(&crate::metrics::v2::build_clone_drift_findings(
        groups, evolution, limit,
    ))
}

struct CloneFindingPayload {
    exact_findings: Vec<Value>,
    prioritized_findings: Vec<Value>,
    families: Vec<Value>,
    remediation_hints: Vec<Value>,
    clone_group_count: usize,
    clone_family_count: usize,
}

fn clone_findings_for_health(
    state: &mut McpState,
    root: &Path,
    snapshot: &Snapshot,
    health: &metrics::HealthReport,
    limit: usize,
) -> (CloneFindingPayload, Option<String>) {
    let (evolution, evolution_error) = evolution_report_for_snapshot(state, root, snapshot);
    let report =
        crate::metrics::v2::build_clone_drift_report(&health.duplicate_groups, evolution.as_ref());
    let prioritized_findings = report
        .prioritized_findings
        .iter()
        .take(limit)
        .cloned()
        .collect::<Vec<_>>();
    let remediation_limit = report.families.len().saturating_mul(4);

    (
        CloneFindingPayload {
            clone_group_count: report.findings.len(),
            clone_family_count: report.families.len(),
            exact_findings: serialized_values(&report.findings),
            prioritized_findings: serialized_values(&prioritized_findings),
            families: serialized_values(&report.families),
            remediation_hints: serialized_values(
                &crate::metrics::v2::build_clone_remediation_hints(
                    &report.families,
                    remediation_limit,
                ),
            ),
        },
        evolution_error,
    )
}

fn visible_clone_ids(findings: &[Value]) -> BTreeSet<String> {
    findings
        .iter()
        .filter(|finding| finding_kind(finding) == "exact_clone_group")
        .filter_map(|finding| {
            finding
                .get("clone_id")
                .and_then(|value| value.as_str())
                .map(str::to_string)
        })
        .collect()
}

fn clone_value_matches_visible_clone_ids(
    value: &Value,
    visible_clone_ids: &BTreeSet<String>,
    key: &str,
) -> bool {
    value
        .get(key)
        .and_then(|value| value.as_array())
        .map(|clone_ids| {
            clone_ids.iter().any(|clone_id| {
                clone_id
                    .as_str()
                    .map(|clone_id| visible_clone_ids.contains(clone_id))
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}

fn filter_clone_values_by_visible_clone_ids(
    values: Vec<Value>,
    visible_clone_ids: &BTreeSet<String>,
    key: &str,
    limit: usize,
) -> Vec<Value> {
    if visible_clone_ids.is_empty() {
        return Vec::new();
    }

    values
        .into_iter()
        .filter(|value| clone_value_matches_visible_clone_ids(value, visible_clone_ids, key))
        .take(limit)
        .collect()
}

fn session_v2_analysis_signature(session_v2: Option<&SessionV2Baseline>) -> Option<u64> {
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

fn current_patch_safety_cache_identity(
    state: &McpState,
    context: &PatchCheckContext,
) -> Option<ScanCacheIdentity> {
    if context.reused_cached_scan {
        state.cached_scan_identity.clone()
    } else {
        context.scan_identity.clone()
    }
}

fn cached_patch_safety_analysis(
    state: &McpState,
    scan_identity: Option<&ScanCacheIdentity>,
    session_signature: Option<u64>,
) -> Option<PatchSafetyAnalysisCache> {
    let scan_identity = scan_identity?;
    let cached = state.cached_patch_safety.as_ref()?;
    if cached.scan_identity.as_ref() == Some(scan_identity)
        && cached.session_signature == session_signature
    {
        return Some(cached.clone());
    }

    None
}

fn cache_patch_safety_analysis(state: &mut McpState, analysis: &PatchSafetyAnalysisCache) {
    if analysis.scan_identity.is_some() {
        state.cached_patch_safety = Some(analysis.clone());
    }
}

fn patch_safety_semantic_error(analysis: &PatchSafetyAnalysisCache) -> Option<String> {
    merge_optional_errors(
        analysis
            .changed_semantic_error
            .clone()
            .or(analysis.all_semantic_error.clone()),
        analysis.clone_error.clone(),
    )
}

fn build_patch_safety_analysis(
    state: &mut McpState,
    root: &Path,
    bundle: &ScanBundle,
    changed_files: &BTreeSet<String>,
    session_v2: Option<&SessionV2Baseline>,
    cache_identity: Option<ScanCacheIdentity>,
) -> PatchSafetyAnalysisCache {
    let session_signature = session_v2_analysis_signature(session_v2);
    if let Some(cached) =
        cached_patch_safety_analysis(state, cache_identity.as_ref(), session_signature)
    {
        return cached;
    }

    let (clone_payload, clone_error) = clone_findings_for_health(
        state,
        root,
        &bundle.snapshot,
        &bundle.health,
        bundle.health.duplicate_groups.len(),
    );
    let structural_reports = crate::metrics::v2::build_structural_debt_reports_with_root(
        root,
        &bundle.snapshot,
        &bundle.health,
    );
    let (rules_config, rules_error) = load_v2_rules_config(state, root);
    let semantic = match analyze_semantic_snapshot(state, root) {
        Ok(semantic) => semantic,
        Err(error) => {
            let suppression_application = apply_suppressions(
                &rules_config,
                finding_values(
                    &clone_payload.exact_findings,
                    &serialized_values(&structural_reports),
                ),
            );
            let analysis = PatchSafetyAnalysisCache {
                scan_identity: cache_identity.clone(),
                session_signature,
                visible_findings: suppression_application.visible_findings,
                suppression_hits: serialized_values(&suppression_application.active_matches),
                suppressed_finding_count: suppression_match_count(
                    &suppression_application.active_matches,
                ),
                expired_suppressions: serialized_values(&suppression_application.expired_matches),
                expired_suppression_match_count: suppression_match_count(
                    &suppression_application.expired_matches,
                ),
                clone_error,
                all_semantic_error: merge_optional_errors(rules_error.clone(), Some(error.clone())),
                changed_semantic_error: merge_optional_errors(rules_error.clone(), Some(error)),
                rules_error,
                ..PatchSafetyAnalysisCache::default()
            };
            cache_patch_safety_analysis(state, &analysis);
            return analysis;
        }
    };

    let all_analysis = semantic
        .as_ref()
        .map(|semantic| {
            build_semantic_analysis_batch(
                &rules_config,
                semantic,
                Some(&bundle.snapshot),
                crate::metrics::v2::ObligationScope::All,
                &BTreeSet::new(),
            )
        })
        .unwrap_or_default();
    let all_finding_values =
        combined_other_finding_values(&all_analysis.findings, &structural_reports);
    let suppression_application = apply_suppressions(
        &rules_config,
        finding_values(&clone_payload.exact_findings, &all_finding_values),
    );
    let changed_scope = analyze_changed_patch_scope(
        state,
        root,
        &rules_config,
        Some(&bundle.snapshot),
        semantic.as_ref(),
        changed_files,
    );

    let analysis = PatchSafetyAnalysisCache {
        scan_identity: cache_identity,
        session_signature,
        visible_findings: suppression_application.visible_findings,
        suppression_hits: serialized_values(&suppression_application.active_matches),
        suppressed_finding_count: suppression_match_count(&suppression_application.active_matches),
        expired_suppressions: serialized_values(&suppression_application.expired_matches),
        expired_suppression_match_count: suppression_match_count(
            &suppression_application.expired_matches,
        ),
        changed_visible_findings: changed_scope
            .suppression_application
            .visible_findings
            .clone(),
        changed_obligations: changed_scope.obligations.clone(),
        changed_touched_concepts: changed_scope.touched_concepts.clone(),
        clone_error,
        all_semantic_error: rules_error.clone(),
        changed_semantic_error: merge_optional_errors(
            rules_error.clone(),
            changed_scope.semantic_error.clone(),
        ),
        rules_error,
    };

    cache_patch_safety_analysis(state, &analysis);

    analysis
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
    let (clone_payload, clone_error) =
        clone_findings_for_health(state, root, snapshot, health, health.duplicate_groups.len());
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

fn semantic_findings_and_obligations(
    state: &mut McpState,
    root: &Path,
    snapshot: Option<&Snapshot>,
    scope: crate::metrics::v2::ObligationScope,
    changed_files: &BTreeSet<String>,
) -> (
    Vec<crate::metrics::v2::SemanticFinding>,
    Vec<crate::metrics::v2::ObligationReport>,
    Option<String>,
) {
    let (analysis, error) = semantic_analysis_batch(state, root, snapshot, scope, changed_files);
    (analysis.findings, analysis.obligations, error)
}

#[derive(Default)]
struct SemanticAnalysisBatch {
    findings: Vec<crate::metrics::v2::SemanticFinding>,
    obligations: Vec<crate::metrics::v2::ObligationReport>,
    state_reports: Vec<crate::metrics::v2::StateIntegrityReport>,
}

fn semantic_analysis_batch(
    state: &mut McpState,
    root: &Path,
    snapshot: Option<&Snapshot>,
    scope: crate::metrics::v2::ObligationScope,
    changed_files: &BTreeSet<String>,
) -> (SemanticAnalysisBatch, Option<String>) {
    let (config, config_error) = load_v2_rules_config(state, root);
    match analyze_semantic_snapshot(state, root) {
        Ok(Some(semantic)) => (
            build_semantic_analysis_batch(&config, &semantic, snapshot, scope, changed_files),
            config_error,
        ),
        Ok(None) => (SemanticAnalysisBatch::default(), config_error),
        Err(error) => (
            SemanticAnalysisBatch::default(),
            merge_optional_errors(config_error, Some(error)),
        ),
    }
}

fn build_semantic_analysis_batch(
    config: &crate::metrics::rules::RulesConfig,
    semantic: &SemanticSnapshot,
    snapshot: Option<&Snapshot>,
    scope: crate::metrics::v2::ObligationScope,
    changed_files: &BTreeSet<String>,
) -> SemanticAnalysisBatch {
    let mut findings = crate::metrics::v2::build_authority_and_access_findings_with_snapshot(
        config, semantic, snapshot,
    );
    let obligations = crate::metrics::v2::build_obligations(config, semantic, scope, changed_files);
    findings.extend(crate::metrics::v2::build_obligation_findings(&obligations));
    let state_scope = if scope == crate::metrics::v2::ObligationScope::Changed {
        crate::metrics::v2::StateScope::Changed
    } else {
        crate::metrics::v2::StateScope::All
    };
    let state_reports = crate::metrics::v2::build_state_integrity_reports(
        config,
        semantic,
        &obligations,
        state_scope,
        changed_files,
    );
    findings.extend(crate::metrics::v2::build_state_integrity_findings(
        &state_reports,
    ));

    SemanticAnalysisBatch {
        findings,
        obligations,
        state_reports,
    }
}

fn finding_payload_map(findings: &[Value]) -> BTreeMap<String, Value> {
    let mut payloads = BTreeMap::new();
    for finding in findings {
        payloads.insert(stable_json_key(finding), finding.clone());
    }
    payloads
}

fn stable_json_key(value: &Value) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| value.to_string())
}

fn serialized_values<T: serde::Serialize>(values: &[T]) -> Vec<Value> {
    values
        .iter()
        .filter_map(|value| serde_json::to_value(value).ok())
        .collect()
}

fn combined_other_finding_values(
    semantic_findings: &[crate::metrics::v2::SemanticFinding],
    structural_reports: &[crate::metrics::v2::StructuralDebtReport],
) -> Vec<Value> {
    let mut findings = serialized_values(semantic_findings);
    findings.extend(serialized_values(structural_reports));
    findings
}

#[derive(Debug, Clone, serde::Serialize, Default)]
struct SuppressionMatch {
    kind: String,
    concept: Option<String>,
    file: Option<String>,
    reason: String,
    expires: Option<String>,
    expired: bool,
    matched_finding_count: usize,
}

#[derive(Debug, Clone, Default)]
struct SuppressionApplication {
    visible_findings: Vec<Value>,
    active_matches: Vec<SuppressionMatch>,
    expired_matches: Vec<SuppressionMatch>,
}

fn finding_values(clone_findings: &[Value], other_findings: &[Value]) -> Vec<Value> {
    let mut findings = clone_findings.to_vec();
    findings.extend(other_findings.iter().cloned());
    findings
}

fn apply_root_suppressions(
    state: &mut McpState,
    root: &Path,
    findings: Vec<Value>,
) -> (SuppressionApplication, Option<String>) {
    let (config, rules_error) = load_v2_rules_config(state, root);
    (apply_suppressions(&config, findings), rules_error)
}

fn suppression_match_count(matches: &[SuppressionMatch]) -> usize {
    matches
        .iter()
        .map(|matched| matched.matched_finding_count)
        .sum()
}

fn apply_suppressions(
    config: &crate::metrics::rules::RulesConfig,
    findings: Vec<Value>,
) -> SuppressionApplication {
    let mut visible_findings = Vec::new();
    let mut active_matches = BTreeMap::<String, SuppressionMatch>::new();
    let mut expired_matches = BTreeMap::<String, SuppressionMatch>::new();

    for finding in findings {
        let mut suppressed = false;
        for suppression in &config.suppress {
            if !suppression_matches_finding(suppression, &finding) {
                continue;
            }

            let expired = suppression_is_expired(suppression);
            let entry = suppression_match_entry(suppression, expired);
            let key = stable_json_key(&serde_json::to_value(&entry).unwrap_or_else(|_| json!({})));
            let target_map = if entry.expired {
                &mut expired_matches
            } else {
                &mut active_matches
            };
            target_map
                .entry(key)
                .and_modify(|matched| matched.matched_finding_count += 1)
                .or_insert_with(|| {
                    let mut matched = entry;
                    matched.matched_finding_count = 1;
                    matched
                });
            suppressed |= !expired;
        }

        if !suppressed {
            visible_findings.push(finding);
        }
    }

    SuppressionApplication {
        visible_findings,
        active_matches: active_matches.into_values().collect(),
        expired_matches: expired_matches.into_values().collect(),
    }
}

fn suppression_match_entry(
    suppression: &crate::metrics::rules::SuppressionRule,
    expired: bool,
) -> SuppressionMatch {
    SuppressionMatch {
        kind: suppression.kind.clone(),
        concept: suppression.concept.clone(),
        file: suppression.file.clone(),
        reason: suppression.reason.clone(),
        expires: suppression.expires.clone(),
        expired,
        matched_finding_count: 0,
    }
}

fn suppression_matches_finding(
    suppression: &crate::metrics::rules::SuppressionRule,
    finding: &Value,
) -> bool {
    if !suppression_kind_matches(&suppression.kind, finding_kind(finding)) {
        return false;
    }
    if let Some(concept) = &suppression.concept {
        if finding_concept_id(finding) != Some(concept.as_str()) {
            return false;
        }
    }
    if let Some(file_pattern) = &suppression.file {
        if !finding_files(finding)
            .iter()
            .any(|file| crate::metrics::rules::glob_match(file_pattern, file))
        {
            return false;
        }
    }

    true
}

fn suppression_kind_matches(pattern: &str, finding_kind: &str) -> bool {
    pattern == "*" || pattern == finding_kind
}

fn finding_kind(finding: &Value) -> &str {
    finding
        .get("kind")
        .and_then(|value| value.as_str())
        .unwrap_or("")
}

fn finding_concept_id(finding: &Value) -> Option<&str> {
    finding.get("concept_id").and_then(|value| value.as_str())
}

fn finding_string_values(finding: &Value, field: &str) -> Vec<String> {
    finding
        .get(field)
        .and_then(|value| value.as_array())
        .map(|values| {
            values
                .iter()
                .filter_map(|value| value.as_str().map(str::to_string))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn finding_files(finding: &Value) -> Vec<String> {
    let files = finding_string_values(finding, "files");
    if !files.is_empty() {
        return files;
    }

    if let Some(path) = finding.get("path").and_then(|value| value.as_str()) {
        return vec![path.to_string()];
    }

    finding
        .get("instances")
        .and_then(|value| value.as_array())
        .map(|instances| {
            instances
                .iter()
                .filter_map(|instance| {
                    instance
                        .get("file")
                        .and_then(|value| value.as_str())
                        .map(str::to_string)
                })
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn finding_scope(finding: &Value) -> String {
    if let Some(scope) = finding.get("scope").and_then(|value| value.as_str()) {
        return scope.to_string();
    }
    if let Some(concept_id) = finding_concept_id(finding) {
        return concept_id.to_string();
    }

    let files = finding_files(finding);
    if files.is_empty() {
        return finding_kind(finding).to_string();
    }
    if files.len() == 1 {
        return files[0].clone();
    }

    files.join("|")
}

fn finding_trust_tier(finding: &Value) -> String {
    finding
        .get("trust_tier")
        .and_then(|value| value.as_str())
        .map(str::to_string)
        .unwrap_or_else(|| trust_tier_for_kind(finding_kind(finding), "trusted"))
}

fn looks_like_tooling_scope(scope: &str) -> bool {
    scope.starts_with("scripts/")
}

fn looks_like_transport_facade_scope(scope: &str) -> bool {
    scope.contains("/ipc.")
        || scope.contains("-ipc.")
        || scope.ends_with("/ipc.ts")
        || scope.ends_with("/ipc.tsx")
        || scope.contains("/browser-http-ipc.")
}

fn is_watchpoint_presentation_kind(kind: &str) -> bool {
    matches!(
        kind,
        "cycle_cluster" | "dead_island" | "clone_family" | "clone_group" | "exact_clone_group"
    )
}

fn is_hardening_note_kind(kind: &str) -> bool {
    matches!(
        kind,
        "closed_domain_exhaustiveness" | "contract_surface_completeness"
    )
}

fn role_tags_include(role_tags: &[String], tag: &str) -> bool {
    role_tags.iter().any(|role_tag| role_tag == tag)
}

fn classify_presentation_class(
    kind: &str,
    trust_tier: &str,
    scope: &str,
    files: &[String],
    role_tags: &[String],
    evidence_count: usize,
    finding_count: usize,
    boundary_pressure_count: usize,
    missing_site_count: usize,
) -> String {
    if trust_tier == "experimental" {
        return "experimental".to_string();
    }
    if trust_tier == "watchpoint" || is_watchpoint_presentation_kind(kind) {
        return "watchpoint".to_string();
    }
    if looks_like_tooling_scope(scope)
        || (!files.is_empty() && files.iter().all(|path| looks_like_tooling_scope(path)))
    {
        return "tooling_debt".to_string();
    }
    if role_tags.iter().any(|tag| tag == "transport_facade")
        || looks_like_transport_facade_scope(scope)
    {
        return "guarded_facade".to_string();
    }
    if is_hardening_note_kind(kind)
        && files.len() <= 2
        && evidence_count <= 2
        && finding_count <= 1
        && boundary_pressure_count == 0
        && missing_site_count <= 1
    {
        return "hardening_note".to_string();
    }

    "structural_debt".to_string()
}

fn finding_presentation_class(finding: &Value) -> String {
    if let Some(classification) = finding
        .get("presentation_class")
        .and_then(|value| value.as_str())
    {
        return classification.to_string();
    }

    let files = dedupe_strings_preserve_order(finding_files(finding));
    let role_tags = finding_string_values(finding, "role_tags");
    let evidence_count = finding_string_values(finding, "evidence").len();
    classify_presentation_class(
        finding_kind(finding),
        &finding_trust_tier(finding),
        &finding_scope(finding),
        &files,
        &role_tags,
        evidence_count,
        1,
        0,
        0,
    )
}

fn classify_leverage_class(
    kind: &str,
    trust_tier: &str,
    presentation_class: &str,
    role_tags: &[String],
    fan_in: Option<usize>,
    fan_out: Option<usize>,
    line_count: Option<usize>,
    max_complexity: Option<usize>,
    cycle_size: Option<usize>,
    cut_candidate_count: Option<usize>,
    guardrail_test_count: Option<usize>,
    boundary_pressure_count: usize,
    missing_site_count: usize,
) -> String {
    if trust_tier == "experimental" || presentation_class == "experimental" {
        return "experimental".to_string();
    }
    if presentation_class == "tooling_debt" {
        return "tooling_debt".to_string();
    }
    if presentation_class == "hardening_note" {
        return "hardening_note".to_string();
    }
    if presentation_class == "guarded_facade" {
        return "boundary_discipline".to_string();
    }
    if kind == "cycle_cluster" {
        if role_tags_include(role_tags, "component_barrel")
            || role_tags_include(role_tags, "guarded_boundary")
            || cycle_size.unwrap_or(0) >= 10
            || cut_candidate_count.unwrap_or(0) > 0
        {
            return "architecture_signal".to_string();
        }
        return "secondary_cleanup".to_string();
    }
    if kind == "dead_island" {
        return "secondary_cleanup".to_string();
    }
    if boundary_pressure_count > 0 && missing_site_count > 0 {
        return "architecture_signal".to_string();
    }
    if role_tags_include(role_tags, "component_barrel")
        || role_tags_include(role_tags, "guarded_boundary")
    {
        return "architecture_signal".to_string();
    }
    if role_tags_include(role_tags, "composition_root")
        || role_tags_include(role_tags, "entry_surface")
    {
        return "regrowth_watchpoint".to_string();
    }
    if role_tags_include(role_tags, "facade_with_extracted_owners") {
        if extracted_owner_facade_needs_secondary_cleanup(
            kind,
            role_tags,
            line_count,
            max_complexity,
            fan_in,
        ) {
            return "secondary_cleanup".to_string();
        }
        return "local_refactor_target".to_string();
    }
    if boundary_pressure_count > 0 || missing_site_count > 0 {
        return "local_refactor_target".to_string();
    }
    if matches!(kind, "clone_family" | "clone_group" | "exact_clone_group") {
        return "secondary_cleanup".to_string();
    }
    if matches!(kind, "dependency_sprawl" | "unstable_hotspot" | "hotspot")
        || guardrail_test_count.unwrap_or(0) > 0
        || fan_out.unwrap_or(0) > 0
    {
        return "local_refactor_target".to_string();
    }
    "secondary_cleanup".to_string()
}

fn extracted_owner_facade_needs_secondary_cleanup(
    kind: &str,
    role_tags: &[String],
    line_count: Option<usize>,
    max_complexity: Option<usize>,
    fan_in: Option<usize>,
) -> bool {
    if role_tags_include(role_tags, "entry_surface") {
        return true;
    }
    if kind == "large_file" {
        return true;
    }
    if line_count.unwrap_or(0) >= 500 {
        return true;
    }
    if max_complexity.unwrap_or(0) >= 20 {
        return true;
    }
    fan_in.unwrap_or(0) >= 20
}

fn is_contained_refactor_surface(
    role_tags: &[String],
    fan_in: Option<usize>,
    fan_out: Option<usize>,
    cycle_size: Option<usize>,
    guardrail_test_count: Option<usize>,
) -> bool {
    let has_extracted_owner_surface = role_tags_include(role_tags, "facade_with_extracted_owners");
    let guardrail_count = guardrail_test_count.unwrap_or(0);
    let inbound_pressure = fan_in.unwrap_or(0);
    let dependency_breadth = fan_out.unwrap_or(0);
    let cycle_span = cycle_size.unwrap_or(0);

    (has_extracted_owner_surface || guardrail_count > 0)
        && dependency_breadth >= 3
        && (inbound_pressure == 0 || inbound_pressure <= 12)
        && (cycle_span == 0 || cycle_span <= 6)
}

#[allow(clippy::too_many_arguments)]
fn classify_leverage_reasons(
    kind: &str,
    trust_tier: &str,
    presentation_class: &str,
    leverage_class: &str,
    role_tags: &[String],
    fan_in: Option<usize>,
    fan_out: Option<usize>,
    cycle_size: Option<usize>,
    cut_candidate_count: Option<usize>,
    guardrail_test_count: Option<usize>,
    boundary_pressure_count: usize,
    missing_site_count: usize,
) -> Vec<String> {
    let mut reasons = Vec::new();
    if trust_tier == "experimental" || presentation_class == "experimental" {
        reasons.push("detector_under_evaluation".to_string());
    }
    match leverage_class {
        "tooling_debt" => reasons.push("tooling_surface_maintenance_burden".to_string()),
        "hardening_note" => reasons.push("narrow_completeness_gap".to_string()),
        "boundary_discipline" => {
            reasons.push("boundary_or_facade_seam_pressure".to_string());
            if fan_in.unwrap_or(0) > 0 {
                reasons.push("heavy_inbound_seam_pressure".to_string());
            }
        }
        "architecture_signal" => {
            if role_tags_include(role_tags, "component_barrel") {
                reasons.push("shared_barrel_boundary_hub".to_string());
            }
            if role_tags_include(role_tags, "guarded_boundary") {
                reasons.push("guardrail_backed_boundary_pressure".to_string());
            }
            if kind == "cycle_cluster" {
                reasons.push("mixed_cycle_pressure".to_string());
            }
            if cut_candidate_count.unwrap_or(0) > 0 {
                reasons.push("high_leverage_cut_candidate".to_string());
            }
            if boundary_pressure_count > 0 {
                reasons.push("ownership_boundary_erosion".to_string());
            }
            if missing_site_count > 0 {
                reasons.push("propagation_burden".to_string());
            }
        }
        "local_refactor_target" => {
            if role_tags_include(role_tags, "facade_with_extracted_owners") {
                reasons.push("extracted_owner_shell_pressure".to_string());
            }
            if guardrail_test_count.unwrap_or(0) > 0 {
                reasons.push("guardrail_backed_refactor_surface".to_string());
            }
            if is_contained_refactor_surface(
                role_tags,
                fan_in,
                fan_out,
                cycle_size,
                guardrail_test_count,
            ) {
                reasons.push("contained_refactor_surface".to_string());
            }
            if fan_out.unwrap_or(0) > 0 {
                reasons.push("contained_dependency_pressure".to_string());
            }
            if boundary_pressure_count > 0 {
                reasons.push("narrower_ownership_split_available".to_string());
            }
            if missing_site_count > 0 {
                reasons.push("explicit_update_surface".to_string());
            }
        }
        "regrowth_watchpoint" => {
            reasons.push("intentionally_central_surface".to_string());
            if fan_out.unwrap_or(0) > 0 {
                reasons.push("fan_out_regrowth_pressure".to_string());
            }
        }
        "secondary_cleanup" => {
            if kind == "dead_island" {
                reasons.push("disconnected_internal_component".to_string());
            } else if matches!(kind, "clone_family" | "clone_group" | "exact_clone_group") {
                reasons.push("duplicate_maintenance_pressure".to_string());
            } else if role_tags_include(role_tags, "facade_with_extracted_owners") {
                reasons.push("secondary_facade_cleanup".to_string());
            } else if cycle_size.unwrap_or(0) > 0 {
                reasons.push("smaller_cycle_watchpoint".to_string());
            } else {
                reasons.push("real_but_lower_leverage_cleanup".to_string());
            }
        }
        _ => {}
    }
    dedupe_strings_preserve_order(reasons)
}

fn finding_numeric_metric(finding: &Value, key: &str) -> Option<usize> {
    finding
        .get("metrics")
        .and_then(|value| value.get(key))
        .or_else(|| finding.get(key))
        .and_then(|value| value.as_u64())
        .map(|value| value as usize)
}

fn finding_leverage_class(finding: &Value) -> String {
    if let Some(classification) = finding
        .get("leverage_class")
        .and_then(|value| value.as_str())
    {
        return classification.to_string();
    }

    let role_tags = finding_string_values(finding, "role_tags");
    classify_leverage_class(
        finding_kind(finding),
        &finding_trust_tier(finding),
        &finding_presentation_class(finding),
        &role_tags,
        finding_numeric_metric(finding, "fan_in")
            .or_else(|| finding_numeric_metric(finding, "inbound_reference_count")),
        finding_numeric_metric(finding, "fan_out"),
        finding_numeric_metric(finding, "line_count"),
        finding_numeric_metric(finding, "max_complexity"),
        finding_numeric_metric(finding, "cycle_size"),
        finding_numeric_metric(finding, "cut_candidate_count"),
        finding_numeric_metric(finding, "guardrail_test_count"),
        finding_numeric_metric(finding, "boundary_pressure_count").unwrap_or(0),
        finding_numeric_metric(finding, "missing_site_count").unwrap_or(0),
    )
}

fn finding_leverage_reasons(finding: &Value) -> Vec<String> {
    let reasons = finding_string_values(finding, "leverage_reasons");
    if !reasons.is_empty() {
        return reasons;
    }

    let role_tags = finding_string_values(finding, "role_tags");
    let leverage_class = finding_leverage_class(finding);
    classify_leverage_reasons(
        finding_kind(finding),
        &finding_trust_tier(finding),
        &finding_presentation_class(finding),
        &leverage_class,
        &role_tags,
        finding_numeric_metric(finding, "fan_in")
            .or_else(|| finding_numeric_metric(finding, "inbound_reference_count")),
        finding_numeric_metric(finding, "fan_out"),
        finding_numeric_metric(finding, "cycle_size"),
        finding_numeric_metric(finding, "cut_candidate_count"),
        finding_numeric_metric(finding, "guardrail_test_count"),
        finding_numeric_metric(finding, "boundary_pressure_count").unwrap_or(0),
        finding_numeric_metric(finding, "missing_site_count").unwrap_or(0),
    )
}

fn decorate_finding_with_classification(finding: &Value) -> Value {
    let presentation_class = finding_presentation_class(finding);
    let leverage_class = finding_leverage_class(finding);
    let leverage_reasons = finding_leverage_reasons(finding);
    let mut finding = finding.clone();
    if let Some(object) = finding.as_object_mut() {
        object.insert(
            "presentation_class".to_string(),
            Value::String(presentation_class),
        );
        object.insert("leverage_class".to_string(), Value::String(leverage_class));
        object.insert("leverage_reasons".to_string(), json!(leverage_reasons));
    }
    finding
}

fn is_experimental_finding(finding: &Value) -> bool {
    finding_trust_tier(finding) == "experimental"
}

fn partition_experimental_findings(findings: &[Value], limit: usize) -> (Vec<Value>, Vec<Value>) {
    let mut visible = Vec::new();
    let mut experimental = Vec::new();

    for finding in findings {
        if is_experimental_finding(finding) {
            if experimental.len() < limit {
                experimental.push(finding.clone());
            }
            continue;
        }
        visible.push(finding.clone());
    }

    (visible, experimental)
}

fn trust_tier_for_kind(kind: &str, default: &str) -> String {
    match kind {
        "cycle_cluster" | "dead_island" => "watchpoint".to_string(),
        "dead_private_code_cluster" => "experimental".to_string(),
        _ => default.to_string(),
    }
}

fn build_finding_details(findings: &[Value], limit: usize) -> Vec<FindingDetail> {
    findings.iter().take(limit).map(finding_detail).collect()
}

fn finding_detail(finding: &Value) -> FindingDetail {
    let kind = finding_kind(finding).to_string();
    let files = dedupe_strings_preserve_order(finding_files(finding));
    let evidence = dedupe_strings_preserve_order(finding_string_values(finding, "evidence"));
    let inspection_focus = finding_detail_inspection_focus(finding);

    annotate_finding_detail(FindingDetail {
        kind: kind.clone(),
        trust_tier: finding_trust_tier(finding),
        presentation_class: finding_presentation_class(finding),
        leverage_class: finding_leverage_class(finding),
        scope: finding_scope(finding),
        severity: finding
            .get("severity")
            .and_then(|value| value.as_str())
            .unwrap_or("low")
            .to_string(),
        summary: finding
            .get("summary")
            .and_then(|value| value.as_str())
            .unwrap_or(kind.as_str())
            .to_string(),
        impact: finding_detail_impact(finding),
        files: files.clone(),
        role_tags: finding_string_values(finding, "role_tags"),
        leverage_reasons: finding_leverage_reasons(finding),
        evidence: evidence.clone(),
        inspection_focus,
        candidate_split_axes: finding_detail_candidate_split_axes(finding),
        related_surfaces: finding_detail_related_surfaces(finding),
        metrics: FindingDetailMetrics {
            file_count: files.len(),
            evidence_count: evidence.len(),
            member_count: finding
                .get("member_count")
                .and_then(|value| value.as_u64())
                .map(|value| value as usize),
            family_score_0_10000: finding
                .get("family_score")
                .or_else(|| finding.get("family_score_0_10000"))
                .and_then(|value| value.as_u64())
                .map(|value| value as u32),
            divergence_score: finding
                .get("divergence_score")
                .and_then(|value| value.as_u64())
                .map(|value| value as u32),
        },
    })
}

fn finding_detail_candidate_split_axes(finding: &Value) -> Vec<String> {
    let axes = finding_string_values(finding, "candidate_split_axes");
    if !axes.is_empty() {
        return axes;
    }

    match finding_kind(finding) {
        "cycle_cluster" => vec!["contract boundary".to_string()],
        "dependency_sprawl" => vec!["dependency boundary".to_string()],
        "unstable_hotspot" => vec!["stable contract boundary".to_string()],
        _ => Vec::new(),
    }
}

fn finding_detail_related_surfaces(finding: &Value) -> Vec<String> {
    let related = finding_string_values(finding, "related_surfaces");
    if !related.is_empty() {
        return related;
    }

    finding_files(finding).into_iter().take(5).collect()
}

fn finding_detail_impact(finding: &Value) -> String {
    if let Some(impact) = finding.get("impact").and_then(|value| value.as_str()) {
        return impact.to_string();
    }

    match finding_kind(finding) {
        "multi_writer_concept" => {
            "Multiple write paths make the concept easier to update inconsistently and harder to debug.".to_string()
        }
        "forbidden_writer" | "writer_outside_allowlist" => {
            "Writes from the wrong layer erode ownership and increase the chance that invariants drift.".to_string()
        }
        "forbidden_raw_read" | "authoritative_import_bypass" => {
            "Bypassing the intended read boundary weakens architectural contracts and can create stale or inconsistent views.".to_string()
        }
        "concept_boundary_pressure" => {
            "Repeated boundary bypasses around the same concept make future changes easier to scatter across the codebase.".to_string()
        }
        "closed_domain_exhaustiveness" => {
            "Finite-domain changes can silently miss one surface unless all required cases stay in sync.".to_string()
        }
        "contract_surface_completeness" => {
            "Related contract surfaces are no longer aligned, so runtime paths can diverge or partially break.".to_string()
        }
        "state_integrity_missing_site" | "state_integrity_unmapped_root" => {
            "State model drift makes lifecycle and restore behavior easier to break through partial edits.".to_string()
        }
        "contract_parity_gap" => {
            "Cross-surface parity drift means different runtime paths may no longer implement the same contract.".to_string()
        }
        "exact_clone_group" | "clone_family" => {
            "Duplicate logic increases the chance that fixes land in one copy but not the others.".to_string()
        }
        _ => "If ignored, this structural inconsistency will keep adding change friction and make future regressions harder to isolate.".to_string(),
    }
}

fn finding_detail_inspection_focus(finding: &Value) -> Vec<String> {
    let focus = finding_string_values(finding, "inspection_focus");
    if !focus.is_empty() {
        return focus;
    }

    let focus = match finding_kind(finding) {
        "multi_writer_concept" | "forbidden_writer" | "writer_outside_allowlist" => vec![
            "inspect which module should own writes for this concept".to_string(),
            "inspect whether the extra write path can be removed or routed through the owner"
                .to_string(),
        ],
        "forbidden_raw_read" | "authoritative_import_bypass" | "concept_boundary_pressure" => vec![
            "inspect whether reads should move behind the canonical accessor or public boundary"
                .to_string(),
        ],
        "closed_domain_exhaustiveness" | "contract_surface_completeness" => vec![
            "inspect which required surfaces should change together and add explicit coverage there"
                .to_string(),
        ],
        "exact_clone_group" | "clone_family" => clone_family_inspection_focus(finding),
        _ => vec![
            "inspect the files and evidence behind this finding before choosing a design fix"
                .to_string(),
        ],
    };

    let mut focus = dedupe_strings_preserve_order(focus);
    focus.truncate(3);
    focus
}

fn suppression_is_expired(suppression: &crate::metrics::rules::SuppressionRule) -> bool {
    let Some(expires) = &suppression.expires else {
        return false;
    };
    let format = iso_date_format();
    let Ok(expiry_date) = Date::parse(expires, format) else {
        return false;
    };
    expiry_date < OffsetDateTime::now_utc().date()
}

fn iso_date_format<'a>() -> &'a [FormatItem<'a>] {
    static FORMAT: &[FormatItem<'_>] = format_description!("[year]-[month]-[day]");
    FORMAT
}

fn state_model_ids_from_findings(
    findings: &[crate::metrics::v2::SemanticFinding],
) -> BTreeSet<String> {
    findings
        .iter()
        .filter(|finding| finding.kind.starts_with("state_model_"))
        .map(|finding| finding.concept_id.clone())
        .collect()
}

fn state_model_ids_from_reports(
    reports: &[crate::metrics::v2::StateIntegrityReport],
) -> BTreeSet<String> {
    reports.iter().map(|report| report.id.clone()).collect()
}

#[derive(Default)]
struct ChangedPatchScope {
    obligations: Vec<crate::metrics::v2::ObligationReport>,
    semantic_error: Option<String>,
    suppression_application: SuppressionApplication,
    touched_concepts: BTreeSet<String>,
}

fn analyze_changed_patch_scope(
    state: &mut McpState,
    root: &Path,
    config: &crate::metrics::rules::RulesConfig,
    snapshot: Option<&Snapshot>,
    semantic: Option<&SemanticSnapshot>,
    changed_files: &BTreeSet<String>,
) -> ChangedPatchScope {
    if changed_files.is_empty() {
        return ChangedPatchScope::default();
    }

    let (analysis, semantic_error) = match semantic {
        Some(semantic) => (
            build_semantic_analysis_batch(
                config,
                semantic,
                snapshot,
                crate::metrics::v2::ObligationScope::Changed,
                changed_files,
            ),
            None,
        ),
        None => semantic_analysis_batch(
            state,
            root,
            snapshot,
            crate::metrics::v2::ObligationScope::Changed,
            changed_files,
        ),
    };
    let mut touched_concepts =
        crate::metrics::v2::changed_concept_ids_from_files(config, changed_files)
            .into_iter()
            .collect::<BTreeSet<_>>();
    touched_concepts.extend(crate::metrics::v2::changed_state_model_ids_from_files(
        config,
        changed_files,
    ));
    touched_concepts.extend(crate::metrics::v2::changed_concepts_from_obligations(
        &analysis.obligations,
    ));
    touched_concepts.extend(state_model_ids_from_reports(&analysis.state_reports));
    touched_concepts.extend(state_model_ids_from_findings(&analysis.findings));
    let changed_findings = serialized_values(&analysis.findings);
    let suppression_application = apply_suppressions(config, changed_findings);

    ChangedPatchScope {
        obligations: analysis.obligations,
        semantic_error,
        suppression_application,
        touched_concepts,
    }
}

fn compute_touched_concept_gate(
    state: &mut McpState,
    root: &Path,
    strict: bool,
) -> Result<Value, String> {
    let (session_v2, session_v2_status) = current_session_v2_baseline_with_status(state, root)?;
    let context = prepare_patch_check_context(state, root, session_v2.as_ref())?;
    let patch_cache_identity = current_patch_safety_cache_identity(state, &context);
    let bundle = context.bundle;
    let changed_files = context.changed_files;

    if !context.reused_cached_scan {
        state.cached_semantic = None;
        state.cached_evolution = None;
    }
    let analysis = build_patch_safety_analysis(
        state,
        root,
        &bundle,
        &changed_files,
        session_v2.as_ref(),
        patch_cache_identity,
    );
    let current_finding_payloads = finding_payload_map(&analysis.visible_findings);
    let (rules_config, _) = load_v2_rules_config(state, root);
    let missing_obligations = analysis
        .changed_obligations
        .iter()
        .filter(|obligation| !obligation.missing_sites.is_empty())
        .cloned()
        .collect::<Vec<_>>();

    let introduced_findings = session_v2
        .as_ref()
        .map(|session_v2| {
            current_finding_payloads
                .iter()
                .filter(|(key, _)| !session_v2.finding_payloads.contains_key(*key))
                .map(|(_, payload)| payload.clone())
                .collect::<Vec<_>>()
        })
        .unwrap_or_else(|| {
            analysis
                .changed_visible_findings
                .iter()
                .filter(|finding| {
                    let concept_id = finding_concept_id(finding).unwrap_or_default();
                    analysis.changed_touched_concepts.is_empty()
                        || analysis.changed_touched_concepts.contains(concept_id)
                })
                .cloned()
                .collect::<Vec<_>>()
        });
    let (visible_introduced_findings, experimental_findings) =
        partition_experimental_findings(&introduced_findings, 10);
    let blocking_findings = visible_introduced_findings
        .iter()
        .filter(|finding| {
            let severity = severity_of_value(finding);
            severity == "high" || (strict && severity == "medium")
        })
        .cloned()
        .collect::<Vec<_>>();
    let decision = if !missing_obligations.is_empty() || !blocking_findings.is_empty() {
        "fail"
    } else {
        "pass"
    };
    let semantic_error = patch_safety_semantic_error(&analysis);
    let summary = if decision == "fail" {
        "Touched-concept regressions detected"
    } else if changed_files.is_empty() {
        "No working-tree changes detected"
    } else {
        "No blocking touched-concept regressions detected"
    };
    let persisted_baseline = load_persisted_baseline(root).ok().flatten();
    let legacy_baseline_delta = persisted_baseline
        .as_ref()
        .or(state.baseline.as_ref())
        .map(|baseline| baseline.diff(&bundle.health));
    let preserved_semantic = state.cached_semantic.clone();
    let preserved_evolution = state.cached_evolution.clone();
    let preserved_patch_safety = state.cached_patch_safety.clone();

    let response = json!({
        "decision": decision,
        "strict": strict,
        "summary": summary,
        "changed_files": changed_files.iter().cloned().collect::<Vec<_>>(),
        "introduced_findings": visible_introduced_findings,
        "experimental_finding_count": experimental_findings.len(),
        "experimental_findings": experimental_findings,
        "blocking_findings": blocking_findings,
        "missing_obligations": missing_obligations,
        "obligation_completeness_0_10000": crate::metrics::v2::obligation_score_0_10000(&analysis.changed_obligations),
        "suppression_hits": analysis.suppression_hits,
        "suppressed_finding_count": analysis.suppressed_finding_count,
        "expired_suppressions": analysis.expired_suppressions,
        "expired_suppression_match_count": analysis.expired_suppression_match_count,
        "rules_error": analysis.rules_error,
        "semantic_error": semantic_error,
        "scan_trust": scan_trust_json(&bundle.metadata),
        "confidence": build_v2_confidence_report(&bundle.metadata, &rules_config, session_v2_status),
        "baseline_delta": legacy_baseline_delta_json(legacy_baseline_delta.as_ref()),
    });

    if !context.reused_cached_scan {
        update_scan_cache(
            state,
            root.to_path_buf(),
            bundle,
            persisted_baseline.or(state.baseline.clone()),
            context.scan_identity,
        );
        state.cached_semantic = preserved_semantic;
        state.cached_evolution = preserved_evolution;
        state.cached_patch_safety = preserved_patch_safety;
    } else if persisted_baseline.is_some() {
        state.baseline = persisted_baseline;
    }

    Ok(response)
}

pub fn cli_save_v2_session(root: &Path) -> Result<Value, String> {
    let mut state = fresh_mcp_state();
    let bundle = do_scan(root)?;
    let baseline = arch::ArchBaseline::from_health(&bundle.health);
    let signal = baseline.quality_signal;
    let baseline_path = save_baseline(root, &baseline)?;
    let (session_v2, suppression_application, semantic_error) = build_session_v2_baseline(
        &mut state,
        root,
        &bundle.snapshot,
        &bundle.health,
        &bundle.metadata,
    );
    let session_v2_baseline_path = save_session_v2_baseline(root, &session_v2)?;
    let session_finding_count = session_v2.finding_payloads.len();
    let (rules_config, _) = load_v2_rules_config(&mut state, root);

    Ok(json!({
        "status": "Baseline saved",
        "quality_signal": (signal * 10000.0).round() as u32,
        "baseline_path": baseline_path,
        "session_v2_baseline_path": session_v2_baseline_path,
        "session_finding_count": session_finding_count,
        "suppression_hits": suppression_application.active_matches,
        "suppressed_finding_count": suppression_match_count(&suppression_application.active_matches),
        "expired_suppressions": suppression_application.expired_matches,
        "expired_suppression_match_count": suppression_match_count(&suppression_application.expired_matches),
        "confidence": build_v2_confidence_report(
            &bundle.metadata,
            &rules_config,
            compatible_session_baseline_status(SESSION_V2_SCHEMA_VERSION),
        ),
        "semantic_error": semantic_error,
        "message": "Run 'sentrux gate' after making changes to evaluate touched-concept regressions"
    }))
}

pub fn cli_evaluate_v2_gate(root: &Path, strict: bool) -> Result<Value, String> {
    let mut state = fresh_mcp_state();
    compute_touched_concept_gate(&mut state, root, strict)
}

fn severity_of_value(value: &Value) -> &str {
    value
        .get("severity")
        .and_then(|severity| severity.as_str())
        .unwrap_or("low")
}

fn is_internal_sentrux_path(path: &str) -> bool {
    path == ".sentrux"
        || path.starts_with(".sentrux/")
        || path == ".sentrux\\"
        || path.starts_with(".sentrux\\")
}

// ══════════════════════════════════════════════════════════════════
//  SCAN
// ══════════════════════════════════════════════════════════════════

pub fn scan_def() -> ToolDef {
    ToolDef {
        name: "scan",
        description: "Scan a directory and compute structural metrics plus scan trust metadata. Must be called before other tools.",
        input_schema: json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Absolute path to the directory to scan" }
            },
            "required": ["path"]
        }),
        min_tier: Tier::Free,
        handler: handle_scan,
        invalidates_evolution: true,
    }
}

fn handle_scan(args: &Value, _tier: &Tier, state: &mut McpState) -> Result<Value, String> {
    let path = args
        .get("path")
        .and_then(|p| p.as_str())
        .ok_or("Missing 'path' argument")?;

    let root = PathBuf::from(path);
    if !root.is_dir() {
        return Err(format!("Not a directory: {path}"));
    }

    let (bundle, scan_identity) = do_scan_with_identity(&root)?;
    let baseline_path = arch::baseline_path(&root);
    let (baseline, baseline_error) = match load_persisted_baseline(&root) {
        Ok(baseline) => (baseline, None),
        Err(error) => (None, Some(error)),
    };
    let (rules_config, config_error) = load_v2_rules_config(state, &root);
    let (_, session_v2_status) = load_session_v2_baseline_status(&root);
    let confidence = build_v2_confidence_report(&bundle.metadata, &rules_config, session_v2_status);

    let result = json!({
        "scanned": path,
        "quality_signal": (bundle.health.quality_signal * 10000.0).round() as u32,
        "files": bundle.snapshot.total_files,
        "lines": bundle.snapshot.total_lines,
        "import_edges": bundle.snapshot.import_graph.len(),
        "scan_trust": scan_trust_json(&bundle.metadata),
        "confidence": confidence,
        "project_shape": project_shape_json(&root, &bundle.snapshot, &rules_config),
        "baseline_loaded": baseline.is_some(),
        "baseline_path": baseline_path,
        "baseline_error": baseline_error,
        "rules_error": config_error,
    });

    update_scan_cache(state, root, bundle, baseline, scan_identity);

    Ok(result)
}

// ══════════════════════════════════════════════════════════════════
//  HEALTH (tier-aware truncation)
// ══════════════════════════════════════════════════════════════════

pub fn health_def() -> ToolDef {
    ToolDef {
        name: "health",
        description: "Get legacy structural context with root-cause breakdown and scan trust metadata. Use `findings`, `obligations`, `gate`, and `session_end` for primary v2 patch-safety output.",
        input_schema: json!({ "type": "object", "properties": {} }),
        min_tier: Tier::Free,
        handler: handle_health,
        invalidates_evolution: false,
    }
}

fn handle_health(_args: &Value, tier: &Tier, state: &mut McpState) -> Result<Value, String> {
    let h = state
        .cached_health
        .clone()
        .ok_or("No scan data. Call 'scan' first.")?;
    let metadata = state
        .cached_scan_metadata
        .as_ref()
        .cloned()
        .ok_or("No scan data. Call 'scan' first.")?;
    let root = state
        .scan_root
        .clone()
        .ok_or("No scan root. Call 'scan' first.")?;
    let (baseline, baseline_error) = match state.baseline.clone() {
        Some(baseline) => (Some(baseline), None),
        None => match load_persisted_baseline(&root) {
            Ok(baseline) => (baseline, None),
            Err(error) => (None, Some(error)),
        },
    };
    let baseline_delta = baseline.as_ref().map(|baseline| baseline.diff(&h));
    let (rules_config, config_error) = load_v2_rules_config(state, &root);
    let (_, session_v2_status) = load_session_v2_baseline_status(&root);
    let rc = &h.root_cause_scores;
    let raw = &h.root_cause_raw;
    // Identify the weakest root cause — this is where improvement effort should focus
    let scores_arr = [
        ("modularity", rc.modularity),
        ("acyclicity", rc.acyclicity),
        ("depth", rc.depth),
        ("equality", rc.equality),
        ("redundancy", rc.redundancy),
    ];
    let bottleneck = scores_arr
        .iter()
        .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
        .map(|(name, _)| *name)
        .unwrap_or("none");

    let s = |v: f64| -> u32 { (v * 10000.0).round() as u32 };
    let mut result = json!({
        "kind": "legacy_structural_context",
        "quality_signal": s(h.quality_signal),
        "bottleneck": bottleneck,
        "root_causes": {
            "modularity":  {"score": s(rc.modularity),  "raw": raw.modularity_q},
            "acyclicity":  {"score": s(rc.acyclicity),  "raw": raw.cycle_count},
            "depth":       {"score": s(rc.depth),       "raw": raw.max_depth},
            "equality":    {"score": s(rc.equality),    "raw": raw.complexity_gini},
            "redundancy":  {"score": s(rc.redundancy),  "raw": raw.redundancy_ratio}
        },
        "total_import_edges": h.total_import_edges,
        "cross_module_edges": h.cross_module_edges,
        "scan_trust": scan_trust_json(&metadata),
        "confidence": build_v2_confidence_report(&metadata, &rules_config, session_v2_status),
        "project_shape": project_shape_json(
            &root,
            state.cached_snapshot.as_ref().ok_or("No scan data. Call 'scan' first.")?,
            &rules_config,
        ),
        "baseline_delta": legacy_baseline_delta_json(baseline_delta.as_ref()),
        "baseline_error": baseline_error,
        "rules_error": config_error,
    });

    // Pro: root-cause-organized diagnostics. Tells AI WHERE to focus for each root cause.
    if tier.is_pro() {
        result["diagnostics"] = json!({
            "modularity": {
                "god_files": h.god_files.iter().map(|f| json!({"path": f.path, "fan_out": f.value})).collect::<Vec<_>>(),
                "hotspot_files": h.hotspot_files.iter().map(|f| json!({"path": f.path, "fan_in": f.value})).collect::<Vec<_>>(),
                "most_unstable": h.most_unstable.iter().take(10).map(|m| json!({"path": m.path, "instability": m.instability, "fan_in": m.fan_in, "fan_out": m.fan_out})).collect::<Vec<_>>(),
            },
            "acyclicity": {
                "cycles": h.circular_dep_files.iter().collect::<Vec<_>>(),
            },
            "depth": {
                "max_depth": h.max_depth,
            },
            "equality": {
                "complex_functions": h.complex_functions.iter().take(20).map(|f| json!({"file": f.file, "func": f.func, "cc": f.value})).collect::<Vec<_>>(),
                "cog_complex_functions": h.cog_complex_functions.iter().take(20).map(|f| json!({"file": f.file, "func": f.func, "cog": f.value})).collect::<Vec<_>>(),
                "long_functions": h.long_functions.iter().take(20).map(|f| json!({"file": f.file, "func": f.func, "lines": f.value})).collect::<Vec<_>>(),
                "large_files": h.long_files.iter().take(10).map(|f| json!({"path": f.path, "lines": f.value})).collect::<Vec<_>>(),
                "high_param_functions": h.high_param_functions.iter().take(20).map(|f| json!({"file": f.file, "func": f.func, "params": f.value})).collect::<Vec<_>>(),
            },
            "redundancy": {
                "dead_functions": h.dead_functions.iter().take(50).map(|f| json!({"file": f.file, "func": f.func, "lines": f.value})).collect::<Vec<_>>(),
                "duplicate_groups": h.duplicate_groups.iter().take(20).map(|g| json!({"instances": g.instances.iter().map(|(file, func, lines)| json!({"file": file, "func": func, "lines": lines})).collect::<Vec<_>>()})).collect::<Vec<_>>(),
            },
        });
    } else {
        result["upgrade"] = json!({
            "message": "Upgrade to Pro for root-cause diagnostics: https://github.com/sentrux/sentrux"
        });
    }

    Ok(result)
}

// ══════════════════════════════════════════════════════════════════
//  FINDINGS
// ══════════════════════════════════════════════════════════════════

pub fn findings_def() -> ToolDef {
    ToolDef {
        name: "findings",
        description: "Return primary v2 patch-safety and technical-debt findings for the current scan, with clone drift, concept debt summaries, debt signals, watchpoints, and confidence metadata.",
        input_schema: json!({
            "type": "object",
            "properties": {
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of findings to return (default 10, max 50)"
                }
            }
        }),
        min_tier: Tier::Free,
        handler: handle_findings,
        invalidates_evolution: false,
    }
}

fn handle_findings(args: &Value, _tier: &Tier, state: &mut McpState) -> Result<Value, String> {
    let health = state
        .cached_health
        .clone()
        .ok_or("No scan data. Call 'scan' first.")?;
    let snapshot = state
        .cached_snapshot
        .as_ref()
        .cloned()
        .ok_or("No scan data. Call 'scan' first.")?;
    let root = state
        .scan_root
        .clone()
        .ok_or("No scan root. Call 'scan' first.")?;
    let metadata = state
        .cached_scan_metadata
        .as_ref()
        .cloned()
        .ok_or("No scan data. Call 'scan' first.")?;
    let limit = args
        .get("limit")
        .and_then(|value| value.as_u64())
        .unwrap_or(10)
        .min(50) as usize;
    let (rules_config, config_error) = load_v2_rules_config(state, &root);
    let (_, session_v2_status) = load_session_v2_baseline_status(&root);
    let (clone_payload, clone_error) = clone_findings_for_health(
        state,
        &root,
        &snapshot,
        &health,
        health.duplicate_groups.len(),
    );
    let (semantic_findings, obligations, semantic_error) = semantic_findings_and_obligations(
        state,
        &root,
        Some(&snapshot),
        crate::metrics::v2::ObligationScope::All,
        &BTreeSet::new(),
    );
    let structural_reports =
        crate::metrics::v2::build_structural_debt_reports_with_root(&root, &snapshot, &health);
    let other_findings = combined_other_finding_values(&semantic_findings, &structural_reports);
    let merged_findings = merge_findings(
        clone_payload.prioritized_findings.clone(),
        other_findings,
        usize::MAX,
    );
    let (suppression_application, suppression_error) =
        apply_root_suppressions(state, &root, merged_findings);
    let (visible_findings, experimental_findings) =
        partition_experimental_findings(&suppression_application.visible_findings, limit);
    let visible_findings = visible_findings
        .into_iter()
        .map(|finding| decorate_finding_with_classification(&finding))
        .collect::<Vec<_>>();
    let experimental_findings = experimental_findings
        .into_iter()
        .map(|finding| decorate_finding_with_classification(&finding))
        .collect::<Vec<_>>();
    let visible_clone_ids = visible_clone_ids(&visible_findings);
    let semantic_finding_count = visible_findings
        .iter()
        .filter(|finding| finding.get("concept_id").is_some())
        .count();
    let findings = visible_findings
        .iter()
        .take(limit)
        .cloned()
        .collect::<Vec<_>>();
    let finding_details = build_finding_details(&visible_findings, limit);
    let clone_families = filter_clone_values_by_visible_clone_ids(
        clone_payload.families,
        &visible_clone_ids,
        "clone_ids",
        limit.min(10),
    );
    let clone_remediations = filter_clone_values_by_visible_clone_ids(
        clone_payload.remediation_hints,
        &visible_clone_ids,
        "clone_ids",
        limit.min(10),
    );
    let debt_outputs = build_debt_report_outputs(
        state,
        &root,
        &snapshot,
        &health,
        &visible_findings,
        &obligations,
        &clone_families,
        &BTreeSet::new(),
        limit.min(5),
    );
    let concept_summary_count = debt_outputs.concept_summaries.len();
    let debt_signal_count = debt_outputs.debt_signals.len();
    let experimental_debt_signal_count = debt_outputs.experimental_debt_signals.len();
    let debt_cluster_count = debt_outputs.debt_clusters.len();
    let watchpoint_count = debt_outputs.watchpoints.len();
    let concept_summaries = debt_outputs.concept_summaries;
    let debt_signals = debt_outputs.debt_signals;
    let experimental_debt_signals = debt_outputs.experimental_debt_signals;
    let debt_clusters = debt_outputs.debt_clusters;
    let watchpoints = debt_outputs.watchpoints;
    let legacy_quality_opportunities = legacy_quality_opportunity_values(&debt_signals);
    let legacy_optimization_priorities = legacy_optimization_priority_values(&watchpoints);
    let debt_context_error = debt_outputs.context_error;

    let mut result = serde_json::Map::new();
    result.insert("kind".to_string(), json!("mixed_findings"));
    result.insert(
        "confidence".to_string(),
        json!(build_v2_confidence_report(
            &metadata,
            &rules_config,
            session_v2_status
        )),
    );
    result.insert(
        "project_shape".to_string(),
        project_shape_json(&root, &snapshot, &rules_config),
    );
    result.insert(
        "clone_group_count".to_string(),
        json!(clone_payload.clone_group_count),
    );
    result.insert(
        "clone_family_count".to_string(),
        json!(clone_payload.clone_family_count),
    );
    result.insert(
        "visible_clone_group_count".to_string(),
        json!(visible_clone_ids.len()),
    );
    result.insert(
        "visible_clone_family_count".to_string(),
        json!(clone_families.len()),
    );
    result.insert("clone_families".to_string(), json!(clone_families));
    result.insert("clone_remediations".to_string(), json!(clone_remediations));
    result.insert(
        "concept_summary_count".to_string(),
        json!(concept_summary_count),
    );
    result.insert("concept_summaries".to_string(), json!(concept_summaries));
    result.insert("debt_signal_count".to_string(), json!(debt_signal_count));
    result.insert("debt_signals".to_string(), json!(debt_signals));
    result.insert(
        "experimental_debt_signal_count".to_string(),
        json!(experimental_debt_signal_count),
    );
    result.insert(
        "experimental_debt_signals".to_string(),
        json!(experimental_debt_signals),
    );
    result.insert("debt_cluster_count".to_string(), json!(debt_cluster_count));
    result.insert("debt_clusters".to_string(), json!(debt_clusters));
    result.insert("watchpoint_count".to_string(), json!(watchpoint_count));
    result.insert("watchpoints".to_string(), json!(watchpoints));
    result.insert(
        "quality_opportunity_count".to_string(),
        json!(debt_signal_count),
    );
    result.insert(
        "quality_opportunities".to_string(),
        json!(legacy_quality_opportunities),
    );
    result.insert(
        "optimization_priority_count".to_string(),
        json!(watchpoint_count),
    );
    result.insert(
        "optimization_priorities".to_string(),
        json!(legacy_optimization_priorities),
    );
    result.insert(
        "semantic_finding_count".to_string(),
        json!(semantic_finding_count),
    );
    result.insert(
        "finding_detail_count".to_string(),
        json!(finding_details.len()),
    );
    result.insert("finding_details".to_string(), json!(finding_details));
    result.insert(
        "experimental_finding_count".to_string(),
        json!(experimental_findings.len()),
    );
    result.insert(
        "experimental_findings".to_string(),
        json!(experimental_findings),
    );
    result.insert(
        "rules_error".to_string(),
        json!(merge_optional_errors(config_error, suppression_error)),
    );
    result.insert(
        "semantic_error".to_string(),
        json!(merge_optional_errors(semantic_error, clone_error)),
    );
    result.insert("debt_context_error".to_string(), json!(debt_context_error));
    result.insert(
        "opportunity_context_error".to_string(),
        json!(debt_context_error),
    );
    result.insert(
        "suppression_hits".to_string(),
        json!(suppression_application.active_matches),
    );
    result.insert(
        "suppressed_finding_count".to_string(),
        json!(suppression_match_count(
            &suppression_application.active_matches
        )),
    );
    result.insert(
        "expired_suppressions".to_string(),
        json!(suppression_application.expired_matches),
    );
    result.insert(
        "expired_suppression_match_count".to_string(),
        json!(suppression_match_count(
            &suppression_application.expired_matches
        )),
    );
    result.insert("findings".to_string(), json!(findings));
    Ok(Value::Object(result))
}

pub fn obligations_def() -> ToolDef {
    ToolDef {
        name: "obligations",
        description: "Return required update sites for configured v2 concepts and conservative closed-domain exhaustiveness gaps.",
        input_schema: json!({
            "type": "object",
            "properties": {
                "scope": {
                    "type": "string",
                    "enum": ["all", "changed"],
                    "description": "Show all obligations or only obligations touched by working-tree changes (default all)."
                },
                "concept": {
                    "type": "string",
                    "description": "Optional concept id filter."
                },
                "file": {
                    "type": "string",
                    "description": "Optional file filter."
                },
                "symbol": {
                    "type": "string",
                    "description": "Optional closed-domain symbol filter."
                }
            }
        }),
        min_tier: Tier::Free,
        handler: handle_obligations,
        invalidates_evolution: false,
    }
}

fn handle_obligations(args: &Value, _tier: &Tier, state: &mut McpState) -> Result<Value, String> {
    let root = state
        .scan_root
        .clone()
        .ok_or("No scan root. Call 'scan' first.")?;
    let scope = match args.get("scope").and_then(|value| value.as_str()) {
        Some("changed") => crate::metrics::v2::ObligationScope::Changed,
        _ => crate::metrics::v2::ObligationScope::All,
    };
    let changed_files = if scope == crate::metrics::v2::ObligationScope::Changed {
        refresh_changed_scope(state, &root)?
    } else {
        BTreeSet::new()
    };
    let concept_filter = args.get("concept").and_then(|value| value.as_str());
    let file_filter = args.get("file").and_then(|value| value.as_str());
    let symbol_filter = args.get("symbol").and_then(|value| value.as_str());
    let cached_snapshot = state.cached_snapshot.clone();

    let (_, obligations, semantic_error) = semantic_findings_and_obligations(
        state,
        &root,
        cached_snapshot.as_deref(),
        scope,
        &changed_files,
    );
    let obligations = obligations
        .into_iter()
        .filter(|obligation| {
            concept_filter
                .map(|concept| obligation.concept_id.as_deref() == Some(concept))
                .unwrap_or(true)
        })
        .filter(|obligation| {
            file_filter
                .map(|file| obligation.files.iter().any(|candidate| candidate == file))
                .unwrap_or(true)
        })
        .filter(|obligation| {
            symbol_filter
                .map(|symbol| obligation.domain_symbol_name.as_deref() == Some(symbol))
                .unwrap_or(true)
        })
        .collect::<Vec<_>>();
    let changed_concepts = crate::metrics::v2::changed_concepts_from_obligations(&obligations);
    let obligation_count = obligations.len();
    let missing_site_count: usize = obligations
        .iter()
        .map(|obligation| obligation.missing_sites.len())
        .sum();
    let context_burden: usize = obligations
        .iter()
        .map(|obligation| obligation.context_burden)
        .sum();
    let obligation_completeness_0_10000 =
        crate::metrics::v2::obligation_score_0_10000(&obligations);

    Ok(json!({
        "kind": "obligations",
        "scope": if scope == crate::metrics::v2::ObligationScope::Changed { "changed" } else { "all" },
        "changed_files": changed_files.iter().cloned().collect::<Vec<_>>(),
        "changed_concepts": changed_concepts,
        "obligation_count": obligation_count,
        "missing_site_count": missing_site_count,
        "context_burden": context_burden,
        "obligation_completeness_0_10000": obligation_completeness_0_10000,
        "semantic_error": semantic_error,
        "obligations": obligations
    }))
}

pub fn parity_def() -> ToolDef {
    ToolDef {
        name: "parity",
        description: "Return explicit contract parity analysis as supporting context for configured v2 contracts.",
        input_schema: json!({
            "type": "object",
            "properties": {
                "scope": {
                    "type": "string",
                    "enum": ["all", "changed"],
                    "description": "Show all configured contracts or only contracts touched by current changes (default all)."
                },
                "contract": {
                    "type": "string",
                    "description": "Optional contract id filter."
                }
            }
        }),
        min_tier: Tier::Free,
        handler: handle_parity,
        invalidates_evolution: false,
    }
}

fn handle_parity(args: &Value, _tier: &Tier, state: &mut McpState) -> Result<Value, String> {
    let root = state
        .scan_root
        .clone()
        .ok_or("No scan root. Call 'scan' first.")?;
    let scope = match args.get("scope").and_then(|value| value.as_str()) {
        Some("changed") => crate::metrics::v2::ParityScope::Changed,
        _ => crate::metrics::v2::ParityScope::All,
    };
    let contract_filter = args.get("contract").and_then(|value| value.as_str());

    let changed_files = if scope == crate::metrics::v2::ParityScope::Changed {
        refresh_changed_scope(state, &root)?
    } else {
        BTreeSet::new()
    };

    let (config, rules_error) = load_v2_rules_config(state, &root);
    let (reports, semantic_error) = match analyze_semantic_snapshot(state, &root) {
        Ok(Some(semantic)) => (
            crate::metrics::v2::build_parity_reports(
                &config,
                &semantic,
                &root,
                scope,
                &changed_files,
            ),
            None,
        ),
        Ok(None) => (
            Vec::new(),
            (!config.contract.is_empty()).then(|| {
                "Contract parity requires TypeScript semantic analysis for configured contracts"
                    .to_string()
            }),
        ),
        Err(error) => (Vec::new(), Some(error)),
    };
    let reports = reports
        .into_iter()
        .filter(|report| {
            contract_filter
                .map(|contract| report.id == contract)
                .unwrap_or(true)
        })
        .collect::<Vec<_>>();
    let findings = crate::metrics::v2::build_parity_findings(&reports);
    let (suppression_application, suppression_rules_error) =
        apply_root_suppressions(state, &root, serialized_values(&findings));
    let missing_cell_count = reports
        .iter()
        .map(|report| report.missing_cells.len())
        .sum::<usize>();
    let assessable_cell_count = reports
        .iter()
        .map(|report| report.satisfied_cells.len() + report.missing_cells.len())
        .sum::<usize>();
    let parity_score_0_10000 = if assessable_cell_count == 0 {
        None
    } else {
        Some(crate::metrics::v2::parity_score_0_10000(&reports))
    };

    Ok(json!({
        "kind": "parity",
        "scope": if scope == crate::metrics::v2::ParityScope::Changed { "changed" } else { "all" },
        "changed_files": changed_files.iter().cloned().collect::<Vec<_>>(),
        "contract_count": reports.len(),
        "assessable_cell_count": assessable_cell_count,
        "missing_cell_count": missing_cell_count,
        "parity_score_0_10000": parity_score_0_10000,
        "rules_error": merge_optional_errors(rules_error, suppression_rules_error),
        "semantic_error": semantic_error,
        "suppression_hits": suppression_application.active_matches,
        "suppressed_finding_count": suppression_match_count(&suppression_application.active_matches),
        "expired_suppressions": suppression_application.expired_matches,
        "expired_suppression_match_count": suppression_match_count(&suppression_application.expired_matches),
        "findings": suppression_application.visible_findings,
        "reports": reports,
    }))
}

pub fn concentration_def() -> ToolDef {
    ToolDef {
        name: "concentration",
        description: "Rank coordination hotspots using static file features, concept writes, complexity, and optional git churn context.",
        input_schema: json!({
            "type": "object",
            "properties": {
                "scope": {
                    "type": "string",
                    "enum": ["all", "changed"],
                    "description": "Show all files or only currently changed files (default all)."
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of hotspot findings to return (default 10)."
                },
                "days": {
                    "type": "integer",
                    "description": "Optional git lookback window in days for churn context."
                }
            }
        }),
        min_tier: Tier::Free,
        handler: handle_concentration,
        invalidates_evolution: false,
    }
}

fn handle_concentration(args: &Value, _tier: &Tier, state: &mut McpState) -> Result<Value, String> {
    let root = state
        .scan_root
        .clone()
        .ok_or("No scan root. Call 'scan' first.")?;
    let scope = match args.get("scope").and_then(|value| value.as_str()) {
        Some("changed") => "changed",
        _ => "all",
    };
    let limit = args
        .get("limit")
        .and_then(|value| value.as_u64())
        .map(|value| value as usize)
        .unwrap_or(10);
    let lookback_days = args
        .get("days")
        .and_then(|value| value.as_u64())
        .map(|value| value as u32);

    let changed_files = if scope == "changed" {
        refresh_changed_scope(state, &root)?
    } else {
        BTreeSet::new()
    };
    let snapshot = state
        .cached_snapshot
        .as_ref()
        .ok_or("No scan data. Call 'scan' first.")?;
    let mut file_paths = crate::app::mcp_server::handlers_evo::build_known_files(snapshot)
        .into_iter()
        .collect::<BTreeSet<_>>();
    if scope == "changed" {
        file_paths.retain(|path| changed_files.contains(path));
    }
    let complexity_map = crate::app::mcp_server::handlers_evo::build_complexity_map(snapshot);

    let (config, rules_error) = load_v2_rules_config(state, &root);
    let (semantic, semantic_error) = match analyze_semantic_snapshot(state, &root) {
        Ok(semantic) => (semantic, None),
        Err(error) => (None, Some(error)),
    };
    let (history, evolution_error) = concentration_history(state, &root, lookback_days);
    let reports = crate::metrics::v2::build_concentration_reports(
        &root,
        &file_paths,
        &complexity_map,
        &config,
        semantic.as_ref(),
        history.as_ref(),
    );
    let findings = crate::metrics::v2::build_concentration_findings(&reports, limit);
    let (suppression_application, suppression_rules_error) =
        apply_root_suppressions(state, &root, serialized_values(&findings));
    let top_reports = reports.iter().take(limit).cloned().collect::<Vec<_>>();

    Ok(json!({
        "kind": "concentration",
        "scope": scope,
        "changed_files": changed_files.iter().cloned().collect::<Vec<_>>(),
        "report_count": reports.len(),
        "finding_count": findings.len(),
        "rules_error": merge_optional_errors(rules_error, suppression_rules_error),
        "semantic_error": semantic_error,
        "evolution_error": evolution_error,
        "suppression_hits": suppression_application.active_matches,
        "suppressed_finding_count": suppression_match_count(&suppression_application.active_matches),
        "expired_suppressions": suppression_application.expired_matches,
        "expired_suppression_match_count": suppression_match_count(&suppression_application.expired_matches),
        "findings": suppression_application.visible_findings,
        "reports": top_reports,
    }))
}

pub fn state_def() -> ToolDef {
    ToolDef {
        name: "state",
        description: "Return conservative state-integrity analysis for configured state models using closed-domain coverage and obligation completeness.",
        input_schema: json!({
            "type": "object",
            "properties": {
                "scope": {
                    "type": "string",
                    "enum": ["all", "changed"],
                    "description": "Show all configured state models or only state models touched by current changes (default all)."
                },
                "id": {
                    "type": "string",
                    "description": "Optional state model id filter."
                }
            }
        }),
        min_tier: Tier::Free,
        handler: handle_state,
        invalidates_evolution: false,
    }
}

fn handle_state(args: &Value, _tier: &Tier, state: &mut McpState) -> Result<Value, String> {
    let root = state
        .scan_root
        .clone()
        .ok_or("No scan root. Call 'scan' first.")?;
    let scope = match args.get("scope").and_then(|value| value.as_str()) {
        Some("changed") => crate::metrics::v2::StateScope::Changed,
        _ => crate::metrics::v2::StateScope::All,
    };
    let state_filter = args.get("id").and_then(|value| value.as_str());
    let changed_files = if scope == crate::metrics::v2::StateScope::Changed {
        refresh_changed_scope(state, &root)?
    } else {
        BTreeSet::new()
    };

    let (config, rules_error) = load_v2_rules_config(state, &root);
    let (reports, semantic_error) = match analyze_semantic_snapshot(state, &root) {
        Ok(Some(semantic)) => {
            let obligation_scope = if scope == crate::metrics::v2::StateScope::Changed {
                crate::metrics::v2::ObligationScope::Changed
            } else {
                crate::metrics::v2::ObligationScope::All
            };
            let obligations = crate::metrics::v2::build_obligations(
                &config,
                &semantic,
                obligation_scope,
                &changed_files,
            );
            (
                crate::metrics::v2::build_state_integrity_reports(
                    &config,
                    &semantic,
                    &obligations,
                    scope,
                    &changed_files,
                ),
                None,
            )
        }
        Ok(None) => (
            Vec::new(),
            (!config.state_model.is_empty()).then(|| {
                "State integrity analysis requires TypeScript semantic analysis for configured state models"
                    .to_string()
            }),
        ),
        Err(error) => (Vec::new(), Some(error)),
    };
    let reports = reports
        .into_iter()
        .filter(|report| {
            state_filter
                .map(|state_model_id| report.id == state_model_id)
                .unwrap_or(true)
        })
        .collect::<Vec<_>>();
    let findings = crate::metrics::v2::build_state_integrity_findings(&reports);
    let (suppression_application, suppression_rules_error) =
        apply_root_suppressions(state, &root, serialized_values(&findings));
    let state_integrity_score_0_10000 = if reports.is_empty() {
        None
    } else {
        Some(crate::metrics::v2::state_integrity_score_0_10000(&reports))
    };
    let missing_variant_count = reports
        .iter()
        .map(|report| report.missing_variants.len())
        .sum::<usize>();
    let missing_site_count = reports
        .iter()
        .map(|report| report.missing_sites.len())
        .sum::<usize>();
    let transition_site_count = reports
        .iter()
        .map(|report| report.transition_sites.len())
        .sum::<usize>();
    let transition_gap_count = reports
        .iter()
        .map(|report| report.transition_gap_sites.len())
        .sum::<usize>();

    Ok(json!({
        "kind": "state",
        "scope": if scope == crate::metrics::v2::StateScope::Changed { "changed" } else { "all" },
        "changed_files": changed_files.iter().cloned().collect::<Vec<_>>(),
        "state_model_count": reports.len(),
        "finding_count": findings.len(),
        "missing_variant_count": missing_variant_count,
        "missing_site_count": missing_site_count,
        "transition_site_count": transition_site_count,
        "transition_gap_count": transition_gap_count,
        "state_integrity_score_0_10000": state_integrity_score_0_10000,
        "rules_error": merge_optional_errors(rules_error, suppression_rules_error),
        "semantic_error": semantic_error,
        "suppression_hits": suppression_application.active_matches,
        "suppressed_finding_count": suppression_match_count(&suppression_application.active_matches),
        "expired_suppressions": suppression_application.expired_matches,
        "expired_suppression_match_count": suppression_match_count(&suppression_application.expired_matches),
        "findings": suppression_application.visible_findings,
        "reports": reports,
    }))
}

fn merge_findings(
    clone_findings: Vec<Value>,
    other_findings: Vec<Value>,
    limit: usize,
) -> Vec<Value> {
    let mut merged: Vec<(u8, Value)> = other_findings
        .into_iter()
        .map(|finding| {
            let severity = finding
                .get("severity")
                .and_then(|value| value.as_str())
                .unwrap_or("low");
            (severity_priority(severity), finding)
        })
        .collect();
    merged.extend(clone_findings.into_iter().map(|finding| {
        let severity = finding
            .get("severity")
            .and_then(|value| value.as_str())
            .unwrap_or("low");
        (severity_priority(severity), finding)
    }));
    merged.sort_by(|left, right| right.0.cmp(&left.0));
    merged
        .into_iter()
        .take(limit)
        .map(|(_, finding)| finding)
        .collect()
}

fn severity_priority(severity: &str) -> u8 {
    match severity {
        "high" => 3,
        "medium" => 2,
        "low" => 1,
        _ => 0,
    }
}

#[derive(Debug, Clone, serde::Serialize, Default)]
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

#[derive(Debug, Clone, serde::Serialize, Default)]
struct DebtSignal {
    kind: String,
    trust_tier: String,
    presentation_class: String,
    leverage_class: String,
    scope: String,
    signal_class: String,
    signal_families: Vec<String>,
    severity: String,
    score_0_10000: u32,
    summary: String,
    impact: String,
    files: Vec<String>,
    role_tags: Vec<String>,
    leverage_reasons: Vec<String>,
    evidence: Vec<String>,
    inspection_focus: Vec<String>,
    candidate_split_axes: Vec<String>,
    related_surfaces: Vec<String>,
    metrics: DebtSignalMetrics,
}

#[derive(Debug, Clone, serde::Serialize, Default)]
struct FindingDetail {
    kind: String,
    trust_tier: String,
    presentation_class: String,
    leverage_class: String,
    scope: String,
    severity: String,
    summary: String,
    impact: String,
    files: Vec<String>,
    role_tags: Vec<String>,
    leverage_reasons: Vec<String>,
    evidence: Vec<String>,
    inspection_focus: Vec<String>,
    candidate_split_axes: Vec<String>,
    related_surfaces: Vec<String>,
    metrics: FindingDetailMetrics,
}

#[derive(Debug, Clone, serde::Serialize, Default)]
struct InspectionWatchpoint {
    kind: String,
    trust_tier: String,
    presentation_class: String,
    leverage_class: String,
    scope: String,
    severity: String,
    score_0_10000: u32,
    summary: String,
    impact: String,
    files: Vec<String>,
    role_tags: Vec<String>,
    leverage_reasons: Vec<String>,
    evidence: Vec<String>,
    inspection_focus: Vec<String>,
    candidate_split_axes: Vec<String>,
    related_surfaces: Vec<String>,
    signal_families: Vec<String>,
    clone_family_count: usize,
    hotspot_count: usize,
    missing_site_count: usize,
    boundary_pressure_count: usize,
}

#[derive(Debug, Clone, serde::Serialize, Default)]
struct DebtCluster {
    trust_tier: String,
    presentation_class: String,
    leverage_class: String,
    scope: String,
    severity: String,
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
struct DebtReportOutputs {
    concept_summaries: Vec<ConceptDebtSummary>,
    debt_signals: Vec<DebtSignal>,
    experimental_debt_signals: Vec<DebtSignal>,
    debt_clusters: Vec<DebtCluster>,
    watchpoints: Vec<InspectionWatchpoint>,
    context_error: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, Default)]
struct DebtSignalMetrics {
    #[serde(skip_serializing_if = "Option::is_none")]
    finding_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    high_severity_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    boundary_pressure_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    obligation_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    missing_site_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    context_burden: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    file_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    line_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    function_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    member_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    recently_touched_file_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    fan_in: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    fan_out: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    instability_0_10000: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    dead_symbol_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    dead_line_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cycle_size: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cut_candidate_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    largest_cycle_after_best_cut: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    inbound_reference_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    public_surface_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reachable_from_tests: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    guardrail_test_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    role_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    divergence_score: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    family_score_0_10000: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    authority_breadth: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    side_effect_breadth: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    timer_retry_weight: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    async_branch_weight: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_complexity: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    churn_commits: Option<u32>,
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

#[derive(Debug, Clone, serde::Serialize, Default)]
struct FindingDetailMetrics {
    file_count: usize,
    evidence_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    member_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    family_score_0_10000: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    divergence_score: Option<u32>,
}

#[allow(clippy::too_many_arguments)]
fn backfill_leverage_fields(
    leverage_class: &mut String,
    leverage_reasons: &mut Vec<String>,
    kind: &str,
    trust_tier: &str,
    presentation_class: &str,
    role_tags: &[String],
    fan_in: Option<usize>,
    fan_out: Option<usize>,
    line_count: Option<usize>,
    max_complexity: Option<usize>,
    cycle_size: Option<usize>,
    cut_candidate_count: Option<usize>,
    guardrail_test_count: Option<usize>,
    boundary_pressure_count: usize,
    missing_site_count: usize,
) {
    if leverage_class.is_empty() {
        *leverage_class = classify_leverage_class(
            kind,
            trust_tier,
            presentation_class,
            role_tags,
            fan_in,
            fan_out,
            line_count,
            max_complexity,
            cycle_size,
            cut_candidate_count,
            guardrail_test_count,
            boundary_pressure_count,
            missing_site_count,
        );
    }

    if leverage_reasons.is_empty() {
        *leverage_reasons = classify_leverage_reasons(
            kind,
            trust_tier,
            presentation_class,
            leverage_class,
            role_tags,
            fan_in,
            fan_out,
            cycle_size,
            cut_candidate_count,
            guardrail_test_count,
            boundary_pressure_count,
            missing_site_count,
        );
    }
}

fn annotate_debt_signal(mut signal: DebtSignal) -> DebtSignal {
    let fan_in = signal
        .metrics
        .fan_in
        .or(signal.metrics.inbound_reference_count);
    backfill_leverage_fields(
        &mut signal.leverage_class,
        &mut signal.leverage_reasons,
        &signal.kind,
        &signal.trust_tier,
        &signal.presentation_class,
        &signal.role_tags,
        fan_in,
        signal.metrics.fan_out,
        signal.metrics.line_count,
        signal.metrics.max_complexity.map(|value| value as usize),
        signal.metrics.cycle_size,
        signal.metrics.cut_candidate_count,
        signal.metrics.guardrail_test_count,
        signal.metrics.boundary_pressure_count.unwrap_or(0),
        signal.metrics.missing_site_count.unwrap_or(0),
    );
    signal
}

fn annotate_finding_detail(mut detail: FindingDetail) -> FindingDetail {
    let fan_in = detail
        .evidence
        .iter()
        .find_map(|entry| entry.strip_prefix("fan-in: "))
        .and_then(|value| value.parse::<usize>().ok());
    let fan_out = detail
        .evidence
        .iter()
        .find_map(|entry| entry.strip_prefix("fan-out: "))
        .and_then(|value| value.parse::<usize>().ok());
    let cycle_size = detail
        .evidence
        .iter()
        .find_map(|entry| entry.strip_prefix("cycle size: "))
        .and_then(|value| value.parse::<usize>().ok());
    let cut_candidate_count = detail
        .evidence
        .iter()
        .find_map(|entry| entry.strip_prefix("candidate cuts: "))
        .and_then(|value| value.parse::<usize>().ok());
    backfill_leverage_fields(
        &mut detail.leverage_class,
        &mut detail.leverage_reasons,
        &detail.kind,
        &detail.trust_tier,
        &detail.presentation_class,
        &detail.role_tags,
        fan_in,
        fan_out,
        None,
        None,
        cycle_size,
        cut_candidate_count,
        None,
        0,
        0,
    );
    detail
}

fn annotate_inspection_watchpoint(mut watchpoint: InspectionWatchpoint) -> InspectionWatchpoint {
    backfill_leverage_fields(
        &mut watchpoint.leverage_class,
        &mut watchpoint.leverage_reasons,
        &watchpoint.kind,
        &watchpoint.trust_tier,
        &watchpoint.presentation_class,
        &watchpoint.role_tags,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        watchpoint.boundary_pressure_count,
        watchpoint.missing_site_count,
    );
    watchpoint
}

fn debt_signal_candidate_files(
    findings: &[Value],
    obligations: &[crate::metrics::v2::ObligationReport],
    clone_families: &[Value],
    extra_files: &BTreeSet<String>,
) -> BTreeSet<String> {
    let mut files = BTreeSet::new();
    for finding in findings {
        files.extend(finding_files(finding));
    }
    for obligation in obligations {
        files.extend(obligation.files.iter().cloned());
    }
    for family in clone_families {
        files.extend(finding_files(family));
    }
    files.extend(extra_files.iter().cloned());
    files
}

fn build_debt_report_outputs(
    state: &mut McpState,
    root: &Path,
    snapshot: &Snapshot,
    health: &metrics::HealthReport,
    findings: &[Value],
    obligations: &[crate::metrics::v2::ObligationReport],
    clone_families: &[Value],
    extra_files: &BTreeSet<String>,
    limit: usize,
) -> DebtReportOutputs {
    let candidate_files =
        debt_signal_candidate_files(findings, obligations, clone_families, extra_files);
    let (concentration_reports, context_error) =
        debt_signal_concentration_reports(state, root, snapshot, &candidate_files);
    let concept_summaries = build_concept_debt_summaries(findings, obligations);
    let structural_reports = structural_reports_for_scope(root, snapshot, health, extra_files);
    let all_debt_signals = collect_debt_signals(
        &concept_summaries,
        &structural_reports,
        findings,
        clone_families,
        &concentration_reports,
    );
    let trusted_debt_signals = all_debt_signals
        .iter()
        .filter(|signal| signal.trust_tier == "trusted")
        .cloned()
        .collect::<Vec<_>>();
    let watchpoint_signals = all_debt_signals
        .iter()
        .filter(|signal| signal.trust_tier == "watchpoint")
        .cloned()
        .collect::<Vec<_>>();
    let experimental_debt_signals = all_debt_signals
        .iter()
        .filter(|signal| signal.trust_tier == "experimental")
        .cloned()
        .collect::<Vec<_>>();
    let debt_signals = truncate_debt_signals(trusted_debt_signals.clone(), limit);
    let cluster_sources = trusted_debt_signals
        .iter()
        .chain(watchpoint_signals.iter())
        .cloned()
        .collect::<Vec<_>>();
    let debt_clusters = build_debt_clusters(&cluster_sources, limit);
    let concept_watchpoints = build_inspection_watchpoints(
        &concept_summaries,
        clone_families,
        &concentration_reports,
        limit,
    );
    let watchpoints = merge_watchpoints(
        concept_watchpoints,
        debt_signal_watchpoints(&watchpoint_signals, limit),
        limit,
    );

    DebtReportOutputs {
        concept_summaries: concept_summaries.into_iter().take(limit).collect(),
        debt_signals,
        experimental_debt_signals: truncate_debt_signals(experimental_debt_signals, limit),
        debt_clusters,
        watchpoints,
        context_error,
    }
}

fn structural_reports_for_scope(
    root: &Path,
    snapshot: &Snapshot,
    health: &metrics::HealthReport,
    scope_files: &BTreeSet<String>,
) -> Vec<crate::metrics::v2::StructuralDebtReport> {
    let reports =
        crate::metrics::v2::build_structural_debt_reports_with_root(root, snapshot, health);
    if scope_files.is_empty() {
        return reports;
    }

    reports
        .into_iter()
        .filter(|report| report.files.iter().any(|path| scope_files.contains(path)))
        .collect()
}

fn build_concept_debt_summaries(
    findings: &[Value],
    obligations: &[crate::metrics::v2::ObligationReport],
) -> Vec<ConceptDebtSummary> {
    let mut aggregates = BTreeMap::<String, ConceptDebtAggregate>::new();

    for finding in findings {
        let Some(concept_id) = finding_concept_id(finding) else {
            continue;
        };
        let entry = aggregates.entry(concept_id.to_string()).or_default();
        let kind = finding_kind(finding).to_string();
        entry.finding_count += 1;
        if severity_of_value(finding) == "high" {
            entry.high_severity_count += 1;
        }
        if matches!(
            kind.as_str(),
            "multi_writer_concept"
                | "forbidden_writer"
                | "writer_outside_allowlist"
                | "forbidden_raw_read"
                | "authoritative_import_bypass"
                | "concept_boundary_pressure"
        ) {
            entry.boundary_pressure_count += 1;
        }
        entry
            .kinds
            .entry(kind)
            .and_modify(|count| *count += 1)
            .or_insert(1);
        entry.files.extend(finding_files(finding));
    }

    for obligation in obligations {
        let Some(concept_id) = obligation.concept_id.as_ref() else {
            continue;
        };
        let entry = aggregates.entry(concept_id.clone()).or_default();
        entry.obligation_count += 1;
        entry.missing_site_count += obligation.missing_sites.len();
        entry.context_burden += obligation.context_burden;
        entry.files.extend(obligation.files.iter().cloned());
    }

    let mut summaries = aggregates
        .into_iter()
        .map(|(concept_id, aggregate)| {
            let ConceptDebtAggregate {
                finding_count,
                high_severity_count,
                boundary_pressure_count,
                obligation_count,
                missing_site_count,
                context_burden,
                kinds,
                files,
            } = aggregate;
            let mut dominant_kinds = kinds.into_iter().collect::<Vec<_>>();
            dominant_kinds
                .sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
            let dominant_kinds = dominant_kinds
                .into_iter()
                .map(|(kind, _)| kind)
                .take(3)
                .collect::<Vec<_>>();
            let score_0_10000 = concept_debt_score(
                finding_count,
                high_severity_count,
                boundary_pressure_count,
                missing_site_count,
                context_burden,
            );
            let inspection_focus =
                concept_debt_inspection_focus(&dominant_kinds, missing_site_count > 0);

            ConceptDebtSummary {
                summary: concept_debt_summary(
                    &concept_id,
                    finding_count,
                    obligation_count,
                    missing_site_count,
                    high_severity_count,
                    boundary_pressure_count,
                ),
                concept_id,
                score_0_10000,
                finding_count,
                high_severity_count,
                boundary_pressure_count,
                obligation_count,
                missing_site_count,
                context_burden,
                dominant_kinds,
                files: files.into_iter().collect(),
                inspection_focus,
            }
        })
        .filter(|summary| summary.finding_count > 0 || summary.missing_site_count > 0)
        .collect::<Vec<_>>();

    summaries.sort_by(|left, right| {
        right
            .score_0_10000
            .cmp(&left.score_0_10000)
            .then_with(|| right.high_severity_count.cmp(&left.high_severity_count))
            .then_with(|| left.concept_id.cmp(&right.concept_id))
    });
    summaries
}

fn collect_debt_signals(
    concept_summaries: &[ConceptDebtSummary],
    structural_reports: &[crate::metrics::v2::StructuralDebtReport],
    findings: &[Value],
    clone_families: &[Value],
    concentration_reports: &[crate::metrics::v2::ConcentrationReport],
) -> Vec<DebtSignal> {
    let mut covered_hotspot_paths = BTreeSet::new();
    let mut signals = concept_summaries
        .iter()
        .filter(|summary| summary.score_0_10000 >= 2500)
        .map(|summary| {
            let related_hotspots = concentration_reports
                .iter()
                .filter(|report| summary.files.iter().any(|path| path == &report.path))
                .collect::<Vec<_>>();
            covered_hotspot_paths.extend(
                related_hotspots
                    .iter()
                    .map(|report| report.path.clone())
                    .collect::<Vec<_>>(),
            );
            let mut score_0_10000 = summary.score_0_10000;
            if let Some(top_hotspot) = related_hotspots.first() {
                score_0_10000 = (score_0_10000 + top_hotspot.score_0_10000 / 3).min(10_000);
            }
            let mut evidence = summary
                .dominant_kinds
                .iter()
                .map(|kind| format!("finding kind: {kind}"))
                .collect::<Vec<_>>();
            if summary.missing_site_count > 0 {
                evidence.push(format!(
                    "missing update sites: {}",
                    summary.missing_site_count
                ));
            }
            if summary.context_burden > 0 {
                evidence.push(format!("context burden: {}", summary.context_burden));
            }
            if let Some(top_hotspot) = related_hotspots.first() {
                evidence.push(format!("hotspot file: {}", top_hotspot.path));
                evidence.extend(top_hotspot.reasons.iter().cloned().take(2));
            }

            annotate_debt_signal(DebtSignal {
                kind: "concept".to_string(),
                trust_tier: "trusted".to_string(),
                presentation_class: classify_presentation_class(
                    "concept",
                    "trusted",
                    &summary.concept_id,
                    &summary.files,
                    &[],
                    evidence.len(),
                    summary.finding_count,
                    summary.boundary_pressure_count,
                    summary.missing_site_count,
                ),
                leverage_class: String::new(),
                scope: summary.concept_id.clone(),
                signal_class: concept_signal_class(summary).to_string(),
                signal_families: concept_signal_families(summary),
                severity: signal_severity(score_0_10000).to_string(),
                score_0_10000,
                summary: summary.summary.clone(),
                impact: concept_signal_impact(summary),
                files: summary.files.clone(),
                role_tags: Vec::new(),
                leverage_reasons: Vec::new(),
                evidence,
                inspection_focus: summary.inspection_focus.clone(),
                candidate_split_axes: concept_candidate_split_axes(summary),
                related_surfaces: summary.files.iter().take(5).cloned().collect(),
                metrics: concept_signal_metrics(summary),
            })
        })
        .collect::<Vec<_>>();

    let structural_signals = structural_reports
        .iter()
        .map(structural_signal)
        .collect::<Vec<_>>();
    covered_hotspot_paths.extend(
        structural_signals
            .iter()
            .filter(|signal| signal.kind == "unstable_hotspot")
            .flat_map(|signal| signal.files.iter().cloned())
            .collect::<Vec<_>>(),
    );
    signals.extend(structural_signals);

    if !clone_families.is_empty() {
        signals.extend(
            clone_families
                .iter()
                .filter_map(clone_family_signal)
                .collect::<Vec<_>>(),
        );
    } else {
        signals.extend(
            findings
                .iter()
                .filter(|finding| finding_kind(finding) == "exact_clone_group")
                .filter_map(clone_group_signal)
                .collect::<Vec<_>>(),
        );
    }

    signals.extend(
        concentration_reports
            .iter()
            .filter(|report| report.score_0_10000 >= 4000)
            .filter(|report| !covered_hotspot_paths.contains(&report.path))
            .filter_map(hotspot_signal)
            .collect::<Vec<_>>(),
    );

    signals.sort_by(|left, right| {
        severity_priority(&right.severity)
            .cmp(&severity_priority(&left.severity))
            .then_with(|| right.score_0_10000.cmp(&left.score_0_10000))
            .then_with(|| left.scope.cmp(&right.scope))
    });
    signals
}

fn truncate_debt_signals(mut signals: Vec<DebtSignal>, limit: usize) -> Vec<DebtSignal> {
    signals.truncate(limit);
    signals
}

fn build_inspection_watchpoints(
    concept_summaries: &[ConceptDebtSummary],
    clone_families: &[Value],
    concentration_reports: &[crate::metrics::v2::ConcentrationReport],
    limit: usize,
) -> Vec<InspectionWatchpoint> {
    let mut watchpoints = concept_summaries
        .iter()
        .filter_map(|summary| {
            let matching_clone_families = related_clone_families(summary, clone_families);
            let matching_hotspots = related_hotspots(summary, concentration_reports);
            if summary.score_0_10000 < 3000
                && matching_clone_families.is_empty()
                && matching_hotspots.is_empty()
                && summary.boundary_pressure_count == 0
            {
                return None;
            }

            let score_0_10000 = inspection_watchpoint_score(
                summary,
                matching_clone_families.len(),
                matching_hotspots.len(),
            );

            Some(annotate_inspection_watchpoint(InspectionWatchpoint {
                kind: "concept_watchpoint".to_string(),
                trust_tier: "watchpoint".to_string(),
                presentation_class: "watchpoint".to_string(),
                leverage_class: String::new(),
                scope: summary.concept_id.clone(),
                severity: signal_severity(score_0_10000).to_string(),
                score_0_10000,
                summary: inspection_watchpoint_summary(
                    summary,
                    matching_clone_families.len(),
                    matching_hotspots.len(),
                ),
                impact: concept_signal_impact(summary),
                files: summary.files.clone(),
                role_tags: Vec::new(),
                leverage_reasons: Vec::new(),
                evidence: inspection_watchpoint_evidence(
                    summary,
                    matching_clone_families.as_slice(),
                    matching_hotspots.as_slice(),
                ),
                inspection_focus: inspection_watchpoint_focus(
                    summary,
                    matching_clone_families.as_slice(),
                    matching_hotspots.as_slice(),
                ),
                candidate_split_axes: concept_candidate_split_axes(summary),
                related_surfaces: summary.files.iter().take(5).cloned().collect(),
                signal_families: inspection_watchpoint_signal_families(
                    summary,
                    matching_clone_families.len(),
                    matching_hotspots.len(),
                ),
                clone_family_count: matching_clone_families.len(),
                hotspot_count: matching_hotspots.len(),
                missing_site_count: summary.missing_site_count,
                boundary_pressure_count: summary.boundary_pressure_count,
            }))
        })
        .collect::<Vec<_>>();

    watchpoints.sort_by(|left, right| {
        severity_priority(&right.severity)
            .cmp(&severity_priority(&left.severity))
            .then_with(|| right.score_0_10000.cmp(&left.score_0_10000))
            .then_with(|| left.scope.cmp(&right.scope))
    });
    watchpoints.truncate(limit);
    watchpoints
}

fn debt_signal_watchpoints(signals: &[DebtSignal], limit: usize) -> Vec<InspectionWatchpoint> {
    let mut watchpoints = signals
        .iter()
        .filter(|signal| signal.trust_tier == "watchpoint")
        .map(|signal| {
            annotate_inspection_watchpoint(InspectionWatchpoint {
                kind: signal.kind.clone(),
                trust_tier: signal.trust_tier.clone(),
                presentation_class: signal.presentation_class.clone(),
                leverage_class: signal.leverage_class.clone(),
                scope: signal.scope.clone(),
                severity: signal.severity.clone(),
                score_0_10000: signal.score_0_10000,
                summary: signal.summary.clone(),
                impact: signal.impact.clone(),
                files: signal.files.clone(),
                role_tags: signal.role_tags.clone(),
                leverage_reasons: signal.leverage_reasons.clone(),
                evidence: signal.evidence.clone(),
                inspection_focus: signal.inspection_focus.clone(),
                candidate_split_axes: signal.candidate_split_axes.clone(),
                related_surfaces: signal.related_surfaces.clone(),
                signal_families: signal.signal_families.clone(),
                clone_family_count: usize::from(signal.kind == "clone_family"),
                hotspot_count: usize::from(signal.kind == "hotspot"),
                missing_site_count: signal.metrics.missing_site_count.unwrap_or(0),
                boundary_pressure_count: signal.metrics.boundary_pressure_count.unwrap_or(0),
            })
        })
        .collect::<Vec<_>>();

    watchpoints.sort_by(|left, right| {
        severity_priority(&right.severity)
            .cmp(&severity_priority(&left.severity))
            .then_with(|| right.score_0_10000.cmp(&left.score_0_10000))
            .then_with(|| left.scope.cmp(&right.scope))
    });
    watchpoints.truncate(limit);
    watchpoints
}

fn merge_watchpoints(
    left: Vec<InspectionWatchpoint>,
    right: Vec<InspectionWatchpoint>,
    limit: usize,
) -> Vec<InspectionWatchpoint> {
    let mut watchpoints = left;
    watchpoints.extend(right);
    watchpoints.sort_by(|left, right| {
        severity_priority(&right.severity)
            .cmp(&severity_priority(&left.severity))
            .then_with(|| right.score_0_10000.cmp(&left.score_0_10000))
            .then_with(|| left.scope.cmp(&right.scope))
    });
    watchpoints.truncate(limit);
    watchpoints
}

fn build_debt_clusters(signals: &[DebtSignal], limit: usize) -> Vec<DebtCluster> {
    let mut visited = BTreeSet::new();
    let mut clusters = Vec::new();

    for start_index in 0..signals.len() {
        if !visited.insert(start_index) {
            continue;
        }

        let component = debt_cluster_component(start_index, signals, &mut visited);
        if component.len() < 2 {
            continue;
        }

        if let Some(cluster) = debt_cluster(&component) {
            clusters.push(cluster);
        }
    }

    clusters.sort_by(|left, right| {
        severity_priority(&right.severity)
            .cmp(&severity_priority(&left.severity))
            .then_with(|| right.score_0_10000.cmp(&left.score_0_10000))
            .then_with(|| left.scope.cmp(&right.scope))
    });
    clusters.truncate(limit);
    clusters
}

fn debt_cluster_component(
    start_index: usize,
    signals: &[DebtSignal],
    visited: &mut BTreeSet<usize>,
) -> Vec<DebtSignal> {
    let mut queue = vec![start_index];
    let mut component = Vec::new();

    while let Some(index) = queue.pop() {
        let signal = signals[index].clone();
        for next_index in 0..signals.len() {
            if visited.contains(&next_index) {
                continue;
            }
            if files_overlap(&signal.files, &signals[next_index].files) {
                visited.insert(next_index);
                queue.push(next_index);
            }
        }
        component.push(signal);
    }

    component
}

fn debt_cluster(signals: &[DebtSignal]) -> Option<DebtCluster> {
    let files = signals
        .iter()
        .flat_map(|signal| signal.files.iter().cloned())
        .collect::<Vec<_>>();
    let files = dedupe_strings_preserve_order(files);
    if files.is_empty() {
        return None;
    }
    let file_count = files.len();

    let mut signal_kinds = signals
        .iter()
        .map(|signal| signal.kind.clone())
        .collect::<Vec<_>>();
    signal_kinds = dedupe_strings_preserve_order(signal_kinds);

    let mut signal_families = signals
        .iter()
        .flat_map(|signal| signal.signal_families.iter().cloned())
        .collect::<Vec<_>>();
    signal_families = dedupe_strings_preserve_order(signal_families);
    let role_tags = dedupe_strings_preserve_order(
        signals
            .iter()
            .flat_map(|signal| signal.role_tags.iter().cloned())
            .collect::<Vec<_>>(),
    );

    let summary = if files.len() == 1 {
        format!(
            "File '{}' intersects {} debt signals: {}",
            files[0],
            signals.len(),
            signal_kinds.join(", ")
        )
    } else {
        format!(
            "Files {} intersect {} debt signals: {}",
            sample_file_labels(&files, 3),
            signals.len(),
            signal_kinds.join(", ")
        )
    };

    let impact = if signal_families.iter().any(|family| family == "ownership")
        && signal_families.iter().any(|family| family == "propagation")
    {
        "Overlapping ownership drift and propagation burden make partial edits easier to miss and harder to validate.".to_string()
    } else if signal_families.iter().any(|family| family == "duplication")
        && signal_families
            .iter()
            .any(|family| family == "coordination")
    {
        "Duplicated logic inside coordination-heavy seams increases the chance that fixes land in one path but not the others.".to_string()
    } else {
        "Multiple overlapping debt signals in the same surface increase change cost and make regressions harder to isolate.".to_string()
    };

    let mut evidence = vec![
        format!("overlapping signals: {}", signals.len()),
        format!("signal kinds: {}", signal_kinds.join(", ")),
        format!("affected files: {}", files.len()),
    ];
    if !role_tags.is_empty() {
        evidence.push(format!("role tags: {}", role_tags.join(", ")));
    }
    evidence.extend(
        signals
            .iter()
            .take(3)
            .map(|signal| format!("{}: {}", signal.kind, signal.summary)),
    );
    evidence = dedupe_strings_preserve_order(evidence);

    let mut inspection_focus = signals
        .iter()
        .flat_map(|signal| signal.inspection_focus.iter().cloned())
        .collect::<Vec<_>>();
    inspection_focus = dedupe_strings_preserve_order(inspection_focus);
    inspection_focus.truncate(4);

    let highest_score = signals
        .iter()
        .map(|signal| signal.score_0_10000)
        .max()
        .unwrap_or(0);
    let trust_tier = cluster_trust_tier(signals);
    let aggregate_bonus = ((signals.len().saturating_sub(1)) as u32 * 500).min(2000);
    let score_0_10000 = (highest_score + aggregate_bonus).min(10_000);

    Some(DebtCluster {
        trust_tier: trust_tier.to_string(),
        presentation_class: cluster_presentation_class(signals),
        leverage_class: cluster_leverage_class(signals),
        scope: format!("cluster:{}", files.join("|")),
        severity: signal_severity(score_0_10000).to_string(),
        score_0_10000,
        summary,
        impact,
        files,
        role_tags,
        leverage_reasons: cluster_leverage_reasons(signals),
        evidence,
        inspection_focus,
        signal_families: signal_families.clone(),
        signal_kinds: signal_kinds.clone(),
        metrics: DebtClusterMetrics {
            signal_count: signals.len(),
            file_count,
            concept_count: signals
                .iter()
                .filter(|signal| signal.kind == "concept")
                .count(),
            clone_family_count: signals
                .iter()
                .filter(|signal| signal.kind == "clone_family")
                .count(),
            hotspot_count: signals
                .iter()
                .filter(|signal| signal.kind == "hotspot" || signal.kind == "unstable_hotspot")
                .count(),
            structural_signal_count: signals
                .iter()
                .filter(|signal| is_structural_debt_signal_kind(&signal.kind))
                .count(),
        },
    })
}

fn cluster_trust_tier(signals: &[DebtSignal]) -> &'static str {
    if signals.iter().any(|signal| signal.trust_tier == "trusted") {
        "trusted"
    } else if signals
        .iter()
        .any(|signal| signal.trust_tier == "watchpoint")
    {
        "watchpoint"
    } else {
        "experimental"
    }
}

fn presentation_class_rank(classification: &str) -> usize {
    match classification {
        "structural_debt" => 0,
        "guarded_facade" => 1,
        "tooling_debt" => 2,
        "hardening_note" => 3,
        "watchpoint" => 4,
        "experimental" => 5,
        _ => 6,
    }
}

fn leverage_class_rank(classification: &str) -> usize {
    match classification {
        "architecture_signal" => 0,
        "boundary_discipline" => 1,
        "local_refactor_target" => 2,
        "regrowth_watchpoint" => 3,
        "secondary_cleanup" => 4,
        "hardening_note" => 5,
        "tooling_debt" => 6,
        "experimental" => 7,
        _ => 8,
    }
}

fn cluster_presentation_class(signals: &[DebtSignal]) -> String {
    signals
        .iter()
        .map(|signal| signal.presentation_class.as_str())
        .min_by_key(|classification| presentation_class_rank(classification))
        .unwrap_or("structural_debt")
        .to_string()
}

fn cluster_leverage_class(signals: &[DebtSignal]) -> String {
    signals
        .iter()
        .map(|signal| signal.leverage_class.as_str())
        .min_by_key(|classification| leverage_class_rank(classification))
        .unwrap_or("secondary_cleanup")
        .to_string()
}

fn cluster_leverage_reasons(signals: &[DebtSignal]) -> Vec<String> {
    dedupe_strings_preserve_order(
        signals
            .iter()
            .flat_map(|signal| signal.leverage_reasons.iter().cloned())
            .collect(),
    )
}

fn is_structural_debt_signal_kind(kind: &str) -> bool {
    matches!(
        kind,
        "large_file"
            | "dependency_sprawl"
            | "unstable_hotspot"
            | "cycle_cluster"
            | "dead_private_code_cluster"
            | "dead_island"
    )
}

fn sample_file_labels(files: &[String], limit: usize) -> String {
    let sample = files.iter().take(limit).cloned().collect::<Vec<_>>();
    if files.len() <= limit {
        return sample.join(", ");
    }
    format!("{}, and {} more", sample.join(", "), files.len() - limit)
}

fn related_clone_families<'a>(
    summary: &ConceptDebtSummary,
    clone_families: &'a [Value],
) -> Vec<&'a Value> {
    clone_families
        .iter()
        .filter(|family| files_overlap(&summary.files, &finding_files(family)))
        .collect()
}

fn related_hotspots<'a>(
    summary: &ConceptDebtSummary,
    concentration_reports: &'a [crate::metrics::v2::ConcentrationReport],
) -> Vec<&'a crate::metrics::v2::ConcentrationReport> {
    concentration_reports
        .iter()
        .filter(|report| summary.files.iter().any(|path| path == &report.path))
        .collect()
}

fn files_overlap(left: &[String], right: &[String]) -> bool {
    let right_files = right.iter().collect::<BTreeSet<_>>();
    left.iter().any(|path| right_files.contains(path))
}

fn inspection_watchpoint_score(
    summary: &ConceptDebtSummary,
    clone_family_count: usize,
    hotspot_count: usize,
) -> u32 {
    let clone_pressure = (clone_family_count as u32 * 900).min(1800);
    let hotspot_pressure = (hotspot_count as u32 * 700).min(1400);
    let compound_bonus = if summary.boundary_pressure_count > 0 && summary.missing_site_count > 0 {
        900
    } else {
        0
    };

    (summary.score_0_10000 + clone_pressure + hotspot_pressure + compound_bonus).min(10_000)
}

fn inspection_watchpoint_summary(
    summary: &ConceptDebtSummary,
    clone_family_count: usize,
    hotspot_count: usize,
) -> String {
    let mut overlaps = Vec::new();
    if summary.boundary_pressure_count > 0 {
        overlaps.push("boundary pressure");
    }
    if summary.missing_site_count > 0 {
        overlaps.push("propagation burden");
    }
    if clone_family_count > 0 {
        overlaps.push("clone overlap");
    }
    if hotspot_count > 0 {
        overlaps.push("coordination hotspot overlap");
    }

    if overlaps.is_empty() {
        return summary.summary.clone();
    }

    format!(
        "Concept '{}' intersects {}",
        summary.concept_id,
        overlaps.join(", ")
    )
}

fn inspection_watchpoint_evidence(
    summary: &ConceptDebtSummary,
    clone_families: &[&Value],
    hotspots: &[&crate::metrics::v2::ConcentrationReport],
) -> Vec<String> {
    let mut evidence = Vec::new();
    if summary.boundary_pressure_count > 0 {
        evidence.push(format!(
            "boundary and ownership findings: {}",
            summary.boundary_pressure_count
        ));
    }
    if summary.missing_site_count > 0 {
        evidence.push(format!(
            "missing update sites: {}",
            summary.missing_site_count
        ));
    }
    if summary.context_burden > 0 {
        evidence.push(format!("context burden: {}", summary.context_burden));
    }
    if !clone_families.is_empty() {
        evidence.push(format!("related clone families: {}", clone_families.len()));
        evidence.extend(
            clone_families
                .iter()
                .take(2)
                .filter_map(|family| family.get("summary").and_then(|value| value.as_str()))
                .map(str::to_string),
        );
    }
    if !hotspots.is_empty() {
        evidence.push(format!("related hotspots: {}", hotspots.len()));
        evidence.extend(
            hotspots
                .iter()
                .take(2)
                .map(|report| format!("hotspot file: {}", report.path)),
        );
    }

    evidence
}

fn inspection_watchpoint_focus(
    summary: &ConceptDebtSummary,
    clone_families: &[&Value],
    hotspots: &[&crate::metrics::v2::ConcentrationReport],
) -> Vec<String> {
    let mut focus = summary.inspection_focus.clone();
    if !clone_families.is_empty() {
        focus.push(
            "inspect whether the repeated clone surfaces represent shared debt or intentional divergence"
                .to_string(),
        );
    }
    if !hotspots.is_empty() {
        focus.push(
            "inspect whether orchestration, storage, and adapter responsibilities are accumulating in one seam"
                .to_string(),
        );
    }
    if summary.boundary_pressure_count > 0 && summary.missing_site_count > 0 {
        focus.push(
            "inspect whether boundary erosion is making the propagation chain easier to miss"
                .to_string(),
        );
    }
    focus = dedupe_strings_preserve_order(focus);
    focus.truncate(4);
    focus
}

fn inspection_watchpoint_signal_families(
    summary: &ConceptDebtSummary,
    clone_family_count: usize,
    hotspot_count: usize,
) -> Vec<String> {
    let mut families = concept_signal_families(summary);
    if clone_family_count > 0 {
        families.push("duplication".to_string());
    }
    if hotspot_count > 0 {
        families.push("coordination".to_string());
    }
    dedupe_strings_preserve_order(families)
}

fn concept_debt_score(
    finding_count: usize,
    high_severity_count: usize,
    boundary_pressure_count: usize,
    missing_site_count: usize,
    context_burden: usize,
) -> u32 {
    let high_pressure = (high_severity_count as u32 * 2200).min(4400);
    let boundary_pressure = (boundary_pressure_count as u32 * 1100).min(3300);
    let finding_pressure = (finding_count as u32 * 900).min(2700);
    let missing_pressure = (missing_site_count as u32 * 700).min(2800);
    let context_pressure = (context_burden as u32 * 80).min(1600);

    (high_pressure + boundary_pressure + finding_pressure + missing_pressure + context_pressure)
        .min(10_000)
}

fn concept_debt_summary(
    concept_id: &str,
    finding_count: usize,
    obligation_count: usize,
    missing_site_count: usize,
    high_severity_count: usize,
    boundary_pressure_count: usize,
) -> String {
    if boundary_pressure_count > 0 && missing_site_count > 0 {
        return format!(
            "Concept '{}' shows {} boundary/ownership findings and {} missing update sites",
            concept_id, boundary_pressure_count, missing_site_count
        );
    }
    if high_severity_count > 0 && missing_site_count > 0 {
        return format!(
            "Concept '{}' shows {} high-severity findings and {} missing update sites",
            concept_id, high_severity_count, missing_site_count
        );
    }
    if missing_site_count > 0 {
        return format!(
            "Concept '{}' spans {} obligation reports with {} missing update sites",
            concept_id, obligation_count, missing_site_count
        );
    }
    if high_severity_count > 0 {
        return format!(
            "Concept '{}' has {} high-severity ownership or access findings",
            concept_id, high_severity_count
        );
    }
    format!(
        "Concept '{}' has {} repeated structural findings",
        concept_id, finding_count
    )
}

fn concept_debt_inspection_focus(
    dominant_kinds: &[String],
    has_missing_sites: bool,
) -> Vec<String> {
    let mut focus = Vec::new();
    for kind in dominant_kinds {
        match kind.as_str() {
            "multi_writer_concept"
            | "forbidden_writer"
            | "writer_outside_allowlist"
            | "concept_boundary_pressure" => {
                focus.push("inspect write ownership and boundary enforcement".to_string());
            }
            "forbidden_raw_read" | "authoritative_import_bypass" => {
                focus.push(
                    "inspect whether reads bypass the canonical accessor or public boundary"
                        .to_string(),
                );
            }
            _ => {}
        }
    }
    if has_missing_sites {
        focus.push(
            "inspect the explicit propagation sites and completeness tests for this concept"
                .to_string(),
        );
    }
    if focus.is_empty() {
        focus.push("inspect the concept boundary and repeated finding kinds".to_string());
    }
    focus = dedupe_strings_preserve_order(focus);
    focus.truncate(3);
    focus
}

fn dedupe_strings_preserve_order(values: Vec<String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    values
        .into_iter()
        .filter(|value| seen.insert(value.clone()))
        .collect()
}

fn signal_severity(score_0_10000: u32) -> &'static str {
    match score_0_10000 {
        6500..=10_000 => "high",
        3000..=6499 => "medium",
        _ => "low",
    }
}

fn concept_signal_class(summary: &ConceptDebtSummary) -> &'static str {
    if summary.boundary_pressure_count > 0 || summary.high_severity_count > 0 {
        "debt"
    } else if summary.missing_site_count > 0 {
        "hardening"
    } else {
        "watchpoint"
    }
}

fn concept_signal_families(summary: &ConceptDebtSummary) -> Vec<String> {
    let mut families = Vec::new();
    if summary.boundary_pressure_count > 0 {
        families.push("ownership".to_string());
        families.push("boundary".to_string());
    }
    if summary.missing_site_count > 0 || summary.obligation_count > 0 {
        families.push("propagation".to_string());
    }
    if summary.high_severity_count > 0 && families.is_empty() {
        families.push("boundary".to_string());
    }
    if families.is_empty() {
        families.push("consistency".to_string());
    }
    dedupe_strings_preserve_order(families)
}

fn concept_signal_metrics(summary: &ConceptDebtSummary) -> DebtSignalMetrics {
    DebtSignalMetrics {
        finding_count: Some(summary.finding_count),
        high_severity_count: Some(summary.high_severity_count),
        boundary_pressure_count: Some(summary.boundary_pressure_count),
        obligation_count: Some(summary.obligation_count),
        missing_site_count: Some(summary.missing_site_count),
        context_burden: Some(summary.context_burden),
        file_count: Some(summary.files.len()),
        ..DebtSignalMetrics::default()
    }
}

fn concept_signal_impact(summary: &ConceptDebtSummary) -> String {
    if summary.boundary_pressure_count > 0 && summary.missing_site_count > 0 {
        return "Split ownership and incomplete propagation make this concept easier to regress through partial edits.".to_string();
    }
    if summary.boundary_pressure_count > 0 {
        return "Unclear ownership or boundary erosion makes the concept harder to reason about and easier to update inconsistently.".to_string();
    }
    if summary.missing_site_count > 0 {
        return "Explicit propagation burden means future changes can miss required update sites unless the concept is hardened.".to_string();
    }
    "Repeated structural findings suggest this concept will remain brittle until the boundary and update path are clearer.".to_string()
}

fn concept_candidate_split_axes(summary: &ConceptDebtSummary) -> Vec<String> {
    let mut axes = Vec::new();
    if summary.boundary_pressure_count > 0 {
        axes.push("ownership boundary".to_string());
    }
    if summary.missing_site_count > 0 {
        axes.push("propagation surface".to_string());
    }
    if axes.is_empty() {
        axes.push("concept boundary".to_string());
    }
    axes
}

fn structural_signal(report: &crate::metrics::v2::StructuralDebtReport) -> DebtSignal {
    annotate_debt_signal(DebtSignal {
        kind: report.kind.clone(),
        trust_tier: report.trust_tier.clone(),
        presentation_class: report.presentation_class.clone(),
        leverage_class: report.leverage_class.clone(),
        scope: report.scope.clone(),
        signal_class: report.signal_class.clone(),
        signal_families: report.signal_families.clone(),
        severity: report.severity.clone(),
        score_0_10000: report.score_0_10000,
        summary: report.summary.clone(),
        impact: report.impact.clone(),
        files: report.files.clone(),
        role_tags: report.role_tags.clone(),
        leverage_reasons: report.leverage_reasons.clone(),
        evidence: report.evidence.clone(),
        inspection_focus: report.inspection_focus.clone(),
        candidate_split_axes: report.candidate_split_axes.clone(),
        related_surfaces: report.related_surfaces.clone(),
        metrics: DebtSignalMetrics {
            file_count: report.metrics.file_count,
            line_count: report.metrics.line_count,
            function_count: report.metrics.function_count,
            fan_in: report.metrics.fan_in,
            fan_out: report.metrics.fan_out,
            instability_0_10000: report.metrics.instability_0_10000,
            dead_symbol_count: report.metrics.dead_symbol_count,
            dead_line_count: report.metrics.dead_line_count,
            cycle_size: report.metrics.cycle_size,
            cut_candidate_count: report.metrics.cut_candidate_count,
            largest_cycle_after_best_cut: report.metrics.largest_cycle_after_best_cut,
            inbound_reference_count: report.metrics.inbound_reference_count,
            public_surface_count: report.metrics.public_surface_count,
            reachable_from_tests: report.metrics.reachable_from_tests,
            guardrail_test_count: report.metrics.guardrail_test_count,
            role_count: report.metrics.role_count,
            max_complexity: report.metrics.max_complexity,
            ..DebtSignalMetrics::default()
        },
    })
}

fn clone_family_inspection_focus(family: &Value) -> Vec<String> {
    let mut focus = family
        .get("remediation_hints")
        .and_then(|value| value.as_array())
        .map(|hints| {
            hints
                .iter()
                .filter_map(|hint| hint.get("kind").and_then(|value| value.as_str()))
                .filter_map(|kind| match kind {
                    "sync_recent_divergence" => Some(
                        "inspect whether recent sibling edits should stay synchronized or intentionally diverge"
                            .to_string(),
                    ),
                    "extract_shared_helper" => Some(
                        "inspect whether the repeated logic is substantial enough to share behind one helper"
                            .to_string(),
                    ),
                    "collapse_clone_family" => Some(
                        "inspect whether the clone family is carrying avoidable duplicate maintenance"
                            .to_string(),
                    ),
                    "add_shared_behavior_tests" => Some(
                        "inspect whether shared behavior tests would make the family safer to change"
                            .to_string(),
                    ),
                    _ => None,
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    focus = dedupe_strings_preserve_order(focus);
    focus.truncate(3);
    focus
}

fn clone_family_candidate_axes(family: &Value) -> Vec<String> {
    let mut axes = family
        .get("remediation_hints")
        .and_then(|value| value.as_array())
        .map(|hints| {
            hints
                .iter()
                .filter_map(|hint| hint.get("kind").and_then(|value| value.as_str()))
                .map(|kind| match kind {
                    "sync_recent_divergence" => "shared behavior boundary".to_string(),
                    "extract_shared_helper" => "shared helper boundary".to_string(),
                    "collapse_clone_family" => "duplicate maintenance boundary".to_string(),
                    "add_shared_behavior_tests" => "shared behavior test boundary".to_string(),
                    _ => kind.replace('_', " "),
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    axes = dedupe_strings_preserve_order(axes);
    axes.truncate(3);
    axes
}

fn clone_family_signal(family: &Value) -> Option<DebtSignal> {
    let summary = family.get("summary")?.as_str()?.to_string();
    let scope = family.get("family_id")?.as_str()?.to_string();
    let severity = family
        .get("severity")
        .and_then(|value| value.as_str())
        .unwrap_or("medium")
        .to_string();
    let score_0_10000 = family
        .get("family_score")
        .and_then(|value| value.as_u64())
        .unwrap_or(0) as u32;
    let files = finding_files(family);
    let evidence = json_string_list(family.get("reasons"))
        .into_iter()
        .take(3)
        .collect();
    let inspection_focus = clone_family_inspection_focus(family);

    Some(annotate_debt_signal(DebtSignal {
        kind: "clone_family".to_string(),
        trust_tier: if score_0_10000 >= 6500 {
            "trusted".to_string()
        } else {
            "watchpoint".to_string()
        },
        presentation_class: "watchpoint".to_string(),
        leverage_class: String::new(),
        scope,
        signal_class: if score_0_10000 >= 6500 {
            "debt".to_string()
        } else {
            "watchpoint".to_string()
        },
        signal_families: vec!["duplication".to_string(), "drift".to_string()],
        severity,
        score_0_10000,
        summary,
        impact: "Duplicate logic across related files increases the chance that a fix lands in only one sibling and the family drifts over time.".to_string(),
        files,
        role_tags: Vec::new(),
        leverage_reasons: Vec::new(),
        evidence,
        inspection_focus,
        candidate_split_axes: clone_family_candidate_axes(family),
        related_surfaces: finding_files(family),
        metrics: DebtSignalMetrics {
            file_count: family
                .get("file_count")
                .and_then(|value| value.as_u64())
                .map(|value| value as usize),
            member_count: family
                .get("member_count")
                .and_then(|value| value.as_u64())
                .map(|value| value as usize),
            recently_touched_file_count: family
                .get("recently_touched_file_count")
                .and_then(|value| value.as_u64())
                .map(|value| value as usize),
            divergence_score: family
                .get("divergence_score")
                .and_then(|value| value.as_u64())
                .map(|value| value as u32),
            family_score_0_10000: Some(score_0_10000),
            ..DebtSignalMetrics::default()
        },
    }))
}

fn clone_group_signal(finding: &Value) -> Option<DebtSignal> {
    let scope = finding.get("clone_id")?.as_str()?.to_string();
    let severity = severity_of_value(finding).to_string();
    let score_0_10000 = finding
        .get("risk_score")
        .and_then(|value| value.as_u64())
        .unwrap_or(0) as u32;
    let summary = finding
        .get("summary")
        .and_then(|value| value.as_str())
        .unwrap_or("Clone group needs consolidation")
        .to_string();
    let files = finding_files(finding);
    let evidence = json_string_list(finding.get("reasons"))
        .into_iter()
        .take(3)
        .collect::<Vec<_>>();

    Some(annotate_debt_signal(DebtSignal {
        kind: "clone_group".to_string(),
        trust_tier: if score_0_10000 >= 6500 {
            "trusted".to_string()
        } else {
            "watchpoint".to_string()
        },
        presentation_class: "watchpoint".to_string(),
        leverage_class: String::new(),
        scope,
        signal_class: if score_0_10000 >= 6500 {
            "debt".to_string()
        } else {
            "watchpoint".to_string()
        },
        signal_families: vec!["duplication".to_string()],
        severity,
        score_0_10000,
        summary,
        impact: "Copy-paste maintenance means the same behavior must be kept in sync across multiple files.".to_string(),
        files,
        role_tags: Vec::new(),
        leverage_reasons: Vec::new(),
        evidence,
        inspection_focus: vec![
            "inspect whether the repeated logic should stay aligned or collapse behind one abstraction"
                .to_string(),
            "inspect whether shared behavior tests would make the copies safer to change"
                .to_string(),
        ],
        candidate_split_axes: vec![
            "shared helper boundary".to_string(),
            "shared behavior test boundary".to_string(),
        ],
        related_surfaces: finding_files(finding),
        metrics: DebtSignalMetrics {
            file_count: Some(finding_files(finding).len()),
            ..DebtSignalMetrics::default()
        },
    }))
}

fn hotspot_signal(report: &crate::metrics::v2::ConcentrationReport) -> Option<DebtSignal> {
    if report.score_0_10000 < 4000 {
        return None;
    }

    Some(annotate_debt_signal(DebtSignal {
        kind: "hotspot".to_string(),
        trust_tier: if report.score_0_10000 >= 6500 {
            "trusted".to_string()
        } else {
            "watchpoint".to_string()
        },
        presentation_class: classify_presentation_class(
            "hotspot",
            if report.score_0_10000 >= 6500 {
                "trusted"
            } else {
                "watchpoint"
            },
            &report.path,
            std::slice::from_ref(&report.path),
            &[],
            report.reasons.len(),
            1,
            0,
            0,
        ),
        leverage_class: String::new(),
        scope: report.path.clone(),
        signal_class: if report.score_0_10000 >= 6500 {
            "debt".to_string()
        } else {
            "watchpoint".to_string()
        },
        signal_families: vec!["coordination".to_string()],
        severity: signal_severity(report.score_0_10000).to_string(),
        score_0_10000: report.score_0_10000,
        summary: format!(
            "File '{}' is carrying coordination hotspot pressure",
            report.path
        ),
        impact: "Side effects, async branches, and retry logic concentrated in one file increase coordination debt and regression risk.".to_string(),
        files: vec![report.path.clone()],
        role_tags: Vec::new(),
        leverage_reasons: Vec::new(),
        evidence: report.reasons.iter().cloned().take(3).collect(),
        inspection_focus: vec![
            "inspect whether orchestration, side effects, and adapters are accumulating in one file"
                .to_string(),
            "inspect whether complexity is local to one seam or repeated across nearby files"
                .to_string(),
        ],
        candidate_split_axes: vec![
            "orchestration boundary".to_string(),
            "side-effect boundary".to_string(),
            "adapter boundary".to_string(),
        ],
        related_surfaces: vec![report.path.clone()],
        metrics: DebtSignalMetrics {
            file_count: Some(1),
            authority_breadth: Some(report.authority_breadth),
            side_effect_breadth: Some(report.side_effect_breadth),
            timer_retry_weight: Some(report.timer_retry_weight),
            async_branch_weight: Some(report.async_branch_weight),
            max_complexity: Some(report.max_complexity),
            churn_commits: Some(report.churn_commits),
            ..DebtSignalMetrics::default()
        },
    }))
}

fn json_string_list(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(|value| value.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(str::to_string))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn legacy_quality_opportunity_values(signals: &[DebtSignal]) -> Vec<Value> {
    signals
        .iter()
        .map(|signal| {
            json!({
                "kind": signal.kind,
                "scope": signal.scope,
                "severity": signal.severity,
                "score_0_10000": signal.score_0_10000,
                "summary": signal.summary,
                "impact": signal.impact,
                "files": signal.files,
                "evidence": signal.evidence,
                "suggested_actions": signal.inspection_focus,
            })
        })
        .collect()
}

fn legacy_optimization_priority_values(watchpoints: &[InspectionWatchpoint]) -> Vec<Value> {
    watchpoints
        .iter()
        .map(|watchpoint| {
            json!({
                "concept_id": watchpoint.scope,
                "severity": watchpoint.severity,
                "score_0_10000": watchpoint.score_0_10000,
                "summary": watchpoint.summary,
                "files": watchpoint.files,
                "evidence": watchpoint.evidence,
                "suggested_actions": watchpoint.inspection_focus,
                "clone_family_count": watchpoint.clone_family_count,
                "hotspot_count": watchpoint.hotspot_count,
                "missing_site_count": watchpoint.missing_site_count,
                "boundary_pressure_count": watchpoint.boundary_pressure_count,
            })
        })
        .collect()
}

fn debt_signal_concentration_reports(
    state: &mut McpState,
    root: &Path,
    snapshot: &Snapshot,
    candidate_files: &BTreeSet<String>,
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
    let (history, evolution_error) = concentration_history(state, root, None);
    let reports = crate::metrics::v2::build_concentration_reports(
        root,
        candidate_files,
        &complexity_map,
        &config,
        semantic.as_ref(),
        history.as_ref(),
    );
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
            evolution_error,
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
mod tests {
    use super::{
        annotate_debt_signal, annotate_finding_detail, annotate_inspection_watchpoint,
        apply_suppressions, build_exact_clone_findings, build_session_v2_baseline,
        changed_files_from_session_context, classify_leverage_class, classify_leverage_reasons,
        classify_presentation_class, cli_evaluate_v2_gate, cli_save_v2_session,
        current_session_v2_baseline_with_status, distinct_file_count, do_scan, fresh_mcp_state,
        handle_concepts, handle_explain_concept, handle_findings, handle_gate, handle_health,
        handle_obligations, handle_scan, handle_session_end, handle_session_start, handle_state,
        handle_trace_symbol, load_persisted_session_v2, load_session_v2_baseline_status,
        load_v2_rules_config, overall_confidence_0_10000, prepare_patch_check_context,
        project_fingerprint, save_session_v2_baseline, state_model_ids_from_findings,
        state_model_ids_from_reports,
        update_scan_cache, DebtSignal, DebtSignalMetrics, FindingDetail, FindingDetailMetrics,
        InspectionWatchpoint,
    };
    use crate::analysis::scanner::common::{ScanMetadata, ScanMode};
    use crate::analysis::semantic::typescript::default_bridge_config;
    use crate::analysis::semantic::{
        ClosedDomain, ExhaustivenessSite, ProjectModel, ReadFact, SemanticCapability,
        SemanticSnapshot, SymbolFact, TransitionSite, WriteFact,
    };
    use crate::app::bridge::TypeScriptBridgeSupervisor;
    use crate::app::mcp_server::{
        McpState, SessionV2Baseline, SessionV2ConfidenceSnapshot, SESSION_V2_SCHEMA_VERSION,
    };
    use crate::license::Tier;
    use crate::metrics::evo::{
        AuthorInfo, CouplingPair, EvolutionReport, FileChurn, TemporalHotspot,
    };
    use crate::metrics::rules::RulesConfig;
    use crate::metrics::DuplicateGroup;
    use serde_json::json;
    use std::collections::{BTreeMap, BTreeSet, HashMap};
    use std::path::Path;
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root(label: &str) -> std::path::PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("sentrux-{label}-{}-{unique}", std::process::id()));
        std::fs::create_dir_all(root.join(".sentrux")).expect("create temp sentrux dir");
        root
    }

    fn write_file(root: &Path, relative_path: &str, contents: &str) {
        let absolute_path = root.join(relative_path);
        if let Some(parent) = absolute_path.parent() {
            std::fs::create_dir_all(parent).expect("create parent directories");
        }
        std::fs::write(&absolute_path, contents).expect("write file");
    }

    fn append_file(root: &Path, relative_path: &str, contents: &str) {
        use std::io::Write;

        let absolute_path = root.join(relative_path);
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(&absolute_path)
            .expect("open file for append");
        file.write_all(contents.as_bytes()).expect("append file");
    }

    fn run_git(root: &Path, args: &[&str]) {
        let status = Command::new("git")
            .arg("-C")
            .arg(root)
            .args(args)
            .status()
            .expect("run git command");
        assert!(status.success(), "git {:?} failed", args);
    }

    fn init_git_repo(root: &Path) {
        run_git(root, &["init"]);
        run_git(root, &["config", "user.email", "test@example.com"]);
        run_git(root, &["config", "user.name", "Sentrux Test"]);
    }

    fn commit_all(root: &Path, message: &str) {
        run_git(root, &["add", "."]);
        run_git(root, &["commit", "-m", message]);
    }

    fn concept_fixture_root() -> std::path::PathBuf {
        let root = temp_root("concept-tools");
        write_file(
            &root,
            ".sentrux/rules.toml",
            r#"
                [[concept]]
                id = "task_git_status"
                kind = "authoritative_state"
                anchors = ["src/store/core.ts::store.taskGitStatus"]
                authoritative_inputs = ["src/domain/task-state.ts::TaskState"]
                allowed_writers = ["src/app/git-status-sync.ts::*"]
                forbid_writers = ["src/store/git-status-polling.ts::*"]
                canonical_accessors = ["src/app/task-presentation.ts::getTaskStatus"]
                forbid_raw_reads = ["src/components/**::store.taskGitStatus"]
                related_tests = ["src/app/task-presentation.test.ts"]

                [[contract]]
                id = "server_state_bootstrap"
                kind = "bootstrap"
                categories_symbol = "src/domain/task-state.ts::TaskState"
                registry_symbol = "src/app/task-presentation.ts::TaskStateRegistry"
                browser_entry = "src/runtime/browser-session.ts"
                required_capabilities = ["snapshot", "live_updates", "versioning"]
            "#,
        );
        write_file(
            &root,
            "src/domain/task-state.ts",
            "export type TaskState = 'idle' | 'running' | 'error';\n",
        );
        write_file(
            &root,
            "src/store/core.ts",
            "export const store = { taskGitStatus: 'idle' as TaskState };\n",
        );
        write_file(
            &root,
            "src/app/git-status-sync.ts",
            "export function syncTaskState(): void {}\n",
        );
        write_file(
            &root,
            "src/store/git-status-polling.ts",
            "export function pollTaskState(): void {}\n",
        );
        write_file(
            &root,
            "src/components/TaskRow.tsx",
            "export function TaskRow(): null { return null; }\n",
        );
        write_file(
            &root,
            "src/app/task-presentation.ts",
            "export const TaskStateRegistry = { version: 1 };\nexport function getTaskStatus(): string { return 'idle'; }\n",
        );
        write_file(
            &root,
            "src/app/task-presentation.test.ts",
            "import { getTaskStatus } from './task-presentation';\nvoid getTaskStatus;\n",
        );
        write_file(
            &root,
            "src/runtime/browser-session.ts",
            "import { TaskStateRegistry } from '../app/task-presentation';\nvoid TaskStateRegistry;\nconst version = 1;\n",
        );
        root
    }

    fn structural_debt_fixture_root() -> std::path::PathBuf {
        let root = temp_root("structural-debt");
        let mut large_file = String::from("import { alpha } from './a';\nimport { beta } from './b';\nexport function render(): number {\n  return alpha() + beta();\n}\n");
        for index in 0..900 {
            large_file.push_str(&format!("export const item{index} = {index};\n"));
        }
        write_file(&root, "src/app.ts", &large_file);
        write_file(
            &root,
            "src/a.ts",
            "import { beta } from './b';\nexport function alpha(): number { return beta(); }\n",
        );
        write_file(
            &root,
            "src/b.ts",
            "import { alpha } from './a';\nexport function beta(): number { return alpha() + 1; }\n",
        );
        root
    }

    fn dead_island_fixture_root() -> std::path::PathBuf {
        let root = temp_root("dead-island");
        write_file(
            &root,
            "src/app.ts",
            "import { live } from './live';\nexport function render(): number { return live(); }\n",
        );
        write_file(
            &root,
            "src/live.ts",
            "export function live(): number { return 1; }\n",
        );
        write_file(
            &root,
            "src/orphan-a.ts",
            "import { orphanB } from './orphan-b';\nfunction orphanA(): number { return orphanB(); }\nexport const orphanValue = orphanA();\n",
        );
        write_file(
            &root,
            "src/orphan-b.ts",
            "import { orphanValue } from './orphan-a';\nfunction orphanB(): number { return orphanValue + 1; }\nconst orphanBValue = orphanB();\n",
        );
        root
    }

    fn dead_private_fixture_root() -> std::path::PathBuf {
        let root = temp_root("dead-private");
        write_file(
            &root,
            "src/app.ts",
            "export function render(): number { return 1; }\n",
        );
        write_file(
            &root,
            "src/stale.ts",
            "function deadAlpha(): number { return 1; }\nfunction deadBeta(): number { return 2; }\nexport const liveValue = 3;\n",
        );
        root
    }

    fn experimental_gate_fixture_root() -> std::path::PathBuf {
        let root = temp_root("experimental-gate");
        write_file(
            &root,
            "src/app.ts",
            "export function render(): number { return 1; }\n",
        );
        root
    }

    fn cli_gate_fixture_root() -> std::path::PathBuf {
        let root = temp_root("cli-v2-gate");
        write_file(
            &root,
            ".sentrux/rules.toml",
            r#"
                [[concept]]
                id = "app_state"
                anchors = ["src/domain/state.ts::AppState"]
            "#,
        );
        write_file(
            &root,
            "package.json",
            r#"{ "name": "cli-gate-fixture", "type": "module" }"#,
        );
        write_file(
            &root,
            "tsconfig.json",
            r#"
                {
                  "compilerOptions": {
                    "module": "esnext",
                    "target": "es2020",
                    "strict": true
                  },
                  "include": ["src/**/*.ts"]
                }
            "#,
        );
        write_file(
            &root,
            "src/domain/state.ts",
            "export type AppState = 'idle' | 'busy';\n",
        );
        root
    }

    fn closed_domain_gate_fixture_root() -> std::path::PathBuf {
        let root = temp_root("closed-domain-gate");
        write_file(
            &root,
            ".sentrux/rules.toml",
            r#"
                [[concept]]
                id = "app_state"
                anchors = ["src/domain/state.ts::AppState"]
            "#,
        );
        write_file(
            &root,
            "package.json",
            r#"{ "name": "closed-domain-gate-fixture", "type": "module" }"#,
        );
        write_file(
            &root,
            "tsconfig.json",
            r#"
                {
                  "compilerOptions": {
                    "module": "esnext",
                    "target": "es2020",
                    "strict": true
                  },
                  "include": ["src/**/*.ts"]
                }
            "#,
        );
        write_file(
            &root,
            "src/domain/state.ts",
            "export type AppState = 'idle' | 'busy';\n",
        );
        write_file(
            &root,
            "src/app/render.ts",
            r#"
                import type { AppState } from '../domain/state';

                function assertNever(value: never): never {
                  throw new Error(String(value));
                }

                export function renderState(state: AppState): string {
                  switch (state) {
                    case 'idle':
                      return 'idle';
                    case 'busy':
                      return 'busy';
                    default:
                      return assertNever(state);
                  }
                }
            "#,
        );
        root
    }

    fn contract_gate_fixture_root() -> std::path::PathBuf {
        let root = temp_root("contract-gate");
        write_file(
            &root,
            ".sentrux/rules.toml",
            r#"
                [[contract]]
                id = "server_state_bootstrap"
                categories_symbol = "src/domain/bootstrap.ts::BOOTSTRAP_CATEGORIES"
                payload_map_symbol = "src/domain/bootstrap.ts::BootstrapPayloadMap"
                registry_symbol = "src/app/bootstrap-registry.ts::BOOTSTRAP_REGISTRY"
                browser_entry = "src/runtime/browser-session.ts"
                electron_entry = "src/app/desktop-session.ts"
            "#,
        );
        write_file(
            &root,
            "package.json",
            r#"{ "name": "contract-gate-fixture", "type": "module" }"#,
        );
        write_file(
            &root,
            "tsconfig.json",
            r#"
                {
                  "compilerOptions": {
                    "module": "esnext",
                    "target": "es2020",
                    "strict": true
                  },
                  "include": ["src/**/*.ts"]
                }
            "#,
        );
        write_file(
            &root,
            "src/domain/bootstrap.ts",
            r#"
                export const BOOTSTRAP_CATEGORIES = ['tasks'] as const;
                export type BootstrapCategory = (typeof BOOTSTRAP_CATEGORIES)[number];
                export type BootstrapPayloadMap = {
                  tasks: { count: number };
                };
            "#,
        );
        write_file(
            &root,
            "src/app/bootstrap-registry.ts",
            r#"
                import type { BootstrapPayloadMap } from '../domain/bootstrap';

                export const BOOTSTRAP_REGISTRY: Record<keyof BootstrapPayloadMap, string> = {
                  tasks: 'tasks',
                };
            "#,
        );
        write_file(
            &root,
            "src/runtime/browser-session.ts",
            r#"
                import { BOOTSTRAP_REGISTRY } from '../app/bootstrap-registry';

                export function startBrowserSession(): number {
                  return Object.keys(BOOTSTRAP_REGISTRY).length;
                }
            "#,
        );
        write_file(
            &root,
            "src/app/desktop-session.ts",
            r#"
                import { BOOTSTRAP_REGISTRY } from './bootstrap-registry';

                export function startDesktopSession(): number {
                  return Object.keys(BOOTSTRAP_REGISTRY).length;
                }
            "#,
        );
        root
    }

    fn concept_fixture_semantic(root: &Path) -> SemanticSnapshot {
        SemanticSnapshot {
            project: ProjectModel {
                root: root.to_string_lossy().to_string(),
                tsconfig_paths: vec!["tsconfig.json".to_string()],
                workspace_files: vec!["package.json".to_string()],
                primary_language: Some("typescript".to_string()),
                fingerprint: "fixture".to_string(),
                repo_archetype: None,
                detected_archetypes: Vec::new(),
            },
            analyzed_files: 6,
            capabilities: vec![
                SemanticCapability::Symbols,
                SemanticCapability::Reads,
                SemanticCapability::Writes,
                SemanticCapability::ClosedDomains,
                SemanticCapability::ClosedDomainSites,
            ],
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
                    id: "task-git-status".to_string(),
                    path: "src/store/core.ts".to_string(),
                    name: "store.taskGitStatus".to_string(),
                    kind: "property".to_string(),
                    line: 1,
                },
                SymbolFact {
                    id: "registry".to_string(),
                    path: "src/app/task-presentation.ts".to_string(),
                    name: "TaskStateRegistry".to_string(),
                    kind: "const".to_string(),
                    line: 1,
                },
            ],
            reads: vec![ReadFact {
                path: "src/components/TaskRow.tsx".to_string(),
                symbol_name: "store.taskGitStatus".to_string(),
                read_kind: "property_access".to_string(),
                line: 6,
            }],
            writes: vec![
                WriteFact {
                    path: "src/app/git-status-sync.ts".to_string(),
                    symbol_name: "store.taskGitStatus".to_string(),
                    write_kind: "store_call".to_string(),
                    line: 4,
                },
                WriteFact {
                    path: "src/store/git-status-polling.ts".to_string(),
                    symbol_name: "store.taskGitStatus".to_string(),
                    write_kind: "store_call".to_string(),
                    line: 8,
                },
            ],
            closed_domains: vec![ClosedDomain {
                path: "src/domain/task-state.ts".to_string(),
                symbol_name: "TaskState".to_string(),
                variants: vec![
                    "idle".to_string(),
                    "running".to_string(),
                    "error".to_string(),
                ],
                line: 1,
            }],
            closed_domain_sites: vec![ExhaustivenessSite {
                path: "src/app/task-presentation.ts".to_string(),
                domain_symbol_name: "TaskState".to_string(),
                site_kind: "switch".to_string(),
                proof_kind: "switch".to_string(),
                covered_variants: vec!["idle".to_string(), "running".to_string()],
                line: 12,
            }],
            transition_sites: Vec::new(),
        }
    }

    fn state_fixture_root() -> std::path::PathBuf {
        let root = temp_root("state-tool");
        write_file(
            &root,
            ".sentrux/rules.toml",
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
        );
        write_file(
            &root,
            "src/runtime/browser-state-sync-controller.ts",
            "export function renderState(state: BrowserSyncState): string { return state; }\n",
        );
        root
    }

    fn state_fixture_semantic(root: &Path) -> SemanticSnapshot {
        SemanticSnapshot {
            project: ProjectModel {
                root: root.to_string_lossy().to_string(),
                tsconfig_paths: vec!["tsconfig.json".to_string()],
                workspace_files: Vec::new(),
                primary_language: Some("typescript".to_string()),
                fingerprint: "state-fixture".to_string(),
                repo_archetype: None,
                detected_archetypes: Vec::new(),
            },
            analyzed_files: 2,
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
            }],
            closed_domain_sites: vec![ExhaustivenessSite {
                path: "src/runtime/browser-state-sync-controller.ts".to_string(),
                domain_symbol_name: "BrowserSyncState".to_string(),
                site_kind: "switch".to_string(),
                proof_kind: "switch".to_string(),
                covered_variants: vec!["idle".to_string(), "running".to_string()],
                line: 6,
            }],
            transition_sites: Vec::new(),
        }
    }

    fn state_with_semantic(root: &Path, semantic: SemanticSnapshot) -> McpState {
        McpState {
            tier: Tier::Free,
            scan_root: Some(root.to_path_buf()),
            cached_snapshot: None,
            cached_scan_metadata: None,
            cached_semantic: Some(semantic),
            cached_health: None,
            cached_arch: None,
            baseline: None,
            session_v2: None,
            cached_evolution: None,
            cached_scan_identity: None,
            cached_rules_identity: None,
            cached_rules_config: None,
            cached_rules_error: None,
            cached_patch_safety: None,
            semantic_bridge: None,
        }
    }

    #[test]
    fn apply_suppressions_hides_matching_findings_and_tracks_hits() {
        let config: RulesConfig = toml::from_str(
            r#"
                [[suppress]]
                kind = "forbidden_writer"
                concept = "task_git_status"
                file = "src/store/**"
                reason = "temporary migration"
                expires = "2099-12-31"
            "#,
        )
        .expect("rules config");
        let findings = vec![
            json!({
                "kind": "forbidden_writer",
                "concept_id": "task_git_status",
                "files": ["src/store/git-status-polling.ts"],
                "summary": "forbidden writer",
            }),
            json!({
                "kind": "forbidden_raw_read",
                "concept_id": "task_git_status",
                "files": ["src/components/TaskRow.tsx"],
                "summary": "raw read",
            }),
        ];

        let application = apply_suppressions(&config, findings);

        assert_eq!(application.visible_findings.len(), 1);
        assert_eq!(application.active_matches.len(), 1);
        assert_eq!(application.active_matches[0].matched_finding_count, 1);
        assert_eq!(
            application.visible_findings[0]["kind"],
            "forbidden_raw_read"
        );
    }

    #[test]
    fn apply_suppressions_keeps_findings_visible_when_expired() {
        let config: RulesConfig = toml::from_str(
            r#"
                [[suppress]]
                kind = "forbidden_writer"
                concept = "task_git_status"
                reason = "expired suppression"
                expires = "2020-01-01"
            "#,
        )
        .expect("rules config");
        let findings = vec![json!({
            "kind": "forbidden_writer",
            "concept_id": "task_git_status",
            "files": ["src/store/git-status-polling.ts"],
            "summary": "forbidden writer",
        })];

        let application = apply_suppressions(&config, findings);

        assert_eq!(application.visible_findings.len(), 1);
        assert!(application.active_matches.is_empty());
        assert_eq!(application.expired_matches.len(), 1);
        assert!(application.expired_matches[0].expired);
    }

    #[test]
    fn exact_clone_findings_filter_same_file_groups() {
        let same_file = DuplicateGroup {
            hash: 1,
            instances: vec![
                ("src/a.ts".into(), "dup_a".into(), 10),
                ("src/a.ts".into(), "dup_b".into(), 10),
            ],
        };
        let cross_file = DuplicateGroup {
            hash: 2,
            instances: vec![
                ("src/a.ts".into(), "dup_a".into(), 12),
                ("src/b.ts".into(), "dup_b".into(), 12),
            ],
        };

        let findings = build_exact_clone_findings(&[same_file, cross_file], 10);

        assert_eq!(findings.len(), 1);
        let filtered_group = DuplicateGroup {
            hash: 2,
            instances: vec![
                ("src/a.ts".into(), "dup_a".into(), 12),
                ("src/b.ts".into(), "dup_b".into(), 12),
            ],
        };
        assert_eq!(distinct_file_count(&filtered_group), 2);
    }

    #[test]
    fn exact_clone_findings_ignore_test_only_and_tiny_groups() {
        let test_only = DuplicateGroup {
            hash: 1,
            instances: vec![
                ("src/a.test.ts".into(), "dup_a".into(), 10),
                ("src/b.test.ts".into(), "dup_b".into(), 10),
            ],
        };
        let tiny_mixed = DuplicateGroup {
            hash: 2,
            instances: vec![
                ("src/a.ts".into(), "dup_a".into(), 1),
                ("src/b.ts".into(), "dup_b".into(), 1),
            ],
        };
        let actionable = DuplicateGroup {
            hash: 3,
            instances: vec![
                ("src/a.ts".into(), "dup_a".into(), 8),
                ("src/b.test.ts".into(), "dup_b".into(), 8),
            ],
        };

        let findings = build_exact_clone_findings(&[test_only, tiny_mixed, actionable], 10);

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0]["kind"], "exact_clone_group");
        assert_eq!(findings[0]["max_lines"], 8);
        assert_eq!(findings[0]["severity"], "medium");
        assert_eq!(findings[0]["files"].as_array().map(Vec::len), Some(2));
    }

    #[test]
    fn findings_surface_clone_families_and_representative_groups() {
        let root = cli_gate_fixture_root();
        write_file(
            &root,
            "src/a.ts",
            "export function duplicateAlpha(): number { return 1; }\n",
        );
        write_file(
            &root,
            "src/b.ts",
            "export function duplicateBeta(): number { return 1; }\n",
        );

        let mut state = fresh_mcp_state();
        handle_scan(
            &json!({"path": root.to_string_lossy().to_string()}),
            &Tier::Free,
            &mut state,
        )
        .expect("scan fixture");
        state
            .cached_health
            .as_mut()
            .expect("cached health")
            .duplicate_groups = vec![
            DuplicateGroup {
                hash: 41,
                instances: vec![
                    ("src/a.ts".into(), "duplicateAlpha".into(), 12),
                    ("src/b.ts".into(), "duplicateBeta".into(), 12),
                ],
            },
            DuplicateGroup {
                hash: 42,
                instances: vec![
                    ("src/a.ts".into(), "duplicateGamma".into(), 10),
                    ("src/b.ts".into(), "duplicateDelta".into(), 10),
                ],
            },
        ];

        let response =
            handle_findings(&json!({"limit": 10}), &Tier::Free, &mut state).expect("findings");

        assert_eq!(response["clone_group_count"], 2);
        assert_eq!(response["clone_family_count"], 1);
        assert_eq!(
            response["clone_families"]
                .as_array()
                .map(|families| families.len()),
            Some(1)
        );
        assert!(response["clone_families"][0]["remediation_hints"]
            .as_array()
            .expect("family remediation hints")
            .iter()
            .any(|hint| hint["kind"] == "extract_shared_helper"));
        assert!(response["clone_remediations"]
            .as_array()
            .expect("clone remediation hints")
            .iter()
            .any(|hint| hint["kind"] == "extract_shared_helper"));
        assert!(response["findings"]
            .as_array()
            .expect("findings array")
            .iter()
            .any(|finding| finding["kind"] == "exact_clone_group"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn findings_surface_concept_summaries_debt_signals_and_watchpoints() {
        let root = concept_fixture_root();
        let mut state = fresh_mcp_state();
        handle_scan(
            &json!({"path": root.to_string_lossy().to_string()}),
            &Tier::Free,
            &mut state,
        )
        .expect("scan fixture");
        state.cached_semantic = Some(concept_fixture_semantic(&root));

        let response =
            handle_findings(&json!({"limit": 10}), &Tier::Free, &mut state).expect("findings");

        assert!(response["confidence"]["scan_confidence_0_10000"].is_u64());
        assert!(response["confidence"]["rule_coverage_0_10000"].is_u64());
        assert_eq!(
            response["confidence"]["semantic_rules_loaded"].as_bool(),
            Some(true)
        );
        assert!(response["concept_summaries"]
            .as_array()
            .expect("concept summaries")
            .iter()
            .any(|summary| summary["concept_id"] == "task_git_status"));
        assert!(response["debt_signals"]
            .as_array()
            .expect("debt signals")
            .iter()
            .any(|signal| {
                signal["kind"] == "concept"
                    && signal["scope"] == "task_git_status"
                    && signal["signal_families"]
                        .as_array()
                        .expect("signal families")
                        .iter()
                        .any(|family| family == "ownership")
            }));
        assert!(response["watchpoints"]
            .as_array()
            .expect("watchpoints")
            .iter()
            .any(|watchpoint| watchpoint["scope"] == "task_git_status"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn findings_surface_finding_details_with_impact_and_focus() {
        let root = concept_fixture_root();
        let mut state = fresh_mcp_state();
        handle_scan(
            &json!({"path": root.to_string_lossy().to_string()}),
            &Tier::Free,
            &mut state,
        )
        .expect("scan fixture");
        state.cached_semantic = Some(concept_fixture_semantic(&root));

        let response =
            handle_findings(&json!({"limit": 10}), &Tier::Free, &mut state).expect("findings");

        assert!(response["finding_details"]
            .as_array()
            .expect("finding details")
            .iter()
            .any(|detail| {
                detail["kind"] == "closed_domain_exhaustiveness"
                    && detail["impact"]
                        .as_str()
                        .is_some_and(|impact| impact.contains("Finite-domain changes"))
                    && detail["inspection_focus"]
                        .as_array()
                        .is_some_and(|focus| !focus.is_empty())
            }));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn presentation_class_demotes_narrow_hardening_notes_and_tooling_surfaces() {
        assert_eq!(
            classify_presentation_class(
                "closed_domain_exhaustiveness",
                "trusted",
                "ConnectionBannerState",
                &[
                    "src/components/app-shell/AppConnectionBanner.tsx".to_string(),
                    "src/runtime/browser-session.ts".to_string(),
                ],
                &[],
                1,
                1,
                0,
                1,
            ),
            "hardening_note"
        );
        assert_eq!(
            classify_presentation_class(
                "large_file",
                "trusted",
                "scripts/session-stress.mjs",
                &["scripts/session-stress.mjs".to_string()],
                &[],
                5,
                1,
                0,
                0,
            ),
            "tooling_debt"
        );
        assert_eq!(
            classify_presentation_class(
                "unstable_hotspot",
                "trusted",
                "src/lib/ipc.ts",
                &["src/lib/ipc.ts".to_string()],
                &["transport_facade".to_string()],
                6,
                1,
                0,
                0,
            ),
            "guarded_facade"
        );
    }

    #[test]
    fn presentation_class_marks_exact_clone_groups_as_watchpoints() {
        assert_eq!(
            classify_presentation_class(
                "exact_clone_group",
                "trusted",
                "src/a.ts|src/b.ts",
                &["src/a.ts".to_string(), "src/b.ts".to_string()],
                &[],
                0,
                1,
                0,
                0,
            ),
            "watchpoint"
        );
    }

    #[test]
    fn leverage_class_uses_size_and_complexity_for_extracted_owner_facades() {
        assert_eq!(
            classify_leverage_class(
                "dependency_sprawl",
                "trusted",
                "structural_debt",
                &[
                    "facade_with_extracted_owners".to_string(),
                    "guarded_seam".to_string(),
                ],
                Some(2),
                Some(28),
                Some(423),
                Some(4),
                None,
                None,
                Some(1),
                0,
                0,
            ),
            "local_refactor_target"
        );
        assert_eq!(
            classify_leverage_class(
                "dependency_sprawl",
                "trusted",
                "structural_debt",
                &[
                    "facade_with_extracted_owners".to_string(),
                    "guarded_seam".to_string(),
                ],
                Some(1),
                Some(22),
                Some(629),
                Some(82),
                None,
                None,
                Some(1),
                0,
                0,
            ),
            "secondary_cleanup"
        );
    }

    #[test]
    fn leverage_reasons_mark_contained_refactor_surfaces() {
        let reasons = classify_leverage_reasons(
            "dependency_sprawl",
            "trusted",
            "structural_debt",
            "local_refactor_target",
            &[
                "facade_with_extracted_owners".to_string(),
                "guarded_seam".to_string(),
            ],
            Some(4),
            Some(14),
            Some(0),
            None,
            Some(1),
            0,
            0,
        );

        assert!(reasons
            .iter()
            .any(|reason| reason == "contained_refactor_surface"));
    }

    #[test]
    fn annotate_debt_signal_preserves_explicit_leverage_metadata() {
        let signal = annotate_debt_signal(DebtSignal {
            kind: "dependency_sprawl".to_string(),
            trust_tier: "trusted".to_string(),
            presentation_class: "structural_debt".to_string(),
            leverage_class: "local_refactor_target".to_string(),
            scope: "src/components/terminal-session.ts".to_string(),
            signal_class: "debt".to_string(),
            signal_families: vec!["coordination".to_string()],
            severity: "medium".to_string(),
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

        assert_eq!(signal.leverage_class, "local_refactor_target");
        assert_eq!(
            signal.leverage_reasons,
            vec!["extracted_owner_shell_pressure".to_string()]
        );
    }

    #[test]
    fn annotate_finding_detail_preserves_explicit_leverage_metadata() {
        let detail = annotate_finding_detail(FindingDetail {
            kind: "unstable_hotspot".to_string(),
            trust_tier: "trusted".to_string(),
            presentation_class: "guarded_facade".to_string(),
            leverage_class: "boundary_discipline".to_string(),
            scope: "src/lib/ipc.ts".to_string(),
            severity: "medium".to_string(),
            summary: "Transport facade is under pressure".to_string(),
            impact: "Glue can absorb domain logic.".to_string(),
            files: vec!["src/lib/ipc.ts".to_string()],
            role_tags: vec!["transport_facade".to_string()],
            leverage_reasons: vec!["boundary_or_facade_seam_pressure".to_string()],
            evidence: vec!["fan-in: 42".to_string()],
            inspection_focus: vec!["inspect policy leakage".to_string()],
            candidate_split_axes: vec!["transport boundary".to_string()],
            related_surfaces: vec!["src/lib/ipc.ts".to_string()],
            metrics: FindingDetailMetrics::default(),
        });

        assert_eq!(detail.leverage_class, "boundary_discipline");
        assert_eq!(
            detail.leverage_reasons,
            vec!["boundary_or_facade_seam_pressure".to_string()]
        );
    }

    #[test]
    fn annotate_inspection_watchpoint_preserves_explicit_leverage_metadata() {
        let watchpoint = annotate_inspection_watchpoint(InspectionWatchpoint {
            kind: "cycle_cluster".to_string(),
            trust_tier: "watchpoint".to_string(),
            presentation_class: "watchpoint".to_string(),
            leverage_class: "architecture_signal".to_string(),
            scope: "src/store/store.ts".to_string(),
            severity: "high".to_string(),
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

        assert_eq!(watchpoint.leverage_class, "architecture_signal");
        assert_eq!(
            watchpoint.leverage_reasons,
            vec!["shared_barrel_boundary_hub".to_string()]
        );
    }

    #[test]
    fn findings_surface_structural_debt_signals() {
        let root = structural_debt_fixture_root();
        let mut state = fresh_mcp_state();
        handle_scan(
            &json!({"path": root.to_string_lossy().to_string()}),
            &Tier::Free,
            &mut state,
        )
        .expect("scan fixture");

        let response =
            handle_findings(&json!({"limit": 25}), &Tier::Free, &mut state).expect("findings");

        assert!(response["debt_signals"]
            .as_array()
            .expect("debt signals")
            .iter()
            .any(|signal| {
                signal["kind"] == "large_file"
                    && signal["scope"] == "src/app.ts"
                    && signal["leverage_class"]
                        .as_str()
                        .is_some_and(|value| !value.is_empty())
                    && signal["leverage_reasons"]
                        .as_array()
                        .is_some_and(|reasons| !reasons.is_empty())
            }));
        assert!(response["watchpoints"]
            .as_array()
            .expect("watchpoints")
            .iter()
            .any(|signal| signal["kind"] == "cycle_cluster"));
        assert!(response["findings"]
            .as_array()
            .expect("findings array")
            .iter()
            .any(|finding| finding["kind"] == "large_file"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn findings_surface_dead_island_signals() {
        let root = dead_island_fixture_root();
        let mut state = fresh_mcp_state();
        handle_scan(
            &json!({"path": root.to_string_lossy().to_string()}),
            &Tier::Free,
            &mut state,
        )
        .expect("scan fixture");

        let response =
            handle_findings(&json!({"limit": 25}), &Tier::Free, &mut state).expect("findings");

        assert!(response["watchpoints"]
            .as_array()
            .expect("watchpoints")
            .iter()
            .any(|signal| signal["kind"] == "dead_island"));
        assert!(response["findings"]
            .as_array()
            .expect("findings array")
            .iter()
            .any(|finding| finding["kind"] == "dead_island"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn findings_isolate_experimental_structural_findings() {
        let root = dead_private_fixture_root();
        let mut state = fresh_mcp_state();
        handle_scan(
            &json!({"path": root.to_string_lossy().to_string()}),
            &Tier::Free,
            &mut state,
        )
        .expect("scan fixture");

        let response =
            handle_findings(&json!({"limit": 25}), &Tier::Free, &mut state).expect("findings");

        assert!(response["experimental_debt_signals"]
            .as_array()
            .expect("experimental debt signals")
            .iter()
            .any(|signal| {
                signal["kind"] == "dead_private_code_cluster"
                    && signal["trust_tier"] == "experimental"
            }));
        assert!(!response["debt_signals"]
            .as_array()
            .expect("debt signals")
            .iter()
            .any(|signal| signal["kind"] == "dead_private_code_cluster"));
        assert!(response["experimental_findings"]
            .as_array()
            .expect("experimental findings")
            .iter()
            .any(|finding| finding["kind"] == "dead_private_code_cluster"));
        assert!(!response["finding_details"]
            .as_array()
            .expect("finding details")
            .iter()
            .any(|detail| detail["kind"] == "dead_private_code_cluster"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn findings_surface_watchpoints_include_trust_tier_and_fixability_fields() {
        let root = dead_island_fixture_root();
        let mut state = fresh_mcp_state();
        handle_scan(
            &json!({"path": root.to_string_lossy().to_string()}),
            &Tier::Free,
            &mut state,
        )
        .expect("scan fixture");

        let response =
            handle_findings(&json!({"limit": 25}), &Tier::Free, &mut state).expect("findings");

        assert!(response["watchpoints"]
            .as_array()
            .expect("watchpoints")
            .iter()
            .any(|watchpoint| {
                watchpoint["kind"] == "dead_island"
                    && watchpoint["trust_tier"] == "watchpoint"
                    && watchpoint["candidate_split_axes"]
                        .as_array()
                        .is_some_and(|axes| !axes.is_empty())
                    && watchpoint["related_surfaces"]
                        .as_array()
                        .is_some_and(|related| !related.is_empty())
            }));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn findings_surface_debt_clusters_for_overlapping_signals() {
        let root = dead_island_fixture_root();
        let mut state = fresh_mcp_state();
        handle_scan(
            &json!({"path": root.to_string_lossy().to_string()}),
            &Tier::Free,
            &mut state,
        )
        .expect("scan fixture");

        let response =
            handle_findings(&json!({"limit": 25}), &Tier::Free, &mut state).expect("findings");

        assert!(response["debt_clusters"]
            .as_array()
            .expect("debt clusters")
            .iter()
            .any(|cluster| {
                cluster["signal_kinds"]
                    .as_array()
                    .expect("signal kinds")
                    .iter()
                    .any(|kind| kind == "dead_island")
                    && cluster["signal_kinds"]
                        .as_array()
                        .expect("signal kinds")
                        .iter()
                        .any(|kind| kind == "cycle_cluster")
            }));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn findings_hide_suppressed_clone_families_and_remediations() {
        let root = cli_gate_fixture_root();
        write_file(
            &root,
            ".sentrux/rules.toml",
            r#"
                [[suppress]]
                kind = "exact_clone_group"
                file = "src/a.ts"
                reason = "temporary clone suppression"
                expires = "2099-12-31"
            "#,
        );
        write_file(
            &root,
            "src/a.ts",
            "export function duplicateAlpha(): number { return 1; }\n",
        );
        write_file(
            &root,
            "src/b.ts",
            "export function duplicateBeta(): number { return 1; }\n",
        );

        let mut state = fresh_mcp_state();
        handle_scan(
            &json!({"path": root.to_string_lossy().to_string()}),
            &Tier::Free,
            &mut state,
        )
        .expect("scan fixture");
        state
            .cached_health
            .as_mut()
            .expect("cached health")
            .duplicate_groups = vec![
            DuplicateGroup {
                hash: 41,
                instances: vec![
                    ("src/a.ts".into(), "duplicateAlpha".into(), 12),
                    ("src/b.ts".into(), "duplicateBeta".into(), 12),
                ],
            },
            DuplicateGroup {
                hash: 42,
                instances: vec![
                    ("src/a.ts".into(), "duplicateGamma".into(), 10),
                    ("src/b.ts".into(), "duplicateDelta".into(), 10),
                ],
            },
        ];

        let response =
            handle_findings(&json!({"limit": 10}), &Tier::Free, &mut state).expect("findings");

        assert_eq!(response["clone_group_count"], 2);
        assert_eq!(response["visible_clone_group_count"], 0);
        assert_eq!(response["visible_clone_family_count"], 0);
        assert!(response["clone_families"]
            .as_array()
            .expect("clone families")
            .is_empty());
        assert!(response["clone_remediations"]
            .as_array()
            .expect("clone remediations")
            .is_empty());
        assert!(!response["findings"]
            .as_array()
            .expect("findings array")
            .iter()
            .any(|finding| finding["kind"] == "exact_clone_group"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn findings_include_all_family_remediation_hints() {
        let root = cli_gate_fixture_root();
        write_file(
            &root,
            "src/a.ts",
            "export function duplicateAlpha(): number { return 1; }\n",
        );
        write_file(
            &root,
            "src/b.ts",
            "export function duplicateBeta(): number { return 1; }\n",
        );

        let mut state = fresh_mcp_state();
        handle_scan(
            &json!({"path": root.to_string_lossy().to_string()}),
            &Tier::Free,
            &mut state,
        )
        .expect("scan fixture");
        state.cached_evolution = Some(EvolutionReport {
            churn: HashMap::from([
                (
                    "src/a.ts".to_string(),
                    FileChurn {
                        commit_count: 4,
                        lines_added: 6,
                        lines_removed: 1,
                        total_churn: 7,
                    },
                ),
                (
                    "src/b.ts".to_string(),
                    FileChurn {
                        commit_count: 0,
                        lines_added: 0,
                        lines_removed: 0,
                        total_churn: 0,
                    },
                ),
            ]),
            coupling_pairs: Vec::<CouplingPair>::new(),
            hotspots: Vec::<TemporalHotspot>::new(),
            code_age: HashMap::from([("src/a.ts".to_string(), 3), ("src/b.ts".to_string(), 90)]),
            authors: HashMap::<String, AuthorInfo>::new(),
            single_author_ratio: 0.0,
            bus_factor_score: 1.0,
            churn_score: 1.0,
            evolution_score: 1.0,
            lookback_days: 90,
            commits_analyzed: 4,
        });
        state
            .cached_health
            .as_mut()
            .expect("cached health")
            .duplicate_groups = vec![
            DuplicateGroup {
                hash: 41,
                instances: vec![
                    ("src/a.ts".into(), "duplicateAlpha".into(), 12),
                    ("src/b.ts".into(), "duplicateBeta".into(), 12),
                ],
            },
            DuplicateGroup {
                hash: 42,
                instances: vec![
                    ("src/a.ts".into(), "duplicateGamma".into(), 10),
                    ("src/b.ts".into(), "duplicateDelta".into(), 10),
                ],
            },
        ];

        let response =
            handle_findings(&json!({"limit": 10}), &Tier::Free, &mut state).expect("findings");
        let family_hints = response["clone_families"][0]["remediation_hints"]
            .as_array()
            .expect("family remediation hints");
        let clone_remediations = response["clone_remediations"]
            .as_array()
            .expect("clone remediations");

        assert!(family_hints.len() >= 3);
        assert_eq!(clone_remediations.len(), family_hints.len());
        assert!(clone_remediations
            .iter()
            .any(|hint| hint["kind"] == "sync_recent_divergence"));
        assert!(clone_remediations
            .iter()
            .any(|hint| hint["kind"] == "extract_shared_helper"));
        assert!(clone_remediations
            .iter()
            .any(|hint| hint["kind"] == "add_shared_behavior_tests"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn overall_confidence_penalizes_partial_and_truncated_scans() {
        let mut metadata = ScanMetadata::empty(ScanMode::Git);
        let baseline = overall_confidence_0_10000(&metadata, 9000, 8000);
        assert_eq!(baseline, 8000);

        metadata.partial = true;
        let partial = overall_confidence_0_10000(&metadata, 9000, 8000);
        assert_eq!(partial, 6400);

        metadata.truncated = true;
        let truncated = overall_confidence_0_10000(&metadata, 9000, 8000);
        assert_eq!(truncated, 4480);
    }

    #[test]
    fn session_v2_baseline_roundtrips_on_disk() {
        let root = temp_root("session-v2-roundtrip");
        let baseline = SessionV2Baseline {
            schema_version: SESSION_V2_SCHEMA_VERSION,
            project_fingerprint: Some(project_fingerprint(&root)),
            sentrux_version: Some(env!("CARGO_PKG_VERSION").to_string()),
            file_hashes: BTreeMap::from([
                ("src/a.ts".to_string(), 11),
                ("src/b.ts".to_string(), 22),
            ]),
            finding_payloads: BTreeMap::from([(
                "finding-1".to_string(),
                json!({"kind": "closed_domain_exhaustiveness", "severity": "high"}),
            )]),
            git_head: Some("abc123".to_string()),
            working_tree_paths: BTreeSet::from(["src/a.ts".to_string()]),
            confidence: SessionV2ConfidenceSnapshot {
                scan_confidence_0_10000: Some(8100),
                rule_coverage_0_10000: Some(7500),
            },
        };

        let path = save_session_v2_baseline(&root, &baseline).expect("save session baseline");
        let loaded = load_persisted_session_v2(&root)
            .expect("load session baseline")
            .expect("session baseline exists");

        assert_eq!(path, root.join(".sentrux").join("session-v2.json"));
        assert_eq!(loaded.file_hashes, baseline.file_hashes);
        assert_eq!(loaded.finding_payloads, baseline.finding_payloads);
        assert_eq!(loaded.git_head, baseline.git_head);
        assert_eq!(loaded.working_tree_paths, baseline.working_tree_paths);
        assert_eq!(loaded.schema_version, SESSION_V2_SCHEMA_VERSION);
        assert_eq!(loaded.project_fingerprint, baseline.project_fingerprint);
        assert_eq!(loaded.sentrux_version, baseline.sentrux_version);
        assert_eq!(
            loaded.confidence.scan_confidence_0_10000,
            baseline.confidence.scan_confidence_0_10000
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn project_fingerprint_matches_across_local_clone() {
        let source = temp_root("project-fingerprint-source");
        write_file(
            &source,
            "src/domain/state.ts",
            "export const state = 'idle';\n",
        );
        init_git_repo(&source);
        commit_all(&source, "initial");

        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let clone = std::env::temp_dir().join(format!(
            "private-integration-crateject-fingerprint-clone-{}-{unique}",
            std::process::id()
        ));
        let status = Command::new("git")
            .arg("clone")
            .arg("--quiet")
            .arg(&source)
            .arg(&clone)
            .status()
            .expect("clone repo");
        assert!(status.success(), "git clone failed");

        assert_eq!(project_fingerprint(&source), project_fingerprint(&clone));

        let _ = std::fs::remove_dir_all(source);
        let _ = std::fs::remove_dir_all(clone);
    }

    #[test]
    fn session_v2_baseline_deserializes_without_git_metadata() {
        let baseline: SessionV2Baseline = serde_json::from_value(json!({
            "file_hashes": { "src/a.ts": 11 },
            "finding_payloads": {}
        }))
        .expect("deserialize legacy session baseline");

        assert_eq!(baseline.schema_version, 1);
        assert_eq!(baseline.file_hashes["src/a.ts"], 11);
        assert!(baseline.git_head.is_none());
        assert!(baseline.working_tree_paths.is_empty());
        assert!(baseline.project_fingerprint.is_none());
        assert!(baseline.sentrux_version.is_none());
        assert!(baseline.confidence.scan_confidence_0_10000.is_none());
    }

    #[test]
    fn session_v2_status_rejects_unsupported_schema_versions() {
        let root = temp_root("session-v2-schema");
        write_file(
            &root,
            ".sentrux/session-v2.json",
            &serde_json::to_string_pretty(&json!({
                "schema_version": SESSION_V2_SCHEMA_VERSION + 1,
                "file_hashes": { "src/a.ts": 11 },
                "finding_payloads": {}
            }))
            .expect("serialize"),
        );

        let (baseline, status) = load_session_v2_baseline_status(&root);

        assert!(baseline.is_none());
        assert!(status.loaded);
        assert!(!status.compatible);
        assert_eq!(status.schema_version, Some(SESSION_V2_SCHEMA_VERSION + 1));
        assert!(status
            .error
            .as_deref()
            .unwrap_or_default()
            .contains("Unsupported"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn session_v2_status_rejects_project_mismatch() {
        let root = closed_domain_gate_fixture_root();
        write_file(
            &root,
            ".sentrux/session-v2.json",
            &serde_json::to_string_pretty(&json!({
                "schema_version": SESSION_V2_SCHEMA_VERSION,
                "project_fingerprint": "different-project",
                "sentrux_version": env!("CARGO_PKG_VERSION"),
                "file_hashes": { "src/domain/state.ts": 11 },
                "finding_payloads": {}
            }))
            .expect("serialize"),
        );

        let mut state = fresh_mcp_state();
        let (baseline, status) =
            current_session_v2_baseline_with_status(&mut state, &root).expect("load status");

        assert!(baseline.is_none());
        assert!(status.loaded);
        assert!(!status.compatible);
        assert_eq!(status.schema_version, Some(SESSION_V2_SCHEMA_VERSION));
        assert!(status
            .error
            .as_deref()
            .unwrap_or_default()
            .contains("project fingerprint"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn malformed_session_v2_baseline_degrades_gracefully() {
        let root = closed_domain_gate_fixture_root();
        write_file(&root, ".sentrux/session-v2.json", "{ invalid json");

        let mut state = fresh_mcp_state();
        handle_scan(
            &json!({"path": root.to_string_lossy().to_string()}),
            &Tier::Free,
            &mut state,
        )
        .expect("scan fixture");

        let health = handle_health(&json!({}), &Tier::Free, &mut state).expect("health");
        let gate = handle_gate(&json!({}), &Tier::Free, &mut state).expect("gate");
        let session_end =
            handle_session_end(&json!({}), &Tier::Free, &mut state).expect("session end");

        assert!(health["confidence"]["session_baseline"]["error"]
            .as_str()
            .unwrap_or_default()
            .contains("Failed to parse"));
        assert_eq!(gate["decision"], "pass");
        assert!(gate["confidence"]["session_baseline"]["error"]
            .as_str()
            .unwrap_or_default()
            .contains("Failed to parse"));
        assert_eq!(session_end["pass"], true);
        assert!(session_end["confidence"]["session_baseline"]["error"]
            .as_str()
            .unwrap_or_default()
            .contains("Failed to parse"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn session_end_uses_legacy_baseline_when_v2_session_is_missing() {
        let root = closed_domain_gate_fixture_root();
        cli_save_v2_session(&root).expect("save v2 session");
        std::fs::remove_file(root.join(".sentrux").join("session-v2.json"))
            .expect("remove v2 session baseline");

        let mut state = fresh_mcp_state();
        state.scan_root = Some(root.clone());

        let response =
            handle_session_end(&json!({}), &Tier::Free, &mut state).expect("session end");

        assert_eq!(response["pass"], true);
        assert_eq!(response["touched_concept_gate"]["decision"], "pass");
        assert!(response["confidence"]["session_baseline"]["loaded"]
            .as_bool()
            .is_some_and(|loaded| !loaded));
        assert!(response["baseline_error"].is_null());

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn session_end_reports_incompatible_session_v2_baseline_in_confidence() {
        let root = closed_domain_gate_fixture_root();
        cli_save_v2_session(&root).expect("save v2 session");
        write_file(
            &root,
            ".sentrux/session-v2.json",
            &serde_json::to_string_pretty(&json!({
                "schema_version": SESSION_V2_SCHEMA_VERSION + 1,
                "file_hashes": {},
                "finding_payloads": {}
            }))
            .expect("serialize"),
        );

        let mut state = fresh_mcp_state();
        state.scan_root = Some(root.clone());

        let response =
            handle_session_end(&json!({}), &Tier::Free, &mut state).expect("session end");

        assert_eq!(response["pass"], true);
        assert_eq!(
            response["confidence"]["session_baseline"]["compatible"],
            false
        );
        assert_eq!(
            response["confidence"]["session_baseline"]["schema_version"],
            SESSION_V2_SCHEMA_VERSION + 1
        );
        assert!(response["confidence"]["session_baseline"]["error"]
            .as_str()
            .unwrap_or_default()
            .contains("Unsupported"));
        assert!(response["baseline_error"].is_null());

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn session_end_reports_project_mismatch_session_v2_baseline_in_confidence() {
        let root = closed_domain_gate_fixture_root();
        cli_save_v2_session(&root).expect("save v2 session");
        write_file(
            &root,
            ".sentrux/session-v2.json",
            &serde_json::to_string_pretty(&json!({
                "schema_version": SESSION_V2_SCHEMA_VERSION,
                "project_fingerprint": "different-project",
                "sentrux_version": env!("CARGO_PKG_VERSION"),
                "file_hashes": {},
                "finding_payloads": {}
            }))
            .expect("serialize"),
        );

        let mut state = fresh_mcp_state();
        state.scan_root = Some(root.clone());

        let response =
            handle_session_end(&json!({}), &Tier::Free, &mut state).expect("session end");

        assert_eq!(response["pass"], true);
        assert_eq!(
            response["confidence"]["session_baseline"]["compatible"],
            false
        );
        assert_eq!(
            response["confidence"]["session_baseline"]["schema_version"],
            SESSION_V2_SCHEMA_VERSION
        );
        assert!(response["confidence"]["session_baseline"]["error"]
            .as_str()
            .unwrap_or_default()
            .contains("project fingerprint"));
        assert!(response["baseline_error"].is_null());

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn health_surfaces_legacy_baseline_delta_and_v2_confidence() {
        let root = closed_domain_gate_fixture_root();
        let mut state = fresh_mcp_state();
        handle_scan(
            &json!({"path": root.to_string_lossy().to_string()}),
            &Tier::Free,
            &mut state,
        )
        .expect("scan fixture");
        handle_session_start(&json!({}), &Tier::Free, &mut state).expect("session start");

        let response = handle_health(&json!({}), &Tier::Free, &mut state).expect("health");

        assert_eq!(response["baseline_delta"]["available"], true);
        assert!(
            response["confidence"]["scan_confidence_0_10000"]
                .as_u64()
                .unwrap_or_default()
                > 0
        );
        assert_eq!(
            response["confidence"]["session_baseline"]["schema_version"],
            SESSION_V2_SCHEMA_VERSION
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn gate_reports_incompatible_session_v2_baseline_in_confidence() {
        let root = closed_domain_gate_fixture_root();
        write_file(
            &root,
            ".sentrux/session-v2.json",
            &serde_json::to_string_pretty(&json!({
                "schema_version": SESSION_V2_SCHEMA_VERSION + 1,
                "file_hashes": {},
                "finding_payloads": {}
            }))
            .expect("serialize"),
        );

        let evaluated = cli_evaluate_v2_gate(&root, false).expect("evaluate v2 gate");

        assert_eq!(evaluated["decision"], "pass");
        assert_eq!(
            evaluated["confidence"]["session_baseline"]["compatible"],
            false
        );
        assert_eq!(
            evaluated["confidence"]["session_baseline"]["schema_version"],
            SESSION_V2_SCHEMA_VERSION + 1
        );
        assert!(evaluated["confidence"]["session_baseline"]["error"]
            .as_str()
            .unwrap_or_default()
            .contains("Unsupported"));
    }

    #[test]
    fn gate_reports_project_mismatch_session_v2_baseline_in_confidence() {
        let root = closed_domain_gate_fixture_root();
        write_file(
            &root,
            ".sentrux/session-v2.json",
            &serde_json::to_string_pretty(&json!({
                "schema_version": SESSION_V2_SCHEMA_VERSION,
                "project_fingerprint": "different-project",
                "sentrux_version": env!("CARGO_PKG_VERSION"),
                "file_hashes": {},
                "finding_payloads": {}
            }))
            .expect("serialize"),
        );

        let evaluated = cli_evaluate_v2_gate(&root, false).expect("evaluate v2 gate");

        assert_eq!(evaluated["decision"], "pass");
        assert_eq!(
            evaluated["confidence"]["session_baseline"]["compatible"],
            false
        );
        assert_eq!(
            evaluated["confidence"]["session_baseline"]["schema_version"],
            SESSION_V2_SCHEMA_VERSION
        );
        assert!(evaluated["confidence"]["session_baseline"]["error"]
            .as_str()
            .unwrap_or_default()
            .contains("project fingerprint"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn session_changed_files_include_committed_renames() {
        let root = cli_gate_fixture_root();
        init_git_repo(&root);
        commit_all(&root, "initial");

        let mut state = fresh_mcp_state();
        let baseline_scan = do_scan(&root).expect("scan baseline");
        let (session_v2, _, _) = build_session_v2_baseline(
            &mut state,
            &root,
            &baseline_scan.snapshot,
            &baseline_scan.health,
            &baseline_scan.metadata,
        );

        run_git(
            &root,
            &["mv", "src/domain/state.ts", "src/domain/app-state.ts"],
        );
        commit_all(&root, "rename state");

        let current_scan = do_scan(&root).expect("scan renamed tree");
        let changed_files = changed_files_from_session_context(
            &root,
            &current_scan.snapshot,
            Some(&session_v2),
            None,
        );

        assert!(changed_files.contains("src/domain/state.ts"));
        assert!(changed_files.contains("src/domain/app-state.ts"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn cli_v2_gate_can_save_and_pass_without_changes() {
        let root = cli_gate_fixture_root();

        let saved = cli_save_v2_session(&root).expect("save v2 session");
        let evaluated = cli_evaluate_v2_gate(&root, false).expect("evaluate v2 gate");

        assert_eq!(saved["status"], "Baseline saved");
        assert!(saved["session_v2_baseline_path"]
            .as_str()
            .unwrap_or_default()
            .ends_with(".sentrux/session-v2.json"));
        assert_eq!(evaluated["decision"], "pass");
        assert_eq!(evaluated["summary"], "No working-tree changes detected");
        assert_eq!(
            evaluated["changed_files"]
                .as_array()
                .map(|files| files.len()),
            Some(0)
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn session_changed_files_detect_revert_from_dirty_baseline() {
        let root = cli_gate_fixture_root();
        init_git_repo(&root);
        commit_all(&root, "initial");

        write_file(
            &root,
            "src/domain/state.ts",
            "export type AppState = 'idle' | 'busy' | 'error';\n",
        );

        let mut state = fresh_mcp_state();
        let dirty_scan = do_scan(&root).expect("scan dirty baseline");
        let (session_v2, _, _) = build_session_v2_baseline(
            &mut state,
            &root,
            &dirty_scan.snapshot,
            &dirty_scan.health,
            &dirty_scan.metadata,
        );

        write_file(
            &root,
            "src/domain/state.ts",
            "export type AppState = 'idle' | 'busy';\n",
        );

        let reverted_scan = do_scan(&root).expect("scan reverted tree");
        let changed_files = changed_files_from_session_context(
            &root,
            &reverted_scan.snapshot,
            Some(&session_v2),
            None,
        );

        assert!(changed_files.contains("src/domain/state.ts"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn patch_check_context_reuses_cached_scan_when_nothing_changed() {
        let root = cli_gate_fixture_root();
        let bundle = do_scan(&root).expect("scan fixture");
        let mut state = fresh_mcp_state();
        update_scan_cache(
            &mut state,
            root.clone(),
            bundle,
            None,
            super::current_scan_identity(&root),
        );

        let context =
            prepare_patch_check_context(&state, &root, None).expect("prepare patch context");

        assert!(context.reused_cached_scan);
        assert!(context.changed_files.is_empty());

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn patch_check_context_reuses_cached_scan_for_same_changed_tree() {
        let root = closed_domain_gate_fixture_root();
        init_git_repo(&root);
        commit_all(&root, "initial");

        let mut state = fresh_mcp_state();
        handle_scan(
            &json!({"path": root.to_string_lossy().to_string()}),
            &Tier::Free,
            &mut state,
        )
        .expect("scan initial tree");
        handle_session_start(&json!({}), &Tier::Free, &mut state).expect("session start");

        write_file(
            &root,
            "src/domain/state.ts",
            "export type AppState = 'idle' | 'busy' | 'error';\n",
        );

        let changed_bundle = do_scan(&root).expect("scan changed tree");
        let baseline = state.baseline.clone();
        update_scan_cache(
            &mut state,
            root.clone(),
            changed_bundle,
            baseline,
            super::current_scan_identity(&root),
        );

        let context = prepare_patch_check_context(&state, &root, state.session_v2.as_ref())
            .expect("prepare patch context on changed tree");

        assert!(context.reused_cached_scan);
        assert!(context.changed_files.contains("src/domain/state.ts"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn patch_check_context_refreshes_cached_scan_when_dirty_contents_change() {
        let root = closed_domain_gate_fixture_root();
        init_git_repo(&root);
        commit_all(&root, "initial");

        let mut state = fresh_mcp_state();
        handle_scan(
            &json!({"path": root.to_string_lossy().to_string()}),
            &Tier::Free,
            &mut state,
        )
        .expect("scan initial tree");
        handle_session_start(&json!({}), &Tier::Free, &mut state).expect("session start");

        write_file(
            &root,
            "src/domain/state.ts",
            "export type AppState = 'idle' | 'busy' | 'error';\n",
        );

        let changed_bundle = do_scan(&root).expect("scan changed tree");
        let baseline = state.baseline.clone();
        update_scan_cache(
            &mut state,
            root.clone(),
            changed_bundle,
            baseline,
            super::current_scan_identity(&root),
        );

        write_file(
            &root,
            "src/domain/state.ts",
            "export type AppState = 'idle' | 'busy' | 'error' | 'paused';\n",
        );

        let context = prepare_patch_check_context(&state, &root, state.session_v2.as_ref())
            .expect("prepare patch context on edited dirty tree");

        assert!(!context.reused_cached_scan);
        assert!(context.changed_files.contains("src/domain/state.ts"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn gate_preserves_derived_caches_on_no_change_path() {
        let root = closed_domain_gate_fixture_root();
        let mut state = fresh_mcp_state();
        handle_scan(
            &json!({"path": root.to_string_lossy().to_string()}),
            &Tier::Free,
            &mut state,
        )
        .expect("scan fixture");
        handle_session_start(&json!({}), &Tier::Free, &mut state).expect("session start");
        handle_obligations(&json!({}), &Tier::Free, &mut state).expect("populate semantic cache");

        assert!(state.cached_semantic.is_some());

        let response = handle_gate(&json!({}), &Tier::Free, &mut state).expect("gate");

        assert_eq!(response["decision"], "pass");
        assert_eq!(response["summary"], "No working-tree changes detected");
        assert!(state.cached_semantic.is_some());

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn gate_and_session_end_preserve_patch_safety_cache_on_changed_tree() {
        let root = closed_domain_gate_fixture_root();
        init_git_repo(&root);
        commit_all(&root, "initial");
        let mut state = fresh_mcp_state();
        handle_scan(
            &json!({"path": root.to_string_lossy().to_string()}),
            &Tier::Free,
            &mut state,
        )
        .expect("scan fixture");
        handle_session_start(&json!({}), &Tier::Free, &mut state).expect("session start");
        write_file(
            &root,
            "src/domain/state.ts",
            "export type AppState = 'idle' | 'busy' | 'error';\n",
        );

        let gate = handle_gate(&json!({}), &Tier::Free, &mut state).expect("gate");
        assert!(gate["changed_files"]
            .as_array()
            .expect("changed files")
            .iter()
            .any(|value| value == "src/domain/state.ts"));
        assert!(state.cached_patch_safety.is_some());

        let session_end =
            handle_session_end(&json!({}), &Tier::Free, &mut state).expect("session end");
        assert!(session_end["changed_files"]
            .as_array()
            .expect("changed files")
            .iter()
            .any(|value| value == "src/domain/state.ts"));
        assert!(state.cached_patch_safety.is_some());

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn session_end_surfaces_debt_signals_for_changed_concept() {
        let root = closed_domain_gate_fixture_root();
        init_git_repo(&root);
        commit_all(&root, "initial");
        let mut state = fresh_mcp_state();
        handle_scan(
            &json!({"path": root.to_string_lossy().to_string()}),
            &Tier::Free,
            &mut state,
        )
        .expect("scan fixture");
        handle_session_start(&json!({}), &Tier::Free, &mut state).expect("session start");
        write_file(
            &root,
            "src/domain/state.ts",
            "export type AppState = 'idle' | 'busy' | 'error';\n",
        );

        let response =
            handle_session_end(&json!({}), &Tier::Free, &mut state).expect("session end");

        assert!(response["concept_summaries"]
            .as_array()
            .expect("concept summaries")
            .iter()
            .any(|summary| summary["concept_id"] == "app_state"));
        assert!(response["debt_signals"]
            .as_array()
            .expect("debt signals")
            .iter()
            .any(|signal| {
                signal["scope"] == "app_state"
                    && signal["signal_class"]
                        .as_str()
                        .is_some_and(|class| class == "hardening" || class == "debt")
            }));
        assert!(response["watchpoints"]
            .as_array()
            .expect("watchpoints")
            .iter()
            .any(|watchpoint| watchpoint["scope"] == "app_state"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn session_end_surfaces_structural_debt_signals_for_changed_file() {
        let root = temp_root("structural-session-end");
        init_git_repo(&root);
        write_file(
            &root,
            "src/app.ts",
            "import { alpha } from './a';\nexport function render(): number { return alpha(); }\n",
        );
        write_file(
            &root,
            "src/a.ts",
            "export function alpha(): number { return 1; }\n",
        );
        commit_all(&root, "initial");

        let mut state = fresh_mcp_state();
        handle_scan(
            &json!({"path": root.to_string_lossy().to_string()}),
            &Tier::Free,
            &mut state,
        )
        .expect("scan fixture");
        handle_session_start(&json!({}), &Tier::Free, &mut state).expect("session start");

        let mut appended = String::new();
        for index in 0..900 {
            appended.push_str(&format!("export const item{index} = {index};\n"));
        }
        append_file(&root, "src/app.ts", &appended);

        let response =
            handle_session_end(&json!({}), &Tier::Free, &mut state).expect("session end");

        assert!(response["debt_signals"]
            .as_array()
            .expect("debt signals")
            .iter()
            .any(|signal| signal["kind"] == "large_file" && signal["scope"] == "src/app.ts"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn session_end_surfaces_dead_island_signals_for_changed_component_file() {
        let root = dead_island_fixture_root();
        init_git_repo(&root);
        commit_all(&root, "initial");

        let mut state = fresh_mcp_state();
        handle_scan(
            &json!({"path": root.to_string_lossy().to_string()}),
            &Tier::Free,
            &mut state,
        )
        .expect("scan fixture");
        handle_session_start(&json!({}), &Tier::Free, &mut state).expect("session start");
        append_file(
            &root,
            "src/orphan-a.ts",
            "\nexport const orphanMarker = 1;\n",
        );

        let response =
            handle_session_end(&json!({}), &Tier::Free, &mut state).expect("session end");

        assert!(response["watchpoints"]
            .as_array()
            .expect("watchpoints")
            .iter()
            .any(|signal| signal["kind"] == "dead_island"));
        assert!(response["finding_details"]
            .as_array()
            .expect("finding details")
            .iter()
            .any(|detail| {
                detail["kind"] == "dead_island"
                    && detail["impact"]
                        .as_str()
                        .is_some_and(|impact| impact.contains("maintenance noise"))
            }));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn session_end_surfaces_debt_clusters_for_changed_component_file() {
        let root = dead_island_fixture_root();
        init_git_repo(&root);
        commit_all(&root, "initial");

        let mut state = fresh_mcp_state();
        handle_scan(
            &json!({"path": root.to_string_lossy().to_string()}),
            &Tier::Free,
            &mut state,
        )
        .expect("scan fixture");
        handle_session_start(&json!({}), &Tier::Free, &mut state).expect("session start");
        append_file(
            &root,
            "src/orphan-a.ts",
            "\nexport const orphanClusterMarker = 1;\n",
        );

        let response =
            handle_session_end(&json!({}), &Tier::Free, &mut state).expect("session end");

        assert!(response["debt_clusters"]
            .as_array()
            .expect("debt clusters")
            .iter()
            .any(|cluster| {
                cluster["signal_kinds"]
                    .as_array()
                    .expect("signal kinds")
                    .iter()
                    .any(|kind| kind == "dead_island")
            }));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn gate_keeps_clone_findings_when_semantic_analysis_fails() {
        let root = cli_gate_fixture_root();
        write_file(
            &root,
            "src/a.ts",
            "export function duplicateAlpha(): number { return 1; }\n",
        );
        write_file(
            &root,
            "src/b.ts",
            "export function duplicateBeta(): number { return 1; }\n",
        );

        let mut state = fresh_mcp_state();
        handle_scan(
            &json!({"path": root.to_string_lossy().to_string()}),
            &Tier::Free,
            &mut state,
        )
        .expect("scan fixture");
        handle_session_start(&json!({}), &Tier::Free, &mut state).expect("session start");
        state
            .cached_health
            .as_mut()
            .expect("cached health")
            .duplicate_groups = vec![DuplicateGroup {
            hash: 41,
            instances: vec![
                ("src/a.ts".into(), "duplicateAlpha".into(), 12),
                ("src/b.ts".into(), "duplicateBeta".into(), 12),
            ],
        }];
        state.cached_semantic = None;
        let mut broken_config = default_bridge_config();
        broken_config.entrypoint = "/definitely/missing-sentrux-bridge.js".to_string();
        state.semantic_bridge = Some(TypeScriptBridgeSupervisor::new(broken_config));

        let gate = handle_gate(&json!({}), &Tier::Free, &mut state).expect("gate");

        assert!(gate["semantic_error"]
            .as_str()
            .expect("semantic error")
            .contains("Semantic analysis unavailable"));
        assert!(gate["introduced_findings"]
            .as_array()
            .expect("introduced findings")
            .iter()
            .any(|finding| finding["kind"] == "exact_clone_group"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn cli_v2_gate_fails_on_closed_domain_regression() {
        let root = closed_domain_gate_fixture_root();
        cli_save_v2_session(&root).expect("save v2 session");
        write_file(
            &root,
            "src/domain/state.ts",
            "export type AppState = 'idle' | 'busy' | 'error';\n",
        );

        let evaluated = cli_evaluate_v2_gate(&root, false).expect("evaluate v2 gate");

        assert_eq!(evaluated["decision"], "fail");
        assert_eq!(evaluated["summary"], "Touched-concept regressions detected");
        assert!(evaluated["changed_files"]
            .as_array()
            .expect("changed files")
            .iter()
            .any(|value| value == "src/domain/state.ts"));
        assert!(evaluated["introduced_findings"]
            .as_array()
            .expect("introduced findings")
            .iter()
            .any(|value| value["kind"] == "closed_domain_exhaustiveness"));
        assert!(evaluated["missing_obligations"]
            .as_array()
            .expect("missing obligations")
            .iter()
            .any(|value| value["domain_symbol_name"] == "AppState"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn cli_v2_gate_ignores_invalid_legacy_baseline_when_v2_session_exists() {
        let root = closed_domain_gate_fixture_root();
        cli_save_v2_session(&root).expect("save v2 session");
        write_file(&root, ".sentrux/baseline.json", "{ invalid json");
        write_file(
            &root,
            "src/domain/state.ts",
            "export type AppState = 'idle' | 'busy' | 'error';\n",
        );

        let evaluated = cli_evaluate_v2_gate(&root, false).expect("evaluate v2 gate");

        assert_eq!(evaluated["decision"], "fail");
        assert!(evaluated["baseline_error"].is_null());
        assert!(evaluated["introduced_findings"]
            .as_array()
            .expect("introduced findings")
            .iter()
            .any(|value| value["kind"] == "closed_domain_exhaustiveness"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn cli_v2_gate_quarantines_experimental_findings() {
        let root = experimental_gate_fixture_root();
        cli_save_v2_session(&root).expect("save v2 session");
        write_file(
            &root,
            "src/stale.ts",
            "function deadAlpha(): number { return 1; }\nfunction deadBeta(): number { return 2; }\nexport const liveValue = 3;\n",
        );

        let evaluated = cli_evaluate_v2_gate(&root, false).expect("evaluate v2 gate");

        assert_eq!(evaluated["decision"], "pass");
        assert!(evaluated["introduced_findings"]
            .as_array()
            .expect("introduced findings")
            .is_empty());
        assert_eq!(evaluated["experimental_finding_count"], 1);
        assert!(evaluated["experimental_findings"]
            .as_array()
            .expect("experimental findings")
            .iter()
            .any(|value| value["kind"] == "dead_private_code_cluster"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn cli_v2_gate_fails_on_contract_surface_regression() {
        let root = contract_gate_fixture_root();
        cli_save_v2_session(&root).expect("save v2 session");
        write_file(
            &root,
            "src/domain/bootstrap.ts",
            r#"
                export const BOOTSTRAP_CATEGORIES = ['tasks', 'reviews'] as const;
                export type BootstrapCategory = (typeof BOOTSTRAP_CATEGORIES)[number];
                export type BootstrapPayloadMap = {
                  tasks: { count: number };
                  reviews: { total: number };
                };
            "#,
        );

        let evaluated = cli_evaluate_v2_gate(&root, false).expect("evaluate v2 gate");

        assert_eq!(evaluated["decision"], "fail");
        assert!(evaluated["missing_obligations"]
            .as_array()
            .expect("missing obligations")
            .iter()
            .any(|value| {
                value["kind"] == "contract_surface_completeness"
                    && value["concept_id"] == "server_state_bootstrap"
            }));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn session_end_works_with_v2_session_when_legacy_baseline_is_missing() {
        let root = closed_domain_gate_fixture_root();
        cli_save_v2_session(&root).expect("save v2 session");
        std::fs::remove_file(root.join(".sentrux").join("baseline.json"))
            .expect("remove legacy baseline");
        write_file(
            &root,
            "src/domain/state.ts",
            "export type AppState = 'idle' | 'busy' | 'error';\n",
        );

        let mut state = fresh_mcp_state();
        state.scan_root = Some(root.clone());

        let response = handle_session_end(&json!({}), &Tier::Free, &mut state)
            .expect("session end without legacy baseline");

        assert_eq!(response["pass"], false);
        assert_eq!(response["summary"], "Touched-concept regressions detected");
        assert!(response["signal_before"].is_null());
        assert!(response["signal_after"].is_null());
        assert!(response["baseline_error"]
            .as_str()
            .unwrap_or_default()
            .contains("Legacy baseline unavailable"));
        assert!(response["introduced_findings"]
            .as_array()
            .expect("introduced findings")
            .iter()
            .any(|value| value["kind"] == "closed_domain_exhaustiveness"));
        assert!(response["missing_obligations"]
            .as_array()
            .expect("missing obligations")
            .iter()
            .any(|value| value["domain_symbol_name"] == "AppState"));
        assert_eq!(response["touched_concept_gate"]["decision"], "fail");

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn session_end_quarantines_experimental_introduced_findings() {
        let root = experimental_gate_fixture_root();
        cli_save_v2_session(&root).expect("save v2 session");
        let baseline_path = root.join(".sentrux").join("baseline.json");
        if baseline_path.exists() {
            std::fs::remove_file(&baseline_path).expect("remove legacy baseline");
        }
        write_file(
            &root,
            "src/stale.ts",
            "function deadAlpha(): number { return 1; }\nfunction deadBeta(): number { return 2; }\nexport const liveValue = 3;\n",
        );

        let mut state = fresh_mcp_state();
        state.scan_root = Some(root.clone());

        let response = handle_session_end(&json!({}), &Tier::Free, &mut state)
            .expect("session end for experimental finding");

        assert_eq!(response["touched_concept_gate"]["decision"], "pass");
        assert!(response["introduced_findings"]
            .as_array()
            .expect("introduced findings")
            .is_empty());
        assert_eq!(response["experimental_finding_count"], 1);
        assert!(response["experimental_findings"]
            .as_array()
            .expect("experimental findings")
            .iter()
            .any(|value| value["kind"] == "dead_private_code_cluster"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn malformed_v2_rules_are_reported() {
        let root = temp_root("rules-error");
        std::fs::write(
            root.join(".sentrux").join("rules.toml"),
            "[[concept]\nid = \"broken\"\nanchors = [",
        )
        .expect("write broken rules");

        let mut state = fresh_mcp_state();
        let (config, error) = load_v2_rules_config(&mut state, &root);

        assert!(config.concept.is_empty());
        assert!(error
            .as_deref()
            .unwrap_or_default()
            .contains("Failed to parse"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn rules_cache_reloads_when_rules_file_changes() {
        let root = temp_root("rules-cache-reload");
        write_file(
            &root,
            ".sentrux/rules.toml",
            r#"
                [[concept]]
                id = "task_git_status"
                anchors = ["src/store/core.ts::store.taskGitStatus"]
                allowed_writers = ["src/app/git-status-sync.ts::*"]
            "#,
        );

        let mut state = fresh_mcp_state();
        let (first_config, first_error) = load_v2_rules_config(&mut state, &root);
        assert_eq!(first_config.concept.len(), 1);
        assert!(first_error.is_none());

        std::thread::sleep(std::time::Duration::from_millis(5));
        write_file(
            &root,
            ".sentrux/rules.toml",
            r#"
                [[concept]]
                id = "broken"
                anchors = [
            "#,
        );

        let (second_config, second_error) = load_v2_rules_config(&mut state, &root);
        assert!(second_config.concept.is_empty());
        assert!(second_error
            .as_deref()
            .unwrap_or_default()
            .contains("Failed to parse"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn explain_concept_returns_related_findings_obligations_and_contracts() {
        let root = concept_fixture_root();
        let semantic = concept_fixture_semantic(&root);
        let mut state = state_with_semantic(&root, semantic);

        let response =
            handle_explain_concept(&json!({"id": "task_git_status"}), &Tier::Free, &mut state)
                .expect("explain concept");

        assert_eq!(response["concept"]["id"], "task_git_status");
        assert!(response["related_contract_ids"]
            .as_array()
            .expect("related contract ids")
            .iter()
            .any(|value| value == "server_state_bootstrap"));
        assert!(response["related_tests"]
            .as_array()
            .expect("related tests")
            .iter()
            .any(
                |value| value["pattern"] == "src/app/task-presentation.test.ts"
                    && value["exists"] == true
            ));
        assert!(response["findings"]
            .as_array()
            .expect("findings")
            .iter()
            .any(|value| value["kind"] == "multi_writer_concept"));
        assert!(response["obligations"]
            .as_array()
            .expect("obligations")
            .iter()
            .any(|value| value["domain_symbol_name"] == "TaskState"));
        assert_eq!(
            response["semantic"]["writes"].as_array().map(Vec::len),
            Some(2)
        );
        assert_eq!(
            response["semantic"]["reads"].as_array().map(Vec::len),
            Some(1)
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn explain_concept_applies_active_suppressions() {
        let root = concept_fixture_root();
        append_file(
            &root,
            ".sentrux/rules.toml",
            r#"

                [[suppress]]
                kind = "forbidden_writer"
                concept = "task_git_status"
                file = "src/store/**"
                reason = "temporary migration"
                expires = "2099-12-31"
            "#,
        );
        let semantic = concept_fixture_semantic(&root);
        let mut state = state_with_semantic(&root, semantic);

        let response =
            handle_explain_concept(&json!({"id": "task_git_status"}), &Tier::Free, &mut state)
                .expect("explain concept");
        let findings = response["findings"].as_array().expect("findings");

        assert!(!findings
            .iter()
            .any(|value| value["kind"] == "forbidden_writer"));
        assert!(response["suppression_hits"]
            .as_array()
            .expect("suppression hits")
            .iter()
            .any(|value| value["kind"] == "forbidden_writer"));
        assert_eq!(response["suppressed_finding_count"], 1);

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn state_tool_surfaces_expired_suppressions_without_hiding_findings() {
        let root = state_fixture_root();
        append_file(
            &root,
            ".sentrux/rules.toml",
            r#"

                [[suppress]]
                kind = "state_model_missing_assert_never"
                concept = "browser_state_sync"
                reason = "expired exception"
                expires = "2020-01-01"
            "#,
        );
        let semantic = state_fixture_semantic(&root);
        let mut state = state_with_semantic(&root, semantic);

        let response = handle_state(&json!({}), &Tier::Free, &mut state).expect("state tool");

        assert!(response["findings"]
            .as_array()
            .expect("state findings")
            .iter()
            .any(|value| value["kind"] == "state_model_missing_assert_never"));
        assert!(response["expired_suppressions"]
            .as_array()
            .expect("expired suppressions")
            .iter()
            .any(|value| value["kind"] == "state_model_missing_assert_never"));
        assert_eq!(response["suppressed_finding_count"], 0);

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn concepts_surface_guardrail_tests_and_inferred_concepts() {
        let root = temp_root("concepts-adoption");
        write_file(
            &root,
            "src/components/TaskRow.architecture.test.ts",
            "expect(store.taskGitStatus).toBeDefined();\n",
        );
        let semantic = concept_fixture_semantic(&root);
        let mut state = state_with_semantic(&root, semantic);

        let response = handle_concepts(&json!({}), &Tier::Free, &mut state).expect("concepts");

        assert_eq!(response["kind"], "concepts");
        assert!(response["rules_error"].is_null());
        assert!(response["semantic_error"].is_null());
        assert_eq!(response["summary"]["configured_concept_count"], 0);
        assert_eq!(response["summary"]["guardrail_test_count"], 1);
        assert_eq!(response["summary"]["matched_guardrail_test_count"], 0);
        assert!(response["inferred_concepts"]
            .as_array()
            .expect("inferred concepts")
            .iter()
            .any(|value| value["id"] == "task_state"));
        assert!(response["inferred_concepts"]
            .as_array()
            .expect("inferred concepts")
            .iter()
            .any(|value| value["id"] == "store_task_git_status"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn project_shape_tool_surfaces_archetypes_and_starter_rules() {
        let root = temp_root("project-shape-tool");
        write_file(
            &root,
            "package.json",
            r#"{
              "dependencies": {
                "next": "15.0.0",
                "react": "19.0.0"
              }
            }"#,
        );
        write_file(&root, "next.config.ts", "export default {};\n");
        write_file(
            &root,
            "src/app/[locale]/layout.tsx",
            "export default function Layout({ children }: { children: React.ReactNode }) { return children; }\n",
        );
        write_file(
            &root,
            "src/app/api/health/route.ts",
            "export async function GET() { return Response.json({ ok: true }); }\n",
        );
        write_file(&root, "src/modules/home/index.ts", "export * from './components';\n");
        write_file(
            &root,
            "src/modules/users/index.ts",
            "export * from './components';\n",
        );
        write_file(
            &root,
            "src/services/users.ts",
            "export async function listUsers() { return []; }\n",
        );
        write_file(
            &root,
            "src/store/session.store.ts",
            "export const sessionStore = {};\n",
        );

        let mut state = fresh_mcp_state();
        handle_scan(
            &json!({"path": root.to_string_lossy().to_string()}),
            &Tier::Free,
            &mut state,
        )
        .expect("scan fixture");

        let response = super::handle_project_shape(&json!({}), &Tier::Free, &mut state)
            .expect("project shape");

        assert_eq!(response["kind"], "project_shape");
        assert!(response["rules_error"].is_null());
        assert_eq!(
            response["shape"]["primary_archetype"],
            "modular_nextjs_frontend"
        );
        assert!(response["shape"]["effective_archetypes"]
            .as_array()
            .expect("effective archetypes")
            .iter()
            .any(|value| value == "modular_nextjs_frontend"));
        assert!(response["shape"]["boundary_roots"]
            .as_array()
            .expect("boundary roots")
            .iter()
            .any(|value| {
                value["kind"] == "feature_modules" && value["root"] == "src/modules"
            }));
        assert!(response["shape"]["module_contracts"]
            .as_array()
            .expect("module contracts")
            .iter()
            .any(|value| {
                value["id"] == "feature_modules" && value["root"] == "src/modules"
            }));
        assert!(response["shape"]["starter_rules_toml"]
            .as_str()
            .unwrap_or_default()
            .contains("[[module_contract]]"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn check_rules_truncates_module_contracts_in_free_tier() {
        let root = temp_root("module-contract-truncation");
        write_file(
            &root,
            ".sentrux/rules.toml",
            r#"
                [[module_contract]]
                id = "feature_modules_a"
                root = "src/modules"

                [[module_contract]]
                id = "feature_modules_b"
                root = "src/modules"

                [[module_contract]]
                id = "feature_modules_c"
                root = "src/modules"

                [[module_contract]]
                id = "feature_modules_d"
                root = "src/modules"
            "#,
        );
        write_file(&root, "src/app/page.tsx", "export default function Page() { return null; }\n");
        write_file(&root, "src/modules/a/index.ts", "export const a = 1;\n");
        write_file(&root, "src/modules/b/index.ts", "export const b = 2;\n");

        let mut state = fresh_mcp_state();
        handle_scan(
            &json!({"path": root.to_string_lossy().to_string()}),
            &Tier::Free,
            &mut state,
        )
        .expect("scan fixture");

        let response =
            super::handle_check_rules(&json!({}), &Tier::Free, &mut state).expect("check rules");

        assert_eq!(response["truncated"]["total_rules_defined"], 4);
        assert_eq!(response["truncated"]["rules_checked"], 3);

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn state_tool_returns_reports_and_findings() {
        let root = state_fixture_root();
        let semantic = state_fixture_semantic(&root);
        let mut state = state_with_semantic(&root, semantic);

        let response = handle_state(&json!({}), &Tier::Free, &mut state).expect("state tool");

        assert_eq!(response["kind"], "state");
        assert_eq!(response["state_model_count"], 1);
        assert_eq!(response["missing_variant_count"], 1);
        assert_eq!(response["transition_site_count"], 0);
        assert_eq!(response["transition_gap_count"], 0);
        assert!(response["findings"]
            .as_array()
            .expect("state findings")
            .iter()
            .any(|value| value["kind"] == "state_model_missing_assert_never"));
        assert!(response["findings"]
            .as_array()
            .expect("state findings")
            .iter()
            .any(|value| value["kind"] == "state_model_missing_variant_coverage"));
        assert!(response["findings"]
            .as_array()
            .expect("state findings")
            .iter()
            .any(|value| value["kind"] == "state_model_missing_transition_sites"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn state_tool_surfaces_transition_gaps() {
        let root = state_fixture_root();
        let mut semantic = state_fixture_semantic(&root);
        semantic.transition_sites = vec![
            TransitionSite {
                path: "src/runtime/browser-state-sync-controller.ts".to_string(),
                domain_symbol_name: "BrowserSyncState".to_string(),
                group_id: "src/runtime/browser-state-sync-controller.ts:6:BrowserSyncState"
                    .to_string(),
                transition_kind: "switch_case".to_string(),
                source_variant: Some("idle".to_string()),
                target_variants: vec!["running".to_string()],
                line: 7,
            },
            TransitionSite {
                path: "src/runtime/browser-state-sync-controller.ts".to_string(),
                domain_symbol_name: "BrowserSyncState".to_string(),
                group_id: "src/runtime/browser-state-sync-controller.ts:6:BrowserSyncState"
                    .to_string(),
                transition_kind: "switch_case".to_string(),
                source_variant: Some("running".to_string()),
                target_variants: vec!["error".to_string()],
                line: 10,
            },
            TransitionSite {
                path: "src/runtime/browser-state-sync-controller.ts".to_string(),
                domain_symbol_name: "BrowserSyncState".to_string(),
                group_id: "src/runtime/browser-state-sync-controller.ts:6:BrowserSyncState"
                    .to_string(),
                transition_kind: "switch_case".to_string(),
                source_variant: Some("error".to_string()),
                target_variants: Vec::new(),
                line: 13,
            },
        ];
        let mut state = state_with_semantic(&root, semantic);

        let response = handle_state(&json!({}), &Tier::Free, &mut state).expect("state tool");

        assert_eq!(response["transition_site_count"], 3);
        assert_eq!(response["transition_gap_count"], 1);
        assert!(response["findings"]
            .as_array()
            .expect("state findings")
            .iter()
            .any(|value| value["kind"] == "state_model_transition_coverage_gap"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn state_model_ids_are_derived_from_state_findings() {
        let ids = state_model_ids_from_findings(&[
            crate::metrics::v2::SemanticFinding {
                kind: "state_model_missing_assert_never".to_string(),
                severity: "medium".to_string(),
                concept_id: "browser_state_sync".to_string(),
                summary: "missing assertNever".to_string(),
                files: vec!["src/runtime/browser-state-sync-controller.ts".to_string()],
                evidence: vec!["BrowserSyncState".to_string()],
            },
            crate::metrics::v2::SemanticFinding {
                kind: "forbidden_writer".to_string(),
                severity: "high".to_string(),
                concept_id: "task_git_status".to_string(),
                summary: "forbidden writer".to_string(),
                files: vec!["src/app/git-status-sync.ts".to_string()],
                evidence: vec!["src/app/git-status-sync.ts::store.taskGitStatus".to_string()],
            },
        ]);

        assert_eq!(
            ids.into_iter().collect::<Vec<_>>(),
            vec!["browser_state_sync"]
        );
    }

    #[test]
    fn state_model_ids_are_derived_from_state_reports() {
        let ids = state_model_ids_from_reports(&[
            crate::metrics::v2::StateIntegrityReport {
                id: "browser_state_sync".to_string(),
                ..Default::default()
            },
            crate::metrics::v2::StateIntegrityReport {
                id: "server_state_bootstrap".to_string(),
                ..Default::default()
            },
        ]);

        assert_eq!(
            ids.into_iter().collect::<Vec<_>>(),
            vec![
                "browser_state_sync".to_string(),
                "server_state_bootstrap".to_string(),
            ]
        );
    }

    #[test]
    fn trace_symbol_uses_scoped_query_for_declaration_and_global_query_for_references() {
        let root = concept_fixture_root();
        let semantic = concept_fixture_semantic(&root);
        let mut state = state_with_semantic(&root, semantic);

        let response = handle_trace_symbol(
            &json!({"symbol": "src/store/core.ts::store.taskGitStatus"}),
            &Tier::Free,
            &mut state,
        )
        .expect("trace symbol");

        assert_eq!(response["symbol"], "src/store/core.ts::store.taskGitStatus");
        assert_eq!(response["declarations"].as_array().map(Vec::len), Some(1));
        assert_eq!(response["writes"].as_array().map(Vec::len), Some(2));
        assert_eq!(response["reads"].as_array().map(Vec::len), Some(1));
        assert!(response["related_concepts"]
            .as_array()
            .expect("related concepts")
            .iter()
            .any(|value| value == "task_git_status"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn trace_symbol_reports_reference_ambiguity_for_scoped_collisions() {
        let root = concept_fixture_root();
        let mut semantic = concept_fixture_semantic(&root);
        semantic.symbols.push(SymbolFact {
            id: "duplicate-task-git-status".to_string(),
            path: "src/legacy/store.ts".to_string(),
            name: "store.taskGitStatus".to_string(),
            kind: "property".to_string(),
            line: 1,
        });
        let mut state = state_with_semantic(&root, semantic);

        let response = handle_trace_symbol(
            &json!({"symbol": "src/store/core.ts::store.taskGitStatus"}),
            &Tier::Free,
            &mut state,
        )
        .expect("trace symbol");

        assert_eq!(response["reads"].as_array().map(Vec::len), Some(0));
        assert_eq!(response["writes"].as_array().map(Vec::len), Some(0));
        assert!(response["reference_ambiguity"]["conflicting_declarations"]
            .as_array()
            .expect("conflicts")
            .iter()
            .any(|value| value["path"] == "src/legacy/store.ts"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn trace_symbol_surfaces_rules_parse_errors_even_when_semantic_analysis_is_available() {
        let root = temp_root("trace-rules-error");
        write_file(
            &root,
            ".sentrux/rules.toml",
            "[[concept]\nid = \"broken\"\nanchors = [",
        );
        write_file(
            &root,
            "src/domain/task-state.ts",
            "export type TaskState = 'idle' | 'running';\n",
        );
        let semantic = SemanticSnapshot {
            project: ProjectModel {
                root: root.to_string_lossy().to_string(),
                tsconfig_paths: vec!["tsconfig.json".to_string()],
                workspace_files: Vec::new(),
                primary_language: Some("typescript".to_string()),
                fingerprint: "broken-rules".to_string(),
                repo_archetype: None,
                detected_archetypes: Vec::new(),
            },
            analyzed_files: 1,
            capabilities: vec![SemanticCapability::Symbols],
            files: Vec::new(),
            symbols: vec![SymbolFact {
                id: "task-state".to_string(),
                path: "src/domain/task-state.ts".to_string(),
                name: "TaskState".to_string(),
                kind: "type_alias".to_string(),
                line: 1,
            }],
            reads: Vec::new(),
            writes: Vec::new(),
            closed_domains: Vec::new(),
            closed_domain_sites: Vec::new(),
            transition_sites: Vec::new(),
        };
        let mut state = state_with_semantic(&root, semantic);

        let response = handle_trace_symbol(
            &json!({"symbol": "src/domain/task-state.ts::TaskState"}),
            &Tier::Free,
            &mut state,
        )
        .expect("trace symbol");

        assert!(response["rules_error"]
            .as_str()
            .unwrap_or_default()
            .contains("Failed to parse"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn trace_symbol_keeps_zero_config_findings_for_scoped_queries() {
        let root = temp_root("trace-zero-config");
        write_file(
            &root,
            "src/domain/task-state.ts",
            "export type TaskState = 'idle' | 'running' | 'error';\n",
        );
        let semantic = SemanticSnapshot {
            project: ProjectModel {
                root: root.to_string_lossy().to_string(),
                tsconfig_paths: vec!["tsconfig.json".to_string()],
                workspace_files: Vec::new(),
                primary_language: Some("typescript".to_string()),
                fingerprint: "zero-config".to_string(),
                repo_archetype: None,
                detected_archetypes: Vec::new(),
            },
            analyzed_files: 1,
            capabilities: vec![
                SemanticCapability::Symbols,
                SemanticCapability::ClosedDomains,
                SemanticCapability::ClosedDomainSites,
            ],
            files: Vec::new(),
            symbols: vec![SymbolFact {
                id: "task-state".to_string(),
                path: "src/domain/task-state.ts".to_string(),
                name: "TaskState".to_string(),
                kind: "type_alias".to_string(),
                line: 1,
            }],
            reads: Vec::new(),
            writes: Vec::new(),
            closed_domains: vec![ClosedDomain {
                path: "src/domain/task-state.ts".to_string(),
                symbol_name: "TaskState".to_string(),
                variants: vec![
                    "idle".to_string(),
                    "running".to_string(),
                    "error".to_string(),
                ],
                line: 1,
            }],
            closed_domain_sites: vec![ExhaustivenessSite {
                path: "src/domain/task-state.ts".to_string(),
                domain_symbol_name: "TaskState".to_string(),
                site_kind: "switch".to_string(),
                proof_kind: "switch".to_string(),
                covered_variants: vec!["idle".to_string(), "running".to_string()],
                line: 3,
            }],
            transition_sites: Vec::new(),
        };
        let mut state = state_with_semantic(&root, semantic);

        let response = handle_trace_symbol(
            &json!({"symbol": "src/domain/task-state.ts::TaskState"}),
            &Tier::Free,
            &mut state,
        )
        .expect("trace symbol");

        assert!(response["findings"]
            .as_array()
            .expect("findings")
            .iter()
            .any(|value| value["kind"] == "closed_domain_exhaustiveness"));
        assert!(response["obligations"]
            .as_array()
            .expect("obligations")
            .iter()
            .any(|value| value["domain_symbol_name"] == "TaskState"));

        let _ = std::fs::remove_dir_all(root);
    }
}

// Redundant tools removed: coupling, cycles, architecture, blast_radius, hottest, level.
// All diagnostics are grouped by root cause in the `health` tool's `diagnostics` field.
// See quality-signal-design.md — one true score, root-cause-organized diagnostics.

// ══════════════════════════════════════════════════════════════════
//  SESSION START
// ══════════════════════════════════════════════════════════════════

pub fn session_start_def() -> ToolDef {
    ToolDef {
        name: "session_start",
        description: "Save current health metrics as baseline for later comparison via 'gate' or 'session_end'.",
        input_schema: json!({ "type": "object", "properties": {} }),
        min_tier: Tier::Free,
        handler: handle_session_start,
        invalidates_evolution: false,
    }
}

fn handle_session_start(
    _args: &Value,
    _tier: &Tier,
    state: &mut McpState,
) -> Result<Value, String> {
    let health = state
        .cached_health
        .clone()
        .ok_or("No scan data. Call 'scan' first.")?;
    let snapshot = state
        .cached_snapshot
        .as_ref()
        .cloned()
        .ok_or("No scan data. Call 'scan' first.")?;
    let root = state
        .scan_root
        .clone()
        .ok_or("No scan root. Call 'scan' first.")?;
    let metadata = state
        .cached_scan_metadata
        .as_ref()
        .cloned()
        .ok_or("No scan data. Call 'scan' first.")?;
    let baseline = arch::ArchBaseline::from_health(&health);
    let signal = baseline.quality_signal;
    let baseline_path = save_baseline(&root, &baseline)?;
    let (session_v2, suppression_application, semantic_error) =
        build_session_v2_baseline(state, &root, &snapshot, &health, &metadata);
    let (rules_config, _) = load_v2_rules_config(state, &root);

    state.baseline = Some(baseline);
    let session_v2_baseline_path = save_session_v2_baseline(&root, &session_v2)?;
    state.session_v2 = Some(session_v2);
    state.cached_patch_safety = None;

    Ok(json!({
        "status": "Baseline saved",
        "quality_signal": (signal * 10000.0).round() as u32,
        "baseline_path": baseline_path,
        "session_v2_baseline_path": session_v2_baseline_path,
        "session_finding_count": state.session_v2.as_ref().map(|baseline| baseline.finding_payloads.len()).unwrap_or(0),
        "suppression_hits": suppression_application.active_matches,
        "suppressed_finding_count": suppression_match_count(&suppression_application.active_matches),
        "expired_suppressions": suppression_application.expired_matches,
        "expired_suppression_match_count": suppression_match_count(&suppression_application.expired_matches),
        "confidence": build_v2_confidence_report(
            &metadata,
            &rules_config,
            compatible_session_baseline_status(SESSION_V2_SCHEMA_VERSION),
        ),
        "semantic_error": semantic_error,
        "message": "Call 'session_end' after making changes to see the diff"
    }))
}

// ══════════════════════════════════════════════════════════════════
//  SESSION END
// ══════════════════════════════════════════════════════════════════

pub fn session_end_def() -> ToolDef {
    ToolDef {
        name: "session_end",
        description: "Re-scan and compare current state against session baseline. Returns diff showing what degraded.",
        input_schema: json!({ "type": "object", "properties": {} }),
        min_tier: Tier::Free,
        handler: handle_session_end,
        invalidates_evolution: true,
    }
}

fn handle_session_end(_args: &Value, _tier: &Tier, state: &mut McpState) -> Result<Value, String> {
    // Clone to avoid borrow conflict: we read root+baseline, then mutate state.
    let root = state
        .scan_root
        .clone()
        .ok_or("No scan root. Call 'scan' first.")?;
    let (session_v2, session_v2_status) = current_session_v2_baseline_with_status(state, &root)?;
    let (baseline, mut baseline_error) = match state.baseline.clone() {
        Some(baseline) => (Some(baseline), None),
        None => match load_persisted_baseline(&root) {
            Ok(baseline) => (baseline, None),
            Err(error) => (None, Some(error)),
        },
    };

    let context = prepare_patch_check_context(state, &root, session_v2.as_ref())?;
    let patch_cache_identity = current_patch_safety_cache_identity(state, &context);
    let bundle = context.bundle;
    let legacy_diff = baseline
        .as_ref()
        .map(|baseline| baseline.diff(&bundle.health));
    let changed_files = context.changed_files;
    if !context.reused_cached_scan {
        state.cached_semantic = None;
        state.cached_evolution = None;
    }
    let analysis = build_patch_safety_analysis(
        state,
        &root,
        &bundle,
        &changed_files,
        session_v2.as_ref(),
        patch_cache_identity,
    );
    let current_finding_payloads = finding_payload_map(&analysis.visible_findings);
    let (rules_config, _) = load_v2_rules_config(state, &root);
    let changed_concepts = analysis
        .changed_touched_concepts
        .iter()
        .cloned()
        .collect::<Vec<_>>();
    let missing_obligations = analysis
        .changed_obligations
        .iter()
        .filter(|obligation| !obligation.missing_sites.is_empty())
        .cloned()
        .collect::<Vec<_>>();
    let introduced_findings = session_v2
        .as_ref()
        .map(|session_v2| {
            current_finding_payloads
                .iter()
                .filter(|(key, _)| !session_v2.finding_payloads.contains_key(*key))
                .map(|(_, payload)| payload.clone())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let (visible_introduced_findings, experimental_findings) =
        partition_experimental_findings(&introduced_findings, 10);
    let mut blocking_findings = visible_introduced_findings
        .iter()
        .filter(|finding| severity_of_value(finding) == "high")
        .cloned()
        .collect::<Vec<_>>();
    if session_v2.is_none() {
        blocking_findings.extend(
            analysis
                .changed_visible_findings
                .iter()
                .filter(|finding| {
                    !is_experimental_finding(finding) && severity_of_value(finding) == "high"
                })
                .cloned(),
        );
    }
    let resolved_findings = session_v2
        .as_ref()
        .map(|session_v2| {
            session_v2
                .finding_payloads
                .iter()
                .filter(|(key, _)| !current_finding_payloads.contains_key(*key))
                .map(|(_, payload)| payload.clone())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let resolved_findings = resolved_findings
        .into_iter()
        .filter(|finding| !is_experimental_finding(finding))
        .map(|finding| decorate_finding_with_classification(&finding))
        .collect::<Vec<_>>();
    let gate_decision = if !missing_obligations.is_empty() || !blocking_findings.is_empty() {
        "fail"
    } else if legacy_diff.as_ref().is_some_and(|diff| diff.degraded)
        || !visible_introduced_findings.is_empty()
    {
        "warn"
    } else {
        "pass"
    };
    let semantic_error = patch_safety_semantic_error(&analysis);
    if baseline.is_none() && baseline_error.is_none() {
        baseline_error =
            Some("Legacy baseline unavailable; structural delta fields were omitted".to_string());
    }
    let introduced_findings = visible_introduced_findings
        .iter()
        .map(decorate_finding_with_classification)
        .collect::<Vec<_>>();
    let (opportunity_findings, experimental_findings) = if session_v2.is_some() {
        (visible_introduced_findings.clone(), experimental_findings)
    } else {
        partition_experimental_findings(&analysis.changed_visible_findings, 10)
    };
    let opportunity_findings = opportunity_findings
        .into_iter()
        .map(|finding| decorate_finding_with_classification(&finding))
        .collect::<Vec<_>>();
    let experimental_findings = experimental_findings
        .into_iter()
        .map(|finding| decorate_finding_with_classification(&finding))
        .collect::<Vec<_>>();
    let finding_details = build_finding_details(&opportunity_findings, 10);
    let debt_outputs = build_debt_report_outputs(
        state,
        &root,
        &bundle.snapshot,
        &bundle.health,
        &opportunity_findings,
        &analysis.changed_obligations,
        &[],
        &changed_files,
        5,
    );
    let concept_summary_count = debt_outputs.concept_summaries.len();
    let debt_signal_count = debt_outputs.debt_signals.len();
    let experimental_debt_signal_count = debt_outputs.experimental_debt_signals.len();
    let debt_cluster_count = debt_outputs.debt_clusters.len();
    let watchpoint_count = debt_outputs.watchpoints.len();
    let concept_summaries = debt_outputs.concept_summaries;
    let debt_signals = debt_outputs.debt_signals;
    let experimental_debt_signals = debt_outputs.experimental_debt_signals;
    let debt_clusters = debt_outputs.debt_clusters;
    let watchpoints = debt_outputs.watchpoints;
    let legacy_quality_opportunities = legacy_quality_opportunity_values(&debt_signals);
    let legacy_optimization_priorities = legacy_optimization_priority_values(&watchpoints);
    let debt_context_error = debt_outputs.context_error;
    let preserved_semantic = state.cached_semantic.clone();
    let preserved_evolution = state.cached_evolution.clone();
    let preserved_patch_safety = state.cached_patch_safety.clone();

    let signal_before = legacy_diff
        .as_ref()
        .map(|diff| (diff.signal_before * 10000.0).round() as i32);
    let signal_after = legacy_diff
        .as_ref()
        .map(|diff| (diff.signal_after * 10000.0).round() as i32);
    let signal_delta = legacy_diff
        .as_ref()
        .map(|diff| ((diff.signal_after - diff.signal_before) * 10000.0).round() as i32);
    let coupling_change = legacy_diff
        .as_ref()
        .map(|diff| vec![diff.coupling_before, diff.coupling_after]);
    let cycles_change = legacy_diff
        .as_ref()
        .map(|diff| vec![diff.cycles_before, diff.cycles_after]);
    let legacy_violations = legacy_diff
        .as_ref()
        .map(|diff| diff.violations.clone())
        .unwrap_or_default();

    let summary = if gate_decision == "fail" {
        "Touched-concept regressions detected"
    } else if legacy_diff.as_ref().is_some_and(|diff| diff.degraded) {
        "Quality degraded"
    } else if legacy_diff.is_none() {
        "Patch safety check complete; legacy structural delta unavailable"
    } else {
        "Quality stable or improved"
    };
    let mut result = serde_json::Map::new();
    result.insert("pass".to_string(), json!(gate_decision != "fail"));
    result.insert("signal_before".to_string(), json!(signal_before));
    result.insert("signal_after".to_string(), json!(signal_after));
    result.insert("signal_delta".to_string(), json!(signal_delta));
    result.insert("coupling_change".to_string(), json!(coupling_change));
    result.insert("cycles_change".to_string(), json!(cycles_change));
    result.insert("violations".to_string(), json!(legacy_violations));
    result.insert("summary".to_string(), json!(summary));
    result.insert(
        "changed_files".to_string(),
        json!(changed_files.iter().cloned().collect::<Vec<_>>()),
    );
    result.insert("changed_concepts".to_string(), json!(changed_concepts));
    result.insert(
        "introduced_findings".to_string(),
        json!(introduced_findings),
    );
    result.insert("resolved_findings".to_string(), json!(resolved_findings));
    result.insert(
        "finding_detail_count".to_string(),
        json!(finding_details.len()),
    );
    result.insert("finding_details".to_string(), json!(finding_details));
    result.insert(
        "experimental_finding_count".to_string(),
        json!(experimental_findings.len()),
    );
    result.insert(
        "experimental_findings".to_string(),
        json!(experimental_findings),
    );
    result.insert(
        "missing_obligations".to_string(),
        json!(missing_obligations),
    );
    result.insert(
        "concept_summary_count".to_string(),
        json!(concept_summary_count),
    );
    result.insert("concept_summaries".to_string(), json!(concept_summaries));
    result.insert("debt_signal_count".to_string(), json!(debt_signal_count));
    result.insert("debt_signals".to_string(), json!(debt_signals));
    result.insert(
        "experimental_debt_signal_count".to_string(),
        json!(experimental_debt_signal_count),
    );
    result.insert(
        "experimental_debt_signals".to_string(),
        json!(experimental_debt_signals),
    );
    result.insert("debt_cluster_count".to_string(), json!(debt_cluster_count));
    result.insert("debt_clusters".to_string(), json!(debt_clusters));
    result.insert("watchpoint_count".to_string(), json!(watchpoint_count));
    result.insert("watchpoints".to_string(), json!(watchpoints));
    result.insert(
        "quality_opportunity_count".to_string(),
        json!(debt_signal_count),
    );
    result.insert(
        "quality_opportunities".to_string(),
        json!(legacy_quality_opportunities),
    );
    result.insert(
        "optimization_priority_count".to_string(),
        json!(watchpoint_count),
    );
    result.insert(
        "optimization_priorities".to_string(),
        json!(legacy_optimization_priorities),
    );
    result.insert(
        "obligation_completeness_0_10000".to_string(),
        json!(crate::metrics::v2::obligation_score_0_10000(
            &analysis.changed_obligations
        )),
    );
    result.insert(
        "touched_concept_gate".to_string(),
        json!({
            "decision": gate_decision,
            "blocking_findings": blocking_findings,
        }),
    );
    result.insert(
        "suppression_hits".to_string(),
        json!(analysis.suppression_hits),
    );
    result.insert(
        "suppressed_finding_count".to_string(),
        json!(analysis.suppressed_finding_count),
    );
    result.insert(
        "expired_suppressions".to_string(),
        json!(analysis.expired_suppressions),
    );
    result.insert(
        "expired_suppression_match_count".to_string(),
        json!(analysis.expired_suppression_match_count),
    );
    result.insert("rules_error".to_string(), json!(analysis.rules_error));
    result.insert("scan_trust".to_string(), scan_trust_json(&bundle.metadata));
    result.insert(
        "confidence".to_string(),
        json!(build_v2_confidence_report(
            &bundle.metadata,
            &rules_config,
            session_v2_status
        )),
    );
    result.insert(
        "baseline_delta".to_string(),
        legacy_baseline_delta_json(legacy_diff.as_ref()),
    );
    result.insert("semantic_error".to_string(), json!(semantic_error));
    result.insert("debt_context_error".to_string(), json!(debt_context_error));
    result.insert(
        "opportunity_context_error".to_string(),
        json!(debt_context_error),
    );
    result.insert("baseline_error".to_string(), json!(baseline_error));
    let result = Value::Object(result);

    if !context.reused_cached_scan {
        update_scan_cache(state, root, bundle, baseline, context.scan_identity);
        state.cached_semantic = preserved_semantic;
        state.cached_evolution = preserved_evolution;
        state.cached_patch_safety = preserved_patch_safety;
    } else {
        state.baseline = baseline;
    }

    Ok(result)
}

pub fn gate_def() -> ToolDef {
    ToolDef {
        name: "gate",
        description: "Evaluate whether the current patch introduces high-confidence touched-concept regressions.",
        input_schema: json!({
            "type": "object",
            "properties": {
                "strict": {
                    "type": "boolean",
                    "description": "If true, medium-severity introduced findings also fail the gate."
                }
            }
        }),
        min_tier: Tier::Free,
        handler: handle_gate,
        invalidates_evolution: true,
    }
}

fn handle_gate(args: &Value, _tier: &Tier, state: &mut McpState) -> Result<Value, String> {
    let root = state
        .scan_root
        .clone()
        .ok_or("No scan root. Call 'scan' first.")?;
    let strict = args
        .get("strict")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    compute_touched_concept_gate(state, &root, strict)
}

// ══════════════════════════════════════════════════════════════════
//  RESCAN
// ══════════════════════════════════════════════════════════════════

pub fn rescan_def() -> ToolDef {
    ToolDef {
        name: "rescan",
        description: "Re-scan the current directory to pick up file changes since last scan.",
        input_schema: json!({ "type": "object", "properties": {} }),
        min_tier: Tier::Free,
        handler: handle_rescan,
        invalidates_evolution: true,
    }
}

fn handle_rescan(_args: &Value, _tier: &Tier, state: &mut McpState) -> Result<Value, String> {
    // Clone root to avoid borrow conflict
    let root = state
        .scan_root
        .clone()
        .ok_or("No scan root. Call 'scan' first.")?;
    let (bundle, scan_identity) = do_scan_with_identity(&root)?;
    let (baseline, baseline_error) = match load_persisted_baseline(&root) {
        Ok(baseline) => (baseline, None),
        Err(error) => (None, Some(error)),
    };

    let result = json!({
        "status": "Rescanned",
        "quality_signal": (bundle.health.quality_signal * 10000.0).round() as u32,
        "files": bundle.snapshot.total_files,
        "scan_trust": scan_trust_json(&bundle.metadata),
        "baseline_loaded": baseline.is_some(),
        "baseline_error": baseline_error
    });

    update_scan_cache(state, root, bundle, baseline, scan_identity);

    Ok(result)
}

// ══════════════════════════════════════════════════════════════════
//  CHECK RULES
// ══════════════════════════════════════════════════════════════════

pub fn check_rules_def() -> ToolDef {
    ToolDef {
        name: "check_rules",
        description: "Check .sentrux/rules.toml architectural constraints. Returns pass/fail with specific violations.",
        input_schema: json!({ "type": "object", "properties": {} }),
        min_tier: Tier::Free,
        handler: handle_check_rules,
        invalidates_evolution: false,
    }
}

pub fn concepts_def() -> ToolDef {
    ToolDef {
        name: "concepts",
        description: "List configured v2 concepts plus guardrail-test evidence, conservative concept suggestions, and rule coverage.",
        input_schema: json!({ "type": "object", "properties": {} }),
        min_tier: Tier::Free,
        handler: handle_concepts,
        invalidates_evolution: false,
    }
}

pub fn project_shape_def() -> ToolDef {
    ToolDef {
        name: "project_shape",
        description: "Detect repo archetypes, candidate boundary roots, module public-API contracts, and starter v2 rules for onboarding a new project shape.",
        input_schema: json!({ "type": "object", "properties": {} }),
        min_tier: Tier::Free,
        handler: handle_project_shape,
        invalidates_evolution: false,
    }
}

fn handle_project_shape(
    _args: &Value,
    _tier: &Tier,
    state: &mut McpState,
) -> Result<Value, String> {
    let root = state
        .scan_root
        .clone()
        .ok_or("No scan root. Call 'scan' first.")?;
    let snapshot = state
        .cached_snapshot
        .clone()
        .ok_or("No scan data. Call 'scan' first.")?;
    let (config, rules_error) = load_v2_rules_config(state, &root);

    Ok(json!({
        "kind": "project_shape",
        "project": config.project,
        "rules_error": rules_error,
        "shape": project_shape_json(&root, &snapshot, &config),
    }))
}

fn handle_concepts(_args: &Value, _tier: &Tier, state: &mut McpState) -> Result<Value, String> {
    let root = state
        .scan_root
        .clone()
        .ok_or("No scan root. Call 'scan' first.")?;
    let (config, rules_error) = load_v2_rules_config(state, &root);
    let graph = crate::analysis::concepts::extract_concept_graph(&config);
    let coverage = config.v2_rule_coverage();
    let guardrail_tests = crate::analysis::concepts::detect_guardrail_tests(&root, &config);
    let (inferred_concepts, semantic_error) = match analyze_semantic_snapshot(state, &root) {
        Ok(Some(semantic)) => (
            crate::analysis::concepts::infer_concepts(&config, &semantic),
            None,
        ),
        Ok(None) => (Vec::new(), None),
        Err(error) => (
            Vec::new(),
            merge_optional_errors(rules_error.clone(), Some(error)),
        ),
    };
    let matched_guardrail_tests = guardrail_tests
        .iter()
        .filter(|test| !test.matched_concepts.is_empty())
        .count();
    let guardrail_test_count = guardrail_tests.len();
    let inferred_concept_count = inferred_concepts.len();

    Ok(json!({
        "kind": "concepts",
        "project": config.project,
        "project_shape": optional_project_shape_json(&root, state.cached_snapshot.as_deref(), &config),
        "rule_coverage": coverage,
        "rules_error": rules_error,
        "semantic_error": semantic_error,
        "concepts": graph.concepts,
        "contracts": graph.contracts,
        "state_models": graph.state_models,
        "guardrail_tests": guardrail_tests,
        "inferred_concepts": inferred_concepts,
        "suppressions": config.suppress,
        "summary": {
            "configured_concept_count": graph.concepts.len(),
            "contract_count": graph.contracts.len(),
            "state_model_count": graph.state_models.len(),
            "guardrail_test_count": guardrail_test_count,
            "matched_guardrail_test_count": matched_guardrail_tests,
            "inferred_concept_count": inferred_concept_count,
        }
    }))
}

pub fn explain_concept_def() -> ToolDef {
    ToolDef {
        name: "explain_concept",
        description: "Show one configured concept with its rules, semantic reads/writes, findings, obligations, and related contracts.",
        input_schema: json!({
            "type": "object",
            "properties": {
                "id": {
                    "type": "string",
                    "description": "Concept id from .sentrux/rules.toml."
                }
            },
            "required": ["id"]
        }),
        min_tier: Tier::Free,
        handler: handle_explain_concept,
        invalidates_evolution: false,
    }
}

fn handle_explain_concept(
    args: &Value,
    _tier: &Tier,
    state: &mut McpState,
) -> Result<Value, String> {
    let root = state
        .scan_root
        .clone()
        .ok_or("No scan root. Call 'scan' first.")?;
    let concept_id = args
        .get("id")
        .and_then(|value| value.as_str())
        .ok_or("Missing 'id' argument")?;
    let config = load_rules_config(&root)?;
    let concept = config
        .concept
        .iter()
        .find(|concept| concept.id == concept_id)
        .cloned()
        .ok_or_else(|| format!("Unknown concept: {concept_id}"))?;
    let graph = crate::analysis::concepts::extract_concept_graph(&config);
    let semantic = analyze_semantic_snapshot(state, &root).ok().flatten();
    let cached_snapshot = state.cached_snapshot.clone();
    let (semantic_findings, obligations, semantic_error) = semantic_findings_and_obligations(
        state,
        &root,
        cached_snapshot.as_deref(),
        crate::metrics::v2::ObligationScope::All,
        &BTreeSet::new(),
    );
    let explain_findings = semantic_findings
        .into_iter()
        .filter(|finding| finding.concept_id == concept_id)
        .collect::<Vec<_>>();
    let (suppression_application, rules_error) =
        apply_root_suppressions(state, &root, serialized_values(&explain_findings));
    let explain_obligations = obligations
        .into_iter()
        .filter(|obligation| obligation.concept_id.as_deref() == Some(concept_id))
        .collect::<Vec<_>>();
    let related_contracts = config
        .contract
        .iter()
        .filter(|contract| contract_relates_to_concept(contract, &concept))
        .map(|contract| contract.id.clone())
        .collect::<BTreeSet<_>>();
    let parity = semantic.as_ref().map(|semantic| {
        let reports = crate::metrics::v2::build_parity_reports(
            &config,
            semantic,
            &root,
            crate::metrics::v2::ParityScope::All,
            &BTreeSet::new(),
        );
        reports
            .into_iter()
            .filter(|report| related_contracts.contains(&report.id))
            .collect::<Vec<_>>()
    });
    let semantic_summary = semantic.as_ref().map(|semantic| {
        let writes = crate::metrics::v2::relevant_writes(&concept, &semantic)
            .into_iter()
            .map(|write| {
                json!({
                    "path": write.path,
                    "symbol_name": write.symbol_name,
                    "write_kind": write.write_kind,
                    "line": write.line,
                })
            })
            .collect::<Vec<_>>();
        let reads = crate::metrics::v2::relevant_reads(&concept, &semantic)
            .into_iter()
            .map(|read| {
                json!({
                    "path": read.path,
                    "symbol_name": read.symbol_name,
                    "read_kind": read.read_kind,
                    "line": read.line,
                })
            })
            .collect::<Vec<_>>();
        json!({
            "writes": writes,
            "reads": reads,
        })
    });
    let related_tests = describe_concept_related_tests(&root, &concept);

    Ok(json!({
        "concept": graph.concepts.into_iter().find(|candidate| candidate.id == concept_id),
        "related_contract_ids": related_contracts.into_iter().collect::<Vec<_>>(),
        "related_tests": related_tests,
        "findings": suppression_application.visible_findings,
        "obligations": explain_obligations,
        "parity": parity,
        "semantic": semantic_summary,
        "rules_error": rules_error,
        "suppression_hits": suppression_application.active_matches,
        "suppressed_finding_count": suppression_match_count(&suppression_application.active_matches),
        "expired_suppressions": suppression_application.expired_matches,
        "expired_suppression_match_count": suppression_match_count(&suppression_application.expired_matches),
        "semantic_error": semantic_error,
    }))
}

pub fn trace_symbol_def() -> ToolDef {
    ToolDef {
        name: "trace_symbol",
        description: "Trace a symbol to declarations, reads, writes, configured concepts, related obligations, and related contracts.",
        input_schema: json!({
            "type": "object",
            "properties": {
                "symbol": {
                    "type": "string",
                    "description": "Symbol name or scoped query like path::Symbol."
                }
            },
            "required": ["symbol"]
        }),
        min_tier: Tier::Free,
        handler: handle_trace_symbol,
        invalidates_evolution: false,
    }
}

fn handle_trace_symbol(args: &Value, _tier: &Tier, state: &mut McpState) -> Result<Value, String> {
    let root = state
        .scan_root
        .clone()
        .ok_or("No scan root. Call 'scan' first.")?;
    let query = args
        .get("symbol")
        .and_then(|value| value.as_str())
        .ok_or("Missing 'symbol' argument")?;
    let (config, rules_error) = load_v2_rules_config(state, &root);
    let semantic = analyze_semantic_snapshot(state, &root)
        .map_err(|error| {
            merge_optional_errors(rules_error.clone(), Some(error))
                .unwrap_or_else(|| "Semantic analysis unavailable".to_string())
        })?
        .ok_or_else(|| {
            merge_optional_errors(
                rules_error.clone(),
                Some(
                    "Symbol tracing requires TypeScript semantic analysis for this project"
                        .to_string(),
                ),
            )
            .unwrap()
        })?;
    let (query_path, query_symbol) = split_symbol_query(query);

    let matched_declarations = semantic
        .symbols
        .iter()
        .filter(|symbol| symbol_query_matches(&symbol.path, &symbol.name, query))
        .collect::<Vec<_>>();
    let ambiguous_declarations = query_path
        .as_deref()
        .map(|scoped_path| {
            semantic
                .symbols
                .iter()
                .filter(|symbol| {
                    symbol.path != scoped_path
                        && symbol_query_matches("", &symbol.name, query_symbol)
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let references_are_ambiguous = query_path.is_some() && !ambiguous_declarations.is_empty();

    let declarations = matched_declarations
        .iter()
        .map(|symbol| {
            json!({
                "id": symbol.id,
                "path": symbol.path,
                "name": symbol.name,
                "kind": symbol.kind,
                "line": symbol.line,
            })
        })
        .collect::<Vec<_>>();
    let reads = semantic
        .reads
        .iter()
        .filter(|read| {
            !references_are_ambiguous && symbol_query_matches("", &read.symbol_name, query)
        })
        .map(|read| {
            json!({
                "path": read.path,
                "symbol_name": read.symbol_name,
                "read_kind": read.read_kind,
                "line": read.line,
            })
        })
        .collect::<Vec<_>>();
    let writes = semantic
        .writes
        .iter()
        .filter(|write| {
            !references_are_ambiguous && symbol_query_matches("", &write.symbol_name, query)
        })
        .map(|write| {
            json!({
                "path": write.path,
                "symbol_name": write.symbol_name,
                "write_kind": write.write_kind,
                "line": write.line,
            })
        })
        .collect::<Vec<_>>();

    let related_concepts = config
        .concept
        .iter()
        .filter(|concept| concept_matches_symbol(concept, query))
        .map(|concept| concept.id.clone())
        .collect::<BTreeSet<_>>();
    let related_contracts = config
        .contract
        .iter()
        .filter(|contract| contract_matches_symbol(contract, query))
        .map(|contract| contract.id.clone())
        .collect::<BTreeSet<_>>();
    let cached_snapshot = state.cached_snapshot.clone();
    let (semantic_findings, obligations, semantic_error) = semantic_findings_and_obligations(
        state,
        &root,
        cached_snapshot.as_deref(),
        crate::metrics::v2::ObligationScope::All,
        &BTreeSet::new(),
    );
    let semantic_error = semantic_error.filter(|error| Some(error) != rules_error.as_ref());
    let findings = semantic_findings
        .into_iter()
        .filter(|finding| {
            related_concepts.contains(&finding.concept_id)
                || symbol_query_matches("", &finding.concept_id, query)
        })
        .collect::<Vec<_>>();
    let (suppression_application, suppression_rules_error) =
        apply_root_suppressions(state, &root, serialized_values(&findings));
    let obligations = obligations
        .into_iter()
        .filter(|obligation| {
            obligation
                .domain_symbol_name
                .as_deref()
                .map(|symbol_name| symbol_query_matches("", symbol_name, query))
                .unwrap_or(false)
                || obligation
                    .concept_id
                    .as_deref()
                    .map(|concept_id| related_concepts.contains(concept_id))
                    .unwrap_or(false)
        })
        .collect::<Vec<_>>();
    let reference_ambiguity = if references_are_ambiguous {
        Some(json!({
            "message": format!(
                "Scoped query '{}' matches additional declarations in other files, so cross-file reads and writes are omitted to avoid false positives",
                query
            ),
            "conflicting_declarations": ambiguous_declarations
                .iter()
                .map(|symbol| {
                    json!({
                        "id": symbol.id,
                        "path": symbol.path,
                        "name": symbol.name,
                        "kind": symbol.kind,
                        "line": symbol.line,
                    })
                })
                .collect::<Vec<_>>(),
        }))
    } else {
        None
    };

    Ok(json!({
        "symbol": query,
        "declarations": declarations,
        "reads": reads,
        "writes": writes,
        "related_concepts": related_concepts.into_iter().collect::<Vec<_>>(),
        "related_contracts": related_contracts.into_iter().collect::<Vec<_>>(),
        "findings": suppression_application.visible_findings,
        "obligations": obligations,
        "rules_error": merge_optional_errors(rules_error, suppression_rules_error),
        "suppression_hits": suppression_application.active_matches,
        "suppressed_finding_count": suppression_match_count(&suppression_application.active_matches),
        "expired_suppressions": suppression_application.expired_matches,
        "expired_suppression_match_count": suppression_match_count(&suppression_application.expired_matches),
        "reference_ambiguity": reference_ambiguity,
        "semantic_error": semantic_error,
    }))
}

fn describe_concept_related_tests(
    root: &Path,
    concept: &crate::metrics::rules::ConceptRule,
) -> Vec<Value> {
    concept
        .related_tests
        .iter()
        .map(|pattern| {
            let matches = matching_project_paths(root, pattern);
            json!({
                "pattern": pattern,
                "matched_files": matches,
                "exists": !matches.is_empty(),
            })
        })
        .collect()
}

fn matching_project_paths(root: &Path, pattern: &str) -> Vec<String> {
    let has_glob = pattern.contains('*') || pattern.contains('?') || pattern.contains('[');
    if !has_glob {
        return if root.join(pattern).exists() {
            vec![pattern.to_string()]
        } else {
            Vec::new()
        };
    }

    let mut matches = ignore::WalkBuilder::new(root)
        .hidden(true)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .build()
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            if !path.is_file() {
                return None;
            }
            let relative_path = path.strip_prefix(root).ok()?;
            let relative_path = relative_path.to_string_lossy().replace('\\', "/");
            if crate::metrics::rules::glob_match(pattern, &relative_path) {
                Some(relative_path)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    matches.sort();
    matches
}

fn concept_rule_files(concept: &crate::metrics::rules::ConceptRule) -> BTreeSet<String> {
    let mut files = BTreeSet::new();
    for scoped_path in concept
        .anchors
        .iter()
        .chain(concept.authoritative_inputs.iter())
        .chain(concept.allowed_writers.iter())
        .chain(concept.forbid_writers.iter())
        .chain(concept.canonical_accessors.iter())
        .chain(concept.forbid_raw_reads.iter())
    {
        if let Some((path, _)) = scoped_path.split_once("::") {
            files.insert(path.to_string());
        }
    }
    files.extend(concept.related_tests.iter().cloned());
    files
}

fn contract_rule_files(contract: &crate::metrics::rules::ContractRule) -> BTreeSet<String> {
    let mut files = BTreeSet::new();
    for scoped_path in [
        contract.categories_symbol.as_deref(),
        contract.payload_map_symbol.as_deref(),
        contract.registry_symbol.as_deref(),
    ]
    .into_iter()
    .flatten()
    {
        if let Some((path, _)) = scoped_path.split_once("::") {
            files.insert(path.to_string());
        }
    }
    files.extend(contract.browser_entry.iter().cloned());
    files.extend(contract.electron_entry.iter().cloned());
    files
}

fn contract_relates_to_concept(
    contract: &crate::metrics::rules::ContractRule,
    concept: &crate::metrics::rules::ConceptRule,
) -> bool {
    let concept_files = concept_rule_files(concept);
    let contract_files = contract_rule_files(contract);
    if !concept_files.is_disjoint(&contract_files) {
        return true;
    }

    let concept_targets = crate::metrics::v2::concept_targets(concept);
    [
        contract.categories_symbol.as_deref(),
        contract.payload_map_symbol.as_deref(),
        contract.registry_symbol.as_deref(),
    ]
    .into_iter()
    .flatten()
    .filter_map(crate::metrics::v2::symbol_from_scoped_path)
    .any(|symbol_name| crate::metrics::v2::symbol_matches_targets(&symbol_name, &concept_targets))
}

fn concept_matches_symbol(concept: &crate::metrics::rules::ConceptRule, query: &str) -> bool {
    let (query_path, query_symbol) = split_symbol_query(query);
    concept
        .anchors
        .iter()
        .chain(concept.authoritative_inputs.iter())
        .chain(concept.allowed_writers.iter())
        .chain(concept.forbid_writers.iter())
        .chain(concept.canonical_accessors.iter())
        .chain(concept.forbid_raw_reads.iter())
        .any(|target| scoped_target_matches_query(target, query_path.as_deref(), query_symbol))
}

fn contract_matches_symbol(contract: &crate::metrics::rules::ContractRule, query: &str) -> bool {
    let (query_path, query_symbol) = split_symbol_query(query);
    [
        contract.categories_symbol.as_deref(),
        contract.payload_map_symbol.as_deref(),
        contract.registry_symbol.as_deref(),
    ]
    .into_iter()
    .flatten()
    .any(|target| scoped_target_matches_query(target, query_path.as_deref(), query_symbol))
}

fn split_symbol_query(query: &str) -> (Option<String>, &str) {
    match query.split_once("::") {
        Some((path, symbol_name)) => (Some(path.replace('\\', "/")), symbol_name),
        None => (None, query),
    }
}

fn symbol_query_matches(path: &str, symbol_name: &str, query: &str) -> bool {
    let (query_path, query_symbol) = split_symbol_query(query);
    if let Some(query_path) = query_path {
        if !path.is_empty() && path != query_path {
            return false;
        }
    }

    crate::metrics::v2::symbol_matches_targets(
        symbol_name,
        &HashSet::from([query_symbol.to_string()]),
    )
}

fn scoped_target_matches_query(target: &str, query_path: Option<&str>, query_symbol: &str) -> bool {
    let (path, symbol_name) = match target.split_once("::") {
        Some(parts) => parts,
        None => return false,
    };
    if let Some(query_path) = query_path {
        if path != query_path {
            return false;
        }
    }

    crate::metrics::v2::symbol_matches_targets(
        symbol_name,
        &HashSet::from([query_symbol.to_string()]),
    )
}

fn handle_check_rules(_args: &Value, tier: &Tier, state: &mut McpState) -> Result<Value, String> {
    let root = state
        .scan_root
        .as_ref()
        .ok_or("No scan root. Call 'scan' first.")?;
    let h = state
        .cached_health
        .as_ref()
        .ok_or("No scan data. Call 'scan' first.")?;
    let a = state
        .cached_arch
        .as_ref()
        .ok_or("No scan data. Call 'scan' first.")?;
    let snap = state
        .cached_snapshot
        .as_ref()
        .ok_or("No scan data. Call 'scan' first.")?;

    let mut config = load_rules_config(root)?;

    // Free tier: max 3 rules (constraints count as 1 if any thresholds set,
    // plus layers, boundaries, and module contracts each count as 1 rule).
    let total_rules = config.constraints.count_active()
        + config.layers.len()
        + config.boundaries.len()
        + config.module_contract.len();
    let truncated = if !tier.is_pro() && total_rules > 3 {
        // Keep constraints (1 rule) + first 2 of layers/boundaries/module contracts.
        let mut remaining = 3usize.saturating_sub(if config.constraints.count_active() > 0 {
            1
        } else {
            0
        });
        config.layers.truncate(remaining.min(config.layers.len()));
        remaining = remaining.saturating_sub(config.layers.len());
        config
            .boundaries
            .truncate(remaining.min(config.boundaries.len()));
        remaining = remaining.saturating_sub(config.boundaries.len());
        config
            .module_contract
            .truncate(remaining.min(config.module_contract.len()));
        true
    } else {
        false
    };

    let result = crate::metrics::rules::check_rules(&config, h, a, &snap.import_graph);
    let v2_rule_coverage = config.v2_rule_coverage();

    let mut response = json!({
        "pass": result.passed,
        "rules_checked": result.rules_checked,
        "violation_count": result.violations.len(),
        "v2_rule_coverage": v2_rule_coverage,
        "violations": result.violations.iter().map(|v| json!({
            "rule": v.rule,
            "severity": format!("{:?}", v.severity),
            "message": v.message,
            "files": v.files
        })).collect::<Vec<_>>(),
        "summary": if result.passed { "✓ All architectural rules pass" }
            else { "✗ Architectural rule violations detected" }
    });
    if truncated {
        response["truncated"] = json!({
            "total_rules_defined": total_rules,
            "rules_checked": result.rules_checked,
            "message": "Checking up to 3 rules. More available with sentrux Pro: https://github.com/sentrux/sentrux"
        });
    }
    Ok(response)
}
