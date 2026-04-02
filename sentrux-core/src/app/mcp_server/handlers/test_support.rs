#![allow(dead_code)]

use crate::analysis::semantic::{
    ClosedDomain, ExhaustivenessProofKind, ExhaustivenessSite, ExhaustivenessSiteKind,
    ProjectModel, ReadFact, SemanticCapability, SemanticSnapshot, SymbolFact, WriteFact,
};
use crate::app::mcp_server::McpState;
use crate::license::Tier;
use std::path::Path;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};
pub(crate) fn temp_root(label: &str) -> std::path::PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root =
        std::env::temp_dir().join(format!("sentrux-{label}-{}-{unique}", std::process::id()));
    std::fs::create_dir_all(root.join(".sentrux")).expect("create temp sentrux dir");
    root
}

pub(crate) fn write_file(root: &Path, relative_path: &str, contents: &str) {
    let absolute_path = root.join(relative_path);
    if let Some(parent) = absolute_path.parent() {
        std::fs::create_dir_all(parent).expect("create parent directories");
    }
    std::fs::write(&absolute_path, contents).expect("write file");
}

pub(crate) fn append_file(root: &Path, relative_path: &str, contents: &str) {
    use std::io::Write;

    let absolute_path = root.join(relative_path);
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open(&absolute_path)
        .expect("open file for append");
    file.write_all(contents.as_bytes()).expect("append file");
}

pub(crate) fn write_session_clone_fixture_files(root: &Path) {
    write_file(
        root,
        "src/source.ts",
        "export function buildAccessUrl(host: string, port: number, token: string): string {\n  return `http://${host}:${port}?token=${token}`;\n}\n\nexport function buildOptionalAccessUrl(\n  host: string | null,\n  port: number,\n  token: string,\n): string | null {\n  if (!host) return null;\n  return buildAccessUrl(host, port, token);\n}\n",
    );
    write_file(
        root,
        "src/copy.ts",
        "export function buildTaskLabel(status: string): string {\n  return status === 'done' ? 'done' : 'todo';\n}\n",
    );
}

pub(crate) fn write_session_clone_duplicate(root: &Path) {
    write_file(
        root,
        "src/copy.ts",
        "export function buildAccessUrl(host: string, port: number, token: string): string {\n  return `http://${host}:${port}?token=${token}`;\n}\n\nexport function buildOptionalAccessUrl(\n  host: string | null,\n  port: number,\n  token: string,\n): string | null {\n  if (!host) return null;\n  return buildAccessUrl(host, port, token);\n}\n",
    );
}

pub(crate) fn run_git(root: &Path, args: &[&str]) {
    let status = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .status()
        .expect("run git command");
    assert!(status.success(), "git {:?} failed", args);
}

pub(crate) fn init_git_repo(root: &Path) {
    run_git(root, &["init"]);
    run_git(root, &["config", "user.email", "test@example.com"]);
    run_git(root, &["config", "user.name", "Sentrux Test"]);
}

pub(crate) fn commit_all(root: &Path, message: &str) {
    run_git(root, &["add", "."]);
    run_git(root, &["commit", "-m", message]);
}

pub(crate) fn concept_fixture_root() -> std::path::PathBuf {
    let root = temp_root("concept-tools");
    write_file(
        &root,
        ".sentrux/rules.toml",
        r#"
                [[concept]]
                id = "task_git_status"
                kind = "authoritative_state"
                anchors = ["src/store/core.ts::store.taskGitStatus"]
                authoritative_inputs = ["src/domain/task-state.ts::TaskState"]
                allowed_writers = ["src/app/git-status-sync.ts::*"]
                forbid_writers = ["src/store/git-status-polling.ts::*"]
                canonical_accessors = ["src/app/task-presentation.ts::getTaskStatus"]
                forbid_raw_reads = ["src/components/**::store.taskGitStatus"]
                related_tests = ["src/app/task-presentation.test.ts"]

                [[contract]]
                id = "server_state_bootstrap"
                kind = "bootstrap"
                categories_symbol = "src/domain/task-state.ts::TaskState"
                registry_symbol = "src/app/task-presentation.ts::TaskStateRegistry"
                browser_entry = "src/runtime/browser-session.ts"
                required_capabilities = ["snapshot", "live_updates", "versioning"]
            "#,
    );
    write_file(
        &root,
        "src/domain/task-state.ts",
        "export type TaskState = 'idle' | 'running' | 'error';\n",
    );
    write_file(
        &root,
        "src/store/core.ts",
        "export const store = { taskGitStatus: 'idle' as TaskState };\n",
    );
    write_file(
        &root,
        "src/app/git-status-sync.ts",
        "export function syncTaskState(): void {}\n",
    );
    write_file(
        &root,
        "src/store/git-status-polling.ts",
        "export function pollTaskState(): void {}\n",
    );
    write_file(
        &root,
        "src/components/TaskRow.tsx",
        "export function TaskRow(): null { return null; }\n",
    );
    write_file(
            &root,
            "src/app/task-presentation.ts",
            "export const TaskStateRegistry = { version: 1 };\nexport function getTaskStatus(): string { return 'idle'; }\n",
        );
    write_file(
        &root,
        "src/app/task-presentation.test.ts",
        "import { getTaskStatus } from './task-presentation';\nvoid getTaskStatus;\n",
    );
    write_file(
            &root,
            "src/runtime/browser-session.ts",
            "import { TaskStateRegistry } from '../app/task-presentation';\nvoid TaskStateRegistry;\nconst version = 1;\n",
        );
    root
}

pub(crate) fn structural_debt_fixture_root() -> std::path::PathBuf {
    let root = temp_root("structural-debt");
    let mut large_file = String::from("import { alpha } from './a';\nimport { beta } from './b';\nexport function render(): number {\n  return alpha() + beta();\n}\n");
    for index in 0..900 {
        large_file.push_str(&format!("export const item{index} = {index};\n"));
    }
    write_file(&root, "src/app.ts", &large_file);
    write_file(
        &root,
        "src/a.ts",
        "import { beta } from './b';\nexport function alpha(): number { return beta(); }\n",
    );
    write_file(
        &root,
        "src/b.ts",
        "import { alpha } from './a';\nexport function beta(): number { return alpha() + 1; }\n",
    );
    root
}

pub(crate) fn dead_island_fixture_root() -> std::path::PathBuf {
    let root = temp_root("dead-island");
    write_file(
        &root,
        "src/app.ts",
        "import { live } from './live';\nexport function render(): number { return live(); }\n",
    );
    write_file(
        &root,
        "src/live.ts",
        "export function live(): number { return 1; }\n",
    );
    write_file(
            &root,
            "src/orphan-a.ts",
            "import { orphanB } from './orphan-b';\nfunction orphanA(): number { return orphanB(); }\nexport const orphanValue = orphanA();\n",
        );
    write_file(
            &root,
            "src/orphan-b.ts",
            "import { orphanValue } from './orphan-a';\nfunction orphanB(): number { return orphanValue + 1; }\nconst orphanBValue = orphanB();\n",
        );
    root
}

pub(crate) fn dead_private_fixture_root() -> std::path::PathBuf {
    let root = temp_root("dead-private");
    write_file(
        &root,
        "src/app.ts",
        "export function render(): number { return 1; }\n",
    );
    write_file(
            &root,
            "src/stale.ts",
            "function deadAlpha(): number { return 1; }\nfunction deadBeta(): number { return 2; }\nexport const liveValue = 3;\n",
        );
    root
}

pub(crate) fn experimental_gate_fixture_root() -> std::path::PathBuf {
    let root = temp_root("experimental-gate");
    write_file(
        &root,
        "src/app.ts",
        "export function render(): number { return 1; }\n",
    );
    root
}

pub(crate) fn cli_gate_fixture_root() -> std::path::PathBuf {
    let root = temp_root("cli-v2-gate");
    write_file(
        &root,
        ".sentrux/rules.toml",
        r#"
                [[concept]]
                id = "app_state"
                anchors = ["src/domain/state.ts::AppState"]
            "#,
    );
    write_file(
        &root,
        "package.json",
        r#"{ "name": "cli-gate-fixture", "type": "module" }"#,
    );
    write_file(
        &root,
        "tsconfig.json",
        r#"
                {
                  "compilerOptions": {
                    "module": "esnext",
                    "target": "es2020",
                    "strict": true
                  },
                  "include": ["src/**/*.ts"]
                }
            "#,
    );
    write_file(
        &root,
        "src/domain/state.ts",
        "export type AppState = 'idle' | 'busy';\n",
    );
    root
}

pub(crate) fn closed_domain_gate_fixture_root() -> std::path::PathBuf {
    let root = temp_root("closed-domain-gate");
    write_file(
        &root,
        ".sentrux/rules.toml",
        r#"
                [[concept]]
                id = "app_state"
                anchors = ["src/domain/state.ts::AppState"]
            "#,
    );
    write_file(
        &root,
        "package.json",
        r#"{ "name": "closed-domain-gate-fixture", "type": "module" }"#,
    );
    write_file(
        &root,
        "tsconfig.json",
        r#"
                {
                  "compilerOptions": {
                    "module": "esnext",
                    "target": "es2020",
                    "strict": true
                  },
                  "include": ["src/**/*.ts"]
                }
            "#,
    );
    write_file(
        &root,
        "src/domain/state.ts",
        "export type AppState = 'idle' | 'busy';\n",
    );
    write_file(
        &root,
        "src/app/render.ts",
        r#"
                import type { AppState } from '../domain/state';

                function assertNever(value: never): never {
                  throw new Error(String(value));
                }

                export function renderState(state: AppState): string {
                  switch (state) {
                    case 'idle':
                      return 'idle';
                    case 'busy':
                      return 'busy';
                    default:
                      return assertNever(state);
                  }
                }
            "#,
    );
    root
}

pub(crate) fn contract_gate_fixture_root() -> std::path::PathBuf {
    let root = temp_root("contract-gate");
    write_file(
        &root,
        ".sentrux/rules.toml",
        r#"
                [[contract]]
                id = "server_state_bootstrap"
                categories_symbol = "src/domain/bootstrap.ts::BOOTSTRAP_CATEGORIES"
                payload_map_symbol = "src/domain/bootstrap.ts::BootstrapPayloadMap"
                registry_symbol = "src/app/bootstrap-registry.ts::BOOTSTRAP_REGISTRY"
                browser_entry = "src/runtime/browser-session.ts"
                electron_entry = "src/app/desktop-session.ts"
            "#,
    );
    write_file(
        &root,
        "package.json",
        r#"{ "name": "contract-gate-fixture", "type": "module" }"#,
    );
    write_file(
        &root,
        "tsconfig.json",
        r#"
                {
                  "compilerOptions": {
                    "module": "esnext",
                    "target": "es2020",
                    "strict": true
                  },
                  "include": ["src/**/*.ts"]
                }
            "#,
    );
    write_file(
        &root,
        "src/domain/bootstrap.ts",
        r#"
                export const BOOTSTRAP_CATEGORIES = ['tasks'] as const;
                export type BootstrapCategory = (typeof BOOTSTRAP_CATEGORIES)[number];
                export type BootstrapPayloadMap = {
                  tasks: { count: number };
                };
            "#,
    );
    write_file(
        &root,
        "src/app/bootstrap-registry.ts",
        r#"
                import type { BootstrapPayloadMap } from '../domain/bootstrap';

                export const BOOTSTRAP_REGISTRY: Record<keyof BootstrapPayloadMap, string> = {
                  tasks: 'tasks',
                };
            "#,
    );
    write_file(
        &root,
        "src/runtime/browser-session.ts",
        r#"
                import { BOOTSTRAP_REGISTRY } from '../app/bootstrap-registry';

                export function startBrowserSession(): number {
                  return Object.keys(BOOTSTRAP_REGISTRY).length;
                }
            "#,
    );
    write_file(
        &root,
        "src/app/desktop-session.ts",
        r#"
                import { BOOTSTRAP_REGISTRY } from './bootstrap-registry';

                export function startDesktopSession(): number {
                  return Object.keys(BOOTSTRAP_REGISTRY).length;
                }
            "#,
    );
    root
}

pub(crate) fn concept_fixture_semantic(root: &Path) -> SemanticSnapshot {
    SemanticSnapshot {
        project: ProjectModel {
            root: root.to_string_lossy().to_string(),
            tsconfig_paths: vec!["tsconfig.json".to_string()],
            workspace_files: vec!["package.json".to_string()],
            primary_language: Some("typescript".to_string()),
            fingerprint: "fixture".to_string(),
            repo_archetype: None,
            detected_archetypes: Vec::new(),
        },
        analyzed_files: 6,
        capabilities: vec![
            SemanticCapability::Symbols,
            SemanticCapability::Reads,
            SemanticCapability::Writes,
            SemanticCapability::ClosedDomains,
            SemanticCapability::ClosedDomainSites,
        ],
        files: Vec::new(),
        symbols: vec![
            SymbolFact {
                id: "task-state".to_string(),
                path: "src/domain/task-state.ts".to_string(),
                name: "TaskState".to_string(),
                kind: "type_alias".to_string(),
                line: 1,
            },
            SymbolFact {
                id: "task-git-status".to_string(),
                path: "src/store/core.ts".to_string(),
                name: "store.taskGitStatus".to_string(),
                kind: "property".to_string(),
                line: 1,
            },
            SymbolFact {
                id: "registry".to_string(),
                path: "src/app/task-presentation.ts".to_string(),
                name: "TaskStateRegistry".to_string(),
                kind: "const".to_string(),
                line: 1,
            },
        ],
        reads: vec![ReadFact {
            path: "src/components/TaskRow.tsx".to_string(),
            symbol_name: "store.taskGitStatus".to_string(),
            read_kind: "property_access".to_string(),
            line: 6,
        }],
        writes: vec![
            WriteFact {
                path: "src/app/git-status-sync.ts".to_string(),
                symbol_name: "store.taskGitStatus".to_string(),
                write_kind: "store_call".to_string(),
                line: 4,
            },
            WriteFact {
                path: "src/store/git-status-polling.ts".to_string(),
                symbol_name: "store.taskGitStatus".to_string(),
                write_kind: "store_call".to_string(),
                line: 8,
            },
        ],
        closed_domains: vec![ClosedDomain {
            path: "src/domain/task-state.ts".to_string(),
            symbol_name: "TaskState".to_string(),
            variants: vec![
                "idle".to_string(),
                "running".to_string(),
                "error".to_string(),
            ],
            line: 1,
            defining_file: Some("src/domain/task-state.ts".to_string()),
        }],
        closed_domain_sites: vec![ExhaustivenessSite {
            path: "src/app/task-presentation.ts".to_string(),
            domain_symbol_name: "TaskState".to_string(),
            defining_file: Some("src/domain/task-state.ts".to_string()),
            site_kind: ExhaustivenessSiteKind::Switch,
            proof_kind: ExhaustivenessProofKind::Switch,
            covered_variants: vec!["idle".to_string(), "running".to_string()],
            line: 12,
        }],
        transition_sites: Vec::new(),
    }
}

pub(crate) fn state_fixture_root() -> std::path::PathBuf {
    let root = temp_root("state-tool");
    write_file(
        &root,
        ".sentrux/rules.toml",
        r#"
                [[concept]]
                id = "browser_sync_state"
                anchors = ["src/domain/browser-sync-state.ts::BrowserSyncState"]

                [[state_model]]
                id = "browser_state_sync"
                roots = ["src/runtime/browser-state-sync-controller.ts"]
                require_exhaustive_switch = true
                require_assert_never = true
            "#,
    );
    write_file(
        &root,
        "src/runtime/browser-state-sync-controller.ts",
        "export function renderState(state: BrowserSyncState): string { return state; }\n",
    );
    root
}

pub(crate) fn state_fixture_semantic(root: &Path) -> SemanticSnapshot {
    SemanticSnapshot {
        project: ProjectModel {
            root: root.to_string_lossy().to_string(),
            tsconfig_paths: vec!["tsconfig.json".to_string()],
            workspace_files: Vec::new(),
            primary_language: Some("typescript".to_string()),
            fingerprint: "state-fixture".to_string(),
            repo_archetype: None,
            detected_archetypes: Vec::new(),
        },
        analyzed_files: 2,
        capabilities: vec![
            SemanticCapability::ClosedDomains,
            SemanticCapability::ClosedDomainSites,
            SemanticCapability::TransitionSites,
        ],
        files: Vec::new(),
        symbols: Vec::new(),
        reads: Vec::new(),
        writes: Vec::new(),
        closed_domains: vec![ClosedDomain {
            path: "src/domain/browser-sync-state.ts".to_string(),
            symbol_name: "BrowserSyncState".to_string(),
            variants: vec![
                "idle".to_string(),
                "running".to_string(),
                "error".to_string(),
            ],
            line: 1,
            defining_file: Some("src/domain/browser-sync-state.ts".to_string()),
        }],
        closed_domain_sites: vec![ExhaustivenessSite {
            path: "src/runtime/browser-state-sync-controller.ts".to_string(),
            domain_symbol_name: "BrowserSyncState".to_string(),
            defining_file: Some("src/domain/browser-sync-state.ts".to_string()),
            site_kind: ExhaustivenessSiteKind::Switch,
            proof_kind: ExhaustivenessProofKind::Switch,
            covered_variants: vec!["idle".to_string(), "running".to_string()],
            line: 6,
        }],
        transition_sites: Vec::new(),
    }
}

pub(crate) fn state_with_semantic(root: &Path, semantic: SemanticSnapshot) -> McpState {
    McpState {
        tier: Tier::Free,
        scan_root: Some(root.to_path_buf()),
        cached_snapshot: None,
        cached_scan_metadata: None,
        cached_semantic: Some(semantic),
        cached_semantic_identity: None,
        cached_semantic_source: None,
        cached_health: None,
        cached_arch: None,
        baseline: None,
        session_v2: None,
        cached_evolution: None,
        cached_scan_identity: None,
        cached_project_shape: None,
        cached_project_shape_identity: None,
        cached_rules_identity: None,
        cached_rules_config: None,
        cached_rules_error: None,
        cached_patch_safety: None,
        semantic_bridge: None,
        agent_session: crate::app::mcp_server::session_telemetry::AgentSessionState::new(),
    }
}
