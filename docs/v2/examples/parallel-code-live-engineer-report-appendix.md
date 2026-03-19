# Parallel Code: Live Analysis Report Appendix

Generated on March 19, 2026 from the live checkout at `<parallel-code-root>`.

This appendix contains the evidence behind
[parallel-code-live-engineer-report.md](<sentrux-root>/docs/v2/examples/parallel-code-live-engineer-report.md).

## Method

The analysis used a disposable clone of the live repo and the current evidence-first v2 surfaces:

- live source repo: [<parallel-code-root>](<parallel-code-root>)
- rules file used for the run: [parallel-code.rules.toml](<sentrux-root>/docs/v2/examples/parallel-code.rules.toml)
- goldens refresh path: [refresh_parallel_code_goldens.sh](<sentrux-root>/scripts/refresh_parallel_code_goldens.sh)

Scope caveat:

- the live repo currently has [.sentrux/baseline.json](<parallel-code-root>/.sentrux/baseline.json)
- it does **not** currently have a repo-owned `.sentrux/rules.toml`
- the run therefore used the bundled example rules file above

## Scan Scope And Confidence

Current scan:

- scanned files: `604`
- scanned lines: `137,347`
- resolved import edges: `1,797`
- kept files from git candidate set: `604 / 738`
- excluded files: `134`
- scan confidence: `8184 / 10000`
- rule coverage: `10000 / 10000`
- semantic rules loaded: `true`
- session baseline loaded for this run: `false`

## Top Current Findings

### Structural debt findings in the top finding set

- File 'server/browser-control-plane.ts' is 1054 lines, above the typescript threshold of 500
- File 'src/components/terminal-view/terminal-session.ts' is 1856 lines, above the typescript threshold of 500
- File 'src/App.tsx' depends on 28 real surfaces, above the typescript threshold of 15
- File 'src/components/TaskPanel.tsx' depends on 28 real surfaces, above the typescript threshold of 15
- Files src/app/agent-catalog.ts, src/app/remote-access.ts, src/app/task-attention.ts, src/app/task-close-state.ts, src/app/task-command-dispatch.ts, src/app/task-command-lease-runtime-subscriptions.ts, src/app/task-command-lease-runtime.ts, src/app/task-command-lease-session.ts, src/app/task-command-lease-takeover.ts, src/app/task-command-lease.ts, src/app/task-convergence.ts, src/app/task-lifecycle-workflows.ts, src/app/task-presentation-status.ts, src/app/task-prompt-workflows.ts, src/app/task-review-state.ts, src/app/task-shell-workflows.ts, src/app/task-workflows.ts, src/lib/runtime-client-id.ts, src/store/agent-output-activity.ts, src/store/agents.ts, src/store/auto-trust.ts, src/store/client-session.ts, src/store/completion.ts, src/store/core.ts, src/store/focus.ts, src/store/keyed-snapshot-record.ts, src/store/navigation.ts, src/store/notification.ts, src/store/peer-presence.ts, src/store/persistence-codecs.ts, src/store/persistence-load-context.ts, src/store/persistence-load.ts, src/store/persistence-projects.ts, src/store/persistence-save.ts, src/store/persistence-terminal-restore.ts, src/store/persistence.ts, src/store/projects.ts, src/store/remote.ts, src/store/review.ts, src/store/state.ts, src/store/store.ts, src/store/task-command-controllers.ts, src/store/task-command-takeovers.ts, src/store/task-git-status.ts, src/store/task-state-cleanup.ts, src/store/taskStatus.ts, src/store/tasks.ts, src/store/terminals.ts, src/store/ui.ts form a dependency cycle
- File 'src/store/store.ts' has 56 inbound references and remains unstable

### Concept finding still present

- Closed domain 'ConnectionBannerState' is missing coverage for variants: connecting, reconnecting, restoring

## Low-Confidence Signal Not Ready For Review Use

The live run also surfaced `dead_private_code_cluster` findings, but spot-checking the sampled symbol names showed obviously live functions in the current repo. Until that detector is fixed, this finding class should not be used in engineer-facing prioritization or cleanup guidance.

## Top Debt Clusters

### Cluster 1

- summary: Files src/App.tsx, src/remote/App.tsx, src/runtime/browser-session.ts intersect 6 debt signals: dependency_sprawl, clone_family, dead_private_code_cluster, large_file, concept
- signal kinds: dependency_sprawl, clone_family, dead_private_code_cluster, large_file, concept
- signal families: coupling, coordination, duplication, drift, staleness, maintainability, size, boundary
- metrics:
  - signal count: `6`
  - file count: `3`
  - structural signal count: `4`

### Cluster 2

- summary: Files server/browser-control-plane.ts, electron/remote/server.ts, electron/remote/ws-transport.test.ts, and 1 more intersect 4 debt signals: large_file, clone_family, dead_private_code_cluster, hotspot
- signal kinds: large_file, clone_family, dead_private_code_cluster, hotspot
- signal families: size, coordination, duplication, drift, staleness, maintainability
- metrics:
  - signal count: `4`
  - file count: `4`
  - structural signal count: `2`

### Cluster 3

- summary: File 'src/components/terminal-view/terminal-session.ts' intersects 4 debt signals: large_file, dead_private_code_cluster, dependency_sprawl, hotspot
- signal kinds: large_file, dead_private_code_cluster, dependency_sprawl, hotspot
- signal families: size, coordination, staleness, maintainability, coupling
- metrics:
  - signal count: `4`
  - file count: `1`
  - structural signal count: `3`

### Cluster 4

- summary: File 'src/components/TaskPanel.tsx' intersects 4 debt signals: dependency_sprawl, large_file, dead_private_code_cluster, hotspot
- signal kinds: dependency_sprawl, large_file, dead_private_code_cluster, hotspot
- signal families: coupling, coordination, size, staleness, maintainability
- metrics:
  - signal count: `4`
  - file count: `1`
  - structural signal count: `3`

### Cluster 5

- summary: Files src/store/review.ts, src/app/agent-catalog.ts, src/app/remote-access.ts, and 46 more intersect 16 debt signals: dead_private_code_cluster, cycle_cluster, hotspot, dependency_sprawl, unstable_hotspot
- signal kinds: dead_private_code_cluster, cycle_cluster, hotspot, dependency_sprawl, unstable_hotspot
- signal families: staleness, maintainability, dependency, layering, coordination, coupling, blast_radius
- metrics:
  - signal count: `16`
  - file count: `49`
  - structural signal count: `12`

## Clone Families Still Surfaced

### [ws-server.ts](<parallel-code-root>/electron/remote/ws-server.ts) / [browser-websocket.ts](<parallel-code-root>/server/browser-websocket.ts)

- family score: `78`
- member count: `4`
- recent commit gap: `3`
- summary: 4 exact clone groups repeat across 2 files and churn differs by 3 recent commit(s) across siblings; sibling file age spans 0 day(s)

### [AgentGlyph.tsx](<parallel-code-root>/src/components/AgentGlyph.tsx) / [RemoteAgentGlyph.tsx](<parallel-code-root>/src/remote/RemoteAgentGlyph.tsx)

- family score: `86`
- member count: `4`
- recent commit gap: `0`
- summary: 4 exact clone groups repeat across 2 files and churn differs by 0 recent commit(s) across siblings; sibling file age spans 1 day(s)

### [App.tsx](<parallel-code-root>/src/App.tsx) / [App.tsx](<parallel-code-root>/src/remote/App.tsx)

- family score: `322`
- member count: `2`
- recent commit gap: `63`
- summary: 2 exact clone groups repeat across 2 files and churn differs by 63 recent commit(s) across siblings; sibling file age spans 0 day(s)

## Configured Concepts And Current State

### `task_git_status`

- live explain artifact: generated during the disposable-clone run and summarized here
- current findings: `0`
- current obligations: `0`
- current related test misses: `src/app/git-status-sync.test.ts`

### `task_presentation_status`

- live explain artifact: generated during the disposable-clone run and summarized here
- current findings: `1`
- current obligations: `1`
- finding summary: `Closed domain 'task_presentation_status' is missing required update sites`
- missing site detail: `no exhaustive mapping or switch site found for 'TaskDotStatus'`
- missing site file: [task-presentation-status.ts](<parallel-code-root>/src/app/task-presentation-status.ts)

### `server_state_bootstrap`

- live explain artifact: generated during the disposable-clone run and summarized here
- current findings: `0`
- current obligations: `0`
- parity score: `10000 / 10000`
- state integrity score: `10000 / 10000`

## Session-End Baseline State

The pass-state session report is clean for the untouched baseline:

- touched concept gate decision: `pass`
- introduced findings: `0`
- finding details in session_end: `0`

That means the repo-level debt surfaces above are baseline inspection signals, not patch-specific regressions from the pass-state session.
