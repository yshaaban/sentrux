# Parallel Code: Committed HEAD Analysis Report Appendix

Generated on March 20, 2026 from a committed HEAD clone of `<parallel-code-root>`.

This appendix contains the evidence behind
[parallel-code-head-engineer-report.md](<sentrux-root>/docs/v2/examples/parallel-code-head-engineer-report.md).

## Method

The analysis used:

- live source repo: [<parallel-code-root>](<parallel-code-root>)
- bundled rules file: [parallel-code.rules.toml](<sentrux-root>/docs/v2/examples/parallel-code.rules.toml)
- goldens refresh path: [refresh_parallel_code_goldens.sh](<sentrux-root>/scripts/refresh_parallel_code_goldens.sh)
- current binary used for the run: [<sentrux-root>/target/debug/sentrux](<sentrux-root>/target/debug/sentrux)

Scope caveat:

- the live repo has `.sentrux/baseline.json`
- it does **not** currently have its own `.sentrux/rules.toml`
- this run therefore still uses the bundled example rules
- this report intentionally ignores uncommitted working-tree changes

## Scan Scope And Confidence

Current scan:

- scanned files: `646`
- scanned lines: `142102`
- kept files from git candidate set: `646 / 780`
- excluded files: `134`
- excluded buckets:
  - build: `12`
  - cache: `0`
  - fixture: `7`
  - generated: `0`
  - vendor: `102`
- resolved imports: `1968`
- unresolved internal imports: `1`
- unresolved external imports: `528`
- unresolved unknown imports: `85`
- scan confidence: `8282 / 10000`
- rule coverage: `10000 / 10000`
- semantic rules loaded: `true`
- session baseline loaded in `findings`: `false`

## Leverage Summary

### cycle:src/app/agent-catalog.ts|src/app/remote-access.ts|src/app/task-attention.ts|src/app/task-close-state.ts|src/app/task-command-dispatch.ts|src/app/task-command-lease-runtime-subscriptions.ts|src/app/task-command-lease-runtime.ts|src/app/task-command-lease-session.ts|src/app/task-command-lease-takeover.ts|src/app/task-command-lease.ts|src/app/task-convergence.ts|src/app/task-lifecycle-workflows.ts|src/app/task-presentation-status.ts|src/app/task-prompt-workflows.ts|src/app/task-review-state.ts|src/app/task-shell-workflows.ts|src/app/task-workflows.ts|src/lib/runtime-client-id.ts|src/store/agent-output-activity.ts|src/store/agents.ts|src/store/auto-trust.ts|src/store/client-session.ts|src/store/completion.ts|src/store/core.ts|src/store/focus.ts|src/store/keyed-snapshot-record.ts|src/store/navigation.ts|src/store/notification.ts|src/store/peer-presence.ts|src/store/persistence-codecs.ts|src/store/persistence-load-context.ts|src/store/persistence-load.ts|src/store/persistence-projects.ts|src/store/persistence-save.ts|src/store/persistence-terminal-restore.ts|src/store/persistence.ts|src/store/projects.ts|src/store/remote.ts|src/store/review.ts|src/store/sidebar-order.ts|src/store/state.ts|src/store/store.ts|src/store/task-command-controllers.ts|src/store/task-command-takeovers.ts|src/store/task-git-status.ts|src/store/task-state-cleanup.ts|src/store/taskStatus.ts|src/store/tasks.ts|src/store/terminals.ts|src/store/ui.ts

- `watchpoint`
- class: `watchpoint`
- leverage: `architecture_signal`
- signal band: `very_high_signal`
- `cycle_cluster`
- summary: `Files src/app/agent-catalog.ts, src/app/remote-access.ts, src/app/task-attention.ts, src/app/task-close-state.ts, src/app/task-command-dispatch.ts, src/app/task-command-lease-runtime-subscriptions.ts, src/app/task-command-lease-runtime.ts, src/app/task-command-lease-session.ts, src/app/task-command-lease-takeover.ts, src/app/task-command-lease.ts, src/app/task-convergence.ts, src/app/task-lifecycle-workflows.ts, src/app/task-presentation-status.ts, src/app/task-prompt-workflows.ts, src/app/task-review-state.ts, src/app/task-shell-workflows.ts, src/app/task-workflows.ts, src/lib/runtime-client-id.ts, src/store/agent-output-activity.ts, src/store/agents.ts, src/store/auto-trust.ts, src/store/client-session.ts, src/store/completion.ts, src/store/core.ts, src/store/focus.ts, src/store/keyed-snapshot-record.ts, src/store/navigation.ts, src/store/notification.ts, src/store/peer-presence.ts, src/store/persistence-codecs.ts, src/store/persistence-load-context.ts, src/store/persistence-load.ts, src/store/persistence-projects.ts, src/store/persistence-save.ts, src/store/persistence-terminal-restore.ts, src/store/persistence.ts, src/store/projects.ts, src/store/remote.ts, src/store/review.ts, src/store/sidebar-order.ts, src/store/state.ts, src/store/store.ts, src/store/task-command-controllers.ts, src/store/task-command-takeovers.ts, src/store/task-git-status.ts, src/store/task-state-cleanup.ts, src/store/taskStatus.ts, src/store/tasks.ts, src/store/terminals.ts, src/store/ui.ts form a dependency cycle`
- impact: The cycle touches a component-facing barrel, which makes it harder to keep broad component access separate from deeper app and runtime seams.
- leverage reasons:
  - `shared_barrel_boundary_hub`
  - `guardrail_backed_boundary_pressure`
  - `mixed_cycle_pressure`
  - `high_leverage_cycle_cut`
- ranking reasons:
  - `shared_barrel_boundary_hub`
  - `guardrail_backed_boundary_hub`
  - `mixed_cycle_architecture_pressure`
- evidence:
  - role tags: `guarded_seam, guarded_boundary, component_barrel`
  - evidence count: `6`
  - file count: `50`
  - cycle size: 50
  - total lines in cycle: 6963
  - peak function complexity inside cycle: 72
  - candidate cuts: 3
  - best cut candidate: src/store/core.ts -> src/store/store.ts (removes 14 cyclic files)
  - role tags in cycle: guarded_seam, guarded_boundary, component_barrel
- candidate split axes:
  - `guarded boundary cut`
  - `app store boundary`
- related surfaces:
  - [core.ts](<parallel-code-root>/src/store/core.ts)
  - [store.ts](<parallel-code-root>/src/store/store.ts)
  - [tasks.ts](<parallel-code-root>/src/store/tasks.ts)
  - [task-workflows.ts](<parallel-code-root>/src/app/task-workflows.ts)
  - [state.ts](<parallel-code-root>/src/store/state.ts)

### [TaskPanel.tsx](<parallel-code-root>/src/components/TaskPanel.tsx)

- `trusted`
- class: `structural_debt`
- leverage: `local_refactor_target`
- signal band: `high_signal`
- `dependency_sprawl`
- summary: `File 'src/components/TaskPanel.tsx' depends on 28 real surfaces, above the typescript threshold of 15`
- impact: Broad dependency fan-out expands change surface and makes orchestration drift harder to localize.
- leverage reasons:
  - `extracted_owner_shell_pressure`
  - `guardrail_backed_refactor_surface`
  - `contained_refactor_surface`
  - `contained_dependency_pressure`
- ranking reasons:
  - `extracted_owner_shell`
  - `guardrail_backed_refactor_surface`
  - `contained_refactor_surface`
- evidence:
  - role tags: `guarded_seam, facade_with_extracted_owners`
  - evidence count: `7`
  - file count: `1`
  - fan-out: 28
  - fan-out threshold: 15
  - instability: 0.93
  - dominant dependency categories: components(19), lib(4), store(3)
  - sample dependencies: src/app/task-ports.ts, src/components/CloseTaskDialog.tsx, src/components/DiffViewerDialog.tsx
  - guardrail tests: src/components/TaskPanel.architecture.test.ts
  - extracted owner factories: createTaskPanelFocusRuntime, createTaskPanelPreviewController, createTaskPanelDialogState
- candidate split axes:
  - `components dependency boundary`
  - `lib dependency boundary`
  - `store dependency boundary`
- related surfaces:
  - [task-ports.ts](<parallel-code-root>/src/app/task-ports.ts)
  - [CloseTaskDialog.tsx](<parallel-code-root>/src/components/CloseTaskDialog.tsx)
  - [DiffViewerDialog.tsx](<parallel-code-root>/src/components/DiffViewerDialog.tsx)
  - [TaskPanel.architecture.test.ts](<parallel-code-root>/src/components/TaskPanel.architecture.test.ts)

### [ipc.ts](<parallel-code-root>/src/lib/ipc.ts)

- `trusted`
- class: `guarded_facade`
- leverage: `boundary_discipline`
- signal band: `high_signal`
- `unstable_hotspot`
- summary: `Guarded transport facade 'src/lib/ipc.ts' has 68 inbound references and remains unstable`
- impact: A transport facade with heavy fan-in needs clear ownership boundaries so lifecycle or domain logic does not leak into transport glue.
- leverage reasons:
  - `guarded_or_transport_facade`
  - `heavy_inbound_seam_pressure`
- ranking reasons:
  - `facade_boundary_surface`
- evidence:
  - role tags: `transport_facade`
  - evidence count: `6`
  - file count: `1`
  - fan-in: 68
  - hotspot threshold: 20
  - fan-out: 12
  - instability: 0.15
  - dominant dependent categories: app(20), components(13), store(13)
  - sample dependents: src/App.tsx, src/app/agent-catalog.ts, src/app/desktop-session-startup.ts
- candidate split axes:
  - `app caller boundary`
  - `components caller boundary`
  - `lib dependency boundary`
- related surfaces:
  - [App.tsx](<parallel-code-root>/src/App.tsx)
  - [agent-catalog.ts](<parallel-code-root>/src/app/agent-catalog.ts)
  - [desktop-session-startup.ts](<parallel-code-root>/src/app/desktop-session-startup.ts)

### [App.tsx](<parallel-code-root>/src/App.tsx)

- `trusted`
- class: `structural_debt`
- leverage: `regrowth_watchpoint`
- signal band: `high_signal`
- `dependency_sprawl`
- summary: `Composition root 'src/App.tsx' depends on 33 real surfaces, above the typescript threshold of 15`
- impact: Broad dependency fan-out in a composition root makes shell wiring and runtime ownership harder to keep separate.
- leverage reasons:
  - `intentionally_central_surface`
  - `fan_out_regrowth_pressure`
- ranking reasons:
  - `composition_root_breadth`
- evidence:
  - role tags: `guarded_seam, composition_root`
  - evidence count: `6`
  - file count: `1`
  - fan-out: 33
  - fan-out threshold: 15
  - instability: 0.97
  - dominant dependency categories: components(16), lib(5), app(4)
  - sample dependencies: src/app/app-action-keys.ts, src/app/app-startup-status.ts, src/app/desktop-session.ts
  - guardrail tests: src/app/store-boundary.architecture.test.ts
- candidate split axes:
  - `components dependency boundary`
  - `lib dependency boundary`
  - `app dependency boundary`
- related surfaces:
  - [app-action-keys.ts](<parallel-code-root>/src/app/app-action-keys.ts)
  - [app-startup-status.ts](<parallel-code-root>/src/app/app-startup-status.ts)
  - [desktop-session.ts](<parallel-code-root>/src/app/desktop-session.ts)
  - [store-boundary.architecture.test.ts](<parallel-code-root>/src/app/store-boundary.architecture.test.ts)

### [terminal-session.ts](<parallel-code-root>/src/components/terminal-view/terminal-session.ts)

- `trusted`
- class: `structural_debt`
- leverage: `secondary_cleanup`
- signal band: `high_signal`
- `dependency_sprawl`
- summary: `File 'src/components/terminal-view/terminal-session.ts' depends on 22 real surfaces, above the typescript threshold of 15`
- impact: Broad dependency fan-out expands change surface and makes orchestration drift harder to localize.
- leverage reasons:
  - `secondary_facade_cleanup`
- ranking reasons:
  - `secondary_facade_pressure`
  - `hotspot_overlap`
  - `multi_signal_cleanup_overlap`
- evidence:
  - role tags: `guarded_seam, facade_with_extracted_owners`
  - evidence count: `7`
  - file count: `1`
  - fan-out: 22
  - fan-out threshold: 15
  - instability: 0.96
  - dominant dependency categories: lib(13), components(4), store(3)
  - sample dependencies: electron/ipc/channels.ts, src/components/terminal-view/terminal-input-pipeline.ts, src/components/terminal-view/terminal-output-pipeline.ts
  - guardrail tests: src/components/terminal-view/terminal-session.architecture.test.ts
  - extracted owner factories: createTerminalInputPipeline, createTerminalOutputPipeline, createTerminalRecoveryRuntime
- candidate split axes:
  - `lib dependency boundary`
  - `components dependency boundary`
  - `store dependency boundary`
- related surfaces:
  - [channels.ts](<parallel-code-root>/electron/ipc/channels.ts)
  - [terminal-input-pipeline.ts](<parallel-code-root>/src/components/terminal-view/terminal-input-pipeline.ts)
  - [terminal-output-pipeline.ts](<parallel-code-root>/src/components/terminal-view/terminal-output-pipeline.ts)
  - [terminal-session.architecture.test.ts](<parallel-code-root>/src/components/terminal-view/terminal-session.architecture.test.ts)

## Architecture Signals

- `cycle:src/app/agent-catalog.ts|src/app/remote-access.ts|src/app/task-attention.ts|src/app/task-close-state.ts|src/app/task-command-dispatch.ts|src/app/task-command-lease-runtime-subscriptions.ts|src/app/task-command-lease-runtime.ts|src/app/task-command-lease-session.ts|src/app/task-command-lease-takeover.ts|src/app/task-command-lease.ts|src/app/task-convergence.ts|src/app/task-lifecycle-workflows.ts|src/app/task-presentation-status.ts|src/app/task-prompt-workflows.ts|src/app/task-review-state.ts|src/app/task-shell-workflows.ts|src/app/task-workflows.ts|src/lib/runtime-client-id.ts|src/store/agent-output-activity.ts|src/store/agents.ts|src/store/auto-trust.ts|src/store/client-session.ts|src/store/completion.ts|src/store/core.ts|src/store/focus.ts|src/store/keyed-snapshot-record.ts|src/store/navigation.ts|src/store/notification.ts|src/store/peer-presence.ts|src/store/persistence-codecs.ts|src/store/persistence-load-context.ts|src/store/persistence-load.ts|src/store/persistence-projects.ts|src/store/persistence-save.ts|src/store/persistence-terminal-restore.ts|src/store/persistence.ts|src/store/projects.ts|src/store/remote.ts|src/store/review.ts|src/store/sidebar-order.ts|src/store/state.ts|src/store/store.ts|src/store/task-command-controllers.ts|src/store/task-command-takeovers.ts|src/store/task-git-status.ts|src/store/task-state-cleanup.ts|src/store/taskStatus.ts|src/store/tasks.ts|src/store/terminals.ts|src/store/ui.ts` `architecture_signal` `very_high_signal` Files src/app/agent-catalog.ts, src/app/remote-access.ts, src/app/task-attention.ts, src/app/task-close-state.ts, src/app/task-command-dispatch.ts, src/app/task-command-lease-runtime-subscriptions.ts, src/app/task-command-lease-runtime.ts, src/app/task-command-lease-session.ts, src/app/task-command-lease-takeover.ts, src/app/task-command-lease.ts, src/app/task-convergence.ts, src/app/task-lifecycle-workflows.ts, src/app/task-presentation-status.ts, src/app/task-prompt-workflows.ts, src/app/task-review-state.ts, src/app/task-shell-workflows.ts, src/app/task-workflows.ts, src/lib/runtime-client-id.ts, src/store/agent-output-activity.ts, src/store/agents.ts, src/store/auto-trust.ts, src/store/client-session.ts, src/store/completion.ts, src/store/core.ts, src/store/focus.ts, src/store/keyed-snapshot-record.ts, src/store/navigation.ts, src/store/notification.ts, src/store/peer-presence.ts, src/store/persistence-codecs.ts, src/store/persistence-load-context.ts, src/store/persistence-load.ts, src/store/persistence-projects.ts, src/store/persistence-save.ts, src/store/persistence-terminal-restore.ts, src/store/persistence.ts, src/store/projects.ts, src/store/remote.ts, src/store/review.ts, src/store/sidebar-order.ts, src/store/state.ts, src/store/store.ts, src/store/task-command-controllers.ts, src/store/task-command-takeovers.ts, src/store/task-git-status.ts, src/store/task-state-cleanup.ts, src/store/taskStatus.ts, src/store/tasks.ts, src/store/terminals.ts, src/store/ui.ts form a dependency cycle
- `src/store/store.ts` `architecture_signal` `high_signal` Component-facing barrel 'src/store/store.ts' has 48 inbound references and remains unstable

## Best Local Refactor Targets

- `src/components/TaskPanel.tsx` `local_refactor_target` `high_signal` File 'src/components/TaskPanel.tsx' depends on 28 real surfaces, above the typescript threshold of 15
- `src/components/ReviewPanel.tsx` `local_refactor_target` `high_signal` File 'src/components/ReviewPanel.tsx' depends on 22 real surfaces, above the typescript threshold of 15

## Boundary Discipline

- `src/lib/ipc.ts` `boundary_discipline` `high_signal` Guarded transport facade 'src/lib/ipc.ts' has 68 inbound references and remains unstable

## Regrowth Watchpoints

- `src/App.tsx` `regrowth_watchpoint` `high_signal` Composition root 'src/App.tsx' depends on 33 real surfaces, above the typescript threshold of 15

## Secondary Cleanup

- `src/components/terminal-view/terminal-session.ts` `secondary_cleanup` `high_signal` File 'src/components/terminal-view/terminal-session.ts' depends on 22 real surfaces, above the typescript threshold of 15
- `browser_state_sync` `secondary_cleanup` `high_signal` State model 'browser_state_sync' has 8 transition branch(es) without an explicit next-state mapping
- `electron/remote/ws-server.ts|server/browser-websocket.ts` `secondary_cleanup` `high_signal` 2 functions share an identical normalized body across recently changed files

## Targeted Hardening Notes

- `ConnectionBannerState` Closed domain 'ConnectionBannerState' is missing coverage for variants: connecting, reconnecting, restoring

## Tooling Debt

- [session-stress.mjs](<parallel-code-root>/scripts/session-stress.mjs) File 'scripts/session-stress.mjs' is 2048 lines, above the javascript threshold of 500

## Top Watchpoints

### cycle:electron/ipc/handlers.ts|electron/ipc/notification-handlers.ts

- `watchpoint`
- leverage: `architecture_signal`
- signal band: `high_signal`
- `cycle_cluster`
- summary: `Files electron/ipc/handlers.ts, electron/ipc/notification-handlers.ts form a dependency cycle`
- ranking reasons:
  - `mixed_cycle_architecture_pressure`
  - `high_leverage_cut_candidate`

### server/browser-channels.ts

- `watchpoint`
- leverage: `local_refactor_target`
- signal band: `moderate_signal`
- `hotspot`
- summary: `File 'server/browser-channels.ts' is carrying coordination hotspot pressure`

### clone-family-0x7e50d49dc16ef925

- `watchpoint`
- leverage: `secondary_cleanup`
- signal band: `supporting_signal`
- `clone_family`
- summary: `4 exact clone groups repeat across 2 files and churn differs by 0 recent commit(s) across siblings; sibling file age spans 1 day(s)`

### clone-family-0x9ebb8dad5cafb9c0

- `watchpoint`
- leverage: `secondary_cleanup`
- signal band: `supporting_signal`
- `clone_family`
- summary: `4 exact clone groups repeat across 2 files and churn differs by 3 recent commit(s) across siblings; sibling file age spans 0 day(s)`

## Trusted Debt Clusters

### cluster:src/store/store.ts|src/app/agent-catalog.ts|src/app/remote-access.ts|src/app/task-attention.ts|src/app/task-close-state.ts|src/app/task-command-dispatch.ts|src/app/task-command-lease-runtime-subscriptions.ts|src/app/task-command-lease-runtime.ts|src/app/task-command-lease-session.ts|src/app/task-command-lease-takeover.ts|src/app/task-command-lease.ts|src/app/task-convergence.ts|src/app/task-lifecycle-workflows.ts|src/app/task-presentation-status.ts|src/app/task-prompt-workflows.ts|src/app/task-review-state.ts|src/app/task-shell-workflows.ts|src/app/task-workflows.ts|src/lib/runtime-client-id.ts|src/store/agent-output-activity.ts|src/store/agents.ts|src/store/auto-trust.ts|src/store/client-session.ts|src/store/completion.ts|src/store/core.ts|src/store/focus.ts|src/store/keyed-snapshot-record.ts|src/store/navigation.ts|src/store/notification.ts|src/store/peer-presence.ts|src/store/persistence-codecs.ts|src/store/persistence-load-context.ts|src/store/persistence-load.ts|src/store/persistence-projects.ts|src/store/persistence-save.ts|src/store/persistence-terminal-restore.ts|src/store/persistence.ts|src/store/projects.ts|src/store/remote.ts|src/store/review.ts|src/store/sidebar-order.ts|src/store/state.ts|src/store/task-command-controllers.ts|src/store/task-command-takeovers.ts|src/store/task-git-status.ts|src/store/task-state-cleanup.ts|src/store/taskStatus.ts|src/store/tasks.ts|src/store/terminals.ts|src/store/ui.ts

- summary: `Files src/store/store.ts, src/app/agent-catalog.ts, src/app/remote-access.ts, and 47 more intersect 10 debt signals: unstable_hotspot, cycle_cluster, hotspot, large_file, dependency_sprawl`
- trust tier: `trusted`
- signal kinds:
  - `unstable_hotspot`
  - `cycle_cluster`
  - `hotspot`
  - `large_file`
  - `dependency_sprawl`

### cluster:src/App.tsx|src/remote/App.tsx

- summary: `Files src/App.tsx, src/remote/App.tsx intersect 2 debt signals: dependency_sprawl, clone_family`
- trust tier: `trusted`
- signal kinds:
  - `dependency_sprawl`
  - `clone_family`

### cluster:src/components/terminal-view/terminal-session.ts

- summary: `File 'src/components/terminal-view/terminal-session.ts' intersects 3 debt signals: hotspot, large_file, dependency_sprawl`
- trust tier: `trusted`
- signal kinds:
  - `hotspot`
  - `large_file`
  - `dependency_sprawl`

### cluster:src/components/PromptInput.tsx

- summary: `File 'src/components/PromptInput.tsx' intersects 2 debt signals: hotspot, large_file`
- trust tier: `trusted`
- signal kinds:
  - `hotspot`
  - `large_file`

### cluster:electron/ipc/handlers.ts|electron/ipc/notification-handlers.ts

- summary: `Files electron/ipc/handlers.ts, electron/ipc/notification-handlers.ts intersect 2 debt signals: dependency_sprawl, cycle_cluster`
- trust tier: `trusted`
- signal kinds:
  - `dependency_sprawl`
  - `cycle_cluster`

## Experimental Side Channel

Current experimental counts:

- experimental findings: `10`
- experimental debt signals: `5`

Representative examples:

- [review.ts](<parallel-code-root>/src/store/review.ts)
- [terminalLatency.ts](<parallel-code-root>/src/lib/terminalLatency.ts)
- [store.ts](<parallel-code-root>/src/arena/store.ts)
- [diff-selection.ts](<parallel-code-root>/src/lib/diff-selection.ts)
- [ui.ts](<parallel-code-root>/src/store/ui.ts)

Current rule:

- these are visible for analyzer follow-up
- they should not be used as maintainer-facing debt guidance until the detector is fixed

## Configured Concepts And Current State

### `browser_state_sync`

- score: `4000 / 10000`
- missing update sites: `0`
- boundary pressure count: `0`
- dominant finding kinds: `state_model_high_context_burden, state_model_transition_coverage_gap`
- summary: Concept 'browser_state_sync' has 1 high-severity ownership or access findings

### `ConnectionBannerState`

- score: `3100 / 10000`
- missing update sites: `0`
- boundary pressure count: `0`
- dominant finding kinds: `closed_domain_exhaustiveness`
- summary: Concept 'ConnectionBannerState' has 1 high-severity ownership or access findings

### `task_presentation_status`

- score: `1680 / 10000`
- missing update sites: `1`
- boundary pressure count: `0`
- dominant finding kinds: `closed_domain_exhaustiveness`
- summary: Concept 'task_presentation_status' spans 1 obligation reports with 1 missing update sites

### `server_state_bootstrap_gate`

- score: `900 / 10000`
- missing update sites: `0`
- boundary pressure count: `0`
- dominant finding kinds: `state_model_missing_transition_sites`
- summary: Concept 'server_state_bootstrap_gate' has 1 repeated structural findings