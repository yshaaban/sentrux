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
