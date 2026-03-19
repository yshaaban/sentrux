# Parallel-Code Proof Snapshot

Generated from: `<sentrux-root>/docs/v2/examples/parallel-code-golden`
Benchmark: `<sentrux-root>/docs/v2/examples/parallel-code-benchmark.json`

## Top Findings

- `high` `closed_domain_exhaustiveness` (ConnectionBannerState) Closed domain 'ConnectionBannerState' is missing coverage for variants: connecting, reconnecting, restoring
- `high` `large_file` File 'server/browser-control-plane.ts' is 1054 lines, above the typescript threshold of 500
- `high` `large_file` File 'src/components/terminal-view/terminal-session.ts' is 1856 lines, above the typescript threshold of 500
- `high` `dependency_sprawl` File 'src/App.tsx' depends on 28 real surfaces, above the typescript threshold of 15
- `high` `dependency_sprawl` File 'src/components/TaskPanel.tsx' depends on 28 real surfaces, above the typescript threshold of 15
- `high` `dead_private_code_cluster` File 'src/components/ScrollingDiffView.tsx' contains 15 uncalled private functions totaling 144 lines
- `high` `dependency_sprawl` File 'src/components/terminal-view/terminal-session.ts' depends on 25 real surfaces, above the typescript threshold of 15
- `high` `dead_private_code_cluster` File 'src/store/review.ts' contains 10 uncalled private functions totaling 121 lines
- `high` `dead_private_code_cluster` File 'src/components/PreviewPanel.tsx' contains 21 uncalled private functions totaling 115 lines
- `high` `cycle_cluster` Files src/app/agent-catalog.ts, src/app/remote-access.ts, src/app/task-attention.ts, src/app/task-close-state.ts, src/app/task-command-dispatch.ts, src/app/task-command-lease-runtime-subscriptions.ts, src/app/task-command-lease-runtime.ts, src/app/task-command-lease-session.ts, src/app/task-command-lease-takeover.ts, src/app/task-command-lease.ts, src/app/task-convergence.ts, src/app/task-lifecycle-workflows.ts, src/app/task-presentation-status.ts, src/app/task-prompt-workflows.ts, src/app/task-review-state.ts, src/app/task-shell-workflows.ts, src/app/task-workflows.ts, src/lib/runtime-client-id.ts, src/store/agent-output-activity.ts, src/store/agents.ts, src/store/auto-trust.ts, src/store/client-session.ts, src/store/completion.ts, src/store/core.ts, src/store/focus.ts, src/store/keyed-snapshot-record.ts, src/store/navigation.ts, src/store/notification.ts, src/store/peer-presence.ts, src/store/persistence-codecs.ts, src/store/persistence-load-context.ts, src/store/persistence-load.ts, src/store/persistence-projects.ts, src/store/persistence-save.ts, src/store/persistence-terminal-restore.ts, src/store/persistence.ts, src/store/projects.ts, src/store/remote.ts, src/store/review.ts, src/store/state.ts, src/store/store.ts, src/store/task-command-controllers.ts, src/store/task-command-takeovers.ts, src/store/task-git-status.ts, src/store/task-state-cleanup.ts, src/store/taskStatus.ts, src/store/tasks.ts, src/store/terminals.ts, src/store/ui.ts form a dependency cycle

## Concept Summaries

- `ConnectionBannerState` score 3100: Concept 'ConnectionBannerState' has 1 high-severity ownership or access findings
- `task_presentation_status` score 1680: Concept 'task_presentation_status' spans 1 obligation reports with 1 missing update sites

## Finding Details

- `high` `closed_domain_exhaustiveness` `ConnectionBannerState`: Closed domain 'ConnectionBannerState' is missing coverage for variants: connecting, reconnecting, restoring
  - impact: Finite-domain changes can silently miss one surface unless all required cases stay in sync.
- `high` `large_file` `server/browser-control-plane.ts`: File 'server/browser-control-plane.ts' is 1054 lines, above the typescript threshold of 500
  - impact: Responsibility concentration increases review friction and makes later splits harder to isolate.
- `high` `large_file` `src/components/terminal-view/terminal-session.ts`: File 'src/components/terminal-view/terminal-session.ts' is 1856 lines, above the typescript threshold of 500
  - impact: Responsibility concentration increases review friction and makes later splits harder to isolate.
- `high` `dependency_sprawl` `src/App.tsx`: File 'src/App.tsx' depends on 28 real surfaces, above the typescript threshold of 15
  - impact: Broad dependency fan-out expands change surface and makes orchestration drift harder to localize.
- `high` `dependency_sprawl` `src/components/TaskPanel.tsx`: File 'src/components/TaskPanel.tsx' depends on 28 real surfaces, above the typescript threshold of 15
  - impact: Broad dependency fan-out expands change surface and makes orchestration drift harder to localize.
- `high` `dead_private_code_cluster` `src/components/ScrollingDiffView.tsx`: File 'src/components/ScrollingDiffView.tsx' contains 15 uncalled private functions totaling 144 lines
  - impact: Stale private code increases maintenance noise and can mislead future edits into reviving obsolete paths.
- `high` `dependency_sprawl` `src/components/terminal-view/terminal-session.ts`: File 'src/components/terminal-view/terminal-session.ts' depends on 25 real surfaces, above the typescript threshold of 15
  - impact: Broad dependency fan-out expands change surface and makes orchestration drift harder to localize.
- `high` `dead_private_code_cluster` `src/store/review.ts`: File 'src/store/review.ts' contains 10 uncalled private functions totaling 121 lines
  - impact: Stale private code increases maintenance noise and can mislead future edits into reviving obsolete paths.
- `high` `dead_private_code_cluster` `src/components/PreviewPanel.tsx`: File 'src/components/PreviewPanel.tsx' contains 21 uncalled private functions totaling 115 lines
  - impact: Stale private code increases maintenance noise and can mislead future edits into reviving obsolete paths.
- `high` `cycle_cluster` `cycle:src/app/agent-catalog.ts|src/app/remote-access.ts|src/app/task-attention.ts|src/app/task-close-state.ts|src/app/task-command-dispatch.ts|src/app/task-command-lease-runtime-subscriptions.ts|src/app/task-command-lease-runtime.ts|src/app/task-command-lease-session.ts|src/app/task-command-lease-takeover.ts|src/app/task-command-lease.ts|src/app/task-convergence.ts|src/app/task-lifecycle-workflows.ts|src/app/task-presentation-status.ts|src/app/task-prompt-workflows.ts|src/app/task-review-state.ts|src/app/task-shell-workflows.ts|src/app/task-workflows.ts|src/lib/runtime-client-id.ts|src/store/agent-output-activity.ts|src/store/agents.ts|src/store/auto-trust.ts|src/store/client-session.ts|src/store/completion.ts|src/store/core.ts|src/store/focus.ts|src/store/keyed-snapshot-record.ts|src/store/navigation.ts|src/store/notification.ts|src/store/peer-presence.ts|src/store/persistence-codecs.ts|src/store/persistence-load-context.ts|src/store/persistence-load.ts|src/store/persistence-projects.ts|src/store/persistence-save.ts|src/store/persistence-terminal-restore.ts|src/store/persistence.ts|src/store/projects.ts|src/store/remote.ts|src/store/review.ts|src/store/state.ts|src/store/store.ts|src/store/task-command-controllers.ts|src/store/task-command-takeovers.ts|src/store/task-git-status.ts|src/store/task-state-cleanup.ts|src/store/taskStatus.ts|src/store/tasks.ts|src/store/terminals.ts|src/store/ui.ts`: Files src/app/agent-catalog.ts, src/app/remote-access.ts, src/app/task-attention.ts, src/app/task-close-state.ts, src/app/task-command-dispatch.ts, src/app/task-command-lease-runtime-subscriptions.ts, src/app/task-command-lease-runtime.ts, src/app/task-command-lease-session.ts, src/app/task-command-lease-takeover.ts, src/app/task-command-lease.ts, src/app/task-convergence.ts, src/app/task-lifecycle-workflows.ts, src/app/task-presentation-status.ts, src/app/task-prompt-workflows.ts, src/app/task-review-state.ts, src/app/task-shell-workflows.ts, src/app/task-workflows.ts, src/lib/runtime-client-id.ts, src/store/agent-output-activity.ts, src/store/agents.ts, src/store/auto-trust.ts, src/store/client-session.ts, src/store/completion.ts, src/store/core.ts, src/store/focus.ts, src/store/keyed-snapshot-record.ts, src/store/navigation.ts, src/store/notification.ts, src/store/peer-presence.ts, src/store/persistence-codecs.ts, src/store/persistence-load-context.ts, src/store/persistence-load.ts, src/store/persistence-projects.ts, src/store/persistence-save.ts, src/store/persistence-terminal-restore.ts, src/store/persistence.ts, src/store/projects.ts, src/store/remote.ts, src/store/review.ts, src/store/state.ts, src/store/store.ts, src/store/task-command-controllers.ts, src/store/task-command-takeovers.ts, src/store/task-git-status.ts, src/store/task-state-cleanup.ts, src/store/taskStatus.ts, src/store/tasks.ts, src/store/terminals.ts, src/store/ui.ts form a dependency cycle
  - impact: The cycle prevents clean layering and makes initialization order and refactors harder to isolate.

## Debt Signals

- `large_file` `server/browser-control-plane.ts` score 8400: File 'server/browser-control-plane.ts' is 1054 lines, above the typescript threshold of 500
- `large_file` `src/components/terminal-view/terminal-session.ts` score 8400: File 'src/components/terminal-view/terminal-session.ts' is 1856 lines, above the typescript threshold of 500
- `dependency_sprawl` `src/App.tsx` score 7986: File 'src/App.tsx' depends on 28 real surfaces, above the typescript threshold of 15
- `dependency_sprawl` `src/components/TaskPanel.tsx` score 7906: File 'src/components/TaskPanel.tsx' depends on 28 real surfaces, above the typescript threshold of 15
- `hotspot` `src/components/terminal-view/terminal-session.ts` score 7800: File 'src/components/terminal-view/terminal-session.ts' is carrying coordination hotspot pressure

## Debt Clusters

- `cluster:src/App.tsx|src/remote/App.tsx|src/runtime/browser-session.ts` score 9986: Files src/App.tsx, src/remote/App.tsx, src/runtime/browser-session.ts intersect 6 debt signals: dependency_sprawl, clone_family, dead_private_code_cluster, large_file, concept
- `cluster:server/browser-control-plane.ts|electron/remote/server.ts|electron/remote/ws-transport.test.ts|tests/harness/websocket-contract-harness.ts` score 9900: Files server/browser-control-plane.ts, electron/remote/server.ts, electron/remote/ws-transport.test.ts, and 1 more intersect 4 debt signals: large_file, clone_family, dead_private_code_cluster, hotspot
- `cluster:src/components/terminal-view/terminal-session.ts` score 9900: File 'src/components/terminal-view/terminal-session.ts' intersects 4 debt signals: large_file, dead_private_code_cluster, dependency_sprawl, hotspot
- `cluster:src/components/TaskPanel.tsx` score 9406: File 'src/components/TaskPanel.tsx' intersects 4 debt signals: dependency_sprawl, large_file, dead_private_code_cluster, hotspot
- `cluster:src/store/review.ts|src/app/agent-catalog.ts|src/app/remote-access.ts|src/app/task-attention.ts|src/app/task-close-state.ts|src/app/task-command-dispatch.ts|src/app/task-command-lease-runtime-subscriptions.ts|src/app/task-command-lease-runtime.ts|src/app/task-command-lease-session.ts|src/app/task-command-lease-takeover.ts|src/app/task-command-lease.ts|src/app/task-convergence.ts|src/app/task-lifecycle-workflows.ts|src/app/task-presentation-status.ts|src/app/task-prompt-workflows.ts|src/app/task-review-state.ts|src/app/task-shell-workflows.ts|src/app/task-workflows.ts|src/lib/runtime-client-id.ts|src/store/agent-output-activity.ts|src/store/agents.ts|src/store/auto-trust.ts|src/store/client-session.ts|src/store/completion.ts|src/store/core.ts|src/store/focus.ts|src/store/keyed-snapshot-record.ts|src/store/navigation.ts|src/store/notification.ts|src/store/peer-presence.ts|src/store/persistence-codecs.ts|src/store/persistence-load-context.ts|src/store/persistence-load.ts|src/store/persistence-projects.ts|src/store/persistence-save.ts|src/store/persistence-terminal-restore.ts|src/store/persistence.ts|src/store/projects.ts|src/store/remote.ts|src/store/state.ts|src/store/store.ts|src/store/task-command-controllers.ts|src/store/task-command-takeovers.ts|src/store/task-git-status.ts|src/store/task-state-cleanup.ts|src/store/taskStatus.ts|src/store/tasks.ts|src/store/terminals.ts|src/store/ui.ts` score 9278: Files src/store/review.ts, src/app/agent-catalog.ts, src/app/remote-access.ts, and 46 more intersect 16 debt signals: dead_private_code_cluster, cycle_cluster, hotspot, dependency_sprawl, unstable_hotspot

## Watchpoints

- `ConnectionBannerState` score 4700: Concept 'ConnectionBannerState' intersects clone overlap, coordination hotspot overlap

## Proof Targets

1. Ownership/boundary: `n/a`
2. Propagation/obligations: `task_presentation_status`
3. Duplication/hotspot: clone cluster:src/App.tsx|src/remote/App.tsx|src/runtime/browser-session.ts / hotspot src/components/terminal-view/terminal-session.ts

## Benchmark Baseline

- cold process total: 16772.8 ms
- warm cached total: 888.7 ms
- warm patch-safety total: 4149.9 ms
