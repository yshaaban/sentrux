# Check Review Packet

- repo root: `<sentrux-root>`
- tool: `check`
- source mode: `combined`
- source path(s):
  - `<sentrux-root>/.sentrux/evals/<live-calibration-run>/codex-batch/codex-session-batch.json`
  - `<sentrux-root>/.sentrux/evals/<live-calibration-run>/codex-batch/sentrux-batch-loop-tidy-control/codex-session.json`
  - `<sentrux-root>/.sentrux/evals/<live-calibration-run>/codex-batch/sentrux-batch-loop-tidy-report-only/codex-session.json`
  - `<sentrux-root>/.sentrux/evals/<live-calibration-run>/codex-batch/sentrux-benchmark-harness-tidy-control/codex-session.json`
  - `<sentrux-root>/.sentrux/evals/<live-calibration-run>/codex-batch/sentrux-benchmark-harness-tidy-fix-first/codex-session.json`
  - `<sentrux-root>/.sentrux/evals/<replay-calibration-run>/replay-batch/diff-replay-batch.json`
  - `<sentrux-root>/.sentrux/evals/<replay-calibration-run>/replay-batch/session-introduced-clone-surface/diff-replay.json`
  - `<sentrux-root>/.sentrux/evals/<replay-calibration-run>/replay-batch/clone-drift-git-aware/diff-replay.json`
  - `<sentrux-root>/.sentrux/evals/<replay-calibration-run>/replay-batch/clone-divergence-priority/diff-replay.json`
  - `<sentrux-root>/.sentrux/evals/<replay-calibration-run>/replay-batch/raw-read-guidance/diff-replay.json`
  - `<sentrux-root>/.sentrux/evals/<replay-calibration-run>/replay-batch/fast-check-zero-config/diff-replay.json`
  - `<sentrux-root>/.sentrux/evals/<replay-calibration-run>/replay-batch/contract-surface-obligations/diff-replay.json`
- generated at: `2026-04-18T12:24:08.678Z`
- sample count: 12
- repair-packet completeness: `12/12` (1)
- top-3 repair-packet completeness: `1`
- top-10 repair-packet completeness: `1`
- kind counts: missing_test_coverage=6, clone_propagation_drift=2, authoritative_import_bypass=1, forbidden_raw_read=1, incomplete_propagation=1, session_introduced_clone=1

| Review ID | Kind | Source | Snapshot | Rank | Scope | Severity | Summary | Evidence | Classification | Action |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `check-1` | `incomplete_propagation` | `contract-surface-obligations` | `replay` | 1 | `defect_injection_toolchain` | `unknown` | Contract 'defect_injection_toolchain' changed in scripts/defect-injection/catalog.mjs but the registry, categories and 2 other required surfaces were not updated | scripts/defect-injection/run-injection.mjs [declared contract site is missing from semantic snapshot: runDefectInjection] · scripts/defect-injection/catalog.mjs [declared contract site is missing from semantic snapshot: createDogfoodCatalog] · scripts/defect-injection/report.mjs [declared contract site is missing from semantic snapshot: buildInjectionReport] · scripts/tests/defect-injection.test.mjs [declared contract site is missing from semantic snapshot: scripts/tests/defect-injection.test.mjs] |  |  |
| `check-2` | `forbidden_raw_read` | `raw-read-guidance` | `replay` | 2 | `task_presentation_status` | `unknown` | Concept 'task_presentation_status' is read from a forbidden raw access path at src/components/SidebarTaskRow.tsx | src/components/SidebarTaskRow.tsx::store.taskGitStatus · preferred accessor: src/app/task-presentation-status.ts::getTaskDotStatus · preferred accessor: src/app/task-presentation-status.ts::getTaskDotStatusLabel · canonical owner: src/app/task-presentation-status.ts::getTaskDotStatus |  |  |
| `check-3` | `authoritative_import_bypass` | `raw-read-guidance` | `replay` | 3 | `task_presentation_status` | `unknown` | Concept 'task_presentation_status' bypasses canonical entrypoint src/app/task-presentation-status.ts at src/components/SidebarTaskRow.tsx | src/components/SidebarTaskRow.tsx -> src/store/core.ts (prefer src/app/task-presentation-status.ts) |  |  |
| `check-4` | `session_introduced_clone` | `session-introduced-clone-surface` | `replay` | 4 | `src/copy.ts and src/source.ts` | `unknown` | This patch introduced a new duplicate implementation across src/copy.ts and src/source.ts. | introduced clone group: clone-0x3adbcef1160c89d · introduced duplicate: src/copy.ts::buildStatusBadge · preferred owner: src/copy.ts::buildStatusBadge · duplicate surface: src/copy.ts · duplicate surface: src/source.ts · 2 functions share an identical normalized body |  |  |
| `check-5` | `clone_propagation_drift` | `clone-drift-git-aware` | `replay` | 5 | `src/copy.ts::buildAccessUrl` | `unknown` | This patch changed src/source.ts::buildAccessUrl inside a known clone group, but sibling clone logic still lives in src/copy.ts::buildAccessUrl. | baseline clone group: clone-0x560b27df11b027c7 · changed clone member: src/source.ts::buildAccessUrl · unchanged clone sibling: src/copy.ts::buildAccessUrl · baseline clone surface: src/copy.ts · baseline clone surface: src/source.ts · 2 functions share an identical normalized body across recently changed files |  |  |
| `check-6` | `clone_propagation_drift` | `clone-drift-git-aware` | `replay` | 6 | `src/copy.ts::buildOptionalAccessUrl` | `unknown` | This patch changed src/source.ts::buildOptionalAccessUrl inside a known clone group, but sibling clone logic still lives in src/copy.ts::buildOptionalAccessUrl. | baseline clone group: clone-0x9eddf7eb671ec567 · changed clone member: src/source.ts::buildOptionalAccessUrl · unchanged clone sibling: src/copy.ts::buildOptionalAccessUrl · baseline clone surface: src/copy.ts · baseline clone surface: src/source.ts · 2 functions share an identical normalized body across recently changed files |  |  |
| `check-7` | `missing_test_coverage` | `fast-check-zero-config` | `replay` | 7 | `src/app/task-dashboard.ts` | `unknown` | New production file src/app/task-dashboard.ts does not have a sibling test | src/app/task-dashboard.ts |  |  |
| `check-8` | `missing_test_coverage` | `raw-read-guidance` | `replay` | 8 | `src/app/task-presentation-status.ts` | `unknown` | New production file src/app/task-presentation-status.ts does not have a sibling test | src/app/task-presentation-status.ts |  |  |
| `check-9` | `missing_test_coverage` | `raw-read-guidance` | `replay` | 9 | `src/components/SidebarTaskRow.tsx` | `unknown` | New production file src/components/SidebarTaskRow.tsx does not have a sibling test | src/components/SidebarTaskRow.tsx |  |  |
| `check-10` | `missing_test_coverage` | `session-introduced-clone-surface` | `replay` | 10 | `src/copy.ts` | `unknown` | New production file src/copy.ts does not have a sibling test | src/copy.ts |  |  |
| `check-11` | `missing_test_coverage` | `fast-check-zero-config` | `replay` | 11 | `src/modules/task-status/index.ts` | `unknown` | New production file src/modules/task-status/index.ts does not have a sibling test | src/modules/task-status/index.ts |  |  |
| `check-12` | `missing_test_coverage` | `fast-check-zero-config` | `replay` | 12 | `src/modules/task-status/internal.ts` | `unknown` | New production file src/modules/task-status/internal.ts does not have a sibling test | src/modules/task-status/internal.ts |  |  |
