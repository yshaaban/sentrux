# Parallel Code: Live Analysis Report For Engineers

Generated on March 19, 2026 from the live checkout at `<parallel-code-root>`.

This report is for an engineer who does not already know `parallel-code` or Sentrux.

## Freshness Gate

- analysis mode: `working_tree`
- commit: `94d010d3b0b7ccc815b74430fed6e3284b809113`
- dirty paths: `0`
- dirty-path fingerprint: `53c1562521679823f3ee3c10c2585dec4c3ecd862d145f868947047fd32725ac`
- tree fingerprint: `638a8939223b6aec43a72405ed82aebe88e59b4dabb5b15bc089fdffd758e84d`
- stale goldens: refused by default unless the goldens are fresh

## What Was Analyzed

- live source checkout: `<parallel-code-root>`
- rules file used for the run: `<sentrux-root>/docs/v2/examples/parallel-code.rules.toml`
- comparison snapshot: `<sentrux-root>/docs/v2/examples/parallel-code-proof-snapshot.json`
- benchmark artifact: `<sentrux-root>/docs/v2/examples/parallel-code-benchmark.json`

## Scan Coverage

- scanned source files: `631`
- scanned lines: `139806`
- git candidate files kept: `631 / 765`
- excluded files: `134`
- resolved import edges: `1908`
- unresolved internal imports: `1`
- unresolved external imports: `518`
- unresolved unknown imports: `83`
- scan confidence: `8248 / 10000`
- rule coverage: `10000 / 10000`
- semantic rules loaded: `true`

## Executive Summary

The current live repo surfaces these primary pressure points:

- `structural_debt` `unstable_hotspot` Component-facing barrel 'src/store/store.ts' has 48 inbound references and remains unstable
- `structural_debt` `dependency_sprawl` Composition root 'src/App.tsx' depends on 32 real surfaces, above the typescript threshold of 15
- `structural_debt` `dependency_sprawl` File 'src/components/TaskPanel.tsx' depends on 28 real surfaces, above the typescript threshold of 15
- `guarded_facade` `unstable_hotspot` Guarded transport facade 'src/lib/ipc.ts' has 67 inbound references and remains unstable
- `secondary_hotspot` File 'src/components/terminal-view/terminal-session.ts' depends on 22 real surfaces, above the typescript threshold of 15

## Strongest Trusted Debt Signals

### [store.ts](<parallel-code-root>/src/store/store.ts)

- class: `structural_debt`
- kind: `unstable_hotspot`
- severity: `high`
- summary: Component-facing barrel 'src/store/store.ts' has 48 inbound references and remains unstable
- impact: A volatile component-facing barrel makes it harder to keep presentation access broad while keeping deeper orchestration changes contained.
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

### [App.tsx](<parallel-code-root>/src/App.tsx)

- class: `structural_debt`
- kind: `dependency_sprawl`
- severity: `high`
- summary: Composition root 'src/App.tsx' depends on 32 real surfaces, above the typescript threshold of 15
- impact: Broad dependency fan-out in a composition root makes shell wiring and runtime ownership harder to keep separate.
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

- class: `structural_debt`
- kind: `dependency_sprawl`
- severity: `high`
- summary: File 'src/components/TaskPanel.tsx' depends on 28 real surfaces, above the typescript threshold of 15
- impact: Broad dependency fan-out expands change surface and makes orchestration drift harder to localize.
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

- class: `guarded_facade`
- kind: `unstable_hotspot`
- severity: `high`
- summary: Guarded transport facade 'src/lib/ipc.ts' has 67 inbound references and remains unstable
- impact: A transport facade with heavy fan-in needs clear ownership boundaries so lifecycle or domain logic does not leak into transport glue.
- candidate split axes:
  - `app caller boundary`
  - `components caller boundary`
  - `lib dependency boundary`
- related surfaces:
  - [App.tsx](<parallel-code-root>/src/App.tsx)
  - [agent-catalog.ts](<parallel-code-root>/src/app/agent-catalog.ts)
  - [desktop-notification-runtime.ts](<parallel-code-root>/src/app/desktop-notification-runtime.ts)

## Secondary Hotspots

### [terminal-session.ts](<parallel-code-root>/src/components/terminal-view/terminal-session.ts)

- class: `structural_debt`
- kind: `dependency_sprawl`
- severity: `high`
- summary: File 'src/components/terminal-view/terminal-session.ts' depends on 22 real surfaces, above the typescript threshold of 15
- impact: Broad dependency fan-out expands change surface and makes orchestration drift harder to localize.
- candidate split axes:
  - `lib dependency boundary`
  - `components dependency boundary`
  - `store dependency boundary`
- related surfaces:
  - [channels.ts](<parallel-code-root>/electron/ipc/channels.ts)
  - [terminal-input-pipeline.ts](<parallel-code-root>/src/components/terminal-view/terminal-input-pipeline.ts)
  - [terminal-output-pipeline.ts](<parallel-code-root>/src/components/terminal-view/terminal-output-pipeline.ts)
  - [terminal-session.architecture.test.ts](<parallel-code-root>/src/components/terminal-view/terminal-session.architecture.test.ts)

## Targeted Hardening Notes

### ConnectionBannerState

- class: `hardening_note`
- kind: `closed_domain_exhaustiveness`
- severity: `high`
- summary: Closed domain 'ConnectionBannerState' is missing coverage for variants: connecting, reconnecting, restoring
- impact: Finite-domain changes can silently miss one surface unless all required cases stay in sync.
- related surfaces:
  - [AppConnectionBanner.tsx](<parallel-code-root>/src/components/app-shell/AppConnectionBanner.tsx)
  - [browser-session.ts](<parallel-code-root>/src/runtime/browser-session.ts)

## Tooling Debt

### [session-stress.mjs](<parallel-code-root>/scripts/session-stress.mjs)

- class: `tooling_debt`
- kind: `large_file`
- severity: `high`
- summary: File 'scripts/session-stress.mjs' is 2048 lines, above the javascript threshold of 500
- impact: Responsibility concentration increases review friction and makes later splits harder to isolate.
- candidate split axes:
  - `scripts dependency boundary`
  - `entry surface split`
  - `private helper surface split`
- related surfaces:
  - [browser-server-client.mjs](<parallel-code-root>/scripts/browser-server-client.mjs)
  - [session-stress-profiles.mjs](<parallel-code-root>/scripts/session-stress-profiles.mjs)

## Watchpoints

- `watchpoint` `cycle_cluster` Files src/app/agent-catalog.ts, src/app/remote-access.ts, src/app/task-attention.ts, src/app/task-close-state.ts, src/app/task-command-dispatch.ts, src/app/task-command-lease-runtime-subscriptions.ts, src/app/task-command-lease-runtime.ts, src/app/task-command-lease-session.ts, src/app/task-command-lease-takeover.ts, src/app/task-command-lease.ts, src/app/task-convergence.ts, src/app/task-lifecycle-workflows.ts, src/app/task-presentation-status.ts, src/app/task-prompt-workflows.ts, src/app/task-review-state.ts, src/app/task-shell-workflows.ts, src/app/task-workflows.ts, src/lib/runtime-client-id.ts, src/store/agent-output-activity.ts, src/store/agents.ts, src/store/auto-trust.ts, src/store/client-session.ts, src/store/completion.ts, src/store/core.ts, src/store/focus.ts, src/store/keyed-snapshot-record.ts, src/store/navigation.ts, src/store/notification.ts, src/store/peer-presence.ts, src/store/persistence-codecs.ts, src/store/persistence-load-context.ts, src/store/persistence-load.ts, src/store/persistence-projects.ts, src/store/persistence-save.ts, src/store/persistence-terminal-restore.ts, src/store/persistence.ts, src/store/projects.ts, src/store/remote.ts, src/store/review.ts, src/store/sidebar-order.ts, src/store/state.ts, src/store/store.ts, src/store/task-command-controllers.ts, src/store/task-command-takeovers.ts, src/store/task-git-status.ts, src/store/task-state-cleanup.ts, src/store/taskStatus.ts, src/store/tasks.ts, src/store/terminals.ts, src/store/ui.ts form a dependency cycle
- `watchpoint` `clone_family` 4 exact clone groups repeat across 2 files and churn differs by 0 recent commit(s) across siblings; sibling file age spans 1 day(s)
- `watchpoint` `clone_family` 4 exact clone groups repeat across 2 files and churn differs by 3 recent commit(s) across siblings; sibling file age spans 0 day(s)
- `watchpoint` `hotspot` File 'server/browser-channels.ts' is carrying coordination hotspot pressure

## Benchmark Baseline

- cold process total: 16772.8 ms
- warm cached total: 888.7 ms
- warm patch-safety total: 4149.9 ms

## Freshness Check Result

- live commit: `94d010d3b0b7ccc815b74430fed6e3284b809113`
- live dirty paths: `0`
- live dirty-path fingerprint: `53c1562521679823f3ee3c10c2585dec4c3ecd862d145f868947047fd32725ac`
- live tree fingerprint: `638a8939223b6aec43a72405ed82aebe88e59b4dabb5b15bc089fdffd758e84d`
- freshness comparison: goldens matched and report generation was allowed

## Source Documents

- proof snapshot: `<sentrux-root>/docs/v2/examples/parallel-code-proof-snapshot.md`
- golden metadata: `<sentrux-root>/docs/v2/examples/parallel-code-golden/metadata.json`