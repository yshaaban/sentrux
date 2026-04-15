# Check Review Packet

- repo root: `<sentrux-root>`
- tool: `check`
- source mode: `mixed`
- source path(s):
  - `<sentrux-root>/.sentrux/evals/2026-04-11T18-16-29-511Z-repo-calibration-loop-sentrux/replay-batch/diff-replay-batch.json`
  - `<sentrux-root>/.sentrux/evals/2026-04-11T18-16-29-511Z-repo-calibration-loop-sentrux/replay-batch/weak-signal-coverage/diff-replay.json`
  - `<sentrux-root>/.sentrux/evals/2026-04-11T18-16-29-511Z-repo-calibration-loop-sentrux/replay-batch/weak-signal-guidance/diff-replay.json`
  - `<sentrux-root>/.sentrux/evals/2026-04-11T18-16-29-511Z-repo-calibration-loop-sentrux/replay-batch/session-introduced-clone-surface/diff-replay.json`
  - `<sentrux-root>/.sentrux/evals/2026-04-11T18-16-29-511Z-repo-calibration-loop-sentrux/replay-batch/clone-drift-git-aware/diff-replay.json`
  - `<sentrux-root>/.sentrux/evals/2026-04-11T18-16-29-511Z-repo-calibration-loop-sentrux/replay-batch/clone-divergence-priority/diff-replay.json`
  - `<sentrux-root>/.sentrux/evals/2026-04-11T18-16-29-511Z-repo-calibration-loop-sentrux/replay-batch/raw-read-guidance/diff-replay.json`
  - `<sentrux-root>/.sentrux/evals/2026-04-11T18-16-29-511Z-repo-calibration-loop-sentrux/replay-batch/repo-local-session-telemetry/diff-replay.json`
  - `<sentrux-root>/.sentrux/evals/2026-04-11T18-16-29-511Z-repo-calibration-loop-sentrux/replay-batch/fast-check-zero-config/diff-replay.json`
  - `<sentrux-root>/.sentrux/evals/2026-04-11T18-16-29-511Z-repo-calibration-loop-sentrux/replay-batch/contract-surface-obligations/diff-replay.json`
  - `./sentrux-defect-injection.json`
  - `./sentrux-remediation.json`
- generated at: `2026-04-12T10:26:58.140Z`
- sample count: 16
- kind counts: large_file=12, forbidden_raw_read=1, incomplete_propagation=1, session_introduced_clone=1, zero_config_boundary_violation=1

| Review ID | Kind | Source | Snapshot | Rank | Scope | Severity | Summary | Classification | Action |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `check-1` | `large_file` | `weak-signal-guidance` | `replay` | 1 | `sentrux-core/src/app/mcp_server/handlers/agent_format.rs` | `unknown` | sentrux-core/src/app/mcp_server/handlers/agent_format.rs grew to 554 lines and should likely be split. |  |  |
| `check-2` | `large_file` | `weak-signal-guidance` | `replay` | 2 | `sentrux-core/src/app/mcp_server/handlers/classification.rs` | `unknown` | sentrux-core/src/app/mcp_server/handlers/classification.rs grew to 947 lines and should likely be split. |  |  |
| `check-3` | `large_file` | `weak-signal-guidance` | `replay` | 3 | `sentrux-core/src/app/mcp_server/handlers/test_support.rs` | `unknown` | sentrux-core/src/app/mcp_server/handlers/test_support.rs grew to 638 lines and should likely be split. |  |  |
| `check-4` | `large_file` | `weak-signal-guidance` | `replay` | 4 | `sentrux-core/src/metrics/v2/mod.rs` | `unknown` | sentrux-core/src/metrics/v2/mod.rs grew to 1235 lines and should likely be split. |  |  |
| `check-5` | `large_file` | `session-introduced-clone-surface` | `replay` | 5 | `sentrux-core/src/app/mcp_server/handlers.rs` | `unknown` | sentrux-core/src/app/mcp_server/handlers.rs grew to 582 lines and should likely be split. |  |  |
| `check-6` | `large_file` | `session-introduced-clone-surface` | `replay` | 6 | `sentrux-core/src/app/mcp_server/handlers/session.rs` | `unknown` | sentrux-core/src/app/mcp_server/handlers/session.rs grew to 844 lines and should likely be split. |  |  |
| `check-7` | `large_file` | `session-introduced-clone-surface` | `replay` | 7 | `sentrux-core/src/app/mcp_server/handlers/test_support.rs` | `unknown` | sentrux-core/src/app/mcp_server/handlers/test_support.rs grew to 583 lines and should likely be split. |  |  |
| `check-8` | `large_file` | `clone-drift-git-aware` | `replay` | 8 | `sentrux-core/src/app/mcp_server/handlers.rs` | `unknown` | sentrux-core/src/app/mcp_server/handlers.rs grew to 3649 lines and should likely be split. |  |  |
| `check-9` | `large_file` | `clone-drift-git-aware` | `replay` | 9 | `sentrux-core/src/metrics/v2/mod.rs` | `unknown` | sentrux-core/src/metrics/v2/mod.rs grew to 567 lines and should likely be split. |  |  |
| `check-10` | `large_file` | `clone-divergence-priority` | `replay` | 10 | `sentrux-core/src/metrics/v2/clones.rs` | `unknown` | sentrux-core/src/metrics/v2/clones.rs grew to 1131 lines and should likely be split. |  |  |
| `check-11` | `large_file` | `raw-read-guidance` | `replay` | 11 | `sentrux-core/src/metrics/v2/mod.rs` | `unknown` | sentrux-core/src/metrics/v2/mod.rs grew to 1219 lines and should likely be split. |  |  |
| `check-12` | `large_file` | `repo-local-session-telemetry` | `replay` | 12 | `sentrux-core/src/app/mcp_server/handlers.rs` | `unknown` | sentrux-core/src/app/mcp_server/handlers.rs grew to 580 lines and should likely be split. |  |  |
| `check-13` | `forbidden_raw_read` | `self_forbidden_raw_read` | `seeded` | 13 | `src/components/SidebarTaskRow.tsx` | `trusted` | Seeded raw-read self-defect surfaced direct boundary guidance and remediated cleanly. | `boundary_discipline` | `promote direct boundary evidence over large-file stand-ins` |
| `check-14` | `incomplete_propagation` | `self_incomplete_propagation` | `seeded` | 14 | `scripts/defect-injection/catalog.mjs` | `trusted` | Seeded contract-surface defect surfaced incomplete propagation directly and remediated cleanly. | `boundary_discipline` | `treat sibling-surface drift as direct evidence` |
| `check-15` | `session_introduced_clone` | `self_session_introduced_clone` | `seeded` | 15 | `src/copy.ts` | `watchpoint` | Seeded post-baseline duplicate introduction produced direct clone evidence and a clean remediation. | `duplication_pressure` | `keep direct clone followthrough visible` |
| `check-16` | `zero_config_boundary_violation` | `self_zero_config_boundary_violation` | `seeded` | 16 | `src/app/task-dashboard.ts` | `watchpoint` | Seeded deep-import violation produced the missing zero-config boundary evidence and remediated cleanly. | `boundary_discipline` | `validate zero-config ranking against crowd-out` |
