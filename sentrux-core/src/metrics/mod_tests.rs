#[cfg(test)]
mod tests {
    use crate::analysis::parser::parse_bytes;
    use crate::core::snapshot::Snapshot;
    use crate::core::types::{FileNode, FuncInfo, StructuralAnalysis};
    use crate::metrics::*;

    use crate::metrics::test_helpers::{edge, file, snap_with_edges};
    use std::sync::Arc;

    fn parsed_file(path: &str, lang: &str, code: &str) -> FileNode {
        let sa = parse_bytes(code.as_bytes(), lang).expect("parse failed");
        let lines = code.lines().count() as u32;
        let name = path.rsplit('/').next().unwrap_or(path).to_string();
        let funcs = sa
            .functions
            .as_ref()
            .map(|functions| functions.len())
            .unwrap_or(0) as u32;

        FileNode {
            path: path.to_string(),
            name,
            is_dir: false,
            lines,
            logic: lines,
            comments: 0,
            blanks: 0,
            funcs,
            mtime: 0.0,
            gs: String::new(),
            lang: lang.to_string(),
            sa: Some(sa),
            children: None,
        }
    }

    fn snapshot_with_parsed_files(files: Vec<FileNode>) -> Snapshot {
        let total_files = files.len() as u32;
        let total_lines = files.iter().map(|file| file.lines).sum::<u32>();
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
                children: Some(files.clone()),
            }),
            total_files,
            total_lines,
            total_dirs: 1,
            import_graph: Vec::new(),
            call_graph: Vec::new(),
            inherit_graph: Vec::new(),
            entry_points: Vec::new(),
            exec_depth: std::collections::HashMap::new(),
        }
    }

    // ── Boundary test: empty graph → grade A, no issues ──
    #[test]
    fn empty_graph_is_healthy() {
        let snap = snap_with_edges(Vec::new(), Vec::new());
        let report = compute_health(&snap);
        assert!(
            report.quality_signal > 0.8,
            "empty graph should have high quality signal"
        );
        assert_eq!(report.coupling_score, 0.0);
        assert_eq!(report.circular_dep_count, 0);
        assert!(report.god_files.is_empty());
        assert!(report.hotspot_files.is_empty());
    }

    // ── Symmetry test: A→B and B→A form a cycle ──
    #[test]
    fn detects_simple_cycle() {
        let edges = vec![edge("src/a.rs", "src/b.rs"), edge("src/b.rs", "src/a.rs")];
        let snap = snap_with_edges(edges, vec![file("src/a.rs"), file("src/b.rs")]);
        let report = compute_health(&snap);
        assert_eq!(report.circular_dep_count, 1);
        assert_eq!(report.circular_dep_files[0].len(), 2);
    }

    // ── Invariance test: intra-directory edges don't increase coupling ──
    #[test]
    fn intra_directory_zero_coupling() {
        // Files in the same subdirectory under dominant dir = intra-module
        let edges = vec![
            edge("src/mod1/a.rs", "src/mod1/b.rs"),
            edge("src/mod1/b.rs", "src/mod1/c.rs"),
        ];
        let snap = snap_with_edges(
            edges,
            vec![
                file("src/mod1/a.rs"),
                file("src/mod1/b.rs"),
                file("src/mod1/c.rs"),
            ],
        );
        let report = compute_health(&snap);
        assert_eq!(report.coupling_score, 0.0);
        assert_eq!(report.cross_module_edges, 0);
    }

    // ── Flat files under dominant dir are separate modules ──
    #[test]
    fn flat_files_under_dominant_are_separate_modules() {
        // src/a.rs and src/b.rs are different modules ("src/a" vs "src/b").
        // Each file directly under the dominant dir gets its own module identity
        // (file stem). This prevents flat src/ layouts from showing 0% coupling
        // when files import each other — that IS cross-module coupling.
        let edges = vec![edge("src/a.rs", "src/b.rs")];
        let snap = snap_with_edges(edges, vec![file("src/a.rs"), file("src/b.rs")]);
        let report = compute_health(&snap);
        assert_eq!(
            report.coupling_score > 0.25,
            true,
            "flat files under dominant dir are separate modules"
        );
        assert_eq!(report.cross_module_edges, 1);
    }

    // ── Root-level files across different dirs are cross-module ──
    #[test]
    fn root_level_files_cross_dir_are_cross_module() {
        // src/a.rs and lib/b.rs are different modules ("src" vs "lib")
        let edges = vec![edge("src/a.rs", "lib/b.rs")];
        let snap = snap_with_edges(edges, vec![file("src/a.rs"), file("lib/b.rs")]);
        let report = compute_health(&snap);
        assert_eq!(
            report.coupling_score > 0.25,
            true,
            "files in different dirs are different modules"
        );
        assert_eq!(report.cross_module_edges, 1);
    }

    // ── Oracle test: 1 cross-module out of 2 edges = 0.5 coupling ──
    #[test]
    fn coupling_score_correct() {
        let edges = vec![
            edge("src/mod1/a.rs", "src/mod1/b.rs"), // same sub-module ("src/mod1")
            edge("src/mod1/a.rs", "lib/c.rs"),      // cross top-module ("src/mod1" → "lib")
        ];
        let snap = snap_with_edges(
            edges,
            vec![
                file("src/mod1/a.rs"),
                file("src/mod1/b.rs"),
                file("lib/c.rs"),
            ],
        );
        let report = compute_health(&snap);
        // Beta(1,1): 1 cross-unstable out of 2 total → (1+1)/(2+2) = 0.5
        assert!(report.coupling_score > 0.3 && report.coupling_score < 0.6);
        assert_eq!(report.cross_module_edges, 1);
    }

    // ── Adaptive module depth: depth-3 boundary detection ──
    #[test]
    fn adaptive_module_depth_single_dominant() {
        // All files under "src/" → dominant dir
        // Module boundaries: depth-2 for flat, depth-3 for nested
        // "src/commands/add.rs" → module "src/commands"
        // "src/commands/utils/helper.rs" → module "src/commands/utils" (depth-3!)
        // These are DIFFERENT modules under depth-3, so edge IS cross-module.
        // Same-depth edges (both at depth-2) within one dir are intra-module.
        let edges = vec![
            edge("src/models/user.rs", "src/models/base.rs"), // same module "src/models"
            edge("src/models/admin.rs", "src/models/base.rs"), // same module "src/models"
        ];
        let snap = snap_with_edges(
            edges,
            vec![
                file("src/models/user.rs"),
                file("src/models/base.rs"),
                file("src/models/admin.rs"),
            ],
        );
        let report = compute_health(&snap);
        assert_eq!(
            report.coupling_score, 0.0,
            "same depth-2 module should not count as cross-module"
        );
    }

    // ── Adaptive: cross second-level dirs within dominant DOES count ──
    #[test]
    fn adaptive_cross_second_level_is_coupling() {
        // src/commands → src/models = cross-module when dominant = "src"
        let edges = vec![
            edge("src/commands/add.rs", "src/models/user.rs"), // cross: commands → models
            edge("src/models/user.rs", "src/models/base.rs"),  // same: models → models
        ];
        let snap = snap_with_edges(
            edges,
            vec![
                file("src/commands/add.rs"),
                file("src/models/user.rs"),
                file("src/models/base.rs"),
            ],
        );
        let report = compute_health(&snap);
        // Beta(1,1): 1 cross-unstable out of 2 → (1+1)/(2+2) = 0.5
        assert!(
            report.coupling_score > 0.3 && report.coupling_score < 0.6,
            "1 cross-module out of 2 edges, got {}",
            report.coupling_score
        );
    }

    // ── Root-level files in dominant dir ARE cross-module with subdirs ──
    // BUG FIX: previously treated as intra-module, masking real coupling.
    // src/app.rs (module "src") → src/layout/types.rs (module "src/layout")
    // are different modules — the coupling metric should reflect this.
    #[test]
    fn root_level_file_cross_module_with_subdirs() {
        let edges = vec![edge("src/app.rs", "src/layout/types.rs")];
        let snap = snap_with_edges(edges, vec![file("src/app.rs"), file("src/layout/types.rs")]);
        let report = compute_health(&snap);
        assert_eq!(
            report.coupling_score > 0.25,
            true,
            "root-level src/app.rs importing src/layout/ IS cross-module"
        );
    }

    // ── No dominant dir: uses first level ──
    #[test]
    fn no_dominant_uses_first_level() {
        // packages/auth → packages/api = cross-module (different first-level)
        // But 50% of endpoints are "packages", 50% are "services" → no dominant
        let edges = vec![edge("packages/auth/login.rs", "services/api/handler.rs")];
        let snap = snap_with_edges(
            edges,
            vec![
                file("packages/auth/login.rs"),
                file("services/api/handler.rs"),
            ],
        );
        let report = compute_health(&snap);
        assert_eq!(
            report.coupling_score > 0.25,
            true,
            "cross first-level dirs = 100% coupling"
        );
    }

    // ── Idempotency test: computing twice gives same result ──
    #[test]
    fn idempotent() {
        let edges = vec![edge("src/a.rs", "lib/b.rs"), edge("lib/b.rs", "src/a.rs")];
        let snap = snap_with_edges(edges, vec![file("src/a.rs"), file("lib/b.rs")]);
        let r1 = compute_health(&snap);
        let r2 = compute_health(&snap);
        assert!((r1.quality_signal - r2.quality_signal).abs() < f64::EPSILON);
        assert_eq!(r1.coupling_score, r2.coupling_score);
        assert_eq!(r1.circular_dep_count, r2.circular_dep_count);
    }

    // ── Monotonicity test: more cycles = worse grade ──
    #[test]
    fn more_cycles_worse_score() {
        // 0 cycles
        let snap0 = snap_with_edges(
            vec![edge("src/a.rs", "src/b.rs")],
            vec![file("src/a.rs"), file("src/b.rs")],
        );
        let r0 = compute_health(&snap0);

        // 1 cycle
        let snap1 = snap_with_edges(
            vec![edge("src/a.rs", "src/b.rs"), edge("src/b.rs", "src/a.rs")],
            vec![file("src/a.rs"), file("src/b.rs")],
        );
        let r1 = compute_health(&snap1);

        assert!(
            r0.quality_signal >= r1.quality_signal,
            "more cycles should lower quality signal"
        );
    }

    // ── Three-node cycle detection ──
    #[test]
    fn detects_three_node_cycle() {
        let edges = vec![
            edge("src/a.rs", "src/b.rs"),
            edge("src/b.rs", "src/c.rs"),
            edge("src/c.rs", "src/a.rs"),
        ];
        let snap = snap_with_edges(
            edges,
            vec![file("src/a.rs"), file("src/b.rs"), file("src/c.rs")],
        );
        let report = compute_health(&snap);
        assert_eq!(report.circular_dep_count, 1);
        assert_eq!(report.circular_dep_files[0].len(), 3);
    }

    // ── Fan-out god file detection ──
    #[test]
    fn detects_god_file() {
        let mut edges = Vec::new();
        let mut files_vec = vec![file("src/god.rs")];
        for i in 0..20 {
            let target = format!("src/dep{}.rs", i);
            edges.push(edge("src/god.rs", &target));
            files_vec.push(file(&target));
        }
        let snap = snap_with_edges(edges, files_vec);
        let report = compute_health(&snap);
        assert_eq!(report.god_files.len(), 1);
        assert_eq!(report.god_files[0].path, "src/god.rs");
        assert_eq!(report.god_files[0].value, 20);
    }

    // ── Complex function detection ──
    #[test]
    fn detects_complex_function() {
        let f = FileNode {
            path: "src/complex.rs".to_string(),
            name: "complex.rs".to_string(),
            is_dir: false,
            lines: 200,
            logic: 150,
            comments: 20,
            blanks: 30,
            funcs: 1,
            mtime: 0.0,
            gs: String::new(),
            lang: "rust".to_string(),
            sa: Some(StructuralAnalysis {
                functions: Some(vec![FuncInfo {
                    n: "monster_func".to_string(),
                    sl: 1,
                    el: 200,
                    ln: 200,
                    cc: Some(25),
                    cog: None,
                    pc: None,
                    bh: None,
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
        };
        let snap = snap_with_edges(Vec::new(), vec![f]);
        let report = compute_health(&snap);
        assert_eq!(report.complex_functions.len(), 1);
        assert_eq!(report.complex_functions[0].func, "monster_func");
        assert_eq!(report.complex_functions[0].value, 25);
        assert_eq!(report.long_functions.len(), 1);
        assert_eq!(report.long_functions[0].value, 200);
    }

    #[test]
    fn dead_functions_ignore_same_file_value_position_references() {
        let file = FileNode {
            path: "src/view.tsx".to_string(),
            name: "view.tsx".to_string(),
            is_dir: false,
            lines: 40,
            logic: 32,
            comments: 0,
            blanks: 8,
            funcs: 2,
            mtime: 0.0,
            gs: String::new(),
            lang: "typescript".to_string(),
            sa: Some(StructuralAnalysis {
                functions: Some(vec![
                    FuncInfo {
                        n: "helper".to_string(),
                        sl: 1,
                        el: 5,
                        ln: 5,
                        cc: Some(1),
                        cog: None,
                        pc: None,
                        bh: None,
                        d: None,
                        co: None,
                        same_file_ref_count: Some(1),
                        is_public: false,
                        is_method: false,
                    },
                    FuncInfo {
                        n: "Screen".to_string(),
                        sl: 7,
                        el: 20,
                        ln: 14,
                        cc: Some(1),
                        cog: None,
                        pc: None,
                        bh: None,
                        d: None,
                        co: None,
                        same_file_ref_count: None,
                        is_public: true,
                        is_method: false,
                    },
                ]),
                cls: None,
                imp: None,
                co: None,
                tags: None,
                comment_lines: None,
            }),
            children: None,
        };
        let snap = snap_with_edges(Vec::new(), vec![file]);
        let report = compute_health(&snap);

        assert!(report.dead_functions.is_empty());
    }

    #[test]
    fn dead_functions_ignore_nested_jsx_helper_references() {
        let sa = parse_bytes(
            br#"
export function VmLimitReachedContent(): any {
    const project = { lastAccessedAt: new Date() };

    function formatLastAccessed(date: Date | null): string {
        if (!date) return 'Never';
        return 'ok';
    }

    return <p>Last accessed: {formatLastAccessed(project.lastAccessedAt)}</p>;
}
"#,
            "typescript",
        )
        .expect("tsx parse failed");
        let file = FileNode {
            path: "src/vm-limit-reached.tsx".to_string(),
            name: "vm-limit-reached.tsx".to_string(),
            is_dir: false,
            lines: 10,
            logic: 8,
            comments: 0,
            blanks: 2,
            funcs: 2,
            mtime: 0.0,
            gs: String::new(),
            lang: "typescript".to_string(),
            sa: Some(sa),
            children: None,
        };
        let snap = snap_with_edges(Vec::new(), vec![file]);
        let report = compute_health(&snap);

        assert!(report.dead_functions.is_empty());
    }

    #[test]
    fn dead_functions_ignore_typescript_overload_signatures() {
        let sa = parse_bytes(
            br#"
export function formatLastAccessed(date: Date | null): string;
export function formatLastAccessed(date: Date | null): string {
    if (!date) return 'Never';
    return 'ok';
}

export function VmLimitReachedContent(): any {
    return <p>{formatLastAccessed(new Date())}</p>;
}
"#,
            "typescript",
        )
        .expect("tsx parse failed");
        let file = FileNode {
            path: "src/vm-limit-overload.tsx".to_string(),
            name: "vm-limit-overload.tsx".to_string(),
            is_dir: false,
            lines: 9,
            logic: 7,
            comments: 0,
            blanks: 2,
            funcs: 2,
            mtime: 0.0,
            gs: String::new(),
            lang: "typescript".to_string(),
            sa: Some(sa),
            children: None,
        };
        let snap = snap_with_edges(Vec::new(), vec![file]);
        let report = compute_health(&snap);

        assert!(report.dead_functions.is_empty());
    }

    #[test]
    fn dead_functions_do_not_ignore_same_file_type_only_mentions() {
        let file = FileNode {
            path: "src/view.ts".to_string(),
            name: "view.ts".to_string(),
            is_dir: false,
            lines: 24,
            logic: 18,
            comments: 0,
            blanks: 6,
            funcs: 1,
            mtime: 0.0,
            gs: String::new(),
            lang: "typescript".to_string(),
            sa: Some(StructuralAnalysis {
                functions: Some(vec![FuncInfo {
                    n: "helper".to_string(),
                    sl: 1,
                    el: 5,
                    ln: 5,
                    cc: Some(1),
                    cog: None,
                    pc: None,
                    bh: None,
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
        };
        let snap = snap_with_edges(Vec::new(), vec![file]);
        let report = compute_health(&snap);

        assert_eq!(report.dead_functions.len(), 1);
        assert_eq!(report.dead_functions[0].func, "helper");
    }

    #[test]
    fn dead_functions_ignore_object_literal_callbacks() {
        let snapshot = snapshot_with_parsed_files(vec![parsed_file(
            "src/table.tsx",
            "typescript",
            r#"
const columns = [{
    cell: () => null,
}];
"#,
        )]);
        let report = compute_health(&snapshot);

        assert!(
            report.dead_functions.is_empty(),
            "object-literal callback keys should not be reported as dead private code"
        );
    }

    #[test]
    fn dead_functions_ignore_framework_lifecycle_methods() {
        let snapshot = snapshot_with_parsed_files(vec![parsed_file(
            "src/ErrorBoundary.tsx",
            "typescript",
            r#"
class ErrorBoundary {
    getDerivedStateFromError() {
        return null;
    }

    componentDidCatch() {}
}
"#,
        )]);
        let report = compute_health(&snapshot);

        assert!(
            report.dead_functions.is_empty(),
            "framework lifecycle methods should not be reported as dead private code"
        );
    }

    #[test]
    fn dead_functions_ignore_exported_typescript_functions() {
        let sa = parse_bytes(
            br#"
export function updateReviewComment(): void {}

export async function markCommentsSent(): Promise<void> {}
"#,
            "typescript",
        )
        .expect("ts parse failed");
        let file = FileNode {
            path: "src/review.ts".to_string(),
            name: "review.ts".to_string(),
            is_dir: false,
            lines: 6,
            logic: 4,
            comments: 0,
            blanks: 2,
            funcs: 2,
            mtime: 0.0,
            gs: String::new(),
            lang: "typescript".to_string(),
            sa: Some(sa),
            children: None,
        };
        let snap = snap_with_edges(Vec::new(), vec![file]);
        let report = compute_health(&snap);

        assert!(report.dead_functions.is_empty());
    }

    #[test]
    fn dead_functions_ignore_exported_tsx_wrapper_functions() {
        let sa = parse_bytes(
            br#"
import { Button } from '@humain-foundation/ui';

type ToastProps = {
    title: string;
};

export function ToastSuccess(props: ToastProps) {
    return <Button>{props.title}</Button>;
}

export function ToastError(props: ToastProps) {
    return <Button>{props.title}</Button>;
}
"#,
            "typescript",
        )
        .expect("tsx parse failed");
        let file = FileNode {
            path: "src/toast-notification.tsx".to_string(),
            name: "toast-notification.tsx".to_string(),
            is_dir: false,
            lines: 13,
            logic: 8,
            comments: 0,
            blanks: 5,
            funcs: 2,
            mtime: 0.0,
            gs: String::new(),
            lang: "typescript".to_string(),
            sa: Some(sa),
            children: None,
        };
        let snap = snap_with_edges(Vec::new(), vec![file]);
        let report = compute_health(&snap);

        assert!(report.dead_functions.is_empty());
    }

    // ── Shannon entropy: single cross-module pair = 0 entropy ──
    #[test]
    fn single_pair_zero_entropy() {
        // Two edges between the same module pair = single pair = entropy 0
        let edges = vec![
            edge("src/mod1/a.rs", "src/mod2/b.rs"),
            edge("src/mod1/c.rs", "src/mod2/b.rs"),
        ];
        let snap = snap_with_edges(
            edges,
            vec![
                file("src/mod1/a.rs"),
                file("src/mod1/c.rs"),
                file("src/mod2/b.rs"),
            ],
        );
        let report = compute_health(&snap);
        assert_eq!(report.entropy, 0.0);
    }
}
