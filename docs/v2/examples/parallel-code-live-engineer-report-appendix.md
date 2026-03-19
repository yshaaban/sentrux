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

- scanned files: `604`
- scanned lines: `136,558`
- kept files from git candidate set: `604 / 738`
- excluded files: `134`
- excluded buckets:
  - vendor: `102`
  - build: `12`
  - fixture: `7`
  - ignored extension: `11`
  - too large: `2`
- resolved imports: `1,798`
- unresolved internal imports: `1`
- unresolved external imports: `491`
- unresolved unknown imports: `75`
- scan confidence: `8184 / 10000`
- rule coverage: `10000 / 10000`
- semantic rules loaded: `true`
- session baseline loaded in `findings`: `false`

## Top Trusted Findings

### [browser-control-plane.ts](<parallel-code-root>/server/browser-control-plane.ts)

- `trusted`
- `large_file`
- summary: `File 'server/browser-control-plane.ts' is 1054 lines, above the typescript threshold of 500`
- evidence:
  - line count: `1054`
  - threshold: `500`
  - function count: `74`
  - peak complexity: `102`
  - outbound dependencies: `12`
- candidate split axes:
  - `electron dependency boundary`
  - `domain dependency boundary`
  - `server dependency boundary`
  - `high-complexity helper extraction`
  - `private helper surface split`

### [terminal-session.ts](<parallel-code-root>/src/components/terminal-view/terminal-session.ts)

- `trusted`
- `large_file`
- summary: `File 'src/components/terminal-view/terminal-session.ts' is 1856 lines, above the typescript threshold of 500`
- evidence:
  - line count: `1856`
  - threshold: `500`
  - function count: `92`
  - peak complexity: `350`
  - outbound dependencies: `25`
- related surfaces:
  - [channels.ts](<parallel-code-root>/electron/ipc/channels.ts)
  - [task-command-lease.ts](<parallel-code-root>/src/app/task-command-lease.ts)
  - [terminal-output-scheduler.ts](<parallel-code-root>/src/app/terminal-output-scheduler.ts)

### [App.tsx](<parallel-code-root>/src/App.tsx)

- `trusted`
- `dependency_sprawl`
- summary: `File 'src/App.tsx' depends on 28 real surfaces, above the typescript threshold of 15`
- evidence:
  - fan-out: `28`
  - instability: `0.97`
  - dominant categories: `components(12), lib(5), app(3)`
  - sample dependencies:
    - [app-action-keys.ts](<parallel-code-root>/src/app/app-action-keys.ts)
    - [desktop-session.ts](<parallel-code-root>/src/app/desktop-session.ts)
    - [task-command-lease.ts](<parallel-code-root>/src/app/task-command-lease.ts)

### [TaskPanel.tsx](<parallel-code-root>/src/components/TaskPanel.tsx)

- `trusted`
- `dependency_sprawl`
- summary: `File 'src/components/TaskPanel.tsx' depends on 28 real surfaces, above the typescript threshold of 15`
- evidence:
  - fan-out: `28`
  - instability: `0.93`
  - dominant categories: `components(17), lib(4), store(3)`
  - sample dependencies:
    - [task-ports.ts](<parallel-code-root>/src/app/task-ports.ts)
    - [CloseTaskDialog.tsx](<parallel-code-root>/src/components/CloseTaskDialog.tsx)
    - [DiffViewerDialog.tsx](<parallel-code-root>/src/components/DiffViewerDialog.tsx)

### [store.ts](<parallel-code-root>/src/store/store.ts)

- `trusted`
- `unstable_hotspot`
- summary: `File 'src/store/store.ts' has 56 inbound references and remains unstable`
- evidence:
  - fan-in: `56`
  - fan-out: `20`
  - instability: `0.26`
  - dominant dependent categories: `components(34), store(11), runtime(5)`
  - sample dependents:
    - [App.tsx](<parallel-code-root>/src/App.tsx)
    - [desktop-browser-runtime.ts](<parallel-code-root>/src/app/desktop-browser-runtime.ts)
    - [desktop-session-startup.ts](<parallel-code-root>/src/app/desktop-session-startup.ts)

## Top Watchpoints

### Store/app cycle cluster

- `watchpoint`
- `cycle_cluster`
- cycle size: `49`
- total lines in cycle: `6750`
- peak complexity inside cycle: `71`
- candidate cuts: `3`
- best cut summary:
  - `src/store/tasks.ts -> src/app/task-workflows.ts`
  - seam kind: `app_store_boundary`
  - cyclic files removed by cut: `10`

Other cut candidates:

1. [core.ts](<parallel-code-root>/src/store/core.ts) -> [store.ts](<parallel-code-root>/src/store/store.ts)
   - seam kind: `local_module_split`
   - reduction: `13`
2. [state.ts](<parallel-code-root>/src/store/state.ts) -> [core.ts](<parallel-code-root>/src/store/core.ts)
   - seam kind: `contract_or_type_extraction`
   - reduction: `7`

### Clone watchpoints

1. [AgentGlyph.tsx](<parallel-code-root>/src/components/AgentGlyph.tsx) / [RemoteAgentGlyph.tsx](<parallel-code-root>/src/remote/RemoteAgentGlyph.tsx)
   - `watchpoint`
   - `clone_family`
   - exact clone groups: `4`

2. [ws-server.ts](<parallel-code-root>/electron/remote/ws-server.ts) / [browser-websocket.ts](<parallel-code-root>/server/browser-websocket.ts)
   - `watchpoint`
   - `clone_family`
   - exact clone groups: `4`
   - churn gap: `3`

3. medium hotspot watchpoints:
   - [browser-channels.ts](<parallel-code-root>/server/browser-channels.ts)
   - [browser-http-ipc.ts](<parallel-code-root>/src/lib/browser-http-ipc.ts)

## Trusted Debt Clusters

### Cluster 1

- summary: `Files src/App.tsx, src/remote/App.tsx, src/runtime/browser-session.ts intersect 4 debt signals: dependency_sprawl, clone_family, large_file, concept`
- trust tier: `trusted`
- signal kinds:
  - `dependency_sprawl`
  - `clone_family`
  - `large_file`
  - `concept`

### Cluster 2

- summary: `Files server/browser-control-plane.ts, electron/remote/server.ts, electron/remote/ws-transport.test.ts, and 1 more intersect 3 debt signals: large_file, clone_family, hotspot`
- trust tier: `trusted`
- signal kinds:
  - `large_file`
  - `clone_family`
  - `hotspot`

### Cluster 3

- summary: `File 'src/components/terminal-view/terminal-session.ts' intersects 3 debt signals: large_file, dependency_sprawl, hotspot`
- trust tier: `trusted`
- signal kinds:
  - `large_file`
  - `dependency_sprawl`
  - `hotspot`

### Cluster 4

- summary: `Files src/store/store.ts, src/app/agent-catalog.ts, src/app/remote-access.ts, and 46 more intersect 9 debt signals: unstable_hotspot, cycle_cluster, hotspot, dependency_sprawl`
- trust tier: `trusted`
- signal kinds:
  - `unstable_hotspot`
  - `cycle_cluster`
  - `hotspot`
  - `dependency_sprawl`

### Cluster 5

- summary: `File 'src/components/TaskPanel.tsx' intersects 3 debt signals: dependency_sprawl, hotspot, large_file`
- trust tier: `trusted`
- signal kinds:
  - `dependency_sprawl`
  - `hotspot`
  - `large_file`

Important note:

- the refreshed clusters no longer include `dead_private_code_cluster`
- that detector is now confined to the experimental side channel

## Experimental Side Channel

Current experimental counts:

- experimental findings: `12`
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

### `task_git_status`

- findings: `0`
- obligations: `0`

### `task_presentation_status`

- findings: `1`
- obligations: `1`
- finding kind: `closed_domain_exhaustiveness`
- missing site path:
  [task-presentation-status.ts](<parallel-code-root>/src/app/task-presentation-status.ts)

### `server_state_bootstrap`

- findings: `0`
- obligations: `0`
- parity score: `10000 / 10000`
- state integrity score: `10000 / 10000`

## Session-End Pass State

Current clean pass-state session:

- `pass: true`
- introduced findings: `0`
- experimental findings: `0`
- debt signals: `5`
- watchpoints: `1`
- summary: `Quality stable or improved`
