use super::scoring::cycle_cluster_score;
use super::*;
use crate::core::types::{CallEdge, EntryPoint, FileNode, ImportEdge, StructuralAnalysis};
use crate::metrics::root_causes::{RootCauseRaw, RootCauseScores};
use crate::metrics::{FileMetric, FuncMetric};
use crate::test_support::temp_root;
use std::collections::HashMap;
use std::sync::Arc;

#[test]
fn reports_large_files_sprawl_hotspots_cycles_and_dead_private_clusters() {
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
                test_file("src/app.ts", 720, 6, 28),
                test_file("src/a.ts", 120, 3, 12),
                test_file("src/b.ts", 120, 3, 16),
                test_file("src/unused.ts", 110, 5, 8),
            ]),
        }),
        total_files: 4,
        total_lines: 1070,
        total_dirs: 1,
        import_graph: vec![
            ImportEdge {
                from_file: "src/app.ts".into(),
                to_file: "src/a.ts".into(),
            },
            ImportEdge {
                from_file: "src/a.ts".into(),
                to_file: "src/b.ts".into(),
            },
            ImportEdge {
                from_file: "src/b.ts".into(),
                to_file: "src/a.ts".into(),
            },
        ],
        call_graph: vec![
            CallEdge {
                from_file: "src/app.ts".into(),
                from_func: "main".into(),
                to_file: "src/a.ts".into(),
                to_func: "helper".into(),
            },
            CallEdge {
                from_file: "src/app.ts".into(),
                from_func: "main".into(),
                to_file: "src/b.ts".into(),
                to_func: "helper".into(),
            },
        ],
        inherit_graph: Vec::new(),
        entry_points: vec![EntryPoint {
            file: "src/app.ts".into(),
            func: "main".into(),
            lang: "typescript".into(),
            confidence: "high".into(),
        }],
        exec_depth: HashMap::new(),
    };
    let health = HealthReport {
        coupling_score: 0.0,
        circular_dep_count: 1,
        circular_dep_files: vec![vec!["src/a.ts".into(), "src/b.ts".into()]],
        total_import_edges: 3,
        cross_module_edges: 0,
        entropy: 0.0,
        entropy_bits: 0.0,
        avg_cohesion: None,
        max_depth: 2,
        god_files: vec![FileMetric {
            path: "src/app.ts".into(),
            value: 8,
        }],
        hotspot_files: vec![FileMetric {
            path: "src/a.ts".into(),
            value: 4,
        }],
        most_unstable: Vec::new(),
        complex_functions: Vec::new(),
        long_functions: Vec::new(),
        cog_complex_functions: Vec::new(),
        high_param_functions: Vec::new(),
        duplicate_groups: Vec::new(),
        dead_functions: vec![
            FuncMetric {
                file: "src/unused.ts".into(),
                func: "orphanAlpha".into(),
                value: 24,
            },
            FuncMetric {
                file: "src/unused.ts".into(),
                func: "orphanBeta".into(),
                value: 20,
            },
        ],
        long_files: vec![FileMetric {
            path: "src/app.ts".into(),
            value: 720,
        }],
        all_function_ccs: Vec::new(),
        all_function_lines: Vec::new(),
        all_file_lines: Vec::new(),
        god_file_ratio: 0.0,
        hotspot_ratio: 0.0,
        complex_fn_ratio: 0.0,
        long_fn_ratio: 0.0,
        comment_ratio: None,
        large_file_count: 1,
        large_file_ratio: 0.0,
        duplication_ratio: 0.0,
        dead_code_ratio: 0.0,
        high_param_ratio: 0.0,
        cog_complex_ratio: 0.0,
        quality_signal: 0.0,
        root_cause_raw: RootCauseRaw {
            modularity_q: 0.0,
            cycle_count: 1,
            max_depth: 2,
            complexity_gini: 0.0,
            redundancy_ratio: 0.0,
        },
        root_cause_scores: RootCauseScores {
            modularity: 0.0,
            acyclicity: 0.0,
            depth: 0.0,
            equality: 0.0,
            redundancy: 0.0,
        },
    };

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
                test_file("src/orphan-a.ts", 90, 2, 6),
                test_file("src/orphan-b.ts", 95, 2, 7),
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
                from_file: "src/orphan-a.ts".into(),
                to_file: "src/orphan-b.ts".into(),
            },
            ImportEdge {
                from_file: "src/orphan-b.ts".into(),
                to_file: "src/orphan-a.ts".into(),
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
    let health = HealthReport {
        coupling_score: 0.0,
        circular_dep_count: 1,
        circular_dep_files: vec![vec!["src/orphan-a.ts".into(), "src/orphan-b.ts".into()]],
        total_import_edges: 3,
        cross_module_edges: 0,
        entropy: 0.0,
        entropy_bits: 0.0,
        avg_cohesion: None,
        max_depth: 1,
        god_files: Vec::new(),
        hotspot_files: Vec::new(),
        most_unstable: Vec::new(),
        complex_functions: Vec::new(),
        long_functions: Vec::new(),
        cog_complex_functions: Vec::new(),
        high_param_functions: Vec::new(),
        duplicate_groups: Vec::new(),
        dead_functions: Vec::new(),
        long_files: Vec::new(),
        all_function_ccs: Vec::new(),
        all_function_lines: Vec::new(),
        all_file_lines: Vec::new(),
        god_file_ratio: 0.0,
        hotspot_ratio: 0.0,
        complex_fn_ratio: 0.0,
        long_fn_ratio: 0.0,
        comment_ratio: None,
        large_file_count: 0,
        large_file_ratio: 0.0,
        duplication_ratio: 0.0,
        dead_code_ratio: 0.0,
        high_param_ratio: 0.0,
        cog_complex_ratio: 0.0,
        quality_signal: 0.0,
        root_cause_raw: RootCauseRaw {
            modularity_q: 0.0,
            cycle_count: 1,
            max_depth: 1,
            complexity_gini: 0.0,
            redundancy_ratio: 0.0,
        },
        root_cause_scores: RootCauseScores {
            modularity: 0.0,
            acyclicity: 0.0,
            depth: 0.0,
            equality: 0.0,
            redundancy: 0.0,
        },
    };

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
    assert!(report
        .candidate_split_axes
        .iter()
        .any(|axis| axis == "facade owner boundary"));
    let _ = std::fs::remove_dir_all(root);
}

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

fn test_file(path: &str, lines: u32, funcs: u32, max_complexity: u32) -> FileNode {
    FileNode {
        path: path.to_string(),
        name: path.rsplit('/').next().unwrap_or(path).to_string(),
        is_dir: false,
        lines,
        logic: lines.saturating_sub(10),
        comments: 5,
        blanks: 5,
        funcs,
        mtime: 0.0,
        gs: String::new(),
        lang: "typescript".to_string(),
        sa: Some(StructuralAnalysis {
            functions: Some(vec![crate::core::types::FuncInfo {
                n: "main".to_string(),
                sl: 1,
                el: lines,
                ln: lines,
                cc: Some(max_complexity),
                cog: Some(max_complexity),
                pc: Some(0),
                bh: Some(1),
                d: None,
                co: None,
                same_file_ref_count: None,
                is_public: false,
                is_method: false,
            }]),
            cls: None,
            imp: None,
            co: None,
            tags: None,
            comment_lines: None,
        }),
        children: None,
    }
}

fn empty_health_report() -> HealthReport {
    HealthReport {
        coupling_score: 0.0,
        circular_dep_count: 0,
        circular_dep_files: Vec::new(),
        total_import_edges: 0,
        cross_module_edges: 0,
        entropy: 0.0,
        entropy_bits: 0.0,
        avg_cohesion: None,
        max_depth: 0,
        god_files: Vec::new(),
        hotspot_files: Vec::new(),
        most_unstable: Vec::new(),
        complex_functions: Vec::new(),
        long_functions: Vec::new(),
        cog_complex_functions: Vec::new(),
        high_param_functions: Vec::new(),
        duplicate_groups: Vec::new(),
        dead_functions: Vec::new(),
        long_files: Vec::new(),
        all_function_ccs: Vec::new(),
        all_function_lines: Vec::new(),
        all_file_lines: Vec::new(),
        god_file_ratio: 0.0,
        hotspot_ratio: 0.0,
        complex_fn_ratio: 0.0,
        long_fn_ratio: 0.0,
        comment_ratio: None,
        large_file_count: 0,
        large_file_ratio: 0.0,
        duplication_ratio: 0.0,
        dead_code_ratio: 0.0,
        high_param_ratio: 0.0,
        cog_complex_ratio: 0.0,
        quality_signal: 0.0,
        root_cause_raw: RootCauseRaw {
            modularity_q: 0.0,
            cycle_count: 0,
            max_depth: 0,
            complexity_gini: 0.0,
            redundancy_ratio: 0.0,
        },
        root_cause_scores: RootCauseScores {
            modularity: 0.0,
            acyclicity: 0.0,
            depth: 0.0,
            equality: 0.0,
            redundancy: 0.0,
        },
    }
}

fn write_file(root: &Path, relative_path: &str, contents: &str) {
    let absolute_path = root.join(relative_path);
    if let Some(parent) = absolute_path.parent() {
        std::fs::create_dir_all(parent).expect("create parent");
    }
    std::fs::write(&absolute_path, contents).expect("write file");
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
