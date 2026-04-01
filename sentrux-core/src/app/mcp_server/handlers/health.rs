use super::*;

fn root_cause_diagnostics_json(health: &metrics::HealthReport) -> Value {
    json!({
        "modularity": {
            "god_files": health.god_files.iter().map(|file| json!({"path": file.path, "fan_out": file.value})).collect::<Vec<_>>(),
            "hotspot_files": health.hotspot_files.iter().map(|file| json!({"path": file.path, "fan_in": file.value})).collect::<Vec<_>>(),
            "most_unstable": health.most_unstable.iter().take(10).map(|module| {
                json!({
                    "path": module.path,
                    "instability": module.instability,
                    "fan_in": module.fan_in,
                    "fan_out": module.fan_out,
                })
            }).collect::<Vec<_>>(),
        },
        "acyclicity": {
            "cycles": health.circular_dep_files.iter().collect::<Vec<_>>(),
        },
        "depth": {
            "max_depth": health.max_depth,
        },
        "equality": {
            "complex_functions": health.complex_functions.iter().take(20).map(|function| json!({"file": function.file, "func": function.func, "cc": function.value})).collect::<Vec<_>>(),
            "cog_complex_functions": health.cog_complex_functions.iter().take(20).map(|function| json!({"file": function.file, "func": function.func, "cog": function.value})).collect::<Vec<_>>(),
            "long_functions": health.long_functions.iter().take(20).map(|function| json!({"file": function.file, "func": function.func, "lines": function.value})).collect::<Vec<_>>(),
            "large_files": health.long_files.iter().take(10).map(|file| json!({"path": file.path, "lines": file.value})).collect::<Vec<_>>(),
            "high_param_functions": health.high_param_functions.iter().take(20).map(|function| json!({"file": function.file, "func": function.func, "params": function.value})).collect::<Vec<_>>(),
        },
        "redundancy": {
            "dead_functions": health.dead_functions.iter().take(50).map(|function| json!({"file": function.file, "func": function.func, "lines": function.value})).collect::<Vec<_>>(),
            "duplicate_groups": health.duplicate_groups.iter().take(20).map(|group| {
                json!({
                    "instances": group.instances.iter().map(|(file, function, lines)| {
                        json!({"file": file, "func": function, "lines": lines})
                    }).collect::<Vec<_>>()
                })
            }).collect::<Vec<_>>(),
        },
    })
}

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

pub(crate) fn handle_health(
    _args: &Value,
    tier: &Tier,
    state: &mut McpState,
) -> Result<Value, String> {
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

        if tier.is_pro() {
            let root_cause_diagnostics = root_cause_diagnostics_json(&h);
            object.insert(
                "root_cause_diagnostics".to_string(),
                root_cause_diagnostics.clone(),
            );

            if let Some(root_cause_object) = root_cause_diagnostics.as_object() {
                extend_diagnostics(object, root_cause_object.clone());
            }
        } else {
            object.insert(
                "upgrade".to_string(),
                json!({
                    "message": "Upgrade to Pro for root-cause diagnostics: https://github.com/sentrux/sentrux"
                }),
            );
        }
    }

    Ok(result)
}
