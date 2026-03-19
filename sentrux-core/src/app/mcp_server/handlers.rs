//! MCP tool handler implementations — core tools.
//!
//! Each handler has the uniform signature: `fn(&Value, &Tier, &mut McpState) -> Result<Value, String>`
//! Each tool also has a `_def()` function returning its `ToolDef` (schema + tier + handler co-located).
//!
//! Tier-aware truncation: detail lists are limited to `tier.detail_limit()` items.
//! Free users see top-3 + total counts. Pro users see everything.

use super::registry::ToolDef;
use super::{McpState, SessionV2Baseline};
use crate::analysis::scanner;
use crate::analysis::scanner::common::ScanMetadata;
use crate::analysis::semantic::SemanticSnapshot;
use crate::core::snapshot::Snapshot;
use crate::license::Tier;
use crate::metrics;
use crate::metrics::arch;
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

fn load_persisted_session_v2(root: &Path) -> Result<Option<SessionV2Baseline>, String> {
    let baseline_path = session_v2_baseline_path(root);
    if !baseline_path.exists() {
        return Ok(None);
    }
    let bytes = std::fs::read(&baseline_path)
        .map_err(|error| format!("Failed to read {}: {error}", baseline_path.display()))?;
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|error| format!("Failed to parse {}: {error}", baseline_path.display()))
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
        suppress: Vec::new(),
    }
}

fn load_v2_rules_config(root: &Path) -> (crate::metrics::rules::RulesConfig, Option<String>) {
    let rules_path = root.join(".sentrux").join("rules.toml");
    if !rules_path.exists() {
        return (empty_rules_config(), None);
    }

    match crate::metrics::rules::RulesConfig::load(&rules_path) {
        Ok(config) => (config, None),
        Err(error) => (empty_rules_config(), Some(error)),
    }
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
    if let Some(session_v2) = &state.session_v2 {
        return Ok(Some(session_v2.clone()));
    }

    let session_v2 = load_persisted_session_v2(root)?;
    if let Some(session_v2) = &session_v2 {
        state.session_v2 = Some(session_v2.clone());
    }
    Ok(session_v2)
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
) {
    let root_changed = state
        .scan_root
        .as_ref()
        .map(|existing_root| existing_root != &root)
        .unwrap_or(false);
    if root_changed {
        state.session_v2 = None;
    }
    state.baseline = baseline;
    state.scan_root = Some(root);
    state.cached_snapshot = Some(Arc::new(bundle.snapshot));
    state.cached_scan_metadata = Some(bundle.metadata);
    state.cached_semantic = None;
    state.cached_health = Some(bundle.health);
    state.cached_arch = Some(bundle.arch_report);
    state.cached_evolution = None;
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
}

fn prepare_patch_check_context(
    state: &McpState,
    root: &Path,
    session_v2: Option<&SessionV2Baseline>,
) -> Result<PatchCheckContext, String> {
    if let Some(bundle) = cached_scan_bundle(state, root) {
        let changed_files = changed_files_from_session_context(root, &bundle.snapshot, session_v2);
        if changed_files.is_empty() {
            return Ok(PatchCheckContext {
                bundle,
                changed_files,
                reused_cached_scan: true,
            });
        }
    }

    let bundle = do_scan(root)?;
    let changed_files = changed_files_from_session_context(root, &bundle.snapshot, session_v2);

    Ok(PatchCheckContext {
        bundle,
        changed_files,
        reused_cached_scan: false,
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
    let head = git_output(root, &["rev-parse", "HEAD"])?;
    let head = head.trim();
    if head.is_empty() {
        return None;
    }
    Some(head.to_string())
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
    paths
        .iter()
        .filter(|path| scanned_paths.contains(*path))
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
) -> BTreeSet<String> {
    match session_v2 {
        Some(session_v2) => {
            if let Some(candidate_paths) =
                changed_session_candidate_paths(root, snapshot, session_v2)
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
        None => working_tree_changed_files(root)
            .map(|changed_files| filter_changed_files_to_snapshot(changed_files, snapshot))
            .unwrap_or_default(),
    }
}

fn changed_session_candidate_paths(
    root: &Path,
    snapshot: &Snapshot,
    session_v2: &SessionV2Baseline,
) -> Option<BTreeSet<String>> {
    let current_working_tree_paths = working_tree_changed_files(root)?;
    let mut candidate_paths = session_v2
        .working_tree_paths
        .union(&current_working_tree_paths)
        .cloned()
        .collect::<BTreeSet<_>>();

    let current_head = current_git_head(root);
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

fn build_clone_drift_finding_values(
    groups: &[crate::metrics::DuplicateGroup],
    evolution: Option<&crate::metrics::evo::EvolutionReport>,
    limit: usize,
) -> Vec<Value> {
    serialized_values(&crate::metrics::v2::build_clone_drift_findings(
        groups, evolution, limit,
    ))
}

fn clone_findings_for_health(
    state: &mut McpState,
    root: &Path,
    snapshot: &Snapshot,
    health: &metrics::HealthReport,
    limit: usize,
) -> (Vec<Value>, Option<String>) {
    let (evolution, evolution_error) = evolution_report_for_snapshot(state, root, snapshot);
    (
        build_clone_drift_finding_values(&health.duplicate_groups, evolution.as_ref(), limit),
        evolution_error,
    )
}

fn build_session_v2_baseline(
    state: &mut McpState,
    root: &Path,
    snapshot: &Snapshot,
    health: &metrics::HealthReport,
) -> (SessionV2Baseline, SuppressionApplication, Option<String>) {
    let file_hashes = snapshot_file_hashes(root, snapshot);
    let git_head = current_git_head(root);
    let working_tree_paths = working_tree_changed_files(root).unwrap_or_default();
    let (clone_findings, clone_error) =
        clone_findings_for_health(state, root, snapshot, health, health.duplicate_groups.len());
    let (semantic_findings, _, semantic_error) = semantic_findings_and_obligations(
        state,
        root,
        crate::metrics::v2::ObligationScope::All,
        &BTreeSet::new(),
    );
    let (config, _) = load_v2_rules_config(root);
    let suppression_application =
        apply_suppressions(&config, finding_values(&clone_findings, &semantic_findings));
    let finding_payloads = finding_payload_map(&suppression_application.visible_findings);

    (
        SessionV2Baseline {
            file_hashes,
            finding_payloads,
            git_head,
            working_tree_paths,
        },
        suppression_application,
        merge_optional_errors(semantic_error, clone_error),
    )
}

fn semantic_findings_and_obligations(
    state: &mut McpState,
    root: &Path,
    scope: crate::metrics::v2::ObligationScope,
    changed_files: &BTreeSet<String>,
) -> (
    Vec<crate::metrics::v2::SemanticFinding>,
    Vec<crate::metrics::v2::ObligationReport>,
    Option<String>,
) {
    let (config, config_error) = load_v2_rules_config(root);
    match analyze_semantic_snapshot(state, root) {
        Ok(Some(semantic)) => {
            let mut findings =
                crate::metrics::v2::build_authority_and_access_findings(&config, &semantic);
            let obligations =
                crate::metrics::v2::build_obligations(&config, &semantic, scope, changed_files);
            findings.extend(crate::metrics::v2::build_obligation_findings(&obligations));
            let state_scope = if scope == crate::metrics::v2::ObligationScope::Changed {
                crate::metrics::v2::StateScope::Changed
            } else {
                crate::metrics::v2::StateScope::All
            };
            let state_reports = crate::metrics::v2::build_state_integrity_reports(
                &config,
                &semantic,
                &obligations,
                state_scope,
                changed_files,
            );
            findings.extend(crate::metrics::v2::build_state_integrity_findings(
                &state_reports,
            ));
            (findings, obligations, config_error)
        }
        Ok(None) => (Vec::new(), Vec::new(), config_error),
        Err(error) => (
            Vec::new(),
            Vec::new(),
            merge_optional_errors(config_error, Some(error)),
        ),
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

fn finding_values(
    clone_findings: &[Value],
    semantic_findings: &[crate::metrics::v2::SemanticFinding],
) -> Vec<Value> {
    let mut findings = clone_findings.to_vec();
    findings.extend(serialized_values(semantic_findings));
    findings
}

fn apply_root_suppressions(
    root: &Path,
    findings: Vec<Value>,
) -> (SuppressionApplication, Option<String>) {
    let (config, rules_error) = load_v2_rules_config(root);
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

fn finding_files(finding: &Value) -> Vec<String> {
    if let Some(files) = finding
        .get("files")
        .and_then(|value| value.as_array())
        .map(|files| {
            files
                .iter()
                .filter_map(|value| value.as_str().map(str::to_string))
                .collect::<Vec<_>>()
        })
    {
        if !files.is_empty() {
            return files;
        }
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
    changed_files: &BTreeSet<String>,
) -> ChangedPatchScope {
    if changed_files.is_empty() {
        return ChangedPatchScope::default();
    }

    state.cached_semantic = None;
    let (changed_semantic_findings, obligations, semantic_error) =
        semantic_findings_and_obligations(
            state,
            root,
            crate::metrics::v2::ObligationScope::Changed,
            changed_files,
        );
    let changed_state_reports =
        changed_state_integrity_reports(state, root, config, &obligations, changed_files);
    let mut touched_concepts =
        crate::metrics::v2::changed_concept_ids_from_files(config, changed_files)
            .into_iter()
            .collect::<BTreeSet<_>>();
    touched_concepts.extend(crate::metrics::v2::changed_state_model_ids_from_files(
        config,
        changed_files,
    ));
    touched_concepts.extend(crate::metrics::v2::changed_concepts_from_obligations(
        &obligations,
    ));
    touched_concepts.extend(state_model_ids_from_reports(&changed_state_reports));
    touched_concepts.extend(state_model_ids_from_findings(&changed_semantic_findings));
    let changed_findings = serialized_values(&changed_semantic_findings);
    let suppression_application = apply_suppressions(config, changed_findings);

    ChangedPatchScope {
        obligations,
        semantic_error,
        suppression_application,
        touched_concepts,
    }
}

fn changed_state_integrity_reports(
    state: &mut McpState,
    root: &Path,
    config: &crate::metrics::rules::RulesConfig,
    changed_obligations: &[crate::metrics::v2::ObligationReport],
    changed_files: &BTreeSet<String>,
) -> Vec<crate::metrics::v2::StateIntegrityReport> {
    match analyze_semantic_snapshot(state, root) {
        Ok(Some(semantic)) => crate::metrics::v2::build_state_integrity_reports(
            config,
            &semantic,
            changed_obligations,
            crate::metrics::v2::StateScope::Changed,
            changed_files,
        ),
        Ok(None) | Err(_) => Vec::new(),
    }
}

fn compute_touched_concept_gate(
    state: &mut McpState,
    root: &Path,
    strict: bool,
) -> Result<Value, String> {
    let session_v2 = current_session_v2_baseline(state, root)?;
    let context = prepare_patch_check_context(state, root, session_v2.as_ref())?;
    let bundle = context.bundle;
    let changed_files = context.changed_files;

    if !context.reused_cached_scan {
        state.cached_semantic = None;
        state.cached_evolution = None;
    }
    let (current_clone_findings, clone_error) = clone_findings_for_health(
        state,
        root,
        &bundle.snapshot,
        &bundle.health,
        bundle.health.duplicate_groups.len(),
    );
    let (all_semantic_findings, _, all_semantic_error) = semantic_findings_and_obligations(
        state,
        root,
        crate::metrics::v2::ObligationScope::All,
        &BTreeSet::new(),
    );
    let (suppression_application, rules_error) = apply_root_suppressions(
        root,
        finding_values(&current_clone_findings, &all_semantic_findings),
    );
    let current_finding_payloads = finding_payload_map(&suppression_application.visible_findings);
    let (rules_config, _) = load_v2_rules_config(root);
    let changed_scope = analyze_changed_patch_scope(state, root, &rules_config, &changed_files);

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
            changed_scope
                .suppression_application
                .visible_findings
                .iter()
                .filter(|finding| {
                    let concept_id = finding_concept_id(finding).unwrap_or_default();
                    changed_scope.touched_concepts.is_empty()
                        || changed_scope.touched_concepts.contains(concept_id)
                })
                .cloned()
                .collect::<Vec<_>>()
        });
    let missing_obligations = changed_scope
        .obligations
        .iter()
        .filter(|obligation| !obligation.missing_sites.is_empty())
        .cloned()
        .collect::<Vec<_>>();
    let blocking_findings = introduced_findings
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
    let semantic_error = merge_optional_errors(
        changed_scope.semantic_error.or(all_semantic_error),
        clone_error,
    );
    let summary = if decision == "fail" {
        "Touched-concept regressions detected"
    } else if changed_files.is_empty() {
        "No working-tree changes detected"
    } else {
        "No blocking touched-concept regressions detected"
    };
    let persisted_baseline = load_persisted_baseline(root).ok().flatten();
    let preserved_semantic = state.cached_semantic.clone();
    let preserved_evolution = state.cached_evolution.clone();

    let response = json!({
        "decision": decision,
        "strict": strict,
        "summary": summary,
        "changed_files": changed_files.iter().cloned().collect::<Vec<_>>(),
        "introduced_findings": introduced_findings,
        "blocking_findings": blocking_findings,
        "missing_obligations": missing_obligations,
        "obligation_completeness_0_10000": crate::metrics::v2::obligation_score_0_10000(&changed_scope.obligations),
        "suppression_hits": suppression_application.active_matches,
        "suppressed_finding_count": suppression_match_count(&suppression_application.active_matches),
        "expired_suppressions": suppression_application.expired_matches,
        "expired_suppression_match_count": suppression_match_count(&suppression_application.expired_matches),
        "rules_error": rules_error,
        "semantic_error": semantic_error,
        "scan_trust": scan_trust_json(&bundle.metadata),
    });

    if !context.reused_cached_scan {
        update_scan_cache(
            state,
            root.to_path_buf(),
            bundle,
            persisted_baseline.or(state.baseline.clone()),
        );
        state.cached_semantic = preserved_semantic;
        state.cached_evolution = preserved_evolution;
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
    let (session_v2, suppression_application, semantic_error) =
        build_session_v2_baseline(&mut state, root, &bundle.snapshot, &bundle.health);
    let session_v2_baseline_path = save_session_v2_baseline(root, &session_v2)?;
    let session_finding_count = session_v2.finding_payloads.len();

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

    let bundle = do_scan(&root)?;
    let baseline_path = arch::baseline_path(&root);
    let (baseline, baseline_error) = match load_persisted_baseline(&root) {
        Ok(baseline) => (baseline, None),
        Err(error) => (None, Some(error)),
    };

    let result = json!({
        "scanned": path,
        "quality_signal": (bundle.health.quality_signal * 10000.0).round() as u32,
        "files": bundle.snapshot.total_files,
        "lines": bundle.snapshot.total_lines,
        "import_edges": bundle.snapshot.import_graph.len(),
        "scan_trust": scan_trust_json(&bundle.metadata),
        "baseline_loaded": baseline.is_some(),
        "baseline_path": baseline_path,
        "baseline_error": baseline_error,
    });

    update_scan_cache(state, root, bundle, baseline);

    Ok(result)
}

// ══════════════════════════════════════════════════════════════════
//  HEALTH (tier-aware truncation)
// ══════════════════════════════════════════════════════════════════

pub fn health_def() -> ToolDef {
    ToolDef {
        name: "health",
        description: "Get quality signal with root-cause breakdown and scan trust metadata. Use the bottleneck and trust data before relying on the composite score.",
        input_schema: json!({ "type": "object", "properties": {} }),
        min_tier: Tier::Free,
        handler: handle_health,
        invalidates_evolution: false,
    }
}

fn handle_health(_args: &Value, tier: &Tier, state: &mut McpState) -> Result<Value, String> {
    let h = state
        .cached_health
        .as_ref()
        .ok_or("No scan data. Call 'scan' first.")?;
    let metadata = state
        .cached_scan_metadata
        .as_ref()
        .ok_or("No scan data. Call 'scan' first.")?;
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
        "scan_trust": scan_trust_json(metadata)
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
        description: "Return actionable findings for the current scan. Includes exact clone groups plus v2 authority/access findings when explicit concept rules are available.",
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
    let limit = args
        .get("limit")
        .and_then(|value| value.as_u64())
        .unwrap_or(10)
        .min(50) as usize;
    let (clone_findings, clone_error) = clone_findings_for_health(
        state,
        &root,
        &snapshot,
        &health,
        health.duplicate_groups.len(),
    );
    let clone_group_count = health
        .duplicate_groups
        .iter()
        .filter(|group| distinct_file_count(group) > 1)
        .count();
    let (semantic_findings, _, semantic_error) = semantic_findings_and_obligations(
        state,
        &root,
        crate::metrics::v2::ObligationScope::All,
        &BTreeSet::new(),
    );
    let merged_findings = merge_findings(clone_findings, semantic_findings, usize::MAX);
    let (suppression_application, rules_error) = apply_root_suppressions(&root, merged_findings);
    let findings = suppression_application
        .visible_findings
        .into_iter()
        .take(limit)
        .collect::<Vec<_>>();

    Ok(json!({
        "kind": "mixed_findings",
        "clone_group_count": clone_group_count,
        "semantic_finding_count": findings.iter().filter(|finding| finding.get("concept_id").is_some()).count(),
        "rules_error": rules_error,
        "semantic_error": merge_optional_errors(semantic_error, clone_error),
        "suppression_hits": suppression_application.active_matches,
        "suppressed_finding_count": suppression_match_count(&suppression_application.active_matches),
        "expired_suppressions": suppression_application.expired_matches,
        "expired_suppression_match_count": suppression_match_count(&suppression_application.expired_matches),
        "findings": findings
    }))
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

    let (_, obligations, semantic_error) =
        semantic_findings_and_obligations(state, &root, scope, &changed_files);
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

    let (config, rules_error) = load_v2_rules_config(&root);
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
        apply_root_suppressions(&root, serialized_values(&findings));
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

    let (config, rules_error) = load_v2_rules_config(&root);
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
        apply_root_suppressions(&root, serialized_values(&findings));
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

    let (config, rules_error) = load_v2_rules_config(&root);
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
        apply_root_suppressions(&root, serialized_values(&findings));
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

    Ok(json!({
        "kind": "state",
        "scope": if scope == crate::metrics::v2::StateScope::Changed { "changed" } else { "all" },
        "changed_files": changed_files.iter().cloned().collect::<Vec<_>>(),
        "state_model_count": reports.len(),
        "finding_count": findings.len(),
        "missing_variant_count": missing_variant_count,
        "missing_site_count": missing_site_count,
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
    semantic_findings: Vec<crate::metrics::v2::SemanticFinding>,
    limit: usize,
) -> Vec<Value> {
    let mut merged: Vec<(u8, Value)> = semantic_findings
        .into_iter()
        .map(|finding| {
            let priority = severity_priority(&finding.severity);
            (
                priority,
                serde_json::to_value(finding).unwrap_or_else(|_| json!({})),
            )
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

#[cfg(test)]
fn build_exact_clone_findings(
    groups: &[crate::metrics::DuplicateGroup],
    limit: usize,
) -> Vec<Value> {
    build_clone_drift_finding_values(groups, None, limit)
}

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
        apply_suppressions, build_exact_clone_findings, build_session_v2_baseline,
        changed_files_from_session_context, cli_evaluate_v2_gate, cli_save_v2_session,
        distinct_file_count, do_scan, fresh_mcp_state, handle_concepts, handle_explain_concept,
        handle_gate, handle_obligations, handle_scan, handle_session_end, handle_session_start,
        handle_state, handle_trace_symbol, load_persisted_session_v2, load_v2_rules_config,
        overall_confidence_0_10000, prepare_patch_check_context, save_session_v2_baseline,
        state_model_ids_from_findings, state_model_ids_from_reports, update_scan_cache,
    };
    use crate::analysis::scanner::common::{ScanMetadata, ScanMode};
    use crate::analysis::semantic::{
        ClosedDomain, ExhaustivenessSite, ProjectModel, ReadFact, SemanticCapability,
        SemanticSnapshot, SymbolFact, WriteFact,
    };
    use crate::app::mcp_server::{McpState, SessionV2Baseline};
    use crate::license::Tier;
    use crate::metrics::rules::RulesConfig;
    use crate::metrics::DuplicateGroup;
    use serde_json::json;
    use std::collections::{BTreeMap, BTreeSet};
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

    fn concept_fixture_semantic(root: &Path) -> SemanticSnapshot {
        SemanticSnapshot {
            project: ProjectModel {
                root: root.to_string_lossy().to_string(),
                tsconfig_paths: vec!["tsconfig.json".to_string()],
                workspace_files: vec!["package.json".to_string()],
                primary_language: Some("typescript".to_string()),
                fingerprint: "fixture".to_string(),
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
            },
            analyzed_files: 2,
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

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn session_v2_baseline_deserializes_without_git_metadata() {
        let baseline: SessionV2Baseline = serde_json::from_value(json!({
            "file_hashes": { "src/a.ts": 11 },
            "finding_payloads": {}
        }))
        .expect("deserialize legacy session baseline");

        assert_eq!(baseline.file_hashes["src/a.ts"], 11);
        assert!(baseline.git_head.is_none());
        assert!(baseline.working_tree_paths.is_empty());
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
        );

        run_git(
            &root,
            &["mv", "src/domain/state.ts", "src/domain/app-state.ts"],
        );
        commit_all(&root, "rename state");

        let current_scan = do_scan(&root).expect("scan renamed tree");
        let changed_files =
            changed_files_from_session_context(&root, &current_scan.snapshot, Some(&session_v2));

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
        let (session_v2, _, _) =
            build_session_v2_baseline(&mut state, &root, &dirty_scan.snapshot, &dirty_scan.health);

        write_file(
            &root,
            "src/domain/state.ts",
            "export type AppState = 'idle' | 'busy';\n",
        );

        let reverted_scan = do_scan(&root).expect("scan reverted tree");
        let changed_files =
            changed_files_from_session_context(&root, &reverted_scan.snapshot, Some(&session_v2));

        assert!(changed_files.contains("src/domain/state.ts"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn patch_check_context_reuses_cached_scan_when_nothing_changed() {
        let root = cli_gate_fixture_root();
        let bundle = do_scan(&root).expect("scan fixture");
        let mut state = fresh_mcp_state();
        update_scan_cache(&mut state, root.clone(), bundle, None);

        let context =
            prepare_patch_check_context(&state, &root, None).expect("prepare patch context");

        assert!(context.reused_cached_scan);
        assert!(context.changed_files.is_empty());

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
    fn malformed_v2_rules_are_reported() {
        let root = temp_root("rules-error");
        std::fs::write(
            root.join(".sentrux").join("rules.toml"),
            "[[concept]\nid = \"broken\"\nanchors = [",
        )
        .expect("write broken rules");

        let (config, error) = load_v2_rules_config(&root);

        assert!(config.concept.is_empty());
        assert!(error
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
    fn state_tool_returns_reports_and_findings() {
        let root = state_fixture_root();
        let semantic = state_fixture_semantic(&root);
        let mut state = state_with_semantic(&root, semantic);

        let response = handle_state(&json!({}), &Tier::Free, &mut state).expect("state tool");

        assert_eq!(response["kind"], "state");
        assert_eq!(response["state_model_count"], 1);
        assert_eq!(response["missing_variant_count"], 1);
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
    let baseline = arch::ArchBaseline::from_health(&health);
    let signal = baseline.quality_signal;
    let baseline_path = save_baseline(&root, &baseline)?;
    let (session_v2, suppression_application, semantic_error) =
        build_session_v2_baseline(state, &root, &snapshot, &health);

    state.baseline = Some(baseline);
    let session_v2_baseline_path = save_session_v2_baseline(&root, &session_v2)?;
    state.session_v2 = Some(session_v2);

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
    let session_v2 = current_session_v2_baseline(state, &root)?;
    let (baseline, mut baseline_error) = match state.baseline.clone() {
        Some(baseline) => (Some(baseline), None),
        None => match load_persisted_baseline(&root) {
            Ok(baseline) => (baseline, None),
            Err(error) => (None, Some(error)),
        },
    };

    let context = prepare_patch_check_context(state, &root, session_v2.as_ref())?;
    let bundle = context.bundle;
    let legacy_diff = baseline
        .as_ref()
        .map(|baseline| baseline.diff(&bundle.health));
    let changed_files = context.changed_files;
    if !context.reused_cached_scan {
        state.cached_semantic = None;
        state.cached_evolution = None;
    }
    let (current_clone_findings, clone_error) = clone_findings_for_health(
        state,
        &root,
        &bundle.snapshot,
        &bundle.health,
        bundle.health.duplicate_groups.len(),
    );
    let (all_semantic_findings, _, all_semantic_error) = semantic_findings_and_obligations(
        state,
        &root,
        crate::metrics::v2::ObligationScope::All,
        &BTreeSet::new(),
    );
    let (suppression_application, rules_error) = apply_root_suppressions(
        &root,
        finding_values(&current_clone_findings, &all_semantic_findings),
    );
    let current_finding_payloads = finding_payload_map(&suppression_application.visible_findings);
    let (rules_config, _) = load_v2_rules_config(&root);
    let changed_scope = analyze_changed_patch_scope(state, &root, &rules_config, &changed_files);
    let changed_concepts = changed_scope
        .touched_concepts
        .iter()
        .cloned()
        .collect::<Vec<_>>();
    let missing_obligations = changed_scope
        .obligations
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
    let mut blocking_findings = introduced_findings
        .iter()
        .filter(|finding| severity_of_value(finding) == "high")
        .cloned()
        .collect::<Vec<_>>();
    if session_v2.is_none() {
        blocking_findings.extend(
            changed_scope
                .suppression_application
                .visible_findings
                .iter()
                .filter(|finding| severity_of_value(finding) == "high")
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
    let gate_decision = if !missing_obligations.is_empty() || !blocking_findings.is_empty() {
        "fail"
    } else if legacy_diff.as_ref().is_some_and(|diff| diff.degraded)
        || !introduced_findings.is_empty()
    {
        "warn"
    } else {
        "pass"
    };
    let semantic_error = merge_optional_errors(
        changed_scope.semantic_error.or(all_semantic_error),
        clone_error,
    );
    if baseline.is_none() && baseline_error.is_none() {
        baseline_error =
            Some("Legacy baseline unavailable; structural delta fields were omitted".to_string());
    }
    let preserved_semantic = state.cached_semantic.clone();
    let preserved_evolution = state.cached_evolution.clone();

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

    let result = json!({
        "pass": gate_decision != "fail",
        "signal_before": signal_before,
        "signal_after": signal_after,
        "signal_delta": signal_delta,
        "coupling_change": coupling_change,
        "cycles_change": cycles_change,
        "violations": legacy_violations,
        "summary": if gate_decision == "fail" {
            "Touched-concept regressions detected"
        } else if legacy_diff.as_ref().is_some_and(|diff| diff.degraded) {
            "Quality degraded"
        } else if legacy_diff.is_none() {
            "Patch safety check complete; legacy structural delta unavailable"
        } else {
            "Quality stable or improved"
        },
        "changed_files": changed_files.iter().cloned().collect::<Vec<_>>(),
        "changed_concepts": changed_concepts,
        "introduced_findings": introduced_findings,
        "resolved_findings": resolved_findings,
        "missing_obligations": missing_obligations,
        "obligation_completeness_0_10000": crate::metrics::v2::obligation_score_0_10000(&changed_scope.obligations),
        "touched_concept_gate": {
            "decision": gate_decision,
            "blocking_findings": blocking_findings,
        },
        "suppression_hits": suppression_application.active_matches,
        "suppressed_finding_count": suppression_match_count(&suppression_application.active_matches),
        "expired_suppressions": suppression_application.expired_matches,
        "expired_suppression_match_count": suppression_match_count(&suppression_application.expired_matches),
        "rules_error": rules_error,
        "scan_trust": scan_trust_json(&bundle.metadata),
        "semantic_error": semantic_error,
        "baseline_error": baseline_error
    });

    if !context.reused_cached_scan {
        update_scan_cache(state, root, bundle, baseline);
        state.cached_semantic = preserved_semantic;
        state.cached_evolution = preserved_evolution;
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
    let bundle = do_scan(&root)?;
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

    update_scan_cache(state, root, bundle, baseline);

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

fn handle_concepts(_args: &Value, _tier: &Tier, state: &mut McpState) -> Result<Value, String> {
    let root = state
        .scan_root
        .clone()
        .ok_or("No scan root. Call 'scan' first.")?;
    let (config, rules_error) = load_v2_rules_config(&root);
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
    let (semantic_findings, obligations, semantic_error) = semantic_findings_and_obligations(
        state,
        &root,
        crate::metrics::v2::ObligationScope::All,
        &BTreeSet::new(),
    );
    let explain_findings = semantic_findings
        .into_iter()
        .filter(|finding| finding.concept_id == concept_id)
        .collect::<Vec<_>>();
    let (suppression_application, rules_error) =
        apply_root_suppressions(&root, serialized_values(&explain_findings));
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
    let (config, rules_error) = load_v2_rules_config(&root);
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
    let (semantic_findings, obligations, semantic_error) = semantic_findings_and_obligations(
        state,
        &root,
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
        apply_root_suppressions(&root, serialized_values(&findings));
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
    // plus layers and boundaries each count as 1 rule).
    let total_rules =
        config.constraints.count_active() + config.layers.len() + config.boundaries.len();
    let truncated = if !tier.is_pro() && total_rules > 3 {
        // Keep constraints (1 rule) + first 2 of layers/boundaries
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
