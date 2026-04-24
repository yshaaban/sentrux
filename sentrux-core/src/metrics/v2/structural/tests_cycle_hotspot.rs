use super::*;

#[test]
fn unstable_hotspot_marks_transport_facades_as_boundary_discipline() {
    let snapshot = Snapshot {
        root: Arc::new(FileNode {
            path: ".".to_string(),
            name: ".".to_string(),
            is_dir: true,
            lines: 0,
            logic: 0,
            comments: 0,
            blanks: 0,
            funcs: 0,
            mtime: 0.0,
            gs: String::new(),
            lang: String::new(),
            sa: None,
            children: Some(vec![
                test_file("src/lib/ipc.ts", 180, 5, 12),
                test_file("src/app/browser-session.ts", 90, 2, 5),
                test_file("src/app/electron-session.ts", 90, 2, 5),
            ]),
        }),
        total_files: 3,
        total_lines: 360,
        total_dirs: 1,
        import_graph: vec![
            ImportEdge {
                from_file: "src/app/browser-session.ts".into(),
                to_file: "src/lib/ipc.ts".into(),
            },
            ImportEdge {
                from_file: "src/app/electron-session.ts".into(),
                to_file: "src/lib/ipc.ts".into(),
            },
        ],
        call_graph: Vec::new(),
        inherit_graph: Vec::new(),
        entry_points: Vec::new(),
        exec_depth: HashMap::new(),
    };
    let mut health = empty_health_report();
    health.hotspot_files = vec![FileMetric {
        path: "src/lib/ipc.ts".into(),
        value: 24,
    }];

    let reports = build_structural_debt_reports(&snapshot, &health);
    let report = reports
        .iter()
        .find(|report| report.kind == "unstable_hotspot")
        .expect("unstable-hotspot report");

    assert!(report.role_tags.iter().any(|tag| tag == "transport_facade"));
    assert_eq!(
        report.leverage_class,
        StructuralLeverageClass::BoundaryDiscipline
    );
    assert!(report
        .leverage_reasons
        .iter()
        .any(|reason| reason == "guarded_or_transport_facade"));
}

#[test]
fn cycle_cluster_score_boosts_boundary_hubs_and_cut_leverage() {
    let base_score = cycle_cluster_score(6, 900, &[], &[]);
    let boosted_score = cycle_cluster_score(
        6,
        900,
        &[
            "component_barrel".to_string(),
            "guarded_boundary".to_string(),
        ],
        &[CycleCutCandidate {
            seam_kind: "guarded_boundary_cut".to_string(),
            reduction_file_count: 3,
            remaining_cycle_size: 2,
            ..CycleCutCandidate::default()
        }],
    );

    assert!(boosted_score > base_score);
}

#[test]
fn cycle_cluster_score_prefers_stronger_cut_reduction_over_weaker_remainder() {
    let stronger_cut = cycle_cluster_score(
        10,
        1_400,
        &["guarded_boundary".to_string()],
        &[CycleCutCandidate {
            seam_kind: "guarded_boundary_cut".to_string(),
            reduction_file_count: 6,
            remaining_cycle_size: 3,
            ..CycleCutCandidate::default()
        }],
    );
    let weaker_cut = cycle_cluster_score(
        10,
        1_400,
        &["guarded_boundary".to_string()],
        &[CycleCutCandidate {
            seam_kind: "guarded_boundary_cut".to_string(),
            reduction_file_count: 2,
            remaining_cycle_size: 8,
            ..CycleCutCandidate::default()
        }],
    );

    assert!(stronger_cut > weaker_cut);
}

#[test]
fn cycle_reports_interaction_only_basis_as_watchpoint_evidence() {
    let snapshot = Snapshot {
        root: Arc::new(FileNode {
            path: ".".to_string(),
            name: ".".to_string(),
            is_dir: true,
            lines: 0,
            logic: 0,
            comments: 0,
            blanks: 0,
            funcs: 0,
            mtime: 0.0,
            gs: String::new(),
            lang: String::new(),
            sa: None,
            children: Some(vec![
                test_file("src/runtime-a.ts", 140, 4, 12),
                test_file("src/runtime-b.ts", 120, 3, 9),
            ]),
        }),
        total_files: 2,
        total_lines: 260,
        total_dirs: 1,
        import_graph: Vec::new(),
        call_graph: vec![
            CallEdge {
                from_file: "src/runtime-a.ts".into(),
                from_func: "runA".into(),
                to_file: "src/runtime-b.ts".into(),
                to_func: "runB".into(),
            },
            CallEdge {
                from_file: "src/runtime-b.ts".into(),
                from_func: "runB".into(),
                to_file: "src/runtime-a.ts".into(),
                to_func: "runA".into(),
            },
        ],
        inherit_graph: Vec::new(),
        entry_points: Vec::new(),
        exec_depth: HashMap::new(),
    };
    let mut health = empty_health_report();
    health.circular_dep_files = vec![vec!["src/runtime-a.ts".into(), "src/runtime-b.ts".into()]];
    health.circular_dep_count = 1;

    let reports = build_structural_debt_reports(&snapshot, &health);
    let report = reports
        .iter()
        .find(|report| report.kind == "cycle_cluster")
        .expect("cycle report");

    assert!(report.summary.contains("interaction/call-only"));
    assert!(report
        .signal_families
        .iter()
        .any(|family| family == "interaction_cycle"));
    assert_eq!(report.signal_class, StructuralSignalClass::Watchpoint);
    assert!(report.score_0_10000 <= 4_900);
    assert!(report
        .evidence
        .iter()
        .any(|entry| entry == "edge basis: interaction/call-only"));
    assert!(report
        .evidence
        .iter()
        .any(|entry| entry.contains("representative interaction/call edges:")));
    assert!(report.cut_candidates.iter().all(|candidate| candidate
        .evidence
        .iter()
        .any(|entry| entry == "edge basis: interaction/call")));
}

#[test]
fn cycle_reports_mixed_basis_and_uses_import_cut_candidates() {
    let snapshot = Snapshot {
        root: Arc::new(FileNode {
            path: ".".to_string(),
            name: ".".to_string(),
            is_dir: true,
            lines: 0,
            logic: 0,
            comments: 0,
            blanks: 0,
            funcs: 0,
            mtime: 0.0,
            gs: String::new(),
            lang: String::new(),
            sa: None,
            children: Some(vec![
                test_file("src/adapter.ts", 160, 4, 14),
                test_file("src/service.ts", 180, 5, 16),
            ]),
        }),
        total_files: 2,
        total_lines: 340,
        total_dirs: 1,
        import_graph: vec![ImportEdge {
            from_file: "src/adapter.ts".into(),
            to_file: "src/service.ts".into(),
        }],
        call_graph: vec![CallEdge {
            from_file: "src/service.ts".into(),
            from_func: "serve".into(),
            to_file: "src/adapter.ts".into(),
            to_func: "adapt".into(),
        }],
        inherit_graph: Vec::new(),
        entry_points: Vec::new(),
        exec_depth: HashMap::new(),
    };
    let mut health = empty_health_report();
    health.circular_dep_files = vec![vec!["src/adapter.ts".into(), "src/service.ts".into()]];
    health.circular_dep_count = 1;

    let reports = build_structural_debt_reports(&snapshot, &health);
    let report = reports
        .iter()
        .find(|report| report.kind == "cycle_cluster")
        .expect("cycle report");

    assert!(report.summary.contains("mixed import/interaction"));
    assert!(report
        .signal_families
        .iter()
        .any(|family| family == "mixed_cycle"));
    assert!(report
        .evidence
        .iter()
        .any(|entry| entry == "edge basis: mixed"));
    assert!(report
        .evidence
        .iter()
        .any(|entry| entry.contains("representative import edges:")));
    assert!(report
        .evidence
        .iter()
        .any(|entry| entry.contains("representative interaction/call edges:")));
    let best_cut = report.cut_candidates.first().expect("import cut candidate");
    assert_eq!(best_cut.source, "src/adapter.ts");
    assert_eq!(best_cut.target, "src/service.ts");
    assert!(best_cut
        .evidence
        .iter()
        .any(|entry| entry == "edge basis: import"));
}

#[test]
fn unstable_hotspot_marks_state_containers_as_architecture_signals() {
    let snapshot = Snapshot {
        root: Arc::new(FileNode {
            path: ".".to_string(),
            name: ".".to_string(),
            is_dir: true,
            lines: 0,
            logic: 0,
            comments: 0,
            blanks: 0,
            funcs: 0,
            mtime: 0.0,
            gs: String::new(),
            lang: String::new(),
            sa: None,
            children: Some(vec![test_file("src/store/chat-input.store.ts", 204, 5, 9)]),
        }),
        total_files: 1,
        total_lines: 204,
        total_dirs: 1,
        import_graph: Vec::new(),
        call_graph: Vec::new(),
        inherit_graph: Vec::new(),
        entry_points: Vec::new(),
        exec_depth: HashMap::new(),
    };
    let mut health = empty_health_report();
    health.hotspot_files = vec![FileMetric {
        path: "src/store/chat-input.store.ts".into(),
        value: 17,
    }];

    let reports = build_structural_debt_reports(&snapshot, &health);
    let report = reports
        .iter()
        .find(|report| report.scope == "src/store/chat-input.store.ts")
        .expect("state container report");

    assert!(report.role_tags.iter().any(|tag| tag == "state_container"));
    assert_eq!(
        report.leverage_class,
        StructuralLeverageClass::ArchitectureSignal
    );
}

#[test]
fn cycle_cut_candidates_prefer_guarded_app_store_boundary_edges() {
    let root = temp_root("sentrux-structural", "cycle-guarded-boundary", &[]);
    write_file(
        &root,
        "src/app/store-boundary.architecture.test.ts",
        "expect(source).not.toContain('store/store');\n",
    );
    write_file(
        &root,
        "src/store/store.ts",
        "export function selectStore() {}\n",
    );

    let snapshot = Snapshot {
        root: Arc::new(FileNode {
            path: ".".to_string(),
            name: ".".to_string(),
            is_dir: true,
            lines: 0,
            logic: 0,
            comments: 0,
            blanks: 0,
            funcs: 0,
            mtime: 0.0,
            gs: String::new(),
            lang: String::new(),
            sa: None,
            children: Some(vec![
                test_file("src/store/store.ts", 220, 6, 18),
                test_file("src/store/core.ts", 180, 4, 11),
                test_file("src/app/task-workflows.ts", 240, 5, 17),
            ]),
        }),
        total_files: 3,
        total_lines: 640,
        total_dirs: 1,
        import_graph: vec![
            ImportEdge {
                from_file: "src/store/store.ts".into(),
                to_file: "src/app/task-workflows.ts".into(),
            },
            ImportEdge {
                from_file: "src/app/task-workflows.ts".into(),
                to_file: "src/store/store.ts".into(),
            },
            ImportEdge {
                from_file: "src/store/core.ts".into(),
                to_file: "src/store/store.ts".into(),
            },
            ImportEdge {
                from_file: "src/store/store.ts".into(),
                to_file: "src/store/core.ts".into(),
            },
        ],
        call_graph: Vec::new(),
        inherit_graph: Vec::new(),
        entry_points: Vec::new(),
        exec_depth: HashMap::new(),
    };
    let mut health = empty_health_report();
    health.circular_dep_files = vec![vec![
        "src/app/task-workflows.ts".into(),
        "src/store/core.ts".into(),
        "src/store/store.ts".into(),
    ]];
    health.circular_dep_count = 1;

    let reports = build_structural_debt_reports_with_root(&root, &snapshot, &health);
    let report = reports
        .iter()
        .find(|report| report.kind == "cycle_cluster")
        .expect("cycle-cluster report");
    let best_cut = report.cut_candidates.first().expect("best cut");

    assert_eq!(best_cut.seam_kind, "guarded_app_store_boundary");
    assert_eq!(
        report.leverage_class,
        StructuralLeverageClass::ArchitectureSignal
    );
    assert!(report
        .leverage_reasons
        .iter()
        .any(|reason| reason == "mixed_cycle_pressure"));
    assert!(report
        .leverage_reasons
        .iter()
        .any(|reason| reason == "high_leverage_cycle_cut"));
    let _ = std::fs::remove_dir_all(root);
}
