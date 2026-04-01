use super::test_support::{concept_fixture_root, concept_fixture_semantic};
use super::{
    fresh_mcp_state, handle_concepts, handle_explain_concept, handle_project_shape, handle_scan,
    handle_trace_symbol,
};
use crate::license::Tier;
use serde_json::json;

#[test]
fn concepts_surface_guardrail_tests_and_inferred_concepts() {
    let root = concept_fixture_root();
    let mut state = fresh_mcp_state();
    handle_scan(
        &json!({"path": root.to_string_lossy().to_string()}),
        &Tier::Free,
        &mut state,
    )
    .expect("scan fixture");
    state.cached_semantic = Some(concept_fixture_semantic(&root));

    let response = handle_concepts(&json!({}), &Tier::Free, &mut state).expect("concepts");

    assert_eq!(response["kind"], "concepts");
    assert!(response["concepts"].is_array());
    assert!(response["contracts"].is_array());
    assert!(response["guardrail_tests"].is_array());
    assert!(response["inferred_concepts"].is_array());
}

#[test]
fn project_shape_tool_surfaces_archetypes_and_starter_rules() {
    let root = concept_fixture_root();
    let mut state = fresh_mcp_state();
    handle_scan(
        &json!({"path": root.to_string_lossy().to_string()}),
        &Tier::Free,
        &mut state,
    )
    .expect("scan fixture");

    let response =
        handle_project_shape(&json!({}), &Tier::Free, &mut state).expect("project shape");

    assert_eq!(response["kind"], "project_shape");
    assert!(response["project_shape"].is_object());
    assert!(response["diagnostics"]["errors"].is_object());
}

#[test]
fn explain_concept_returns_related_findings_obligations_and_contracts() {
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
        handle_explain_concept(&json!({"id": "task_git_status"}), &Tier::Free, &mut state)
            .expect("explain concept");

    assert_eq!(response["kind"], "explain_concept");
    assert_eq!(response["concept"]["id"], "task_git_status");
    assert!(response["related_contract_ids"].is_array());
    assert!(response["findings"].is_array());
    assert!(response["obligations"].is_array());
}

#[test]
fn trace_symbol_uses_scoped_query_for_declaration_and_global_query_for_references() {
    let root = concept_fixture_root();
    let mut state = fresh_mcp_state();
    handle_scan(
        &json!({"path": root.to_string_lossy().to_string()}),
        &Tier::Free,
        &mut state,
    )
    .expect("scan fixture");
    state.cached_semantic = Some(concept_fixture_semantic(&root));

    let response = handle_trace_symbol(
        &json!({"symbol": "src/app/task-presentation.ts::TaskStateRegistry"}),
        &Tier::Free,
        &mut state,
    )
    .expect("trace symbol");

    assert_eq!(response["kind"], "trace_symbol");
    assert!(response["declarations"].is_array());
    assert!(response["reads"].is_array());
    assert!(response["writes"].is_array());
}
