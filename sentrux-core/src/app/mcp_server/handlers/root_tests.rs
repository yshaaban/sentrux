use super::checkpoint::load_persisted_session_v2;
use super::{overall_confidence_0_10000, project_fingerprint, save_session_v2_baseline};
use super::test_support::{commit_all, init_git_repo, temp_root, write_file};
use crate::analysis::scanner::common::{ScanMetadata, ScanMode};
use crate::app::mcp_server::{
    SessionV2Baseline, SessionV2ConfidenceSnapshot, SESSION_V2_SCHEMA_VERSION,
};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

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
fn session_v2_baseline_roundtrips_on_disk() {
    let root = temp_root("session-v2-roundtrip");
    let baseline = SessionV2Baseline {
        schema_version: SESSION_V2_SCHEMA_VERSION,
        project_fingerprint: Some(project_fingerprint(&root)),
        sentrux_version: Some(env!("CARGO_PKG_VERSION").to_string()),
        file_hashes: BTreeMap::from([("src/a.ts".to_string(), 11), ("src/b.ts".to_string(), 22)]),
        finding_payloads: BTreeMap::from([(
            "finding-1".to_string(),
            json!({"kind": "closed_domain_exhaustiveness", "severity": "high"}),
        )]),
        git_head: Some("abc123".to_string()),
        working_tree_paths: BTreeSet::from(["src/a.ts".to_string()]),
        confidence: SessionV2ConfidenceSnapshot {
            scan_confidence_0_10000: Some(8100),
            rule_coverage_0_10000: Some(7500),
        },
    };

    let path = save_session_v2_baseline(&root, &baseline).expect("save session baseline");
    let loaded = load_persisted_session_v2(&root)
        .expect("load session baseline")
        .expect("session baseline exists");

    assert_eq!(path, root.join(".sentrux").join("session-v2.json"));
    assert_eq!(loaded.file_hashes, baseline.file_hashes);
    assert_eq!(loaded.finding_payloads, baseline.finding_payloads);
    assert_eq!(loaded.git_head, baseline.git_head);
    assert_eq!(loaded.working_tree_paths, baseline.working_tree_paths);

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn project_fingerprint_matches_across_local_clone() {
    let source = temp_root("project-fingerprint-source");
    write_file(
        &source,
        "src/domain/state.ts",
        "export const state = 'idle';\n",
    );
    init_git_repo(&source);
    commit_all(&source, "initial");

    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let clone = std::env::temp_dir().join(format!(
        "private-integration-crateject-fingerprint-clone-{}-{unique}",
        std::process::id()
    ));
    let status = Command::new("git")
        .arg("clone")
        .arg("--quiet")
        .arg(&source)
        .arg(&clone)
        .status()
        .expect("clone repo");
    assert!(status.success(), "git clone failed");

    assert_eq!(project_fingerprint(&source), project_fingerprint(&clone));

    let _ = std::fs::remove_dir_all(source);
    let _ = std::fs::remove_dir_all(clone);
}
