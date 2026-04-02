use super::brief::handle_agent_brief;
use super::test_support::{
    append_file, commit_all, concept_fixture_root, concept_fixture_semantic, init_git_repo,
    temp_root, write_session_clone_duplicate, write_session_clone_fixture_files,
};
use super::{fresh_mcp_state, handle_scan, handle_session_start};
use crate::license::Tier;
use serde_json::json;

#[test]
fn patch_brief_marks_evolution_unavailable_on_fast_path() {
    let root = concept_fixture_root();
    init_git_repo(&root);
    commit_all(&root, "initial");
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

    let response = handle_agent_brief(
        &json!({"mode": "patch", "limit": 3}),
        &Tier::Free,
        &mut state,
    )
    .expect("patch brief");

    assert_eq!(
        response["diagnostics"]["availability"]["evolution"],
        json!(false)
    );
    assert_eq!(response["diagnostics"]["partial_results"], json!(true));
    assert!(response["actions"].is_array());
    assert_eq!(
        response["action_count"],
        response["actions"]
            .as_array()
            .map(|items| items.len())
            .unwrap_or_default()
    );
}

#[test]
fn patch_brief_surfaces_session_introduced_clone_actions() {
    let root = temp_root("brief-session-introduced-clone");
    write_session_clone_fixture_files(&root);
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
    write_session_clone_duplicate(&root);
    handle_scan(
        &json!({"path": root.to_string_lossy().to_string()}),
        &Tier::Free,
        &mut state,
    )
    .expect("rescan fixture");

    let response = handle_agent_brief(
        &json!({"mode": "patch", "limit": 3}),
        &Tier::Free,
        &mut state,
    )
    .expect("patch brief");

    assert!(response["introduced_findings"]
        .as_array()
        .expect("introduced findings")
        .iter()
        .any(|finding| finding["kind"] == "session_introduced_clone"));
    assert_eq!(
        response["actions"][0]["kind"],
        json!("session_introduced_clone")
    );
}
