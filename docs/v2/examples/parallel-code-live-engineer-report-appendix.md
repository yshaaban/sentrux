# Parallel Code: Live Analysis Report Appendix

Generated on March 19, 2026 from the live checkout at `<parallel-code-root>`.

This appendix contains the evidence behind
[parallel-code-live-engineer-report.md](<sentrux-root>/docs/v2/examples/parallel-code-live-engineer-report.md).

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

## Scan Scope And Confidence

Current scan:

- scanned files: `622`
- scanned lines: `137959`
- kept files from git candidate set: `622 / 756`
- excluded files: `134`
- excluded buckets:
  - build: `12`
  - cache: `0`
  - fixture: `7`
  - generated: `0`
  - vendor: `102`
- resolved imports: `1871`
- unresolved internal imports: `1`
- unresolved external imports: `508`
- unresolved unknown imports: `79`
- scan confidence: `8228 / 10000`
- rule coverage: `10000 / 10000`
- semantic rules loaded: `true`
- session baseline loaded in `findings`: `false`

## Top Trusted Findings

### ConnectionBannerState

- `trusted`
- `closed_domain_exhaustiveness`
- summary: `Closed domain 'ConnectionBannerState' is missing coverage for variants: connecting, reconnecting, restoring`
- evidence:
  - evidence count: `1`
  - file count: `2`
  - src/components/app-shell/AppConnectionBanner.tsx [missing variants: connecting, reconnecting, restoring]
- related surfaces:
  - [AppConnectionBanner.tsx](<parallel-code-root>/src/components/app-shell/AppConnectionBanner.tsx)
  - [browser-session.ts](<parallel-code-root>/src/runtime/browser-session.ts)

### [App.tsx](<parallel-code-root>/src/App.tsx)

- `trusted`
- `dependency_sprawl`
- summary: `Composition root 'src/App.tsx' depends on 32 real surfaces, above the typescript threshold of 15`
- evidence:
  - role tags: `guarded_seam, composition_root`
  - evidence count: `6`
  - file count: `1`
  - fan-out: 32
  - fan-out threshold: 15
  - instability: 0.97
  - dominant dependency categories: components(16), lib(5), app(3)
  - sample dependencies: src/app/app-action-keys.ts, src/app/desktop-session.ts, src/app/task-command-lease.ts
  - guardrail tests: src/app/store-boundary.architecture.test.ts
- candidate split axes:
  - `components dependency boundary`
  - `lib dependency boundary`
  - `app dependency boundary`
- related surfaces:
  - [app-action-keys.ts](<parallel-code-root>/src/app/app-action-keys.ts)
  - [desktop-session.ts](<parallel-code-root>/src/app/desktop-session.ts)
  - [task-command-lease.ts](<parallel-code-root>/src/app/task-command-lease.ts)
  - [store-boundary.architecture.test.ts](<parallel-code-root>/src/app/store-boundary.architecture.test.ts)

### [TaskPanel.tsx](<parallel-code-root>/src/components/TaskPanel.tsx)

- `trusted`
- `dependency_sprawl`
- summary: `File 'src/components/TaskPanel.tsx' depends on 28 real surfaces, above the typescript threshold of 15`
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

### [store.ts](<parallel-code-root>/src/store/store.ts)

- `trusted`
- `unstable_hotspot`
- summary: `Component-facing barrel 'src/store/store.ts' has 47 inbound references and remains unstable`
- evidence:
  - role tags: `guarded_seam, guarded_boundary, component_barrel`
  - evidence count: `8`
  - file count: `1`
  - fan-in: 47
  - hotspot threshold: 20
  - fan-out: 20
  - instability: 0.30
  - dominant dependent categories: components(33), store(11), arena(2)
  - sample dependents: src/App.tsx, src/arena/ConfigScreen.tsx, src/arena/ResultsScreen.tsx
  - guardrail tests: src/app/store-boundary.architecture.test.ts, src/components/TaskPanel.architecture.test.ts
  - guarded boundary literals: store/core, store/store, store.taskCommandControllers, store.focusedPanel[, setTaskFocusedPanel
- candidate split axes:
  - `components caller boundary`
  - `store caller boundary`
  - `store dependency boundary`
- related surfaces:
  - [App.tsx](<parallel-code-root>/src/App.tsx)
  - [ConfigScreen.tsx](<parallel-code-root>/src/arena/ConfigScreen.tsx)
  - [ResultsScreen.tsx](<parallel-code-root>/src/arena/ResultsScreen.tsx)
  - [store-boundary.architecture.test.ts](<parallel-code-root>/src/app/store-boundary.architecture.test.ts)
  - [TaskPanel.architecture.test.ts](<parallel-code-root>/src/components/TaskPanel.architecture.test.ts)

### [session-stress.mjs](<parallel-code-root>/scripts/session-stress.mjs)

- `trusted`
- `large_file`
- summary: `File 'scripts/session-stress.mjs' is 2048 lines, above the javascript threshold of 500`
- evidence:
  - role tags: `entry_surface`
  - evidence count: `5`
  - file count: `1`
  - line count: 2048
  - large-file threshold: 500
  - function count: 91
  - peak function complexity: 26
  - outbound dependencies: 2
- candidate split axes:
  - `scripts dependency boundary`
  - `entry surface split`
  - `private helper surface split`
- related surfaces:
  - [browser-server-client.mjs](<parallel-code-root>/scripts/browser-server-client.mjs)
  - [session-stress-profiles.mjs](<parallel-code-root>/scripts/session-stress-profiles.mjs)

### [ipc.ts](<parallel-code-root>/src/lib/ipc.ts)

- `trusted`
- `unstable_hotspot`
- summary: `File 'src/lib/ipc.ts' has 66 inbound references and remains unstable`
- evidence:
  - evidence count: `6`
  - file count: `1`
  - fan-in: 66
  - hotspot threshold: 20
  - fan-out: 12
  - instability: 0.15
  - dominant dependent categories: app(18), components(13), store(13)
  - sample dependents: src/App.tsx, src/app/agent-catalog.ts, src/app/desktop-session-startup.ts
- candidate split axes:
  - `app caller boundary`
  - `components caller boundary`
  - `lib dependency boundary`
- related surfaces:
  - [App.tsx](<parallel-code-root>/src/App.tsx)
  - [agent-catalog.ts](<parallel-code-root>/src/app/agent-catalog.ts)
  - [desktop-session-startup.ts](<parallel-code-root>/src/app/desktop-session-startup.ts)

## Top Watchpoints

### cycle:src/app/agent-catalog.ts|src/app/remote-access.ts|src/app/task-attention.ts|src/app/task-close-state.ts|src/app/task-command-dispatch.ts|src/app/task-command-lease-runtime-subscriptions.ts|src/app/task-command-lease-runtime.ts|src/app/task-command-lease-session.ts|src/app/task-command-lease-takeover.ts|src/app/task-command-lease.ts|src/app/task-convergence.ts|src/app/task-lifecycle-workflows.ts|src/app/task-presentation-status.ts|src/app/task-prompt-workflows.ts|src/app/task-review-state.ts|src/app/task-shell-workflows.ts|src/app/task-workflows.ts|src/lib/runtime-client-id.ts|src/store/agent-output-activity.ts|src/store/agents.ts|src/store/auto-trust.ts|src/store/client-session.ts|src/store/completion.ts|src/store/core.ts|src/store/focus.ts|src/store/keyed-snapshot-record.ts|src/store/navigation.ts|src/store/notification.ts|src/store/peer-presence.ts|src/store/persistence-codecs.ts|src/store/persistence-load-context.ts|src/store/persistence-load.ts|src/store/persistence-projects.ts|src/store/persistence-save.ts|src/store/persistence-terminal-restore.ts|src/store/persistence.ts|src/store/projects.ts|src/store/remote.ts|src/store/review.ts|src/store/state.ts|src/store/store.ts|src/store/task-command-controllers.ts|src/store/task-command-takeovers.ts|src/store/task-git-status.ts|src/store/task-state-cleanup.ts|src/store/taskStatus.ts|src/store/tasks.ts|src/store/terminals.ts|src/store/ui.ts

- `watchpoint`
- `cycle_cluster`
- summary: `Files src/app/agent-catalog.ts, src/app/remote-access.ts, src/app/task-attention.ts, src/app/task-close-state.ts, src/app/task-command-dispatch.ts, src/app/task-command-lease-runtime-subscriptions.ts, src/app/task-command-lease-runtime.ts, src/app/task-command-lease-session.ts, src/app/task-command-lease-takeover.ts, src/app/task-command-lease.ts, src/app/task-convergence.ts, src/app/task-lifecycle-workflows.ts, src/app/task-presentation-status.ts, src/app/task-prompt-workflows.ts, src/app/task-review-state.ts, src/app/task-shell-workflows.ts, src/app/task-workflows.ts, src/lib/runtime-client-id.ts, src/store/agent-output-activity.ts, src/store/agents.ts, src/store/auto-trust.ts, src/store/client-session.ts, src/store/completion.ts, src/store/core.ts, src/store/focus.ts, src/store/keyed-snapshot-record.ts, src/store/navigation.ts, src/store/notification.ts, src/store/peer-presence.ts, src/store/persistence-codecs.ts, src/store/persistence-load-context.ts, src/store/persistence-load.ts, src/store/persistence-projects.ts, src/store/persistence-save.ts, src/store/persistence-terminal-restore.ts, src/store/persistence.ts, src/store/projects.ts, src/store/remote.ts, src/store/review.ts, src/store/state.ts, src/store/store.ts, src/store/task-command-controllers.ts, src/store/task-command-takeovers.ts, src/store/task-git-status.ts, src/store/task-state-cleanup.ts, src/store/taskStatus.ts, src/store/tasks.ts, src/store/terminals.ts, src/store/ui.ts form a dependency cycle`

### clone-family-0x7e50d49dc16ef925

- `watchpoint`
- `clone_family`
- summary: `4 exact clone groups repeat across 2 files and churn differs by 0 recent commit(s) across siblings; sibling file age spans 1 day(s)`

### clone-family-0x9ebb8dad5cafb9c0

- `watchpoint`
- `clone_family`
- summary: `4 exact clone groups repeat across 2 files and churn differs by 3 recent commit(s) across siblings; sibling file age spans 0 day(s)`

### server/browser-channels.ts

- `watchpoint`
- `hotspot`
- summary: `File 'server/browser-channels.ts' is carrying coordination hotspot pressure`

### src/lib/browser-http-ipc.ts

- `watchpoint`
- `hotspot`
- summary: `File 'src/lib/browser-http-ipc.ts' is carrying coordination hotspot pressure`

## Trusted Debt Clusters

### cluster:src/store/store.ts|src/app/agent-catalog.ts|src/app/remote-access.ts|src/app/task-attention.ts|src/app/task-close-state.ts|src/app/task-command-dispatch.ts|src/app/task-command-lease-runtime-subscriptions.ts|src/app/task-command-lease-runtime.ts|src/app/task-command-lease-session.ts|src/app/task-command-lease-takeover.ts|src/app/task-command-lease.ts|src/app/task-convergence.ts|src/app/task-lifecycle-workflows.ts|src/app/task-presentation-status.ts|src/app/task-prompt-workflows.ts|src/app/task-review-state.ts|src/app/task-shell-workflows.ts|src/app/task-workflows.ts|src/lib/runtime-client-id.ts|src/store/agent-output-activity.ts|src/store/agents.ts|src/store/auto-trust.ts|src/store/client-session.ts|src/store/completion.ts|src/store/core.ts|src/store/focus.ts|src/store/keyed-snapshot-record.ts|src/store/navigation.ts|src/store/notification.ts|src/store/peer-presence.ts|src/store/persistence-codecs.ts|src/store/persistence-load-context.ts|src/store/persistence-load.ts|src/store/persistence-projects.ts|src/store/persistence-save.ts|src/store/persistence-terminal-restore.ts|src/store/persistence.ts|src/store/projects.ts|src/store/remote.ts|src/store/review.ts|src/store/state.ts|src/store/task-command-controllers.ts|src/store/task-command-takeovers.ts|src/store/task-git-status.ts|src/store/task-state-cleanup.ts|src/store/taskStatus.ts|src/store/tasks.ts|src/store/terminals.ts|src/store/ui.ts

- summary: `Files src/store/store.ts, src/app/agent-catalog.ts, src/app/remote-access.ts, and 46 more intersect 9 debt signals: unstable_hotspot, cycle_cluster, hotspot, dependency_sprawl`
- trust tier: `trusted`
- signal kinds:
  - `unstable_hotspot`
  - `cycle_cluster`
  - `hotspot`
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

### cluster:scripts/session-stress.mjs

- summary: `File 'scripts/session-stress.mjs' intersects 2 debt signals: large_file, hotspot`
- trust tier: `trusted`
- signal kinds:
  - `large_file`
  - `hotspot`

## Experimental Side Channel

Current experimental counts:

- experimental findings: `10`
- experimental debt signals: `5`

Representative examples:

- [ScrollingDiffView.tsx](<parallel-code-root>/src/components/ScrollingDiffView.tsx)
- [review.ts](<parallel-code-root>/src/store/review.ts)
- [PreviewPanel.tsx](<parallel-code-root>/src/components/PreviewPanel.tsx)
- [SidebarTaskRow.tsx](<parallel-code-root>/src/components/SidebarTaskRow.tsx)
- [server.ts](<parallel-code-root>/electron/remote/server.ts)

Current rule:

- these are visible for analyzer follow-up
- they should not be used as maintainer-facing debt guidance until the detector is fixed

## Configured Concepts And Current State

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