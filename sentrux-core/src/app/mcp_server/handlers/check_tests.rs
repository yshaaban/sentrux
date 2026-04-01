use super::test_support::{commit_all, init_git_repo, temp_root, write_file};
use super::{fresh_mcp_state, handle_check, handle_scan};
use crate::analysis::project_shape::{ModuleContractSuggestion, ProjectShapeReport};
use crate::license::Tier;
use serde_json::json;

#[test]
fn check_returns_partial_when_changed_scope_is_unavailable() {
    let root = temp_root("check-unavailable-scope");
    write_file(
        &root,
        "src/app.ts",
        "export function render(): number { return 1; }\n",
    );

    let mut state = fresh_mcp_state();
    handle_scan(
        &json!({"path": root.to_string_lossy().to_string()}),
        &Tier::Free,
        &mut state,
    )
    .expect("scan fixture");

    let response = handle_check(&json!({}), &Tier::Free, &mut state).expect("check response");

    assert_eq!(response["issues"], json!([]));
    assert_eq!(
        response["diagnostics"]["availability"]["changed_scope"],
        json!(false)
    );
    assert_eq!(
        response["diagnostics"]["availability"]["semantic"],
        json!(false)
    );
    assert_eq!(
        response["diagnostics"]["availability"]["rules"],
        json!(false)
    );
    assert_eq!(
        response["diagnostics"]["availability"]["evolution"],
        json!(false)
    );
    assert_eq!(response["diagnostics"]["partial_results"], json!(true));
}

#[test]
fn check_returns_clean_result_when_changed_scope_is_known_empty() {
    let root = temp_root("check-empty-scope");
    write_file(
        &root,
        "src/app.ts",
        "export function render(): number { return 1; }\n",
    );
    init_git_repo(&root);
    commit_all(&root, "initial");

    let mut state = fresh_mcp_state();
    handle_scan(
        &json!({"path": root.to_string_lossy().to_string()}),
        &Tier::Free,
        &mut state,
    )
    .expect("scan fixture");

    let response = handle_check(&json!({}), &Tier::Free, &mut state).expect("check response");

    assert_eq!(response["issues"], json!([]));
    assert_eq!(response["changed_files"], json!([]));
    assert_eq!(response["gate"], json!("pass"));
    assert_eq!(
        response["diagnostics"]["availability"]["changed_scope"],
        json!(true)
    );
    assert_eq!(
        response["diagnostics"]["availability"]["semantic"],
        json!(false)
    );
    assert_eq!(
        response["diagnostics"]["availability"]["rules"],
        json!(false)
    );
    assert_eq!(
        response["diagnostics"]["availability"]["evolution"],
        json!(false)
    );
    assert_eq!(response["diagnostics"]["partial_results"], json!(false));
}

#[test]
fn check_warns_when_missing_test_signal_is_skipped_without_session_baseline() {
    let root = temp_root("check-missing-test-baseline-warning");
    write_file(
        &root,
        "src/app.ts",
        "export function render(): number { return 1; }\n",
    );
    init_git_repo(&root);
    commit_all(&root, "initial");
    write_file(
        &root,
        "src/task-health-monitor.ts",
        "export function monitorTaskHealth(): number { return 1; }\n",
    );

    let mut state = fresh_mcp_state();
    handle_scan(
        &json!({"path": root.to_string_lossy().to_string()}),
        &Tier::Free,
        &mut state,
    )
    .expect("scan fixture");

    let response = handle_check(&json!({}), &Tier::Free, &mut state).expect("check response");
    let warnings = response["diagnostics"]["warnings"]
        .as_array()
        .expect("warnings array");

    assert!(warnings.iter().any(|warning| {
        warning
            .as_str()
            .is_some_and(|warning| warning.contains("missing test coverage checks skipped"))
    }));
    assert!(response["issues"]
        .as_array()
        .expect("issues array")
        .iter()
        .all(|issue| issue["kind"] != "missing_test_coverage"));
}

#[test]
fn check_applies_suppressions_to_inferred_boundary_findings() {
    let root = temp_root("check-boundary-suppression");
    write_file(
        &root,
        ".sentrux/rules.toml",
        r#"
            [[suppress]]
            kind = "zero_config_boundary_violation"
            file = "src/app/feature.ts"
            reason = "approved exception"
        "#,
    );
    write_file(
        &root,
        "src/module/index.ts",
        "export { value } from './internal';\n",
    );
    write_file(&root, "src/module/internal.ts", "export const value = 1;\n");
    write_file(
        &root,
        "src/app/feature.ts",
        "import { value } from '../module/internal';\nvoid value;\n",
    );
    init_git_repo(&root);
    commit_all(&root, "initial");
    write_file(
        &root,
        "src/app/feature.ts",
        "import { value } from '../module/internal';\nexport const feature = value + 1;\n",
    );

    let mut state = fresh_mcp_state();
    handle_scan(
        &json!({"path": root.to_string_lossy().to_string()}),
        &Tier::Free,
        &mut state,
    )
    .expect("scan fixture");
    state.cached_project_shape = Some(ProjectShapeReport {
        module_contracts: vec![ModuleContractSuggestion {
            id: "module_api".to_string(),
            root: "src/module".to_string(),
            public_api: vec!["src/module/index.ts".to_string()],
            nested_public_api: Vec::new(),
            confidence: "high".to_string(),
            evidence: vec!["detected boundary root".to_string()],
        }],
        ..ProjectShapeReport::default()
    });
    state.cached_project_shape_identity = state.cached_scan_identity.clone();

    let response = handle_check(&json!({}), &Tier::Free, &mut state).expect("check response");

    assert!(response["issues"]
        .as_array()
        .expect("issues array")
        .iter()
        .all(|issue| issue["kind"] != "zero_config_boundary_violation"));
}
