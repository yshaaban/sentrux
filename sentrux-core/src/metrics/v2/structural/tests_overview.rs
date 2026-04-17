use super::*;

#[test]
fn reports_large_files_sprawl_hotspots_cycles_and_dead_private_clusters() {
    let snapshot = sample_structural_snapshot();
    let health = sample_structural_health();

    let reports = build_structural_debt_reports(&snapshot, &health);
    let kinds = reports
        .iter()
        .map(|report| report.kind.as_str())
        .collect::<Vec<_>>();

    assert!(kinds.contains(&"large_file"));
    assert!(kinds.contains(&"dependency_sprawl"));
    assert!(kinds.contains(&"unstable_hotspot"));
    assert!(kinds.contains(&"cycle_cluster"));
    assert!(kinds.contains(&"dead_private_code_cluster"));
}

#[test]
fn reports_dead_island_for_disconnected_internal_cycle() {
    let snapshot = dead_island_snapshot();
    let health = dead_island_health();

    let reports = build_structural_debt_reports(&snapshot, &health);
    assert!(has_dead_island_report(
        &reports,
        &["src/orphan-a.ts", "src/orphan-b.ts"]
    ));
}

#[test]
fn dead_private_cluster_reports_dedupe_same_symbol_entries() {
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
            children: Some(vec![test_file("src/view.tsx", 120, 3, 8)]),
        }),
        total_files: 1,
        total_lines: 120,
        total_dirs: 1,
        import_graph: Vec::new(),
        call_graph: Vec::new(),
        inherit_graph: Vec::new(),
        entry_points: Vec::new(),
        exec_depth: HashMap::new(),
    };
    let health = HealthReport {
        dead_functions: vec![
            FuncMetric {
                file: "src/view.tsx".into(),
                func: "formatLastAccessed".into(),
                value: 10,
            },
            FuncMetric {
                file: "src/view.tsx".into(),
                func: "formatLastAccessed".into(),
                value: 3,
            },
        ],
        ..empty_health_report()
    };

    let reports = build_structural_debt_reports(&snapshot, &health);

    assert!(
        reports
            .iter()
            .all(|report| report.kind != "dead_private_code_cluster"),
        "duplicate symbol entries should not fabricate a dead-private cluster"
    );
}

#[test]
fn reports_dead_island_for_disconnected_non_cycle_component_when_entry_points_exist() {
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
                test_file("src/app.ts", 120, 2, 10),
                test_file("src/live.ts", 80, 2, 8),
                test_file("src/orphan-root.ts", 90, 2, 6),
                test_file("src/orphan-leaf.ts", 95, 2, 7),
            ]),
        }),
        total_files: 4,
        total_lines: 385,
        total_dirs: 1,
        import_graph: vec![
            ImportEdge {
                from_file: "src/app.ts".into(),
                to_file: "src/live.ts".into(),
            },
            ImportEdge {
                from_file: "src/orphan-root.ts".into(),
                to_file: "src/orphan-leaf.ts".into(),
            },
        ],
        call_graph: Vec::new(),
        inherit_graph: Vec::new(),
        entry_points: vec![EntryPoint {
            file: "src/app.ts".into(),
            func: "main".into(),
            lang: "typescript".into(),
            confidence: "high".into(),
        }],
        exec_depth: HashMap::new(),
    };
    let health = empty_health_report();

    let reports = build_structural_debt_reports(&snapshot, &health);
    assert!(has_dead_island_report(
        &reports,
        &["src/orphan-leaf.ts", "src/orphan-root.ts"]
    ));
}

#[test]
fn does_not_report_dead_island_for_zero_inbound_root_when_no_entry_points_exist() {
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
                test_file("src/root.ts", 120, 2, 10),
                test_file("src/helper.ts", 80, 2, 8),
            ]),
        }),
        total_files: 2,
        total_lines: 200,
        total_dirs: 1,
        import_graph: vec![ImportEdge {
            from_file: "src/root.ts".into(),
            to_file: "src/helper.ts".into(),
        }],
        call_graph: Vec::new(),
        inherit_graph: Vec::new(),
        entry_points: Vec::new(),
        exec_depth: HashMap::new(),
    };
    let health = empty_health_report();

    let reports = build_structural_debt_reports(&snapshot, &health);
    assert!(!reports.iter().any(|report| report.kind == "dead_island"));
}

#[test]
fn does_not_report_dead_island_for_support_only_components() {
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
                test_file("src/app.ts", 120, 2, 10),
                test_file(".github/ISSUE_TEMPLATE/config.yml", 20, 1, 1),
                test_file("plugins/gdscript/tests/sample.gd", 25, 1, 1),
            ]),
        }),
        total_files: 3,
        total_lines: 165,
        total_dirs: 1,
        import_graph: vec![
            ImportEdge {
                from_file: "src/app.ts".into(),
                to_file: "src/app.ts".into(),
            },
            ImportEdge {
                from_file: ".github/ISSUE_TEMPLATE/config.yml".into(),
                to_file: "plugins/gdscript/tests/sample.gd".into(),
            },
        ],
        call_graph: Vec::new(),
        inherit_graph: Vec::new(),
        entry_points: vec![EntryPoint {
            file: "src/app.ts".into(),
            func: "main".into(),
            lang: "typescript".into(),
            confidence: "high".into(),
        }],
        exec_depth: HashMap::new(),
    };
    let health = empty_health_report();

    let reports = build_structural_debt_reports(&snapshot, &health);

    assert!(
        !reports.iter().any(|report| report.kind == "dead_island"),
        "support-only components should not surface as dead-island debt"
    );
}

#[test]
fn structural_classification_enums_serialize_to_legacy_strings() {
    let report = StructuralDebtReport {
        kind: "large_file".to_string(),
        trust_tier: StructuralTrustTier::Trusted,
        presentation_class: StructuralPresentationClass::GuardedFacade,
        leverage_class: StructuralLeverageClass::BoundaryDiscipline,
        scope: "src/lib/ipc.ts".to_string(),
        signal_class: StructuralSignalClass::Debt,
        signal_families: vec!["coupling".to_string()],
        severity: FindingSeverity::High,
        score_0_10000: 8200,
        summary: "summary".to_string(),
        impact: "impact".to_string(),
        files: vec!["src/lib/ipc.ts".to_string()],
        role_tags: Vec::new(),
        leverage_reasons: Vec::new(),
        evidence: vec!["fan-in: 8".to_string()],
        inspection_focus: vec!["inspect boundary".to_string()],
        candidate_split_axes: Vec::new(),
        related_surfaces: Vec::new(),
        cut_candidates: Vec::new(),
        metrics: StructuralDebtMetrics::default(),
    };

    let value = serde_json::to_value(&report).expect("serialize structural debt report");

    assert_eq!(value["trust_tier"], "trusted");
    assert_eq!(value["presentation_class"], "guarded_facade");
    assert_eq!(value["leverage_class"], "boundary_discipline");
    assert_eq!(value["signal_class"], "debt");
}
