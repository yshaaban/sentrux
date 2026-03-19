# Parallel Code: Committed HEAD Analysis Report For Engineers

Generated on March 19, 2026 from a committed HEAD clone of `<parallel-code-root>`.

This report is for an engineer who does not already know `parallel-code` or Sentrux.

## Freshness Gate

- analysis mode: `head_clone`
- commit: `76772b8a37d5de0d0ffba06130218c5f12e40511`
- dirty paths: `40`
- dirty-path fingerprint: `162e0a2b31f06fc89cd46ae27a65c66e3ae1d6fb203e1bfde5dd56e9d1b00c89`
- tree fingerprint: `eb4e0b159a20846aaa3b02cf14ada52f6ac461aa3d344d37f40b319a7cdd6af3`
- stale goldens: refused by default unless the goldens are fresh

## What Was Analyzed

- live source checkout: `<parallel-code-root>`
- report scope: committed `HEAD` only
- ignored working-tree changes outside HEAD: `40`
- rules file used for the run: `<sentrux-root>/docs/v2/examples/parallel-code.rules.toml`
- comparison snapshot: `<sentrux-root>/docs/v2/examples/parallel-code-head-proof-snapshot.json`
- benchmark artifact: `<sentrux-root>/docs/v2/examples/parallel-code-benchmark.json`

## Scan Coverage

- scanned source files: `638`
- scanned lines: `141131`
- git candidate files kept: `638 / 772`
- excluded files: `134`
- resolved import edges: `1932`
- unresolved internal imports: `1`
- unresolved external imports: `523`
- unresolved unknown imports: `83`
- scan confidence: `8264 / 10000`
- rule coverage: `10000 / 10000`
- semantic rules loaded: `true`

## Executive Summary

The current analysis surfaces these highest-leverage improvement targets:

- `architecture_signal` `unstable_hotspot` Component-facing barrel 'src/store/store.ts' has 48 inbound references and remains unstable
- `local_refactor_target` `dependency_sprawl` File 'src/components/TaskPanel.tsx' depends on 28 real surfaces, above the typescript threshold of 15
- `boundary_discipline` `unstable_hotspot` Guarded transport facade 'src/lib/ipc.ts' has 68 inbound references and remains unstable
- `regrowth_watchpoint` `dependency_sprawl` Composition root 'src/App.tsx' depends on 32 real surfaces, above the typescript threshold of 15
- `secondary_cleanup` `dependency_sprawl` File 'src/components/terminal-view/terminal-session.ts' depends on 22 real surfaces, above the typescript threshold of 15

## Architecture Signals

### [store.ts](<parallel-code-root>/src/store/store.ts)

- trust tier: `trusted`
- class: `structural_debt`
- leverage: `architecture_signal`
- kind: `unstable_hotspot`
- severity: `high`
- summary: Component-facing barrel 'src/store/store.ts' has 48 inbound references and remains unstable
- impact: A volatile component-facing barrel makes it harder to keep presentation access broad while keeping deeper orchestration changes contained.
- leverage reasons:
  - `shared_barrel_boundary_hub`
  - `guardrail_backed_boundary_pressure`
  - `high_inbound_dependency_pressure`
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

### cycle:src/app/agent-catalog.ts|src/app/remote-access.ts|src/app/task-attention.ts|src/app/task-close-state.ts|src/app/task-command-dispatch.ts|src/app/task-command-lease-runtime-subscriptions.ts|src/app/task-command-lease-runtime.ts|src/app/task-command-lease-session.ts|src/app/task-command-lease-takeover.ts|src/app/task-command-lease.ts|src/app/task-convergence.ts|src/app/task-lifecycle-workflows.ts|src/app/task-presentation-status.ts|src/app/task-prompt-workflows.ts|src/app/task-review-state.ts|src/app/task-shell-workflows.ts|src/app/task-workflows.ts|src/lib/runtime-client-id.ts|src/store/agent-output-activity.ts|src/store/agents.ts|src/store/auto-trust.ts|src/store/client-session.ts|src/store/completion.ts|src/store/core.ts|src/store/focus.ts|src/store/keyed-snapshot-record.ts|src/store/navigation.ts|src/store/notification.ts|src/store/peer-presence.ts|src/store/persistence-codecs.ts|src/store/persistence-load-context.ts|src/store/persistence-load.ts|src/store/persistence-projects.ts|src/store/persistence-save.ts|src/store/persistence-terminal-restore.ts|src/store/persistence.ts|src/store/projects.ts|src/store/remote.ts|src/store/review.ts|src/store/sidebar-order.ts|src/store/state.ts|src/store/store.ts|src/store/task-command-controllers.ts|src/store/task-command-takeovers.ts|src/store/task-git-status.ts|src/store/task-state-cleanup.ts|src/store/taskStatus.ts|src/store/tasks.ts|src/store/terminals.ts|src/store/ui.ts

- trust tier: `watchpoint`
- class: `watchpoint`
- leverage: `architecture_signal`
- kind: `cycle_cluster`
- severity: `high`
- summary: Files src/app/agent-catalog.ts, src/app/remote-access.ts, src/app/task-attention.ts, src/app/task-close-state.ts, src/app/task-command-dispatch.ts, src/app/task-command-lease-runtime-subscriptions.ts, src/app/task-command-lease-runtime.ts, src/app/task-command-lease-session.ts, src/app/task-command-lease-takeover.ts, src/app/task-command-lease.ts, src/app/task-convergence.ts, src/app/task-lifecycle-workflows.ts, src/app/task-presentation-status.ts, src/app/task-prompt-workflows.ts, src/app/task-review-state.ts, src/app/task-shell-workflows.ts, src/app/task-workflows.ts, src/lib/runtime-client-id.ts, src/store/agent-output-activity.ts, src/store/agents.ts, src/store/auto-trust.ts, src/store/client-session.ts, src/store/completion.ts, src/store/core.ts, src/store/focus.ts, src/store/keyed-snapshot-record.ts, src/store/navigation.ts, src/store/notification.ts, src/store/peer-presence.ts, src/store/persistence-codecs.ts, src/store/persistence-load-context.ts, src/store/persistence-load.ts, src/store/persistence-projects.ts, src/store/persistence-save.ts, src/store/persistence-terminal-restore.ts, src/store/persistence.ts, src/store/projects.ts, src/store/remote.ts, src/store/review.ts, src/store/sidebar-order.ts, src/store/state.ts, src/store/store.ts, src/store/task-command-controllers.ts, src/store/task-command-takeovers.ts, src/store/task-git-status.ts, src/store/task-state-cleanup.ts, src/store/taskStatus.ts, src/store/tasks.ts, src/store/terminals.ts, src/store/ui.ts form a dependency cycle
- impact: The cycle touches a component-facing barrel, which makes it harder to keep broad component access separate from deeper app and runtime seams.
- leverage reasons:
  - `shared_barrel_boundary_hub`
  - `guardrail_backed_boundary_pressure`
  - `mixed_cycle_pressure`
  - `high_leverage_cycle_cut`
- candidate split axes:
  - `guarded boundary cut`
  - `app store boundary`
- related surfaces:
  - [core.ts](<parallel-code-root>/src/store/core.ts)
  - [store.ts](<parallel-code-root>/src/store/store.ts)
  - [tasks.ts](<parallel-code-root>/src/store/tasks.ts)
  - [task-workflows.ts](<parallel-code-root>/src/app/task-workflows.ts)
  - [state.ts](<parallel-code-root>/src/store/state.ts)

## Best Local Refactor Targets

### [TaskPanel.tsx](<parallel-code-root>/src/components/TaskPanel.tsx)

- trust tier: `trusted`
- class: `structural_debt`
- leverage: `local_refactor_target`
- kind: `dependency_sprawl`
- severity: `high`
- summary: File 'src/components/TaskPanel.tsx' depends on 28 real surfaces, above the typescript threshold of 15
- impact: Broad dependency fan-out expands change surface and makes orchestration drift harder to localize.
- leverage reasons:
  - `extracted_owner_shell_pressure`
  - `guardrail_backed_refactor_surface`
  - `contained_dependency_pressure`
- candidate split axes:
  - `components dependency boundary`
  - `lib dependency boundary`
  - `store dependency boundary`
- related surfaces:
  - [task-ports.ts](<parallel-code-root>/src/app/task-ports.ts)
  - [CloseTaskDialog.tsx](<parallel-code-root>/src/components/CloseTaskDialog.tsx)
  - [DiffViewerDialog.tsx](<parallel-code-root>/src/components/DiffViewerDialog.tsx)
  - [TaskPanel.architecture.test.ts](<parallel-code-root>/src/components/TaskPanel.architecture.test.ts)

### [ReviewPanel.tsx](<parallel-code-root>/src/components/ReviewPanel.tsx)

- trust tier: `trusted`
- class: `structural_debt`
- leverage: `local_refactor_target`
- kind: `dependency_sprawl`
- severity: `high`
- summary: File 'src/components/ReviewPanel.tsx' depends on 22 real surfaces, above the typescript threshold of 15
- impact: Broad dependency fan-out expands change surface and makes orchestration drift harder to localize.
- leverage reasons:
  - `extracted_owner_shell_pressure`
  - `guardrail_backed_refactor_surface`
  - `contained_dependency_pressure`
- candidate split axes:
  - `app dependency boundary`
  - `components dependency boundary`
  - `lib dependency boundary`
- related surfaces:
  - [channels.ts](<parallel-code-root>/electron/ipc/channels.ts)
  - [async-request-guard.ts](<parallel-code-root>/src/app/async-request-guard.ts)
  - [review-diffs.ts](<parallel-code-root>/src/app/review-diffs.ts)
  - [review-surfaces.architecture.test.ts](<parallel-code-root>/src/components/review-surfaces.architecture.test.ts)

## Boundary Discipline

### [ipc.ts](<parallel-code-root>/src/lib/ipc.ts)

- trust tier: `trusted`
- class: `guarded_facade`
- leverage: `boundary_discipline`
- kind: `unstable_hotspot`
- severity: `high`
- summary: Guarded transport facade 'src/lib/ipc.ts' has 68 inbound references and remains unstable
- impact: A transport facade with heavy fan-in needs clear ownership boundaries so lifecycle or domain logic does not leak into transport glue.
- leverage reasons:
  - `guarded_or_transport_facade`
  - `heavy_inbound_seam_pressure`
- candidate split axes:
  - `app caller boundary`
  - `components caller boundary`
  - `lib dependency boundary`
- related surfaces:
  - [App.tsx](<parallel-code-root>/src/App.tsx)
  - [agent-catalog.ts](<parallel-code-root>/src/app/agent-catalog.ts)
  - [desktop-session-startup.ts](<parallel-code-root>/src/app/desktop-session-startup.ts)

## Regrowth Watchpoints

### [App.tsx](<parallel-code-root>/src/App.tsx)

- trust tier: `trusted`
- class: `structural_debt`
- leverage: `regrowth_watchpoint`
- kind: `dependency_sprawl`
- severity: `high`
- summary: Composition root 'src/App.tsx' depends on 32 real surfaces, above the typescript threshold of 15
- impact: Broad dependency fan-out in a composition root makes shell wiring and runtime ownership harder to keep separate.
- leverage reasons:
  - `intentionally_central_surface`
  - `fan_out_regrowth_pressure`
- candidate split axes:
  - `components dependency boundary`
  - `lib dependency boundary`
  - `app dependency boundary`
- related surfaces:
  - [app-action-keys.ts](<parallel-code-root>/src/app/app-action-keys.ts)
  - [desktop-session.ts](<parallel-code-root>/src/app/desktop-session.ts)
  - [task-command-lease.ts](<parallel-code-root>/src/app/task-command-lease.ts)
  - [store-boundary.architecture.test.ts](<parallel-code-root>/src/app/store-boundary.architecture.test.ts)

## Secondary Cleanup

### [terminal-session.ts](<parallel-code-root>/src/components/terminal-view/terminal-session.ts)

- trust tier: `trusted`
- class: `structural_debt`
- leverage: `secondary_cleanup`
- kind: `dependency_sprawl`
- severity: `high`
- summary: File 'src/components/terminal-view/terminal-session.ts' depends on 22 real surfaces, above the typescript threshold of 15
- impact: Broad dependency fan-out expands change surface and makes orchestration drift harder to localize.
- leverage reasons:
  - `secondary_facade_cleanup`
- candidate split axes:
  - `lib dependency boundary`
  - `components dependency boundary`
  - `store dependency boundary`
- related surfaces:
  - [channels.ts](<parallel-code-root>/electron/ipc/channels.ts)
  - [terminal-input-pipeline.ts](<parallel-code-root>/src/components/terminal-view/terminal-input-pipeline.ts)
  - [terminal-output-pipeline.ts](<parallel-code-root>/src/components/terminal-view/terminal-output-pipeline.ts)
  - [terminal-session.architecture.test.ts](<parallel-code-root>/src/components/terminal-view/terminal-session.architecture.test.ts)

### [browser-websocket.ts](<parallel-code-root>/electron/remote/ws-server.ts|server/browser-websocket.ts)

- trust tier: `trusted`
- class: `watchpoint`
- leverage: `secondary_cleanup`
- kind: `exact_clone_group`
- severity: `high`
- summary: 2 functions share an identical normalized body across recently changed files
- impact: Duplicate logic increases the chance that fixes land in one copy but not the others.
- leverage reasons:
  - `duplicate_maintenance_pressure`
- related surfaces:
  - [ws-server.ts](<parallel-code-root>/electron/remote/ws-server.ts)
  - [browser-websocket.ts](<parallel-code-root>/server/browser-websocket.ts)

### [RemoteAgentGlyph.tsx](<parallel-code-root>/src/components/AgentGlyph.tsx|src/remote/RemoteAgentGlyph.tsx)

- trust tier: `trusted`
- class: `watchpoint`
- leverage: `secondary_cleanup`
- kind: `exact_clone_group`
- severity: `high`
- summary: 2 functions share an identical normalized body across recently changed files
- impact: Duplicate logic increases the chance that fixes land in one copy but not the others.
- leverage reasons:
  - `duplicate_maintenance_pressure`
- related surfaces:
  - [AgentGlyph.tsx](<parallel-code-root>/src/components/AgentGlyph.tsx)
  - [RemoteAgentGlyph.tsx](<parallel-code-root>/src/remote/RemoteAgentGlyph.tsx)

## Targeted Hardening Notes

### ConnectionBannerState

- trust tier: `trusted`
- class: `hardening_note`
- leverage: `hardening_note`
- kind: `closed_domain_exhaustiveness`
- severity: `high`
- summary: Closed domain 'ConnectionBannerState' is missing coverage for variants: connecting, reconnecting, restoring
- impact: Finite-domain changes can silently miss one surface unless all required cases stay in sync.
- leverage reasons:
  - `narrow_completeness_gap`
- related surfaces:
  - [AppConnectionBanner.tsx](<parallel-code-root>/src/components/app-shell/AppConnectionBanner.tsx)
  - [browser-session.ts](<parallel-code-root>/src/runtime/browser-session.ts)

## Tooling Debt

### [session-stress.mjs](<parallel-code-root>/scripts/session-stress.mjs)

- trust tier: `trusted`
- class: `tooling_debt`
- leverage: `tooling_debt`
- kind: `large_file`
- severity: `high`
- summary: File 'scripts/session-stress.mjs' is 2048 lines, above the javascript threshold of 500
- impact: Responsibility concentration increases review friction and makes later splits harder to isolate.
- leverage reasons:
  - `tooling_surface_maintenance_burden`
- candidate split axes:
  - `scripts dependency boundary`
  - `entry surface split`
  - `private helper surface split`
- related surfaces:
  - [browser-server-client.mjs](<parallel-code-root>/scripts/browser-server-client.mjs)
  - [session-stress-profiles.mjs](<parallel-code-root>/scripts/session-stress-profiles.mjs)

## Watchpoints

- `watchpoint` `local_refactor_target` `hotspot` File 'server/browser-channels.ts' is carrying coordination hotspot pressure
- `watchpoint` `local_refactor_target` `hotspot` File 'src/lib/browser-http-ipc.ts' is carrying coordination hotspot pressure
- `watchpoint` `secondary_cleanup` `clone_family` 4 exact clone groups repeat across 2 files and churn differs by 0 recent commit(s) across siblings; sibling file age spans 1 day(s)
- `watchpoint` `secondary_cleanup` `clone_family` 4 exact clone groups repeat across 2 files and churn differs by 3 recent commit(s) across siblings; sibling file age spans 0 day(s)

## Benchmark Baseline

- cold process total: 16772.8 ms
- warm cached total: 888.7 ms
- warm patch-safety total: 4149.9 ms

## Freshness Check Result

- live commit: `76772b8a37d5de0d0ffba06130218c5f12e40511`
- live dirty paths: `40`
- live dirty-path fingerprint: `2db83fe36579d9eca04948fe5c91eb568b9eed05a838639ca5cdab4c204978a1`
- live tree fingerprint: `909d3c33a72de0eb08030e4a191402dcf7dd7d697d1507b1ebd4cc370d5e726b`
- freshness comparison: goldens matched and report generation was allowed

## Source Documents

- proof snapshot: `<sentrux-root>/docs/v2/examples/parallel-code-head-proof-snapshot.md`
- golden metadata: `<sentrux-root>/docs/v2/examples/parallel-code-head-golden/metadata.json`