use super::{
    is_internal_sentrux_path, semantic_rules_loaded, session_v2_schema_supported, McpState,
    ScanCacheIdentity, SessionV2Baseline,
};
use crate::analysis::scanner::common::ScanMetadata;
use crate::core::snapshot::Snapshot;
use crate::metrics::arch;
use crate::metrics::rules::RulesConfig;
use std::collections::{BTreeMap, BTreeSet};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct SessionBaselineStatus {
    pub loaded: bool,
    pub compatible: bool,
    pub schema_version: Option<u32>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct V2ConfidenceReport {
    pub scan_confidence_0_10000: u32,
    pub rule_coverage_0_10000: u32,
    pub semantic_rules_loaded: bool,
    pub session_baseline: SessionBaselineStatus,
}

fn missing_session_baseline_status() -> SessionBaselineStatus {
    SessionBaselineStatus {
        loaded: false,
        compatible: false,
        schema_version: None,
        error: None,
    }
}

pub(crate) fn compatible_session_baseline_status(schema_version: u32) -> SessionBaselineStatus {
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

pub(crate) fn ratio_score_0_10000(numerator: usize, denominator: usize) -> u32 {
    if denominator == 0 {
        return 10000;
    }
    ((numerator as f64 / denominator as f64) * 10000.0).round() as u32
}

pub(crate) fn overall_confidence_0_10000(
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

pub(crate) fn scan_confidence_0_10000(metadata: &ScanMetadata) -> u32 {
    let scope_coverage = ratio_score_0_10000(metadata.kept_files, metadata.candidate_files);
    let resolution_confidence = ratio_score_0_10000(
        metadata.resolution.resolved,
        metadata.resolution.resolved + metadata.resolution.unresolved_internal,
    );
    overall_confidence_0_10000(metadata, scope_coverage, resolution_confidence)
}

pub(crate) fn build_v2_confidence_report(
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

pub(crate) fn load_persisted_baseline(root: &Path) -> Result<Option<arch::ArchBaseline>, String> {
    let baseline_path = arch::baseline_path(root);
    if !baseline_path.exists() {
        return Ok(None);
    }
    arch::ArchBaseline::load(&baseline_path).map(Some)
}

fn session_v2_baseline_path(root: &Path) -> PathBuf {
    root.join(".sentrux").join("session-v2.json")
}

pub(crate) fn load_session_v2_baseline_status(
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
                    super::super::MIN_SUPPORTED_SESSION_V2_SCHEMA_VERSION,
                    super::SESSION_V2_SCHEMA_VERSION
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
#[allow(dead_code)]
pub(crate) fn load_persisted_session_v2(root: &Path) -> Result<Option<SessionV2Baseline>, String> {
    Ok(load_session_v2_baseline_status(root).0)
}

fn ensure_parent_directory(target_path: &Path, description: &str) -> Result<(), String> {
    let Some(parent) = target_path.parent() else {
        return Ok(());
    };

    std::fs::create_dir_all(parent).map_err(|error| {
        format!(
            "Failed to create {description} directory {}: {error}",
            parent.display()
        )
    })
}

pub(crate) fn save_baseline(root: &Path, baseline: &arch::ArchBaseline) -> Result<PathBuf, String> {
    let baseline_path = arch::baseline_path(root);
    ensure_parent_directory(&baseline_path, "baseline")?;
    baseline.save(&baseline_path)?;
    Ok(baseline_path)
}

pub(crate) fn save_session_v2_baseline(
    root: &Path,
    baseline: &SessionV2Baseline,
) -> Result<PathBuf, String> {
    let baseline_path = session_v2_baseline_path(root);
    ensure_parent_directory(&baseline_path, "session baseline")?;
    let payload = serde_json::to_vec_pretty(baseline)
        .map_err(|error| format!("Failed to serialize session baseline: {error}"))?;
    std::fs::write(&baseline_path, payload)
        .map_err(|error| format!("Failed to write {}: {error}", baseline_path.display()))?;
    Ok(baseline_path)
}

pub(crate) fn current_session_v2_baseline(
    state: &mut McpState,
    root: &Path,
) -> Result<Option<SessionV2Baseline>, String> {
    current_session_v2_baseline_with_status(state, root).map(|(baseline, _)| baseline)
}

pub(crate) fn current_session_v2_baseline_with_status(
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

pub(crate) fn current_scan_identity(root: &Path) -> Option<ScanCacheIdentity> {
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

pub(crate) fn working_tree_changed_files(root: &Path) -> Option<BTreeSet<String>> {
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

pub(crate) fn current_git_head(root: &Path) -> Option<String> {
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

pub(crate) fn project_fingerprint(root: &Path) -> String {
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
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    fingerprint_source.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

pub(crate) fn diff_paths_between_heads(
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

pub(crate) fn parse_name_status_paths(line: &str) -> Vec<String> {
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

pub(crate) fn snapshot_file_hashes(root: &Path, snapshot: &Snapshot) -> BTreeMap<String, u64> {
    snapshot_file_hashes_for_paths(root, snapshot, &scanned_file_paths(snapshot))
}

pub(crate) fn snapshot_file_hashes_for_paths(
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

pub(crate) fn file_hashes_for_paths(
    root: &Path,
    paths: &BTreeSet<String>,
) -> BTreeMap<String, u64> {
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

pub(crate) fn diff_file_hashes(
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

pub(crate) fn diff_file_hashes_for_paths(
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

pub(crate) fn scanned_file_paths(snapshot: &Snapshot) -> BTreeSet<String> {
    crate::core::snapshot::flatten_files_ref(snapshot.root.as_ref())
        .into_iter()
        .map(|file| file.path.clone())
        .collect()
}

pub(crate) fn filter_changed_files_to_snapshot(
    changed_files: BTreeSet<String>,
    snapshot: &Snapshot,
) -> BTreeSet<String> {
    let scanned_paths = scanned_file_paths(snapshot);
    changed_files
        .into_iter()
        .filter(|path| scanned_paths.contains(path))
        .collect()
}

pub(crate) fn changed_files_from_session_context(
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

pub(crate) fn changed_session_candidate_paths(
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

pub(crate) fn filter_changed_files_to_session_scope(
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

pub(crate) fn stable_hash(bytes: &[u8]) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    bytes.hash(&mut hasher);
    hasher.finish()
}
