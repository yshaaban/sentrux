use super::test_support::{
    concept_fixture_root, concept_fixture_semantic, structural_debt_fixture_root,
};
use super::{
    apply_suppressions, build_exact_clone_findings, distinct_file_count, fresh_mcp_state,
    handle_findings, handle_scan, overall_confidence_0_10000,
};
use crate::analysis::scanner::common::{ScanMetadata, ScanMode};
use crate::license::Tier;
use crate::metrics::DuplicateGroup;
use serde_json::json;

#[test]
fn apply_suppressions_hides_matching_findings_and_tracks_hits() {
    let config: crate::metrics::rules::RulesConfig = toml::from_str(
        r#"
            [[suppress]]
            kind = "forbidden_writer"
            concept = "task_git_status"
            file = "src/store/**"
            reason = "temporary migration"
            expires = "2099-12-31"
        "#,
    )
    .expect("rules config");
    let findings = vec![
        json!({
            "kind": "forbidden_writer",
            "concept_id": "task_git_status",
            "files": ["src/store/git-status-polling.ts"],
            "summary": "forbidden writer",
        }),
        json!({
            "kind": "forbidden_raw_read",
            "concept_id": "task_git_status",
            "files": ["src/components/TaskRow.tsx"],
            "summary": "raw read",
        }),
    ];

    let application = apply_suppressions(&config, findings);

    assert_eq!(application.visible_findings.len(), 1);
    assert_eq!(application.active_matches.len(), 1);
    assert_eq!(application.active_matches[0].matched_finding_count, 1);
    assert_eq!(
        application.visible_findings[0]["kind"],
        "forbidden_raw_read"
    );
}

#[test]
fn exact_clone_findings_filter_same_file_groups() {
    let same_file = DuplicateGroup {
        hash: 1,
        instances: vec![
            ("src/a.ts".into(), "dup_a".into(), 10),
            ("src/a.ts".into(), "dup_b".into(), 10),
        ],
    };
    let cross_file = DuplicateGroup {
        hash: 2,
        instances: vec![
            ("src/a.ts".into(), "dup_a".into(), 12),
            ("src/b.ts".into(), "dup_b".into(), 12),
        ],
    };

    let findings = build_exact_clone_findings(&[same_file, cross_file], 10);

    assert_eq!(findings.len(), 1);
    assert_eq!(
        distinct_file_count(&DuplicateGroup {
            hash: 2,
            instances: vec![
                ("src/a.ts".into(), "dup_a".into(), 12),
                ("src/b.ts".into(), "dup_b".into(), 12),
            ],
        }),
        2
    );
}

#[test]
fn overall_confidence_penalizes_partial_and_truncated_scans() {
    let mut metadata = ScanMetadata::empty(ScanMode::Git);
    let baseline = overall_confidence_0_10000(&metadata, 9000, 8000);
    assert_eq!(baseline, 8000);

    metadata.partial = true;
    let partial = overall_confidence_0_10000(&metadata, 9000, 8000);
    assert_eq!(partial, 6400);

    metadata.truncated = true;
    let truncated = overall_confidence_0_10000(&metadata, 9000, 8000);
    assert_eq!(truncated, 4480);
}

#[test]
fn findings_surface_concept_summaries_debt_signals_and_watchpoints() {
    let root = concept_fixture_root();
    let mut state = fresh_mcp_state();
    handle_scan(
        &json!({"path": root.to_string_lossy().to_string()}),
        &Tier::Free,
        &mut state,
    )
    .expect("scan concept fixture");
    state.cached_semantic = Some(concept_fixture_semantic(&root));

    let response = handle_findings(&json!({}), &Tier::Free, &mut state).expect("findings");

    assert!(response["concept_summaries"].is_array());
    assert!(response["debt_signals"].is_array());
    assert!(response["watchpoints"].is_array());
    assert!(response["finding_details"].is_array());
}

#[test]
fn findings_surface_structural_debt_signals() {
    let root = structural_debt_fixture_root();
    let mut state = fresh_mcp_state();
    handle_scan(
        &json!({"path": root.to_string_lossy().to_string()}),
        &Tier::Free,
        &mut state,
    )
    .expect("scan structural fixture");

    let response = handle_findings(&json!({}), &Tier::Free, &mut state).expect("findings");
    let debt_signals = response["debt_signals"]
        .as_array()
        .expect("debt signals array");

    assert!(!debt_signals.is_empty());
    assert!(debt_signals.iter().all(|signal| signal
        .get("kind")
        .and_then(|value| value.as_str())
        .is_some()));
}
