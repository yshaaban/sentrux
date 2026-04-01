use super::*;

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

pub(crate) fn handle_scan(
    args: &Value,
    _tier: &Tier,
    state: &mut McpState,
) -> Result<Value, String> {
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

    let mut result = json!({
        "kind": "scan",
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
    });
    if let Some(object) = result.as_object_mut() {
        insert_error_diagnostics(
            object,
            vec![
                DiagnosticEntry::new("baseline", baseline_error.clone()),
                DiagnosticEntry::new("rules", config_error.clone()),
            ],
            Vec::new(),
        );
    }

    update_scan_cache(state, root, bundle, baseline, scan_identity);

    Ok(result)
}

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

pub(crate) fn handle_rescan(
    _args: &Value,
    _tier: &Tier,
    state: &mut McpState,
) -> Result<Value, String> {
    let root = state
        .scan_root
        .clone()
        .ok_or("No scan root. Call 'scan' first.")?;
    let (bundle, scan_identity) = do_scan_with_identity(&root)?;
    let (baseline, baseline_error) = match load_persisted_baseline(&root) {
        Ok(baseline) => (baseline, None),
        Err(error) => (None, Some(error)),
    };
    let (rules_config, config_error) = load_v2_rules_config(state, &root);
    let (_, session_v2_status) = load_session_v2_baseline_status(&root);
    let confidence = build_v2_confidence_report(&bundle.metadata, &rules_config, session_v2_status);

    let mut result = json!({
        "kind": "rescan",
        "status": "Rescanned",
        "scanned": root,
        "quality_signal": (bundle.health.quality_signal * 10000.0).round() as u32,
        "files": bundle.snapshot.total_files,
        "scan_trust": scan_trust_json(&bundle.metadata),
        "confidence": confidence,
        "project_shape": project_shape_json(&root, &bundle.snapshot, &rules_config),
        "baseline_loaded": baseline.is_some(),
    });
    if let Some(object) = result.as_object_mut() {
        insert_error_diagnostics(
            object,
            vec![
                DiagnosticEntry::new("baseline", baseline_error.clone()),
                DiagnosticEntry::new("rules", config_error.clone()),
            ],
            Vec::new(),
        );
    }

    update_scan_cache(state, root, bundle, baseline, scan_identity);

    Ok(result)
}
