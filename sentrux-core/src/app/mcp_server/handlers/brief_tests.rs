use super::brief::handle_agent_brief;
use super::test_support::{
    append_file, commit_all, concept_fixture_root, concept_fixture_semantic, init_git_repo,
};
use super::{fresh_mcp_state, handle_scan};
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
}
