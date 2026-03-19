# Parallel-Code Proof Snapshot

Generated from: `<sentrux-root>/docs/v2/examples/parallel-code-head-golden`
Benchmark: `<sentrux-root>/docs/v2/examples/parallel-code-benchmark.json`

## Freshness

- analysis mode: `head_clone`
- commit: `ba6954bbd37c4bc2a2147672335bab3f51530b44`
- dirty paths: `0`
- dirty-path fingerprint: `53c1562521679823f3ee3c10c2585dec4c3ecd862d145f868947047fd32725ac`
- tree fingerprint: `d40dadfe68e7261ef2d8911f450c38de8e14e78791d015580de50ac28bdf3b24`
- analyzed tree fingerprint: `d40dadfe68e7261ef2d8911f450c38de8e14e78791d015580de50ac28bdf3b24`
- rules sha256: `548daed030bff265a920dc8dc68c0665d4bf6d3768127a1668d74887f7d4c6fa`
- binary sha256: `a20f59ee9ec7cfcb8c235eae2631c58b8d72e63ab6b513b548633ca6c19d8334`
- dirty path list: none

## Top Findings

- `watchpoint` `watchpoint` `architecture_signal` `very_high_signal` `cycle_cluster` Files src/app/agent-catalog.ts, src/app/remote-access.ts, src/app/task-attention.ts, src/app/task-close-state.ts, src/app/task-command-dispatch.ts, src/app/task-command-lease-runtime-subscriptions.ts, src/app/task-command-lease-runtime.ts, src/app/task-command-lease-session.ts, src/app/task-command-lease-takeover.ts, src/app/task-command-lease.ts, src/app/task-convergence.ts, src/app/task-lifecycle-workflows.ts, src/app/task-presentation-status.ts, src/app/task-prompt-workflows.ts, src/app/task-review-state.ts, src/app/task-shell-workflows.ts, src/app/task-workflows.ts, src/lib/runtime-client-id.ts, src/store/agent-output-activity.ts, src/store/agents.ts, src/store/auto-trust.ts, src/store/client-session.ts, src/store/completion.ts, src/store/core.ts, src/store/focus.ts, src/store/keyed-snapshot-record.ts, src/store/navigation.ts, src/store/notification.ts, src/store/peer-presence.ts, src/store/persistence-codecs.ts, src/store/persistence-load-context.ts, src/store/persistence-load.ts, src/store/persistence-projects.ts, src/store/persistence-save.ts, src/store/persistence-terminal-restore.ts, src/store/persistence.ts, src/store/projects.ts, src/store/remote.ts, src/store/review.ts, src/store/sidebar-order.ts, src/store/state.ts, src/store/store.ts, src/store/task-command-controllers.ts, src/store/task-command-takeovers.ts, src/store/task-git-status.ts, src/store/task-state-cleanup.ts, src/store/taskStatus.ts, src/store/tasks.ts, src/store/terminals.ts, src/store/ui.ts form a dependency cycle
  - ranking reasons: `shared_barrel_boundary_hub, guardrail_backed_boundary_hub, mixed_cycle_architecture_pressure`
- `trusted` `structural_debt` `local_refactor_target` `high_signal` `dependency_sprawl` File 'src/components/TaskPanel.tsx' depends on 28 real surfaces, above the typescript threshold of 15
  - ranking reasons: `extracted_owner_shell, guardrail_backed_refactor_surface, contained_refactor_surface`
- `trusted` `guarded_facade` `boundary_discipline` `high_signal` `unstable_hotspot` Guarded transport facade 'src/lib/ipc.ts' has 68 inbound references and remains unstable
  - ranking reasons: `facade_boundary_surface`
- `trusted` `structural_debt` `regrowth_watchpoint` `high_signal` `dependency_sprawl` Composition root 'src/App.tsx' depends on 33 real surfaces, above the typescript threshold of 15
  - ranking reasons: `composition_root_breadth`
- `trusted` `structural_debt` `secondary_cleanup` `high_signal` `dependency_sprawl` File 'src/components/terminal-view/terminal-session.ts' depends on 22 real surfaces, above the typescript threshold of 15
  - ranking reasons: `secondary_facade_pressure, hotspot_overlap, multi_signal_cleanup_overlap`
- `trusted` `hardening_note` `hardening_note` `high_signal` `closed_domain_exhaustiveness` Closed domain 'ConnectionBannerState' is missing coverage for variants: connecting, reconnecting, restoring
  - ranking reasons: `narrow_surface_hardening`
- `trusted` `tooling_debt` `tooling_debt` `high_signal` `large_file` File 'scripts/session-stress.mjs' is 2048 lines, above the javascript threshold of 500
  - ranking reasons: `tooling_maintenance_surface`

## Experimental Findings

- `high` `dead_private_code_cluster` File 'src/components/ScrollingDiffView.tsx' contains 18 uncalled private functions totaling 272 lines
- `high` `dead_private_code_cluster` File 'src/store/review.ts' contains 10 uncalled private functions totaling 121 lines
- `high` `dead_private_code_cluster` File 'src/components/PreviewPanel.tsx' contains 21 uncalled private functions totaling 115 lines
- `high` `dead_private_code_cluster` File 'electron/remote/server.ts' contains 3 uncalled private functions totaling 193 lines
- `high` `dead_private_code_cluster` File 'src/components/SidebarTaskRow.tsx' contains 17 uncalled private functions totaling 103 lines
- `high` `dead_private_code_cluster` File 'src/remote/ws.ts' contains 5 uncalled private functions totaling 89 lines
- `high` `dead_private_code_cluster` File 'src/lib/terminalLatency.ts' contains 6 uncalled private functions totaling 79 lines
- `medium` `dead_private_code_cluster` File 'src/remote/touch-gestures.ts' contains 6 uncalled private functions totaling 74 lines
- `medium` `dead_private_code_cluster` File 'src/arena/ResultsScreen.tsx' contains 7 uncalled private functions totaling 62 lines
- `medium` `dead_private_code_cluster` File 'src/lib/drag-reorder.ts' contains 4 uncalled private functions totaling 60 lines

## Concept Summaries

- `ConnectionBannerState` `supporting_signal`: Concept 'ConnectionBannerState' has 1 high-severity ownership or access findings
- `task_presentation_status` `supporting_signal`: Concept 'task_presentation_status' spans 1 obligation reports with 1 missing update sites

## Finding Details

- `trusted` `high` `closed_domain_exhaustiveness` `ConnectionBannerState`: Closed domain 'ConnectionBannerState' is missing coverage for variants: connecting, reconnecting, restoring
  - impact: Finite-domain changes can silently miss one surface unless all required cases stay in sync.
- `watchpoint` `high` `cycle_cluster` `cycle:src/app/agent-catalog.ts|src/app/remote-access.ts|src/app/task-attention.ts|src/app/task-close-state.ts|src/app/task-command-dispatch.ts|src/app/task-command-lease-runtime-subscriptions.ts|src/app/task-command-lease-runtime.ts|src/app/task-command-lease-session.ts|src/app/task-command-lease-takeover.ts|src/app/task-command-lease.ts|src/app/task-convergence.ts|src/app/task-lifecycle-workflows.ts|src/app/task-presentation-status.ts|src/app/task-prompt-workflows.ts|src/app/task-review-state.ts|src/app/task-shell-workflows.ts|src/app/task-workflows.ts|src/lib/runtime-client-id.ts|src/store/agent-output-activity.ts|src/store/agents.ts|src/store/auto-trust.ts|src/store/client-session.ts|src/store/completion.ts|src/store/core.ts|src/store/focus.ts|src/store/keyed-snapshot-record.ts|src/store/navigation.ts|src/store/notification.ts|src/store/peer-presence.ts|src/store/persistence-codecs.ts|src/store/persistence-load-context.ts|src/store/persistence-load.ts|src/store/persistence-projects.ts|src/store/persistence-save.ts|src/store/persistence-terminal-restore.ts|src/store/persistence.ts|src/store/projects.ts|src/store/remote.ts|src/store/review.ts|src/store/sidebar-order.ts|src/store/state.ts|src/store/store.ts|src/store/task-command-controllers.ts|src/store/task-command-takeovers.ts|src/store/task-git-status.ts|src/store/task-state-cleanup.ts|src/store/taskStatus.ts|src/store/tasks.ts|src/store/terminals.ts|src/store/ui.ts`: Files src/app/agent-catalog.ts, src/app/remote-access.ts, src/app/task-attention.ts, src/app/task-close-state.ts, src/app/task-command-dispatch.ts, src/app/task-command-lease-runtime-subscriptions.ts, src/app/task-command-lease-runtime.ts, src/app/task-command-lease-session.ts, src/app/task-command-lease-takeover.ts, src/app/task-command-lease.ts, src/app/task-convergence.ts, src/app/task-lifecycle-workflows.ts, src/app/task-presentation-status.ts, src/app/task-prompt-workflows.ts, src/app/task-review-state.ts, src/app/task-shell-workflows.ts, src/app/task-workflows.ts, src/lib/runtime-client-id.ts, src/store/agent-output-activity.ts, src/store/agents.ts, src/store/auto-trust.ts, src/store/client-session.ts, src/store/completion.ts, src/store/core.ts, src/store/focus.ts, src/store/keyed-snapshot-record.ts, src/store/navigation.ts, src/store/notification.ts, src/store/peer-presence.ts, src/store/persistence-codecs.ts, src/store/persistence-load-context.ts, src/store/persistence-load.ts, src/store/persistence-projects.ts, src/store/persistence-save.ts, src/store/persistence-terminal-restore.ts, src/store/persistence.ts, src/store/projects.ts, src/store/remote.ts, src/store/review.ts, src/store/sidebar-order.ts, src/store/state.ts, src/store/store.ts, src/store/task-command-controllers.ts, src/store/task-command-takeovers.ts, src/store/task-git-status.ts, src/store/task-state-cleanup.ts, src/store/taskStatus.ts, src/store/tasks.ts, src/store/terminals.ts, src/store/ui.ts form a dependency cycle
  - impact: The cycle touches a component-facing barrel, which makes it harder to keep broad component access separate from deeper app and runtime seams.
- `trusted` `high` `dependency_sprawl` `src/App.tsx`: Composition root 'src/App.tsx' depends on 33 real surfaces, above the typescript threshold of 15
  - impact: Broad dependency fan-out in a composition root makes shell wiring and runtime ownership harder to keep separate.
- `trusted` `high` `dependency_sprawl` `src/components/TaskPanel.tsx`: File 'src/components/TaskPanel.tsx' depends on 28 real surfaces, above the typescript threshold of 15
  - impact: Broad dependency fan-out expands change surface and makes orchestration drift harder to localize.
- `trusted` `high` `unstable_hotspot` `src/store/store.ts`: Component-facing barrel 'src/store/store.ts' has 48 inbound references and remains unstable
  - impact: A volatile component-facing barrel makes it harder to keep presentation access broad while keeping deeper orchestration changes contained.
- `watchpoint` `high` `cycle_cluster` `cycle:electron/ipc/handlers.ts|electron/ipc/notification-handlers.ts`: Files electron/ipc/handlers.ts, electron/ipc/notification-handlers.ts form a dependency cycle
  - impact: The cycle prevents clean layering and makes initialization order and refactors harder to isolate.
- `trusted` `high` `large_file` `scripts/session-stress.mjs`: File 'scripts/session-stress.mjs' is 2048 lines, above the javascript threshold of 500
  - impact: Responsibility concentration increases review friction and makes later splits harder to isolate.
- `trusted` `high` `unstable_hotspot` `src/lib/ipc.ts`: Guarded transport facade 'src/lib/ipc.ts' has 68 inbound references and remains unstable
  - impact: A transport facade with heavy fan-in needs clear ownership boundaries so lifecycle or domain logic does not leak into transport glue.
- `trusted` `high` `dependency_sprawl` `src/components/terminal-view/terminal-session.ts`: File 'src/components/terminal-view/terminal-session.ts' depends on 22 real surfaces, above the typescript threshold of 15
  - impact: Broad dependency fan-out expands change surface and makes orchestration drift harder to localize.
- `trusted` `high` `dependency_sprawl` `src/components/ReviewPanel.tsx`: File 'src/components/ReviewPanel.tsx' depends on 22 real surfaces, above the typescript threshold of 15
  - impact: Broad dependency fan-out expands change surface and makes orchestration drift harder to localize.

## Debt Signals

- `trusted` `dependency_sprawl` `src/App.tsx` `high_signal`: Composition root 'src/App.tsx' depends on 33 real surfaces, above the typescript threshold of 15
- `trusted` `dependency_sprawl` `src/components/TaskPanel.tsx` `high_signal`: File 'src/components/TaskPanel.tsx' depends on 28 real surfaces, above the typescript threshold of 15
- `trusted` `unstable_hotspot` `src/store/store.ts` `high_signal`: Component-facing barrel 'src/store/store.ts' has 48 inbound references and remains unstable
- `trusted` `hotspot` `src/components/PromptInput.tsx` `high_signal`: File 'src/components/PromptInput.tsx' is carrying coordination hotspot pressure
- `trusted` `hotspot` `src/components/terminal-view/terminal-session.ts` `high_signal`: File 'src/components/terminal-view/terminal-session.ts' is carrying coordination hotspot pressure

## Experimental Debt Signals

- `dead_private_code_cluster` `src/components/ScrollingDiffView.tsx` `high_signal`: File 'src/components/ScrollingDiffView.tsx' contains 18 uncalled private functions totaling 272 lines
- `dead_private_code_cluster` `src/store/review.ts` `high_signal`: File 'src/store/review.ts' contains 10 uncalled private functions totaling 121 lines
- `dead_private_code_cluster` `src/components/PreviewPanel.tsx` `high_signal`: File 'src/components/PreviewPanel.tsx' contains 21 uncalled private functions totaling 115 lines
- `dead_private_code_cluster` `electron/remote/server.ts` `high_signal`: File 'electron/remote/server.ts' contains 3 uncalled private functions totaling 193 lines
- `dead_private_code_cluster` `src/components/SidebarTaskRow.tsx` `high_signal`: File 'src/components/SidebarTaskRow.tsx' contains 17 uncalled private functions totaling 103 lines

## Debt Clusters

- `trusted` `cluster:src/store/store.ts|src/app/agent-catalog.ts|src/app/remote-access.ts|src/app/task-attention.ts|src/app/task-close-state.ts|src/app/task-command-dispatch.ts|src/app/task-command-lease-runtime-subscriptions.ts|src/app/task-command-lease-runtime.ts|src/app/task-command-lease-session.ts|src/app/task-command-lease-takeover.ts|src/app/task-command-lease.ts|src/app/task-convergence.ts|src/app/task-lifecycle-workflows.ts|src/app/task-presentation-status.ts|src/app/task-prompt-workflows.ts|src/app/task-review-state.ts|src/app/task-shell-workflows.ts|src/app/task-workflows.ts|src/lib/runtime-client-id.ts|src/store/agent-output-activity.ts|src/store/agents.ts|src/store/auto-trust.ts|src/store/client-session.ts|src/store/completion.ts|src/store/core.ts|src/store/focus.ts|src/store/keyed-snapshot-record.ts|src/store/navigation.ts|src/store/notification.ts|src/store/peer-presence.ts|src/store/persistence-codecs.ts|src/store/persistence-load-context.ts|src/store/persistence-load.ts|src/store/persistence-projects.ts|src/store/persistence-save.ts|src/store/persistence-terminal-restore.ts|src/store/persistence.ts|src/store/projects.ts|src/store/remote.ts|src/store/review.ts|src/store/sidebar-order.ts|src/store/state.ts|src/store/task-command-controllers.ts|src/store/task-command-takeovers.ts|src/store/task-git-status.ts|src/store/task-state-cleanup.ts|src/store/taskStatus.ts|src/store/tasks.ts|src/store/terminals.ts|src/store/ui.ts` `very_high_signal`: Files src/store/store.ts, src/app/agent-catalog.ts, src/app/remote-access.ts, and 47 more intersect 10 debt signals: unstable_hotspot, cycle_cluster, hotspot, large_file, dependency_sprawl
- `trusted` `cluster:src/App.tsx|src/remote/App.tsx` `very_high_signal`: Files src/App.tsx, src/remote/App.tsx intersect 2 debt signals: dependency_sprawl, clone_family
- `trusted` `cluster:src/components/terminal-view/terminal-session.ts` `high_signal`: File 'src/components/terminal-view/terminal-session.ts' intersects 3 debt signals: hotspot, large_file, dependency_sprawl
- `trusted` `cluster:src/components/PromptInput.tsx` `high_signal`: File 'src/components/PromptInput.tsx' intersects 2 debt signals: hotspot, large_file
- `trusted` `cluster:electron/ipc/handlers.ts|electron/ipc/notification-handlers.ts` `high_signal`: Files electron/ipc/handlers.ts, electron/ipc/notification-handlers.ts intersect 2 debt signals: dependency_sprawl, cycle_cluster

## Watchpoints

- `watchpoint` `cycle:src/app/agent-catalog.ts|src/app/remote-access.ts|src/app/task-attention.ts|src/app/task-close-state.ts|src/app/task-command-dispatch.ts|src/app/task-command-lease-runtime-subscriptions.ts|src/app/task-command-lease-runtime.ts|src/app/task-command-lease-session.ts|src/app/task-command-lease-takeover.ts|src/app/task-command-lease.ts|src/app/task-convergence.ts|src/app/task-lifecycle-workflows.ts|src/app/task-presentation-status.ts|src/app/task-prompt-workflows.ts|src/app/task-review-state.ts|src/app/task-shell-workflows.ts|src/app/task-workflows.ts|src/lib/runtime-client-id.ts|src/store/agent-output-activity.ts|src/store/agents.ts|src/store/auto-trust.ts|src/store/client-session.ts|src/store/completion.ts|src/store/core.ts|src/store/focus.ts|src/store/keyed-snapshot-record.ts|src/store/navigation.ts|src/store/notification.ts|src/store/peer-presence.ts|src/store/persistence-codecs.ts|src/store/persistence-load-context.ts|src/store/persistence-load.ts|src/store/persistence-projects.ts|src/store/persistence-save.ts|src/store/persistence-terminal-restore.ts|src/store/persistence.ts|src/store/projects.ts|src/store/remote.ts|src/store/review.ts|src/store/sidebar-order.ts|src/store/state.ts|src/store/store.ts|src/store/task-command-controllers.ts|src/store/task-command-takeovers.ts|src/store/task-git-status.ts|src/store/task-state-cleanup.ts|src/store/taskStatus.ts|src/store/tasks.ts|src/store/terminals.ts|src/store/ui.ts` `very_high_signal`: Files src/app/agent-catalog.ts, src/app/remote-access.ts, src/app/task-attention.ts, src/app/task-close-state.ts, src/app/task-command-dispatch.ts, src/app/task-command-lease-runtime-subscriptions.ts, src/app/task-command-lease-runtime.ts, src/app/task-command-lease-session.ts, src/app/task-command-lease-takeover.ts, src/app/task-command-lease.ts, src/app/task-convergence.ts, src/app/task-lifecycle-workflows.ts, src/app/task-presentation-status.ts, src/app/task-prompt-workflows.ts, src/app/task-review-state.ts, src/app/task-shell-workflows.ts, src/app/task-workflows.ts, src/lib/runtime-client-id.ts, src/store/agent-output-activity.ts, src/store/agents.ts, src/store/auto-trust.ts, src/store/client-session.ts, src/store/completion.ts, src/store/core.ts, src/store/focus.ts, src/store/keyed-snapshot-record.ts, src/store/navigation.ts, src/store/notification.ts, src/store/peer-presence.ts, src/store/persistence-codecs.ts, src/store/persistence-load-context.ts, src/store/persistence-load.ts, src/store/persistence-projects.ts, src/store/persistence-save.ts, src/store/persistence-terminal-restore.ts, src/store/persistence.ts, src/store/projects.ts, src/store/remote.ts, src/store/review.ts, src/store/sidebar-order.ts, src/store/state.ts, src/store/store.ts, src/store/task-command-controllers.ts, src/store/task-command-takeovers.ts, src/store/task-git-status.ts, src/store/task-state-cleanup.ts, src/store/taskStatus.ts, src/store/tasks.ts, src/store/terminals.ts, src/store/ui.ts form a dependency cycle
- `watchpoint` `cycle:electron/ipc/handlers.ts|electron/ipc/notification-handlers.ts` `high_signal`: Files electron/ipc/handlers.ts, electron/ipc/notification-handlers.ts form a dependency cycle
- `watchpoint` `clone-family-0x7e50d49dc16ef925` `supporting_signal`: 4 exact clone groups repeat across 2 files and churn differs by 0 recent commit(s) across siblings; sibling file age spans 1 day(s)
- `watchpoint` `clone-family-0x9ebb8dad5cafb9c0` `supporting_signal`: 4 exact clone groups repeat across 2 files and churn differs by 3 recent commit(s) across siblings; sibling file age spans 0 day(s)
- `watchpoint` `server/browser-channels.ts` `moderate_signal`: File 'server/browser-channels.ts' is carrying coordination hotspot pressure

## Proof Targets

1. Ownership/boundary: `n/a`
2. Propagation/obligations: `task_presentation_status`
3. Duplication/hotspot: clone cluster:src/App.tsx|src/remote/App.tsx / hotspot src/components/PromptInput.tsx

## Benchmark Baseline

- cold process total: 16772.8 ms
- warm cached total: 888.7 ms
- warm patch-safety total: 4149.9 ms
