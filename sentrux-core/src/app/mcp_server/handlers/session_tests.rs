use super::test_support::{
    append_file, cli_gate_fixture_root, concept_fixture_root, concept_fixture_semantic, write_file,
};
use super::{
    cli_evaluate_v2_gate, cli_save_v2_session, fresh_mcp_state, handle_scan, handle_session_end,
    prepare_patch_check_context,
};
use crate::license::Tier;
use serde_json::json;

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
