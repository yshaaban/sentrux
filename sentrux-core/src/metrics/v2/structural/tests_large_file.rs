use super::*;

#[test]
fn large_file_guarded_facade_reports_role_tags_and_guardrail_evidence() {
    let root = temp_root("sentrux-structural", "guarded-facade", &[]);
    write_file(
        &root,
        "src/components/terminal-session.architecture.test.ts",
        "\
            expect(source).toContain('createTerminalInputPipeline');\n\
            expect(source).toContain('createTerminalOutputPipeline');\n\
        ",
    );
    write_file(
        &root,
        "src/components/terminal-session.ts",
        "export function main() {}\n",
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
            children: Some(vec![test_file(
                "src/components/terminal-session.ts",
                720,
                24,
                51,
            )]),
        }),
        total_files: 1,
        total_lines: 720,
        total_dirs: 1,
        import_graph: Vec::new(),
        call_graph: Vec::new(),
        inherit_graph: Vec::new(),
        entry_points: Vec::new(),
        exec_depth: HashMap::new(),
    };
    let mut health = empty_health_report();
    health.long_files = vec![FileMetric {
        path: "src/components/terminal-session.ts".into(),
        value: 720,
    }];

    let reports = build_structural_debt_reports_with_root(&root, &snapshot, &health);
    let report = reports
        .iter()
        .find(|report| report.kind == "large_file")
        .expect("large-file report");

    assert!(report.role_tags.iter().any(|tag| tag == "guarded_seam"));
    assert!(report
        .role_tags
        .iter()
        .any(|tag| tag == "facade_with_extracted_owners"));
    assert_eq!(
        report.leverage_class,
        StructuralLeverageClass::SecondaryCleanup
    );
    assert!(report
        .leverage_reasons
        .iter()
        .any(|reason| reason == "secondary_facade_cleanup"));
    assert!(report.summary.contains("Guarded facade file"));
    assert!(report
        .evidence
        .iter()
        .any(|entry| entry.contains("guardrail tests:")));
    assert!(report.related_surfaces.is_empty());
    assert!(report
        .evidence
        .iter()
        .any(|entry| entry.contains("suggested split axes:")));
    assert!(report
        .inspection_focus
        .iter()
        .any(|entry| entry.contains("facade owner boundary")));
    assert!(report
        .candidate_split_axes
        .iter()
        .any(|axis| axis == "facade owner boundary"));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn large_file_surfaces_actionable_split_evidence_for_dependency_boundaries() {
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
                test_file("src/App.tsx", 620, 18, 31),
                test_file("src/components/app-shell/Chrome.tsx", 80, 2, 5),
                test_file("src/providers/runtime.ts", 70, 3, 4),
            ]),
        }),
        total_files: 4,
        total_lines: 810,
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
            ImportEdge {
                from_file: "src/App.tsx".into(),
                to_file: "src/providers/runtime.ts".into(),
            },
        ],
        call_graph: Vec::new(),
        inherit_graph: Vec::new(),
        entry_points: Vec::new(),
        exec_depth: HashMap::new(),
    };
    let mut health = empty_health_report();
    health.long_files = vec![FileMetric {
        path: "src/App.tsx".into(),
        value: 620,
    }];

    let reports = build_structural_debt_reports(&snapshot, &health);
    let report = reports
        .iter()
        .find(|report| report.kind == "large_file")
        .expect("large-file report");

    assert!(report.role_tags.iter().any(|tag| tag == "composition_root"));
    assert!(report
        .candidate_split_axes
        .iter()
        .any(|axis| axis == "components dependency boundary"));
    assert!(report
        .candidate_split_axes
        .iter()
        .any(|axis| axis == "providers dependency boundary"));
    assert_eq!(
        report.related_surfaces,
        vec![
            "src/components/app-shell/Chrome.tsx".to_string(),
            "src/providers/runtime.ts".to_string(),
        ]
    );
    assert!(report.evidence.iter().any(|entry| {
        entry == "related surfaces to peel out first: src/components/app-shell/Chrome.tsx, src/providers/runtime.ts"
    }));
    assert!(report.evidence.iter().any(|entry| {
        entry == "recommended first cut: move the behavior that couples to src/components/app-shell/Chrome.tsx behind the components dependency boundary"
    }));
    assert!(report.inspection_focus.iter().any(|entry| {
        entry.contains("components dependency boundary")
            && entry.contains("src/components/app-shell/Chrome.tsx")
    }));
}

#[test]
fn large_file_keeps_script_entry_surfaces_generic() {
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
            children: Some(vec![test_file("scripts/session-stress.mjs", 2048, 12, 14)]),
        }),
        total_files: 1,
        total_lines: 2048,
        total_dirs: 1,
        import_graph: Vec::new(),
        call_graph: Vec::new(),
        inherit_graph: Vec::new(),
        entry_points: vec![EntryPoint {
            file: "scripts/session-stress.mjs".into(),
            func: "main".into(),
            lang: "javascript".into(),
            confidence: "high".into(),
        }],
        exec_depth: HashMap::new(),
    };
    let mut health = empty_health_report();
    health.long_files = vec![FileMetric {
        path: "scripts/session-stress.mjs".into(),
        value: 2048,
    }];

    let reports = build_structural_debt_reports(&snapshot, &health);
    let report = reports
        .iter()
        .find(|report| report.kind == "large_file")
        .expect("large-file report");

    assert_eq!(report.leverage_class, StructuralLeverageClass::ToolingDebt);
    assert!(report
        .leverage_reasons
        .iter()
        .any(|reason| reason == "tooling_surface_maintenance_burden"));
    assert!(report
        .summary
        .starts_with("File 'scripts/session-stress.mjs'"));
    assert!(!report.impact.contains("composition root"));
}
