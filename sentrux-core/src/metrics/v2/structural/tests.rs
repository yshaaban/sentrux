use super::scoring::cycle_cluster_score;
use super::*;
use crate::core::types::{CallEdge, EntryPoint, FileNode, ImportEdge, StructuralAnalysis};
use crate::metrics::root_causes::{RootCauseRaw, RootCauseScores};
use crate::metrics::{FileMetric, FuncMetric};
use crate::test_support::temp_root;
use std::collections::HashMap;
use std::sync::Arc;

#[path = "tests_cycle_hotspot.rs"]
mod cycle_hotspot;
#[path = "tests_dependency.rs"]
mod dependency;
#[path = "tests_large_file.rs"]
mod large_file;
#[path = "tests_overview.rs"]
mod overview;

fn sample_structural_snapshot() -> Snapshot {
    Snapshot {
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
        entry_points: vec![sample_entry_point()],
        exec_depth: HashMap::new(),
    }
}

fn sample_structural_health() -> HealthReport {
    HealthReport {
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
    }
}

fn dead_island_snapshot() -> Snapshot {
    Snapshot {
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
        entry_points: vec![sample_entry_point()],
        exec_depth: HashMap::new(),
    }
}

fn dead_island_health() -> HealthReport {
    HealthReport {
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
    }
}

fn sample_entry_point() -> EntryPoint {
    EntryPoint {
        file: "src/app.ts".into(),
        func: "main".into(),
        lang: "typescript".into(),
        confidence: "high".into(),
    }
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
