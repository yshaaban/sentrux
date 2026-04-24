use super::test_support::{
    append_file, cli_gate_fixture_root, commit_all, concept_fixture_root, concept_fixture_semantic,
    experimental_gate_fixture_root, init_git_repo, temp_root,
    ts_bridge_transport_gate_fixture_root, write_file, write_session_clone_duplicate,
    write_session_clone_fixture_files, write_session_clone_followthrough_fixture_files,
    write_session_clone_followthrough_source_drift,
};
use super::{
    cli_evaluate_v2_gate, cli_save_v2_session, fresh_mcp_state, handle_scan, handle_session_end,
    prepare_patch_check_context,
};
use crate::app::mcp_server::SESSION_V2_SCHEMA_VERSION;
use crate::license::Tier;
use serde_json::json;
use std::fs;

fn prepare_ts_bridge_transport_gate_root() -> std::path::PathBuf {
    let root = ts_bridge_transport_gate_fixture_root();
    init_git_repo(&root);
    commit_all(&root, "initial");
    cli_save_v2_session(&root).expect("save v2 session");
    root
}

#[test]
fn patch_check_context_reuses_cached_scan_when_nothing_changed() {
    let root = cli_gate_fixture_root();
    let mut state = fresh_mcp_state();
    handle_scan(
        &json!({"path": root.to_string_lossy().to_string()}),
        &Tier::Free,
        &mut state,
    )
    .expect("scan fixture");

    let context = prepare_patch_check_context(&state, &root, None).expect("patch context");

    assert!(context.reused_cached_scan);
    assert!(context.changed_files.is_empty());
}

#[test]
fn gate_skips_patch_safety_analysis_when_known_scope_is_clean() {
    let root = cli_gate_fixture_root();
    cli_save_v2_session(&root).expect("save v2 session");

    let mut state = fresh_mcp_state();
    handle_scan(
        &json!({"path": root.to_string_lossy().to_string()}),
        &Tier::Free,
        &mut state,
    )
    .expect("scan fixture");

    let response = super::session::handle_gate(&json!({"strict": true}), &Tier::Free, &mut state)
        .expect("gate");

    assert_eq!(response["decision"], json!("pass"));
    assert_eq!(
        response["summary"],
        json!("No working-tree changes detected")
    );
    assert!(response["introduced_findings"]
        .as_array()
        .is_some_and(|items| items.is_empty()));
    assert!(state.cached_patch_safety.is_none());
}

#[test]
fn session_end_works_with_v2_session_when_legacy_baseline_is_missing() {
    let root = cli_gate_fixture_root();
    cli_save_v2_session(&root).expect("save v2 session");
    let legacy_baseline = root.join(".sentrux").join("baseline.json");
    if legacy_baseline.exists() {
        std::fs::remove_file(&legacy_baseline).expect("remove legacy baseline");
    }

    let mut state = fresh_mcp_state();
    handle_scan(
        &json!({"path": root.to_string_lossy().to_string()}),
        &Tier::Free,
        &mut state,
    )
    .expect("scan fixture");

    let response = handle_session_end(&json!({}), &Tier::Free, &mut state).expect("session end");

    assert!(response.get("summary").is_some());
    assert!(response["diagnostics"]["errors"].is_object());
    assert!(response["actions"].is_array());
    assert!(response.get("baseline_error").is_none());
}

#[test]
fn session_end_skips_patch_safety_analysis_when_known_scope_is_clean() {
    let root = cli_gate_fixture_root();
    cli_save_v2_session(&root).expect("save v2 session");

    let mut state = fresh_mcp_state();
    handle_scan(
        &json!({"path": root.to_string_lossy().to_string()}),
        &Tier::Free,
        &mut state,
    )
    .expect("scan fixture");

    let response = handle_session_end(&json!({}), &Tier::Free, &mut state).expect("session end");

    assert_eq!(response["pass"], json!(true));
    assert!(response["introduced_findings"]
        .as_array()
        .is_some_and(|items| items.is_empty()));
    assert!(response["actions"]
        .as_array()
        .is_some_and(|items| items.is_empty()));
    assert!(state.cached_patch_safety.is_none());
}

#[test]
fn cli_v2_gate_ignores_invalid_legacy_baseline_when_v2_session_exists() {
    let root = cli_gate_fixture_root();
    cli_save_v2_session(&root).expect("save v2 session");
    write_file(&root, ".sentrux/baseline.json", "{ invalid json");

    let response = cli_evaluate_v2_gate(&root, false).expect("evaluate gate");

    assert!(response.get("decision").is_some());
    assert!(response["diagnostics"]["errors"].is_object());
    assert!(response.get("baseline_error").is_none());
}

#[test]
fn gate_ignores_protocol_only_ts_bridge_transport_refactors() {
    let root = prepare_ts_bridge_transport_gate_root();
    append_file(
        &root,
        "ts-bridge/src/protocol.ts",
        "\nexport function protocolHelper(): boolean { return true; }\n",
    );

    let response = cli_evaluate_v2_gate(&root, true).expect("evaluate gate");

    assert_eq!(response["decision"], json!("pass"));
    assert_eq!(response["missing_obligations"], json!([]));
    assert!(response["introduced_findings"]
        .as_array()
        .is_some_and(|findings| findings
            .iter()
            .all(|finding| finding["kind"] != "contract_surface_completeness")));
}

#[test]
fn gate_requires_transport_contract_followthrough_when_transport_surface_changes() {
    let root = prepare_ts_bridge_transport_gate_root();
    append_file(
        &root,
        "ts-bridge/src/transport.ts",
        "\nexport function transportHelper(): number { return 1; }\n",
    );

    let response = cli_evaluate_v2_gate(&root, true).expect("evaluate gate");
    let missing_obligations = response["missing_obligations"]
        .as_array()
        .expect("missing obligations");

    assert_eq!(response["decision"], json!("fail"));
    assert!(!missing_obligations.is_empty());
    assert!(missing_obligations.iter().any(|obligation| {
        obligation["kind"] == "contract_surface_completeness"
            && obligation["summary"]
                .as_str()
                .is_some_and(|summary| summary.contains("ts_bridge_semantic_transport"))
    }));
    assert!(missing_obligations.iter().any(|obligation| {
        obligation["repair_packet"]["required_fields"]["repair_surface"] == json!(true)
            && obligation["fix_hint"]
                .as_str()
                .is_some_and(|hint| hint.contains("evidence"))
    }));
}

#[test]
fn session_end_keeps_legacy_delta_when_v2_session_schema_is_unsupported() {
    let root = cli_gate_fixture_root();
    cli_save_v2_session(&root).expect("save v2 session");
    let unsupported_version = SESSION_V2_SCHEMA_VERSION + 1;
    write_file(
        &root,
        ".sentrux/session-v2.json",
        &format!(
            "{}\n",
            serde_json::to_string_pretty(&json!({
                "schema_version": unsupported_version,
                "file_hashes": {
                    "src/app.ts": 11
                },
                "finding_payloads": {},
                "git_head": null
            }))
            .expect("serialize incompatible v2 baseline")
        ),
    );

    let mut state = fresh_mcp_state();
    handle_scan(
        &json!({"path": root.to_string_lossy().to_string()}),
        &Tier::Free,
        &mut state,
    )
    .expect("scan fixture");

    let response = handle_session_end(&json!({}), &Tier::Free, &mut state).expect("session end");

    assert_eq!(
        response["confidence"]["session_baseline"]["loaded"],
        json!(true)
    );
    assert_eq!(
        response["confidence"]["session_baseline"]["compatible"],
        json!(false)
    );
    assert_eq!(
        response["confidence"]["session_baseline"]["schema_version"],
        json!(unsupported_version)
    );
    assert!(response["confidence"]["session_baseline"]["error"]
        .as_str()
        .is_some_and(|error| error.contains("Unsupported v2 session baseline schema version")));
    assert_eq!(response["baseline_delta"]["available"], json!(true));
}

#[test]
fn session_end_surfaces_debt_signals_for_changed_concept() {
    let root = concept_fixture_root();
    cli_save_v2_session(&root).expect("save v2 session");
    append_file(
        &root,
        "src/store/git-status-polling.ts",
        "export const addedWriter = true;\n",
    );

    let mut state = fresh_mcp_state();
    handle_scan(
        &json!({"path": root.to_string_lossy().to_string()}),
        &Tier::Free,
        &mut state,
    )
    .expect("scan fixture");
    state.cached_semantic = Some(concept_fixture_semantic(&root));

    let response = handle_session_end(&json!({}), &Tier::Free, &mut state).expect("session end");

    assert!(response["changed_files"].is_array());
    assert!(response["debt_signals"].is_array());
    assert!(response["actions"].is_array());
}

#[test]
fn session_end_routes_dead_private_clusters_to_experimental_debt_signals() {
    let root = experimental_gate_fixture_root();
    init_git_repo(&root);
    commit_all(&root, "initial");
    cli_save_v2_session(&root).expect("save v2 session");
    write_file(
        &root,
        "src/stale.ts",
        "function deadAlpha(): number { return 1; }\nfunction deadBeta(): number { return 2; }\nexport const liveValue = 3;\n",
    );

    let mut state = fresh_mcp_state();
    handle_scan(
        &json!({"path": root.to_string_lossy().to_string()}),
        &Tier::Free,
        &mut state,
    )
    .expect("scan fixture");

    let response = handle_session_end(&json!({}), &Tier::Free, &mut state).expect("session end");
    let experimental_findings = response["experimental_findings"]
        .as_array()
        .expect("experimental findings");
    let experimental_debt_signals = response["experimental_debt_signals"]
        .as_array()
        .expect("experimental debt signals");

    assert!(experimental_findings
        .iter()
        .all(|finding| finding["kind"] != "dead_private_code_cluster"));
    assert!(experimental_debt_signals
        .iter()
        .any(|finding| finding["kind"] == "dead_private_code_cluster"));
}

#[test]
fn session_end_promotes_session_introduced_clone_findings() {
    let root = temp_root("session-introduced-clone");
    write_session_clone_fixture_files(&root);
    init_git_repo(&root);
    commit_all(&root, "initial");
    cli_save_v2_session(&root).expect("save v2 session");
    write_session_clone_duplicate(&root);

    let mut state = fresh_mcp_state();
    handle_scan(
        &json!({"path": root.to_string_lossy().to_string()}),
        &Tier::Free,
        &mut state,
    )
    .expect("scan fixture");

    let response = handle_session_end(&json!({}), &Tier::Free, &mut state).expect("session end");
    let introduced = response["introduced_findings"]
        .as_array()
        .expect("introduced findings");
    let clone_findings = response["introduced_clone_findings"]
        .as_array()
        .expect("introduced clone findings");

    assert!(introduced
        .iter()
        .any(|finding| finding["kind"] == "session_introduced_clone"));
    assert!(introduced.iter().any(|finding| {
        finding["kind"] == "session_introduced_clone"
            && finding["repair_packet"]["complete"] == json!(true)
    }));
    assert!(!clone_findings.is_empty());
    assert!(clone_findings
        .iter()
        .all(|finding| finding["kind"] == "session_introduced_clone"));
    assert!(response["actions"][0]["fix_hint"]
        .as_str()
        .is_some_and(|hint| {
            hint.contains("src/copy.ts::") && hint.contains("src/source.ts::")
        }));
}

#[test]
fn session_end_promotes_clone_propagation_drift_findings() {
    let root = temp_root("session-clone-propagation-drift");
    write_session_clone_followthrough_fixture_files(&root);
    init_git_repo(&root);
    commit_all(&root, "initial");
    cli_save_v2_session(&root).expect("save v2 session");
    write_session_clone_followthrough_source_drift(&root);

    let mut state = fresh_mcp_state();
    handle_scan(
        &json!({"path": root.to_string_lossy().to_string()}),
        &Tier::Free,
        &mut state,
    )
    .expect("scan fixture");

    let response = handle_session_end(&json!({}), &Tier::Free, &mut state).expect("session end");
    let introduced = response["introduced_findings"]
        .as_array()
        .expect("introduced findings");
    let clone_findings = response["introduced_clone_findings"]
        .as_array()
        .expect("introduced clone findings");

    assert!(introduced
        .iter()
        .any(|finding| finding["kind"] == "clone_propagation_drift"));
    assert!(clone_findings
        .iter()
        .any(|finding| finding["kind"] == "clone_propagation_drift"));
    assert_eq!(
        response["actions"][0]["kind"],
        json!("clone_propagation_drift")
    );
    assert_eq!(
        response["signal_summary"]["introduced_clone_propagation_drift_count"],
        json!(1)
    );
    assert_eq!(
        response["signal_summary"]["action_quality"]["top_action_source"],
        json!("clone")
    );
    assert_eq!(
        response["signal_summary"]["regression_detected"],
        json!(true)
    );

    let event_log = root.join(".sentrux").join("agent-session-events.jsonl");
    let last_event = fs::read_to_string(event_log)
        .expect("read event log")
        .lines()
        .last()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("parse event"))
        .expect("session end event");

    assert_eq!(last_event["event_type"], json!("session_ended"));
    assert_eq!(
        last_event["signal_summary"]["introduced_clone_propagation_drift_count"],
        json!(1)
    );
    assert_eq!(
        last_event["signal_summary"]["action_quality"]["top_action_source"],
        json!("clone")
    );
}

#[test]
fn session_end_signal_summary_tracks_missing_propagation_obligations() {
    let root = prepare_ts_bridge_transport_gate_root();
    append_file(
        &root,
        "ts-bridge/src/transport.ts",
        "\nexport function transportHelper(): number { return 1; }\n",
    );

    let mut state = fresh_mcp_state();
    handle_scan(
        &json!({"path": root.to_string_lossy().to_string()}),
        &Tier::Free,
        &mut state,
    )
    .expect("scan fixture");

    let response = handle_session_end(&json!({}), &Tier::Free, &mut state).expect("session end");

    assert!(
        response["signal_summary"]["missing_propagation_obligation_count"]
            .as_u64()
            .is_some_and(|count| count >= 1)
    );
    assert_eq!(
        response["signal_summary"]["regression_detected"],
        json!(true)
    );
    assert_eq!(response["signal_summary"]["clear_to_stop"], json!(false));
}
