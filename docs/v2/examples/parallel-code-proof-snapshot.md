# Parallel-Code Proof Snapshot

Generated from: `<sentrux-root>/docs/v2/examples/parallel-code-golden`
Benchmark: `<sentrux-root>/docs/v2/examples/parallel-code-benchmark.json`

## Top Findings

- `trusted` `high` `closed_domain_exhaustiveness` (ConnectionBannerState) Closed domain 'ConnectionBannerState' is missing coverage for variants: connecting, reconnecting, restoring
- `trusted` `high` `large_file` File 'server/browser-control-plane.ts' is 1054 lines, above the typescript threshold of 500
- `trusted` `high` `large_file` File 'src/components/terminal-view/terminal-session.ts' is 1856 lines, above the typescript threshold of 500
- `trusted` `high` `dependency_sprawl` File 'src/App.tsx' depends on 28 real surfaces, above the typescript threshold of 15
- `trusted` `high` `dependency_sprawl` File 'src/components/TaskPanel.tsx' depends on 28 real surfaces, above the typescript threshold of 15
- `trusted` `high` `dependency_sprawl` File 'src/components/terminal-view/terminal-session.ts' depends on 25 real surfaces, above the typescript threshold of 15
- `watchpoint` `high` `cycle_cluster` Files src/app/agent-catalog.ts, src/app/remote-access.ts, src/app/task-attention.ts, src/app/task-close-state.ts, src/app/task-command-dispatch.ts, src/app/task-command-lease-runtime-subscriptions.ts, src/app/task-command-lease-runtime.ts, src/app/task-command-lease-session.ts, src/app/task-command-lease-takeover.ts, src/app/task-command-lease.ts, src/app/task-convergence.ts, src/app/task-lifecycle-workflows.ts, src/app/task-presentation-status.ts, src/app/task-prompt-workflows.ts, src/app/task-review-state.ts, src/app/task-shell-workflows.ts, src/app/task-workflows.ts, src/lib/runtime-client-id.ts, src/store/agent-output-activity.ts, src/store/agents.ts, src/store/auto-trust.ts, src/store/client-session.ts, src/store/completion.ts, src/store/core.ts, src/store/focus.ts, src/store/keyed-snapshot-record.ts, src/store/navigation.ts, src/store/notification.ts, src/store/peer-presence.ts, src/store/persistence-codecs.ts, src/store/persistence-load-context.ts, src/store/persistence-load.ts, src/store/persistence-projects.ts, src/store/persistence-save.ts, src/store/persistence-terminal-restore.ts, src/store/persistence.ts, src/store/projects.ts, src/store/remote.ts, src/store/review.ts, src/store/state.ts, src/store/store.ts, src/store/task-command-controllers.ts, src/store/task-command-takeovers.ts, src/store/task-git-status.ts, src/store/task-state-cleanup.ts, src/store/taskStatus.ts, src/store/tasks.ts, src/store/terminals.ts, src/store/ui.ts form a dependency cycle
- `trusted` `high` `unstable_hotspot` File 'src/store/store.ts' has 56 inbound references and remains unstable
- `trusted` `high` `unstable_hotspot` File 'src/lib/ipc.ts' has 63 inbound references and remains unstable
- `trusted` `high` `large_file` File 'scripts/session-stress.mjs' is 2048 lines, above the javascript threshold of 500

## Experimental Findings

- `high` `dead_private_code_cluster` File 'src/components/ScrollingDiffView.tsx' contains 15 uncalled private functions totaling 144 lines
- `high` `dead_private_code_cluster` File 'src/store/review.ts' contains 10 uncalled private functions totaling 121 lines
- `high` `dead_private_code_cluster` File 'src/components/PreviewPanel.tsx' contains 21 uncalled private functions totaling 115 lines
- `high` `dead_private_code_cluster` File 'src/components/SidebarTaskRow.tsx' contains 19 uncalled private functions totaling 109 lines
- `high` `dead_private_code_cluster` File 'electron/remote/server.ts' contains 3 uncalled private functions totaling 193 lines
- `high` `dead_private_code_cluster` File 'src/remote/ws.ts' contains 5 uncalled private functions totaling 89 lines
- `high` `dead_private_code_cluster` File 'src/lib/terminalLatency.ts' contains 6 uncalled private functions totaling 79 lines
- `medium` `dead_private_code_cluster` File 'src/remote/touch-gestures.ts' contains 6 uncalled private functions totaling 74 lines
- `medium` `dead_private_code_cluster` File 'src/arena/ResultsScreen.tsx' contains 7 uncalled private functions totaling 62 lines
- `medium` `dead_private_code_cluster` File 'src/lib/drag-reorder.ts' contains 4 uncalled private functions totaling 60 lines

## Concept Summaries

- `ConnectionBannerState` score 3100: Concept 'ConnectionBannerState' has 1 high-severity ownership or access findings
- `task_presentation_status` score 1680: Concept 'task_presentation_status' spans 1 obligation reports with 1 missing update sites

## Finding Details

- `trusted` `high` `closed_domain_exhaustiveness` `ConnectionBannerState`: Closed domain 'ConnectionBannerState' is missing coverage for variants: connecting, reconnecting, restoring
  - impact: Finite-domain changes can silently miss one surface unless all required cases stay in sync.
- `trusted` `high` `large_file` `server/browser-control-plane.ts`: File 'server/browser-control-plane.ts' is 1054 lines, above the typescript threshold of 500
  - impact: Responsibility concentration increases review friction and makes later splits harder to isolate.
- `trusted` `high` `large_file` `src/components/terminal-view/terminal-session.ts`: File 'src/components/terminal-view/terminal-session.ts' is 1856 lines, above the typescript threshold of 500
  - impact: Responsibility concentration increases review friction and makes later splits harder to isolate.
- `trusted` `high` `dependency_sprawl` `src/App.tsx`: File 'src/App.tsx' depends on 28 real surfaces, above the typescript threshold of 15
  - impact: Broad dependency fan-out expands change surface and makes orchestration drift harder to localize.
- `trusted` `high` `dependency_sprawl` `src/components/TaskPanel.tsx`: File 'src/components/TaskPanel.tsx' depends on 28 real surfaces, above the typescript threshold of 15
  - impact: Broad dependency fan-out expands change surface and makes orchestration drift harder to localize.
- `trusted` `high` `dependency_sprawl` `src/components/terminal-view/terminal-session.ts`: File 'src/components/terminal-view/terminal-session.ts' depends on 25 real surfaces, above the typescript threshold of 15
  - impact: Broad dependency fan-out expands change surface and makes orchestration drift harder to localize.
- `watchpoint` `high` `cycle_cluster` `cycle:src/app/agent-catalog.ts|src/app/remote-access.ts|src/app/task-attention.ts|src/app/task-close-state.ts|src/app/task-command-dispatch.ts|src/app/task-command-lease-runtime-subscriptions.ts|src/app/task-command-lease-runtime.ts|src/app/task-command-lease-session.ts|src/app/task-command-lease-takeover.ts|src/app/task-command-lease.ts|src/app/task-convergence.ts|src/app/task-lifecycle-workflows.ts|src/app/task-presentation-status.ts|src/app/task-prompt-workflows.ts|src/app/task-review-state.ts|src/app/task-shell-workflows.ts|src/app/task-workflows.ts|src/lib/runtime-client-id.ts|src/store/agent-output-activity.ts|src/store/agents.ts|src/store/auto-trust.ts|src/store/client-session.ts|src/store/completion.ts|src/store/core.ts|src/store/focus.ts|src/store/keyed-snapshot-record.ts|src/store/navigation.ts|src/store/notification.ts|src/store/peer-presence.ts|src/store/persistence-codecs.ts|src/store/persistence-load-context.ts|src/store/persistence-load.ts|src/store/persistence-projects.ts|src/store/persistence-save.ts|src/store/persistence-terminal-restore.ts|src/store/persistence.ts|src/store/projects.ts|src/store/remote.ts|src/store/review.ts|src/store/state.ts|src/store/store.ts|src/store/task-command-controllers.ts|src/store/task-command-takeovers.ts|src/store/task-git-status.ts|src/store/task-state-cleanup.ts|src/store/taskStatus.ts|src/store/tasks.ts|src/store/terminals.ts|src/store/ui.ts`: Files src/app/agent-catalog.ts, src/app/remote-access.ts, src/app/task-attention.ts, src/app/task-close-state.ts, src/app/task-command-dispatch.ts, src/app/task-command-lease-runtime-subscriptions.ts, src/app/task-command-lease-runtime.ts, src/app/task-command-lease-session.ts, src/app/task-command-lease-takeover.ts, src/app/task-command-lease.ts, src/app/task-convergence.ts, src/app/task-lifecycle-workflows.ts, src/app/task-presentation-status.ts, src/app/task-prompt-workflows.ts, src/app/task-review-state.ts, src/app/task-shell-workflows.ts, src/app/task-workflows.ts, src/lib/runtime-client-id.ts, src/store/agent-output-activity.ts, src/store/agents.ts, src/store/auto-trust.ts, src/store/client-session.ts, src/store/completion.ts, src/store/core.ts, src/store/focus.ts, src/store/keyed-snapshot-record.ts, src/store/navigation.ts, src/store/notification.ts, src/store/peer-presence.ts, src/store/persistence-codecs.ts, src/store/persistence-load-context.ts, src/store/persistence-load.ts, src/store/persistence-projects.ts, src/store/persistence-save.ts, src/store/persistence-terminal-restore.ts, src/store/persistence.ts, src/store/projects.ts, src/store/remote.ts, src/store/review.ts, src/store/state.ts, src/store/store.ts, src/store/task-command-controllers.ts, src/store/task-command-takeovers.ts, src/store/task-git-status.ts, src/store/task-state-cleanup.ts, src/store/taskStatus.ts, src/store/tasks.ts, src/store/terminals.ts, src/store/ui.ts form a dependency cycle
  - impact: The cycle prevents clean layering and makes initialization order and refactors harder to isolate.
- `trusted` `high` `unstable_hotspot` `src/store/store.ts`: File 'src/store/store.ts' has 56 inbound references and remains unstable
  - impact: High fan-in plus instability increases blast radius and makes small edits harder to contain.
- `trusted` `high` `unstable_hotspot` `src/lib/ipc.ts`: File 'src/lib/ipc.ts' has 63 inbound references and remains unstable
  - impact: High fan-in plus instability increases blast radius and makes small edits harder to contain.
- `trusted` `high` `large_file` `scripts/session-stress.mjs`: File 'scripts/session-stress.mjs' is 2048 lines, above the javascript threshold of 500
  - impact: Responsibility concentration increases review friction and makes later splits harder to isolate.

## Debt Signals

- `trusted` `large_file` `server/browser-control-plane.ts` score 8400: File 'server/browser-control-plane.ts' is 1054 lines, above the typescript threshold of 500
- `trusted` `large_file` `src/components/terminal-view/terminal-session.ts` score 8400: File 'src/components/terminal-view/terminal-session.ts' is 1856 lines, above the typescript threshold of 500
- `trusted` `dependency_sprawl` `src/App.tsx` score 7986: File 'src/App.tsx' depends on 28 real surfaces, above the typescript threshold of 15
- `trusted` `dependency_sprawl` `src/components/TaskPanel.tsx` score 7906: File 'src/components/TaskPanel.tsx' depends on 28 real surfaces, above the typescript threshold of 15
- `trusted` `hotspot` `src/components/terminal-view/terminal-session.ts` score 7800: File 'src/components/terminal-view/terminal-session.ts' is carrying coordination hotspot pressure

## Experimental Debt Signals

- `dead_private_code_cluster` `src/components/ScrollingDiffView.tsx` score 7692: File 'src/components/ScrollingDiffView.tsx' contains 15 uncalled private functions totaling 144 lines
- `dead_private_code_cluster` `src/store/review.ts` score 7278: File 'src/store/review.ts' contains 10 uncalled private functions totaling 121 lines
- `dead_private_code_cluster` `src/components/PreviewPanel.tsx` score 7170: File 'src/components/PreviewPanel.tsx' contains 21 uncalled private functions totaling 115 lines
- `dead_private_code_cluster` `src/components/SidebarTaskRow.tsx` score 7062: File 'src/components/SidebarTaskRow.tsx' contains 19 uncalled private functions totaling 109 lines
- `dead_private_code_cluster` `electron/remote/server.ts` score 7000: File 'electron/remote/server.ts' contains 3 uncalled private functions totaling 193 lines

## Debt Clusters

- `trusted` `cluster:src/App.tsx|src/remote/App.tsx|src/runtime/browser-session.ts` score 9486: Files src/App.tsx, src/remote/App.tsx, src/runtime/browser-session.ts intersect 4 debt signals: dependency_sprawl, clone_family, large_file, concept
- `trusted` `cluster:server/browser-control-plane.ts|electron/remote/server.ts|electron/remote/ws-transport.test.ts|tests/harness/websocket-contract-harness.ts` score 9400: Files server/browser-control-plane.ts, electron/remote/server.ts, electron/remote/ws-transport.test.ts, and 1 more intersect 3 debt signals: large_file, clone_family, hotspot
- `trusted` `cluster:src/components/terminal-view/terminal-session.ts` score 9400: File 'src/components/terminal-view/terminal-session.ts' intersects 3 debt signals: large_file, dependency_sprawl, hotspot
- `trusted` `cluster:src/store/store.ts|src/app/agent-catalog.ts|src/app/remote-access.ts|src/app/task-attention.ts|src/app/task-close-state.ts|src/app/task-command-dispatch.ts|src/app/task-command-lease-runtime-subscriptions.ts|src/app/task-command-lease-runtime.ts|src/app/task-command-lease-session.ts|src/app/task-command-lease-takeover.ts|src/app/task-command-lease.ts|src/app/task-convergence.ts|src/app/task-lifecycle-workflows.ts|src/app/task-presentation-status.ts|src/app/task-prompt-workflows.ts|src/app/task-review-state.ts|src/app/task-shell-workflows.ts|src/app/task-workflows.ts|src/lib/runtime-client-id.ts|src/store/agent-output-activity.ts|src/store/agents.ts|src/store/auto-trust.ts|src/store/client-session.ts|src/store/completion.ts|src/store/core.ts|src/store/focus.ts|src/store/keyed-snapshot-record.ts|src/store/navigation.ts|src/store/notification.ts|src/store/peer-presence.ts|src/store/persistence-codecs.ts|src/store/persistence-load-context.ts|src/store/persistence-load.ts|src/store/persistence-projects.ts|src/store/persistence-save.ts|src/store/persistence-terminal-restore.ts|src/store/persistence.ts|src/store/projects.ts|src/store/remote.ts|src/store/review.ts|src/store/state.ts|src/store/task-command-controllers.ts|src/store/task-command-takeovers.ts|src/store/task-git-status.ts|src/store/task-state-cleanup.ts|src/store/taskStatus.ts|src/store/tasks.ts|src/store/terminals.ts|src/store/ui.ts` score 9162: Files src/store/store.ts, src/app/agent-catalog.ts, src/app/remote-access.ts, and 46 more intersect 9 debt signals: unstable_hotspot, cycle_cluster, hotspot, dependency_sprawl
- `trusted` `cluster:src/components/TaskPanel.tsx` score 8906: File 'src/components/TaskPanel.tsx' intersects 3 debt signals: dependency_sprawl, hotspot, large_file

## Watchpoints

- `watchpoint` `cycle:src/app/agent-catalog.ts|src/app/remote-access.ts|src/app/task-attention.ts|src/app/task-close-state.ts|src/app/task-command-dispatch.ts|src/app/task-command-lease-runtime-subscriptions.ts|src/app/task-command-lease-runtime.ts|src/app/task-command-lease-session.ts|src/app/task-command-lease-takeover.ts|src/app/task-command-lease.ts|src/app/task-convergence.ts|src/app/task-lifecycle-workflows.ts|src/app/task-presentation-status.ts|src/app/task-prompt-workflows.ts|src/app/task-review-state.ts|src/app/task-shell-workflows.ts|src/app/task-workflows.ts|src/lib/runtime-client-id.ts|src/store/agent-output-activity.ts|src/store/agents.ts|src/store/auto-trust.ts|src/store/client-session.ts|src/store/completion.ts|src/store/core.ts|src/store/focus.ts|src/store/keyed-snapshot-record.ts|src/store/navigation.ts|src/store/notification.ts|src/store/peer-presence.ts|src/store/persistence-codecs.ts|src/store/persistence-load-context.ts|src/store/persistence-load.ts|src/store/persistence-projects.ts|src/store/persistence-save.ts|src/store/persistence-terminal-restore.ts|src/store/persistence.ts|src/store/projects.ts|src/store/remote.ts|src/store/review.ts|src/store/state.ts|src/store/store.ts|src/store/task-command-controllers.ts|src/store/task-command-takeovers.ts|src/store/task-git-status.ts|src/store/task-state-cleanup.ts|src/store/taskStatus.ts|src/store/tasks.ts|src/store/terminals.ts|src/store/ui.ts` score 7162: Files src/app/agent-catalog.ts, src/app/remote-access.ts, src/app/task-attention.ts, src/app/task-close-state.ts, src/app/task-command-dispatch.ts, src/app/task-command-lease-runtime-subscriptions.ts, src/app/task-command-lease-runtime.ts, src/app/task-command-lease-session.ts, src/app/task-command-lease-takeover.ts, src/app/task-command-lease.ts, src/app/task-convergence.ts, src/app/task-lifecycle-workflows.ts, src/app/task-presentation-status.ts, src/app/task-prompt-workflows.ts, src/app/task-review-state.ts, src/app/task-shell-workflows.ts, src/app/task-workflows.ts, src/lib/runtime-client-id.ts, src/store/agent-output-activity.ts, src/store/agents.ts, src/store/auto-trust.ts, src/store/client-session.ts, src/store/completion.ts, src/store/core.ts, src/store/focus.ts, src/store/keyed-snapshot-record.ts, src/store/navigation.ts, src/store/notification.ts, src/store/peer-presence.ts, src/store/persistence-codecs.ts, src/store/persistence-load-context.ts, src/store/persistence-load.ts, src/store/persistence-projects.ts, src/store/persistence-save.ts, src/store/persistence-terminal-restore.ts, src/store/persistence.ts, src/store/projects.ts, src/store/remote.ts, src/store/review.ts, src/store/state.ts, src/store/store.ts, src/store/task-command-controllers.ts, src/store/task-command-takeovers.ts, src/store/task-git-status.ts, src/store/task-state-cleanup.ts, src/store/taskStatus.ts, src/store/tasks.ts, src/store/terminals.ts, src/store/ui.ts form a dependency cycle
- `watchpoint` `clone-family-0x7e50d49dc16ef925` score 86: 4 exact clone groups repeat across 2 files and churn differs by 0 recent commit(s) across siblings; sibling file age spans 2 day(s)
- `watchpoint` `clone-family-0x9ebb8dad5cafb9c0` score 78: 4 exact clone groups repeat across 2 files and churn differs by 3 recent commit(s) across siblings; sibling file age spans 0 day(s)
- `watchpoint` `server/browser-channels.ts` score 6433: File 'server/browser-channels.ts' is carrying coordination hotspot pressure
- `watchpoint` `src/lib/browser-http-ipc.ts` score 6400: File 'src/lib/browser-http-ipc.ts' is carrying coordination hotspot pressure

## Proof Targets

1. Ownership/boundary: `n/a`
2. Propagation/obligations: `task_presentation_status`
3. Duplication/hotspot: clone cluster:src/App.tsx|src/remote/App.tsx|src/runtime/browser-session.ts / hotspot src/components/terminal-view/terminal-session.ts

## Benchmark Baseline

- cold process total: 16772.8 ms
- warm cached total: 888.7 ms
- warm patch-safety total: 4149.9 ms
