use super::*;

#[test]
fn dependency_sprawl_marks_contained_extracted_owner_shell_as_local_refactor_target() {
    let root = temp_root("sentrux-structural", "extracted-owner-shell", &[]);
    write_file(
        &root,
        "src/components/TaskPanel.architecture.test.ts",
        "\
            expect(source).toContain('createTaskPanelFocusRuntime');\n\
            expect(source).toContain('createTaskPanelPreviewController');\n\
            expect(source).toContain('createTaskPanelDialogState');\n\
        ",
    );
    write_file(
        &root,
        "src/components/TaskPanel.tsx",
        "export function TaskPanel() {}\n",
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
                test_file("src/components/TaskPanel.tsx", 423, 59, 4),
                test_file("src/app/task-ports.ts", 40, 2, 3),
                test_file("src/components/CloseTaskDialog.tsx", 60, 3, 4),
                test_file("src/components/DiffViewerDialog.tsx", 70, 4, 4),
            ]),
        }),
        total_files: 4,
        total_lines: 593,
        total_dirs: 1,
        import_graph: vec![
            ImportEdge {
                from_file: "src/components/TaskPanel.tsx".into(),
                to_file: "src/app/task-ports.ts".into(),
            },
            ImportEdge {
                from_file: "src/components/TaskPanel.tsx".into(),
                to_file: "src/components/CloseTaskDialog.tsx".into(),
            },
            ImportEdge {
                from_file: "src/components/TaskPanel.tsx".into(),
                to_file: "src/components/DiffViewerDialog.tsx".into(),
            },
        ],
        call_graph: Vec::new(),
        inherit_graph: Vec::new(),
        entry_points: Vec::new(),
        exec_depth: HashMap::new(),
    };
    let mut health = empty_health_report();
    health.god_files = vec![FileMetric {
        path: "src/components/TaskPanel.tsx".into(),
        value: 28,
    }];

    let reports = build_structural_debt_reports_with_root(&root, &snapshot, &health);
    let report = reports
        .iter()
        .find(|report| report.scope == "src/components/TaskPanel.tsx")
        .expect("task-panel dependency-sprawl report");

    assert!(report.role_tags.iter().any(|tag| tag == "guarded_seam"));
    assert!(report
        .role_tags
        .iter()
        .any(|tag| tag == "facade_with_extracted_owners"));
    assert_eq!(
        report.leverage_class,
        StructuralLeverageClass::LocalRefactorTarget
    );
    assert!(report
        .leverage_reasons
        .iter()
        .any(|reason| reason == "extracted_owner_shell_pressure"));
    assert!(report
        .leverage_reasons
        .iter()
        .any(|reason| reason == "guardrail_backed_refactor_surface"));
    assert!(report
        .leverage_reasons
        .iter()
        .any(|reason| reason == "contained_refactor_surface"));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn dependency_sprawl_reports_guarded_boundary_role_from_architecture_test_literal() {
    let root = temp_root("sentrux-structural", "guarded-boundary", &[]);
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
                test_file("src/components/App.tsx", 120, 3, 8),
                test_file("src/components/TaskPanel.tsx", 120, 3, 8),
            ]),
        }),
        total_files: 3,
        total_lines: 460,
        total_dirs: 1,
        import_graph: vec![
            ImportEdge {
                from_file: "src/store/store.ts".into(),
                to_file: "src/components/App.tsx".into(),
            },
            ImportEdge {
                from_file: "src/store/store.ts".into(),
                to_file: "src/components/TaskPanel.tsx".into(),
            },
        ],
        call_graph: Vec::new(),
        inherit_graph: Vec::new(),
        entry_points: Vec::new(),
        exec_depth: HashMap::new(),
    };
    let mut health = empty_health_report();
    health.god_files = vec![FileMetric {
        path: "src/store/store.ts".into(),
        value: 17,
    }];

    let reports = build_structural_debt_reports_with_root(&root, &snapshot, &health);
    let report = reports
        .iter()
        .find(|report| report.kind == "dependency_sprawl")
        .expect("dependency-sprawl report");

    assert!(report.role_tags.iter().any(|tag| tag == "component_barrel"));
    assert!(report.role_tags.iter().any(|tag| tag == "guarded_boundary"));
    assert_eq!(
        report.leverage_class,
        StructuralLeverageClass::ArchitectureSignal
    );
    assert!(report
        .leverage_reasons
        .iter()
        .any(|reason| reason == "shared_barrel_boundary_hub"));
    assert!(report
        .leverage_reasons
        .iter()
        .any(|reason| reason == "guardrail_backed_boundary_pressure"));
    assert!(report.summary.contains("Component-facing barrel"));
    assert!(report
        .evidence
        .iter()
        .any(|entry| entry.contains("guarded boundary literals:")));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn dependency_sprawl_softens_direct_entry_composition_root() {
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
                test_file("src/main.tsx", 40, 1, 1),
                test_file("src/App.tsx", 260, 8, 24),
                test_file("src/components/app-shell/Chrome.tsx", 80, 2, 5),
                test_file("src/app/desktop-session.ts", 80, 2, 5),
            ]),
        }),
        total_files: 4,
        total_lines: 460,
        total_dirs: 1,
        import_graph: vec![
            ImportEdge {
                from_file: "src/main.tsx".into(),
                to_file: "src/App.tsx".into(),
            },
            ImportEdge {
                from_file: "src/App.tsx".into(),
                to_file: "src/components/app-shell/Chrome.tsx".into(),
            },
            ImportEdge {
                from_file: "src/App.tsx".into(),
                to_file: "src/app/desktop-session.ts".into(),
            },
        ],
        call_graph: Vec::new(),
        inherit_graph: Vec::new(),
        entry_points: vec![EntryPoint {
            file: "src/main.tsx".into(),
            func: "bootstrap".into(),
            lang: "typescript".into(),
            confidence: "high".into(),
        }],
        exec_depth: HashMap::new(),
    };
    let mut health = empty_health_report();
    health.god_files = vec![FileMetric {
        path: "src/App.tsx".into(),
        value: 22,
    }];

    let reports = build_structural_debt_reports(&snapshot, &health);
    let report = reports
        .iter()
        .find(|report| report.kind == "dependency_sprawl")
        .expect("dependency-sprawl report");

    assert!(report.role_tags.iter().any(|tag| tag == "composition_root"));
    assert_eq!(
        report.leverage_class,
        StructuralLeverageClass::RegrowthWatchpoint
    );
    assert!(report
        .leverage_reasons
        .iter()
        .any(|reason| reason == "intentionally_central_surface"));
    assert!(report.summary.contains("Composition root"));
    assert!(report.impact.contains("composition root"));
}

#[test]
fn dependency_sprawl_softens_direct_index_import_when_entry_points_are_missing() {
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
                test_file("src/index.tsx", 40, 1, 1),
                test_file("src/App.tsx", 260, 8, 24),
                test_file("src/components/app-shell/Chrome.tsx", 80, 2, 5),
            ]),
        }),
        total_files: 3,
        total_lines: 380,
        total_dirs: 1,
        import_graph: vec![
            ImportEdge {
                from_file: "src/index.tsx".into(),
                to_file: "src/App.tsx".into(),
            },
            ImportEdge {
                from_file: "src/App.tsx".into(),
                to_file: "src/components/app-shell/Chrome.tsx".into(),
            },
        ],
        call_graph: Vec::new(),
        inherit_graph: Vec::new(),
        entry_points: Vec::new(),
        exec_depth: HashMap::new(),
    };
    let mut health = empty_health_report();
    health.god_files = vec![FileMetric {
        path: "src/App.tsx".into(),
        value: 18,
    }];

    let reports = build_structural_debt_reports(&snapshot, &health);
    let report = reports
        .iter()
        .find(|report| report.kind == "dependency_sprawl")
        .expect("dependency-sprawl report");

    assert!(report.role_tags.iter().any(|tag| tag == "composition_root"));
    assert!(report.summary.contains("Composition root"));
}

#[test]
fn dependency_sprawl_marks_nextjs_route_surfaces_as_regrowth_watchpoints() {
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
            children: Some(vec![test_file("src/app/[locale]/layout.tsx", 220, 6, 12)]),
        }),
        total_files: 1,
        total_lines: 220,
        total_dirs: 1,
        import_graph: Vec::new(),
        call_graph: Vec::new(),
        inherit_graph: Vec::new(),
        entry_points: Vec::new(),
        exec_depth: HashMap::new(),
    };
    let mut health = empty_health_report();
    health.god_files = vec![FileMetric {
        path: "src/app/[locale]/layout.tsx".into(),
        value: 18,
    }];

    let reports = build_structural_debt_reports(&snapshot, &health);
    let report = reports
        .iter()
        .find(|report| report.scope == "src/app/[locale]/layout.tsx")
        .expect("route surface report");

    assert!(report.role_tags.iter().any(|tag| tag == "route_surface"));
    assert_eq!(
        report.leverage_class,
        StructuralLeverageClass::RegrowthWatchpoint
    );
}

#[test]
fn dependency_sprawl_marks_service_http_surfaces_as_entry_surfaces() {
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
            children: Some(vec![test_file("src/routes/users.ts", 180, 4, 11)]),
        }),
        total_files: 1,
        total_lines: 180,
        total_dirs: 1,
        import_graph: Vec::new(),
        call_graph: Vec::new(),
        inherit_graph: Vec::new(),
        entry_points: Vec::new(),
        exec_depth: HashMap::new(),
    };
    let mut health = empty_health_report();
    health.god_files = vec![FileMetric {
        path: "src/routes/users.ts".into(),
        value: 15,
    }];

    let reports = build_structural_debt_reports(&snapshot, &health);
    let report = reports
        .iter()
        .find(|report| report.scope == "src/routes/users.ts")
        .expect("http handler report");

    assert!(report
        .role_tags
        .iter()
        .any(|tag| tag == "http_handler_surface"));
    assert!(report.role_tags.iter().any(|tag| tag == "entry_surface"));
}
