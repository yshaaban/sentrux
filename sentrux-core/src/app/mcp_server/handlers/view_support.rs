use super::*;
use crate::analysis::project_shape::{
    detect_project_shape, render_starter_rules, ProjectShapeReport,
};

pub(crate) fn scan_trust_json(metadata: &ScanMetadata) -> Value {
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

fn project_shape_report(
    root: &Path,
    snapshot: &Snapshot,
    config: &RulesConfig,
) -> ProjectShapeReport {
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

pub(crate) fn project_shape_report_cached(
    state: &mut McpState,
    root: &Path,
    snapshot: &Snapshot,
    config: &RulesConfig,
) -> crate::analysis::project_shape::ProjectShapeReport {
    if state.scan_root.as_deref() == Some(root)
        && state.cached_project_shape_identity == state.cached_scan_identity
    {
        if let Some(shape) = &state.cached_project_shape {
            return shape.clone();
        }
    }

    let shape = project_shape_report(root, snapshot, config);
    if state.scan_root.as_deref() == Some(root) {
        state.cached_project_shape = Some(shape.clone());
        state.cached_project_shape_identity = state.cached_scan_identity.clone();
    }
    shape
}

fn project_shape_to_json(
    shape: crate::analysis::project_shape::ProjectShapeReport,
    config: &RulesConfig,
) -> Value {
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

pub(crate) fn project_shape_json_cached(
    state: &mut McpState,
    root: &Path,
    snapshot: &Snapshot,
    config: &RulesConfig,
) -> Value {
    project_shape_to_json(
        project_shape_report_cached(state, root, snapshot, config),
        config,
    )
}

pub(crate) fn optional_project_shape_json(
    state: &mut McpState,
    root: &Path,
    snapshot: Option<&Snapshot>,
    config: &RulesConfig,
) -> Value {
    let Some(snapshot) = snapshot else {
        return json!({
            "available": false,
            "error": "No scan data. Call 'scan' first.",
        });
    };
    project_shape_json_cached(state, root, snapshot, config)
}

pub(crate) fn legacy_baseline_delta_json(diff: Option<&arch::ArchDiff>) -> Value {
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
