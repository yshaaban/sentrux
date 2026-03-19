# Parallel-Code Proof Snapshot

Generated from: `<sentrux-root>/docs/v2/examples/parallel-code-head-golden`
Benchmark: `<sentrux-root>/docs/v2/examples/parallel-code-benchmark.json`

## Freshness

- analysis mode: `head_clone`
- commit: `76772b8a37d5de0d0ffba06130218c5f12e40511`
- dirty paths: `40`
- dirty-path fingerprint: `162e0a2b31f06fc89cd46ae27a65c66e3ae1d6fb203e1bfde5dd56e9d1b00c89`
- tree fingerprint: `eb4e0b159a20846aaa3b02cf14ada52f6ac461aa3d344d37f40b319a7cdd6af3`
- analyzed tree fingerprint: `a92bdf85a885d7fc99990337410748282da77ca0f14adcc122b474c15922d78b`
- rules sha256: `548daed030bff265a920dc8dc68c0665d4bf6d3768127a1668d74887f7d4c6fa`
- binary sha256: `3abb8a24a18351ba28557c8a5eaff7e7131e6bff9f86c4965a5d1ff9d994ce06`
- dirty path list:
  - `docs/ARCHITECTURE.md`
  - `docs/TESTING.md`
  - `docs/UPSTREAM-DIVERGENCE.md`
  - `src/App.test.tsx`
  - `src/App.tsx`
  - `src/app/desktop-session-startup.ts`
  - `src/app/desktop-session.test.ts`
  - `src/app/desktop-session.ts`
  - `src/app/task-notification-capabilities.test.tsx`
  - `src/app/task-notification-capabilities.ts`
  - `src/app/task-notification-runtime.test.tsx`
  - `src/components/DisplayNameDialog.tsx`
  - `src/components/IconButton.tsx`
  - `src/components/SettingsDialog.test.tsx`
  - `src/components/SettingsDialog.tsx`
  - `src/components/Sidebar.test.tsx`
  - `src/components/Sidebar.tsx`
  - `src/components/SidebarFooter.test.tsx`
  - `src/components/SidebarFooter.tsx`
  - `src/components/TerminalStartupChip.test.tsx`
  - `src/components/TerminalStartupChip.tsx`
  - `src/components/sidebar/SidebarProjectsSection.tsx`
  - `src/domain/task-notification.ts`
  - `src/store/client-session.test.ts`
  - `src/store/client-session.ts`
  - `src/store/core.ts`
  - `src/store/persistence-codecs.ts`
  - `src/store/persistence-legacy-state.ts`
  - `src/store/persistence-load.ts`
  - `src/store/persistence.test.ts`
  - `src/store/types.ts`
  - `src/store/ui.ts`
  - `src/test/store-test-helpers.ts`
  - `src/app/app-startup-status.test.ts`
  - `src/app/app-startup-status.ts`
  - `src/components/DisplayNameDialog.test.tsx`
  - `src/components/sidebar/SidebarProjectsSection.test.tsx`
  - `src/components/sidebar/SidebarSectionHeader.tsx`
  - `src/store/sidebar-sections.ts`
  - `src/store/task-notification-preference.ts`

## Top Findings

- `trusted` `structural_debt` `architecture_signal` `high` `unstable_hotspot` Component-facing barrel 'src/store/store.ts' has 48 inbound references and remains unstable
- `trusted` `structural_debt` `local_refactor_target` `high` `dependency_sprawl` File 'src/components/TaskPanel.tsx' depends on 28 real surfaces, above the typescript threshold of 15
- `trusted` `guarded_facade` `boundary_discipline` `high` `unstable_hotspot` Guarded transport facade 'src/lib/ipc.ts' has 68 inbound references and remains unstable
- `trusted` `structural_debt` `regrowth_watchpoint` `high` `dependency_sprawl` Composition root 'src/App.tsx' depends on 32 real surfaces, above the typescript threshold of 15
- `trusted` `structural_debt` `secondary_cleanup` `high` `dependency_sprawl` File 'src/components/terminal-view/terminal-session.ts' depends on 22 real surfaces, above the typescript threshold of 15
- `trusted` `hardening_note` `hardening_note` `high` `closed_domain_exhaustiveness` Closed domain 'ConnectionBannerState' is missing coverage for variants: connecting, reconnecting, restoring
- `trusted` `tooling_debt` `tooling_debt` `high` `large_file` File 'scripts/session-stress.mjs' is 2048 lines, above the javascript threshold of 500

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

- `ConnectionBannerState` score 3100: Concept 'ConnectionBannerState' has 1 high-severity ownership or access findings
- `task_presentation_status` score 1680: Concept 'task_presentation_status' spans 1 obligation reports with 1 missing update sites

## Finding Details

- `trusted` `high` `closed_domain_exhaustiveness` `ConnectionBannerState`: Closed domain 'ConnectionBannerState' is missing coverage for variants: connecting, reconnecting, restoring
  - impact: Finite-domain changes can silently miss one surface unless all required cases stay in sync.
- `watchpoint` `high` `cycle_cluster` `cycle:src/app/agent-catalog.ts|src/app/remote-access.ts|src/app/task-attention.ts|src/app/task-close-state.ts|src/app/task-command-dispatch.ts|src/app/task-command-lease-runtime-subscriptions.ts|src/app/task-command-lease-runtime.ts|src/app/task-command-lease-session.ts|src/app/task-command-lease-takeover.ts|src/app/task-command-lease.ts|src/app/task-convergence.ts|src/app/task-lifecycle-workflows.ts|src/app/task-presentation-status.ts|src/app/task-prompt-workflows.ts|src/app/task-review-state.ts|src/app/task-shell-workflows.ts|src/app/task-workflows.ts|src/lib/runtime-client-id.ts|src/store/agent-output-activity.ts|src/store/agents.ts|src/store/auto-trust.ts|src/store/client-session.ts|src/store/completion.ts|src/store/core.ts|src/store/focus.ts|src/store/keyed-snapshot-record.ts|src/store/navigation.ts|src/store/notification.ts|src/store/peer-presence.ts|src/store/persistence-codecs.ts|src/store/persistence-load-context.ts|src/store/persistence-load.ts|src/store/persistence-projects.ts|src/store/persistence-save.ts|src/store/persistence-terminal-restore.ts|src/store/persistence.ts|src/store/projects.ts|src/store/remote.ts|src/store/review.ts|src/store/sidebar-order.ts|src/store/state.ts|src/store/store.ts|src/store/task-command-controllers.ts|src/store/task-command-takeovers.ts|src/store/task-git-status.ts|src/store/task-state-cleanup.ts|src/store/taskStatus.ts|src/store/tasks.ts|src/store/terminals.ts|src/store/ui.ts`: Files src/app/agent-catalog.ts, src/app/remote-access.ts, src/app/task-attention.ts, src/app/task-close-state.ts, src/app/task-command-dispatch.ts, src/app/task-command-lease-runtime-subscriptions.ts, src/app/task-command-lease-runtime.ts, src/app/task-command-lease-session.ts, src/app/task-command-lease-takeover.ts, src/app/task-command-lease.ts, src/app/task-convergence.ts, src/app/task-lifecycle-workflows.ts, src/app/task-presentation-status.ts, src/app/task-prompt-workflows.ts, src/app/task-review-state.ts, src/app/task-shell-workflows.ts, src/app/task-workflows.ts, src/lib/runtime-client-id.ts, src/store/agent-output-activity.ts, src/store/agents.ts, src/store/auto-trust.ts, src/store/client-session.ts, src/store/completion.ts, src/store/core.ts, src/store/focus.ts, src/store/keyed-snapshot-record.ts, src/store/navigation.ts, src/store/notification.ts, src/store/peer-presence.ts, src/store/persistence-codecs.ts, src/store/persistence-load-context.ts, src/store/persistence-load.ts, src/store/persistence-projects.ts, src/store/persistence-save.ts, src/store/persistence-terminal-restore.ts, src/store/persistence.ts, src/store/projects.ts, src/store/remote.ts, src/store/review.ts, src/store/sidebar-order.ts, src/store/state.ts, src/store/store.ts, src/store/task-command-controllers.ts, src/store/task-command-takeovers.ts, src/store/task-git-status.ts, src/store/task-state-cleanup.ts, src/store/taskStatus.ts, src/store/tasks.ts, src/store/terminals.ts, src/store/ui.ts form a dependency cycle
  - impact: The cycle touches a component-facing barrel, which makes it harder to keep broad component access separate from deeper app and runtime seams.
- `trusted` `high` `dependency_sprawl` `src/App.tsx`: Composition root 'src/App.tsx' depends on 32 real surfaces, above the typescript threshold of 15
  - impact: Broad dependency fan-out in a composition root makes shell wiring and runtime ownership harder to keep separate.
- `trusted` `high` `dependency_sprawl` `src/components/TaskPanel.tsx`: File 'src/components/TaskPanel.tsx' depends on 28 real surfaces, above the typescript threshold of 15
  - impact: Broad dependency fan-out expands change surface and makes orchestration drift harder to localize.
- `trusted` `high` `unstable_hotspot` `src/store/store.ts`: Component-facing barrel 'src/store/store.ts' has 48 inbound references and remains unstable
  - impact: A volatile component-facing barrel makes it harder to keep presentation access broad while keeping deeper orchestration changes contained.
- `trusted` `high` `large_file` `scripts/session-stress.mjs`: File 'scripts/session-stress.mjs' is 2048 lines, above the javascript threshold of 500
  - impact: Responsibility concentration increases review friction and makes later splits harder to isolate.
- `trusted` `high` `unstable_hotspot` `src/lib/ipc.ts`: Guarded transport facade 'src/lib/ipc.ts' has 68 inbound references and remains unstable
  - impact: A transport facade with heavy fan-in needs clear ownership boundaries so lifecycle or domain logic does not leak into transport glue.
- `trusted` `high` `dependency_sprawl` `src/components/terminal-view/terminal-session.ts`: File 'src/components/terminal-view/terminal-session.ts' depends on 22 real surfaces, above the typescript threshold of 15
  - impact: Broad dependency fan-out expands change surface and makes orchestration drift harder to localize.
- `trusted` `high` `dependency_sprawl` `src/components/ReviewPanel.tsx`: File 'src/components/ReviewPanel.tsx' depends on 22 real surfaces, above the typescript threshold of 15
  - impact: Broad dependency fan-out expands change surface and makes orchestration drift harder to localize.
- `trusted` `high` `exact_clone_group` `electron/remote/ws-server.ts|server/browser-websocket.ts`: 2 functions share an identical normalized body across recently changed files
  - impact: Duplicate logic increases the chance that fixes land in one copy but not the others.

## Debt Signals

- `trusted` `dependency_sprawl` `src/App.tsx` score 8424: Composition root 'src/App.tsx' depends on 32 real surfaces, above the typescript threshold of 15
- `trusted` `dependency_sprawl` `src/components/TaskPanel.tsx` score 7906: File 'src/components/TaskPanel.tsx' depends on 28 real surfaces, above the typescript threshold of 15
- `trusted` `unstable_hotspot` `src/store/store.ts` score 7180: Component-facing barrel 'src/store/store.ts' has 48 inbound references and remains unstable
- `trusted` `hotspot` `src/components/PromptInput.tsx` score 7144: File 'src/components/PromptInput.tsx' is carrying coordination hotspot pressure
- `trusted` `hotspot` `src/components/terminal-view/terminal-session.ts` score 6800: File 'src/components/terminal-view/terminal-session.ts' is carrying coordination hotspot pressure

## Experimental Debt Signals

- `dead_private_code_cluster` `src/components/ScrollingDiffView.tsx` score 7900: File 'src/components/ScrollingDiffView.tsx' contains 18 uncalled private functions totaling 272 lines
- `dead_private_code_cluster` `src/store/review.ts` score 7278: File 'src/store/review.ts' contains 10 uncalled private functions totaling 121 lines
- `dead_private_code_cluster` `src/components/PreviewPanel.tsx` score 7170: File 'src/components/PreviewPanel.tsx' contains 21 uncalled private functions totaling 115 lines
- `dead_private_code_cluster` `electron/remote/server.ts` score 7000: File 'electron/remote/server.ts' contains 3 uncalled private functions totaling 193 lines
- `dead_private_code_cluster` `src/components/SidebarTaskRow.tsx` score 6954: File 'src/components/SidebarTaskRow.tsx' contains 17 uncalled private functions totaling 103 lines

## Debt Clusters

- `trusted` `cluster:src/store/store.ts|src/app/agent-catalog.ts|src/app/remote-access.ts|src/app/task-attention.ts|src/app/task-close-state.ts|src/app/task-command-dispatch.ts|src/app/task-command-lease-runtime-subscriptions.ts|src/app/task-command-lease-runtime.ts|src/app/task-command-lease-session.ts|src/app/task-command-lease-takeover.ts|src/app/task-command-lease.ts|src/app/task-convergence.ts|src/app/task-lifecycle-workflows.ts|src/app/task-presentation-status.ts|src/app/task-prompt-workflows.ts|src/app/task-review-state.ts|src/app/task-shell-workflows.ts|src/app/task-workflows.ts|src/lib/runtime-client-id.ts|src/store/agent-output-activity.ts|src/store/agents.ts|src/store/auto-trust.ts|src/store/client-session.ts|src/store/completion.ts|src/store/core.ts|src/store/focus.ts|src/store/keyed-snapshot-record.ts|src/store/navigation.ts|src/store/notification.ts|src/store/peer-presence.ts|src/store/persistence-codecs.ts|src/store/persistence-load-context.ts|src/store/persistence-load.ts|src/store/persistence-projects.ts|src/store/persistence-save.ts|src/store/persistence-terminal-restore.ts|src/store/persistence.ts|src/store/projects.ts|src/store/remote.ts|src/store/review.ts|src/store/sidebar-order.ts|src/store/state.ts|src/store/task-command-controllers.ts|src/store/task-command-takeovers.ts|src/store/task-git-status.ts|src/store/task-state-cleanup.ts|src/store/taskStatus.ts|src/store/tasks.ts|src/store/terminals.ts|src/store/ui.ts` score 10000: Files src/store/store.ts, src/app/agent-catalog.ts, src/app/remote-access.ts, and 47 more intersect 10 debt signals: unstable_hotspot, cycle_cluster, hotspot, large_file, dependency_sprawl
- `trusted` `cluster:src/App.tsx|src/remote/App.tsx` score 8924: Files src/App.tsx, src/remote/App.tsx intersect 2 debt signals: dependency_sprawl, clone_family
- `trusted` `cluster:src/components/terminal-view/terminal-session.ts` score 7800: File 'src/components/terminal-view/terminal-session.ts' intersects 3 debt signals: hotspot, large_file, dependency_sprawl
- `trusted` `cluster:src/components/PromptInput.tsx` score 7644: File 'src/components/PromptInput.tsx' intersects 2 debt signals: hotspot, large_file
- `trusted` `cluster:scripts/session-stress.mjs` score 7220: File 'scripts/session-stress.mjs' intersects 2 debt signals: large_file, hotspot

## Watchpoints

- `watchpoint` `cycle:src/app/agent-catalog.ts|src/app/remote-access.ts|src/app/task-attention.ts|src/app/task-close-state.ts|src/app/task-command-dispatch.ts|src/app/task-command-lease-runtime-subscriptions.ts|src/app/task-command-lease-runtime.ts|src/app/task-command-lease-session.ts|src/app/task-command-lease-takeover.ts|src/app/task-command-lease.ts|src/app/task-convergence.ts|src/app/task-lifecycle-workflows.ts|src/app/task-presentation-status.ts|src/app/task-prompt-workflows.ts|src/app/task-review-state.ts|src/app/task-shell-workflows.ts|src/app/task-workflows.ts|src/lib/runtime-client-id.ts|src/store/agent-output-activity.ts|src/store/agents.ts|src/store/auto-trust.ts|src/store/client-session.ts|src/store/completion.ts|src/store/core.ts|src/store/focus.ts|src/store/keyed-snapshot-record.ts|src/store/navigation.ts|src/store/notification.ts|src/store/peer-presence.ts|src/store/persistence-codecs.ts|src/store/persistence-load-context.ts|src/store/persistence-load.ts|src/store/persistence-projects.ts|src/store/persistence-save.ts|src/store/persistence-terminal-restore.ts|src/store/persistence.ts|src/store/projects.ts|src/store/remote.ts|src/store/review.ts|src/store/sidebar-order.ts|src/store/state.ts|src/store/store.ts|src/store/task-command-controllers.ts|src/store/task-command-takeovers.ts|src/store/task-git-status.ts|src/store/task-state-cleanup.ts|src/store/taskStatus.ts|src/store/tasks.ts|src/store/terminals.ts|src/store/ui.ts` score 10000: Files src/app/agent-catalog.ts, src/app/remote-access.ts, src/app/task-attention.ts, src/app/task-close-state.ts, src/app/task-command-dispatch.ts, src/app/task-command-lease-runtime-subscriptions.ts, src/app/task-command-lease-runtime.ts, src/app/task-command-lease-session.ts, src/app/task-command-lease-takeover.ts, src/app/task-command-lease.ts, src/app/task-convergence.ts, src/app/task-lifecycle-workflows.ts, src/app/task-presentation-status.ts, src/app/task-prompt-workflows.ts, src/app/task-review-state.ts, src/app/task-shell-workflows.ts, src/app/task-workflows.ts, src/lib/runtime-client-id.ts, src/store/agent-output-activity.ts, src/store/agents.ts, src/store/auto-trust.ts, src/store/client-session.ts, src/store/completion.ts, src/store/core.ts, src/store/focus.ts, src/store/keyed-snapshot-record.ts, src/store/navigation.ts, src/store/notification.ts, src/store/peer-presence.ts, src/store/persistence-codecs.ts, src/store/persistence-load-context.ts, src/store/persistence-load.ts, src/store/persistence-projects.ts, src/store/persistence-save.ts, src/store/persistence-terminal-restore.ts, src/store/persistence.ts, src/store/projects.ts, src/store/remote.ts, src/store/review.ts, src/store/sidebar-order.ts, src/store/state.ts, src/store/store.ts, src/store/task-command-controllers.ts, src/store/task-command-takeovers.ts, src/store/task-git-status.ts, src/store/task-state-cleanup.ts, src/store/taskStatus.ts, src/store/tasks.ts, src/store/terminals.ts, src/store/ui.ts form a dependency cycle
- `watchpoint` `clone-family-0x7e50d49dc16ef925` score 86: 4 exact clone groups repeat across 2 files and churn differs by 0 recent commit(s) across siblings; sibling file age spans 1 day(s)
- `watchpoint` `clone-family-0x9ebb8dad5cafb9c0` score 78: 4 exact clone groups repeat across 2 files and churn differs by 3 recent commit(s) across siblings; sibling file age spans 0 day(s)
- `watchpoint` `server/browser-channels.ts` score 6433: File 'server/browser-channels.ts' is carrying coordination hotspot pressure
- `watchpoint` `src/lib/browser-http-ipc.ts` score 6400: File 'src/lib/browser-http-ipc.ts' is carrying coordination hotspot pressure

## Proof Targets

1. Ownership/boundary: `n/a`
2. Propagation/obligations: `task_presentation_status`
3. Duplication/hotspot: clone cluster:src/App.tsx|src/remote/App.tsx / hotspot src/components/PromptInput.tsx

## Benchmark Baseline

- cold process total: 16772.8 ms
- warm cached total: 888.7 ms
- warm patch-safety total: 4149.9 ms
