use super::{
    build_clone_drift_findings, build_clone_drift_report, build_clone_remediation_hints,
    CloneFamilySummary, CloneRemediationHint, FindingSeverity, RemediationPriority,
};
use crate::metrics::evolution::{
    AuthorInfo, CouplingPair, EvolutionReport, FileChurn, TemporalHotspot,
};
use crate::metrics::DuplicateGroup;
use std::collections::HashMap;

fn test_evolution() -> EvolutionReport {
    EvolutionReport {
        churn: HashMap::from([
            (
                "src/a.ts".to_string(),
                FileChurn {
                    commit_count: 4,
                    lines_added: 10,
                    lines_removed: 2,
                    total_churn: 12,
                },
            ),
            (
                "src/b.ts".to_string(),
                FileChurn {
                    commit_count: 0,
                    lines_added: 0,
                    lines_removed: 0,
                    total_churn: 0,
                },
            ),
        ]),
        coupling_pairs: Vec::<CouplingPair>::new(),
        hotspots: Vec::<TemporalHotspot>::new(),
        code_age: HashMap::from([("src/a.ts".to_string(), 3), ("src/b.ts".to_string(), 90)]),
        last_modified_epoch: HashMap::from([
            ("src/a.ts".to_string(), 1_000_000),
            ("src/b.ts".to_string(), 1_000_000 - (87 * 86_400)),
        ]),
        authors: HashMap::<String, AuthorInfo>::new(),
        single_author_ratio: 0.0,
        bus_factor_score: 1.0,
        churn_score: 1.0,
        evolution_score: 1.0,
        lookback_days: 90,
        commits_analyzed: 5,
    }
}

#[test]
fn clone_drift_findings_include_stable_ids_and_git_context() {
    let groups = vec![DuplicateGroup {
        hash: 42,
        instances: vec![
            ("src/a.ts".to_string(), "dup_a".to_string(), 12),
            ("src/b.ts".to_string(), "dup_b".to_string(), 12),
        ],
    }];

    let findings = build_clone_drift_findings(&groups, Some(&test_evolution()), 10);

    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].clone_id, "clone-0x0000000000002a");
    assert_eq!(findings[0].max_commit_count, 4);
    assert_eq!(findings[0].youngest_age_days, Some(3));
    assert!(findings[0].asymmetric_recent_change);
    assert_eq!(findings[0].severity, FindingSeverity::High);
}

#[test]
fn clone_drift_filters_test_only_and_tiny_groups() {
    let groups = vec![
        DuplicateGroup {
            hash: 1,
            instances: vec![
                ("src/a.test.ts".to_string(), "dup_a".to_string(), 10),
                ("src/b.test.ts".to_string(), "dup_b".to_string(), 10),
            ],
        },
        DuplicateGroup {
            hash: 2,
            instances: vec![
                ("src/a.ts".to_string(), "dup_a".to_string(), 1),
                ("src/b.ts".to_string(), "dup_b".to_string(), 1),
            ],
        },
    ];

    let findings = build_clone_drift_findings(&groups, None, 10);

    assert!(findings.is_empty());
}

#[test]
fn clone_drift_counts_recent_activity_per_file() {
    let groups = vec![DuplicateGroup {
        hash: 99,
        instances: vec![
            ("src/a.ts".to_string(), "dup_a".to_string(), 12),
            ("src/a.ts".to_string(), "dup_b".to_string(), 12),
            ("src/b.ts".to_string(), "dup_c".to_string(), 12),
        ],
    }];

    let findings = build_clone_drift_findings(&groups, Some(&test_evolution()), 10);

    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].recently_touched_file_count, 1);
    assert!(findings[0].asymmetric_recent_change);
    assert_eq!(findings[0].reasons[0], "identical logic spans 2 files");
    assert_eq!(findings[0].instances[0].file, "src/a.ts");
    assert_eq!(findings[0].instances[1].file, "src/a.ts");
    assert_eq!(findings[0].instances[2].file, "src/b.ts");
}

#[test]
fn clone_drift_report_groups_same_file_set_into_families() {
    let groups = vec![
        DuplicateGroup {
            hash: 7,
            instances: vec![
                ("src/a.ts".to_string(), "dup_a".to_string(), 12),
                ("src/b.ts".to_string(), "dup_b".to_string(), 12),
            ],
        },
        DuplicateGroup {
            hash: 8,
            instances: vec![
                ("src/a.ts".to_string(), "dup_c".to_string(), 9),
                ("src/b.ts".to_string(), "dup_d".to_string(), 9),
            ],
        },
    ];

    let report = build_clone_drift_report(&groups, Some(&test_evolution()));

    assert_eq!(report.findings.len(), 2);
    assert_eq!(report.families.len(), 1);
    assert_eq!(report.families[0].member_count, 2);
    assert_eq!(report.families[0].file_count, 2);
    assert_eq!(report.families[0].distinct_file_set_count, 1);
    assert!(!report.families[0].mixed_file_sets);
    assert_eq!(report.families[0].clone_ids.len(), 2);
    assert_eq!(report.families[0].commit_count_gap, Some(4));
    assert_eq!(report.families[0].age_days_gap, Some(87));
    assert!(report.families[0].divergence_score > 0);
    assert!(report.families[0]
        .summary
        .contains("churn differs by 4 recent commit(s)"));
    assert!(report.families[0]
        .reasons
        .iter()
        .any(|reason| reason.contains("family churn spans a gap of 4 recent commit(s)")));
    assert!(report.families[0]
        .reasons
        .iter()
        .any(|reason| reason.contains("family file age spans a gap of 87 day(s)")));
    assert!(report.families[0]
        .remediation_hints
        .iter()
        .any(|hint| hint.kind == "sync_recent_divergence"));
    assert!(report.families[0]
        .remediation_hints
        .iter()
        .any(|hint| hint.kind == "extract_shared_helper"));
    assert_eq!(
        report.prioritized_findings[0].clone_id,
        report.families[0].representative_clone_id
    );
}

#[test]
fn clone_drift_report_prioritizes_more_divergent_families_first() {
    let groups = vec![
        DuplicateGroup {
            hash: 10,
            instances: vec![
                ("src/a.ts".to_string(), "dup_a".to_string(), 12),
                ("src/b.ts".to_string(), "dup_b".to_string(), 12),
            ],
        },
        DuplicateGroup {
            hash: 11,
            instances: vec![
                ("src/a.ts".to_string(), "dup_c".to_string(), 10),
                ("src/b.ts".to_string(), "dup_d".to_string(), 10),
            ],
        },
        DuplicateGroup {
            hash: 12,
            instances: vec![
                ("src/c.ts".to_string(), "dup_e".to_string(), 10),
                ("src/d.ts".to_string(), "dup_f".to_string(), 10),
            ],
        },
        DuplicateGroup {
            hash: 13,
            instances: vec![
                ("src/c.ts".to_string(), "dup_g".to_string(), 10),
                ("src/d.ts".to_string(), "dup_h".to_string(), 10),
            ],
        },
    ];

    let mut evolution = test_evolution();
    evolution.churn.insert(
        "src/a.ts".to_string(),
        crate::metrics::evolution::FileChurn {
            commit_count: 10,
            lines_added: 14,
            lines_removed: 2,
            total_churn: 16,
        },
    );
    evolution.churn.insert(
        "src/b.ts".to_string(),
        crate::metrics::evolution::FileChurn {
            commit_count: 9,
            lines_added: 12,
            lines_removed: 1,
            total_churn: 13,
        },
    );
    evolution.churn.insert(
        "src/c.ts".to_string(),
        crate::metrics::evolution::FileChurn {
            commit_count: 10,
            lines_added: 14,
            lines_removed: 2,
            total_churn: 16,
        },
    );
    evolution.churn.insert(
        "src/d.ts".to_string(),
        crate::metrics::evolution::FileChurn {
            commit_count: 1,
            lines_added: 1,
            lines_removed: 0,
            total_churn: 1,
        },
    );
    evolution.code_age.insert("src/a.ts".to_string(), 4);
    evolution.code_age.insert("src/b.ts".to_string(), 5);
    evolution.code_age.insert("src/c.ts".to_string(), 4);
    evolution.code_age.insert("src/d.ts".to_string(), 5);

    let report = build_clone_drift_report(&groups, Some(&evolution));

    assert_eq!(report.families.len(), 2);
    assert_eq!(report.prioritized_findings.len(), 4);
    assert!(report.families[0].divergence_score > report.families[1].divergence_score);
    assert_eq!(
        report.prioritized_findings[0].clone_id,
        report.families[0].representative_clone_id
    );
    assert!(report.families[0]
        .remediation_hints
        .iter()
        .any(|hint| hint.kind == "extract_shared_helper"));
    assert!(report.families[0]
        .remediation_hints
        .iter()
        .any(|hint| hint.kind == "sync_recent_divergence"));
    assert!(report.families[0]
        .summary
        .contains("churn differs by 9 recent commit(s)"));
    assert_eq!(report.families[0].commit_count_gap, Some(9));
}

#[test]
fn clone_drift_report_groups_overlapping_file_sets_into_one_family() {
    let groups = vec![
        DuplicateGroup {
            hash: 21,
            instances: vec![
                ("src/a.ts".to_string(), "dup_a".to_string(), 12),
                ("src/b.ts".to_string(), "dup_b".to_string(), 12),
            ],
        },
        DuplicateGroup {
            hash: 22,
            instances: vec![
                ("src/a.ts".to_string(), "dup_c".to_string(), 10),
                ("src/b.ts".to_string(), "dup_d".to_string(), 10),
                ("src/c.ts".to_string(), "dup_e".to_string(), 10),
            ],
        },
    ];

    let report = build_clone_drift_report(&groups, Some(&test_evolution()));

    assert_eq!(report.families.len(), 1);
    assert_eq!(report.families[0].member_count, 2);
    assert_eq!(report.families[0].file_count, 3);
    assert_eq!(report.families[0].distinct_file_set_count, 2);
    assert!(report.families[0].mixed_file_sets);
    assert!(report.families[0]
        .summary
        .contains("overlapping sibling sets"));
    assert!(report.families[0]
        .reasons
        .iter()
        .any(|reason| reason.contains("overlapping file set(s)")));
    assert!(report.families[0]
        .remediation_hints
        .iter()
        .any(|hint| hint.kind == "review_family_boundaries"));
}

#[test]
fn clone_family_age_gap_uses_stable_epoch_difference() {
    let groups = vec![
        DuplicateGroup {
            hash: 30,
            instances: vec![
                ("src/a.ts".to_string(), "dup_a".to_string(), 12),
                ("src/b.ts".to_string(), "dup_b".to_string(), 12),
            ],
        },
        DuplicateGroup {
            hash: 31,
            instances: vec![
                ("src/a.ts".to_string(), "dup_c".to_string(), 10),
                ("src/b.ts".to_string(), "dup_d".to_string(), 10),
            ],
        },
    ];

    let evolution = EvolutionReport {
        churn: HashMap::from([
            (
                "src/a.ts".to_string(),
                FileChurn {
                    commit_count: 1,
                    lines_added: 1,
                    lines_removed: 0,
                    total_churn: 1,
                },
            ),
            (
                "src/b.ts".to_string(),
                FileChurn {
                    commit_count: 1,
                    lines_added: 1,
                    lines_removed: 0,
                    total_churn: 1,
                },
            ),
        ]),
        coupling_pairs: Vec::<CouplingPair>::new(),
        hotspots: Vec::<TemporalHotspot>::new(),
        code_age: HashMap::from([("src/a.ts".to_string(), 1), ("src/b.ts".to_string(), 0)]),
        last_modified_epoch: HashMap::from([
            ("src/a.ts".to_string(), 1_000_000),
            ("src/b.ts".to_string(), 1_000_000 + (12 * 60 * 60)),
        ]),
        authors: HashMap::<String, AuthorInfo>::new(),
        single_author_ratio: 0.0,
        bus_factor_score: 1.0,
        churn_score: 1.0,
        evolution_score: 1.0,
        lookback_days: 90,
        commits_analyzed: 2,
    };

    let report = build_clone_drift_report(&groups, Some(&evolution));
    assert_eq!(report.families.len(), 1);
    assert_eq!(report.families[0].age_days_gap, Some(0));
}

#[test]
fn clone_remediation_hints_round_robin_across_families() {
    let families = vec![
        CloneFamilySummary {
            family_id: "family-a".to_string(),
            remediation_hints: vec![
                CloneRemediationHint {
                    kind: "sync_recent_divergence".to_string(),
                    priority: RemediationPriority::High,
                    summary: "sync".to_string(),
                    files: vec!["src/a.ts".to_string(), "src/b.ts".to_string()],
                    clone_ids: vec!["clone-a".to_string()],
                },
                CloneRemediationHint {
                    kind: "extract_shared_helper".to_string(),
                    priority: RemediationPriority::Medium,
                    summary: "extract".to_string(),
                    files: vec!["src/a.ts".to_string(), "src/b.ts".to_string()],
                    clone_ids: vec!["clone-a".to_string()],
                },
            ],
            ..CloneFamilySummary::default()
        },
        CloneFamilySummary {
            family_id: "family-b".to_string(),
            remediation_hints: vec![CloneRemediationHint {
                kind: "collapse_clone_family".to_string(),
                priority: RemediationPriority::Medium,
                summary: "collapse".to_string(),
                files: vec!["src/c.ts".to_string(), "src/d.ts".to_string()],
                clone_ids: vec!["clone-b".to_string()],
            }],
            ..CloneFamilySummary::default()
        },
    ];

    let hints = build_clone_remediation_hints(&families, 3);

    assert_eq!(hints.len(), 3);
    assert_eq!(hints[0].kind, "sync_recent_divergence");
    assert_eq!(hints[1].kind, "collapse_clone_family");
    assert_eq!(hints[2].kind, "extract_shared_helper");
}

#[test]
fn remediation_priority_serializes_to_legacy_strings() {
    let hint = CloneRemediationHint {
        kind: "sync_recent_divergence".to_string(),
        priority: RemediationPriority::High,
        summary: "sync".to_string(),
        files: vec!["src/a.ts".to_string()],
        clone_ids: vec!["clone-a".to_string()],
    };

    let value = serde_json::to_value(&hint).expect("serialize hint");

    assert_eq!(value["priority"], "high");
}
