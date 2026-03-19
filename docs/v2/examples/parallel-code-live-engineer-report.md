# Parallel Code: Live Analysis Report For Engineers

Generated on March 19, 2026 from the live checkout at `<parallel-code-root>`.

This report is for an engineer who does not already know `parallel-code` or Sentrux.

## Freshness Gate

- analysis mode: `working_tree`
- commit: `cf21f8733a9d800ec2a41239500e01e01ab8cc4b`
- dirty paths: `2`
- dirty-path fingerprint: `f1d370a4de031e83ee00b998f24ac88ff9f4d1338a336da4d92f7dd165c14541`
- tree fingerprint: `3c54adb61f586d027b231a19a49fe87cc64be8957098769c0447c353883d26fa`
- stale goldens: refused by default unless the goldens are fresh

## What Was Analyzed

- live source checkout: `<parallel-code-root>`
- rules file used for the run: `<sentrux-root>/docs/v2/examples/parallel-code.rules.toml`
- comparison snapshot: `<sentrux-root>/docs/v2/examples/parallel-code-proof-snapshot.json`
- benchmark artifact: `<sentrux-root>/docs/v2/examples/parallel-code-benchmark.json`

## Scan Coverage

- scanned source files: `622`
- scanned lines: `137959`
- git candidate files kept: `622 / 756`
- excluded files: `134`
- resolved import edges: `1871`
- unresolved internal imports: `1`
- unresolved external imports: `508`
- unresolved unknown imports: `79`
- scan confidence: `8228 / 10000`
- rule coverage: `10000 / 10000`
- semantic rules loaded: `true`

## Executive Summary

The current live repo surfaces the same core architecture pressure points as the proof snapshot:

- `dependency_sprawl` Composition root 'src/App.tsx' depends on 32 real surfaces, above the typescript threshold of 15
- `dependency_sprawl` File 'src/components/TaskPanel.tsx' depends on 28 real surfaces, above the typescript threshold of 15
- `unstable_hotspot` Component-facing barrel 'src/store/store.ts' has 47 inbound references and remains unstable
- `large_file` File 'scripts/session-stress.mjs' is 2048 lines, above the javascript threshold of 500
- `unstable_hotspot` File 'src/lib/ipc.ts' has 66 inbound references and remains unstable

## Strongest Trusted Debt Signals

### ConnectionBannerState

- kind: `closed_domain_exhaustiveness`
- severity: `high`
- summary: Closed domain 'ConnectionBannerState' is missing coverage for variants: connecting, reconnecting, restoring
- impact: Finite-domain changes can silently miss one surface unless all required cases stay in sync.
- related surfaces:
  - `src/components/app-shell/AppConnectionBanner.tsx`
  - `src/runtime/browser-session.ts`

### src/App.tsx

- kind: `dependency_sprawl`
- severity: `high`
- summary: Composition root 'src/App.tsx' depends on 32 real surfaces, above the typescript threshold of 15
- impact: Broad dependency fan-out in a composition root makes shell wiring and runtime ownership harder to keep separate.
- candidate split axes:
  - `components dependency boundary`
  - `lib dependency boundary`
  - `app dependency boundary`
- related surfaces:
  - `src/app/app-action-keys.ts`
  - `src/app/desktop-session.ts`
  - `src/app/task-command-lease.ts`
  - `src/app/store-boundary.architecture.test.ts`

### src/components/TaskPanel.tsx

- kind: `dependency_sprawl`
- severity: `high`
- summary: File 'src/components/TaskPanel.tsx' depends on 28 real surfaces, above the typescript threshold of 15
- impact: Broad dependency fan-out expands change surface and makes orchestration drift harder to localize.
- candidate split axes:
  - `components dependency boundary`
  - `lib dependency boundary`
  - `store dependency boundary`
- related surfaces:
  - `src/app/task-ports.ts`
  - `src/components/CloseTaskDialog.tsx`
  - `src/components/DiffViewerDialog.tsx`
  - `src/components/TaskPanel.architecture.test.ts`

### src/store/store.ts

- kind: `unstable_hotspot`
- severity: `high`
- summary: Component-facing barrel 'src/store/store.ts' has 47 inbound references and remains unstable
- impact: A volatile component-facing barrel makes it harder to keep presentation access broad while keeping deeper orchestration changes contained.
- candidate split axes:
  - `components caller boundary`
  - `store caller boundary`
  - `store dependency boundary`
- related surfaces:
  - `src/App.tsx`
  - `src/arena/ConfigScreen.tsx`
  - `src/arena/ResultsScreen.tsx`
  - `src/app/store-boundary.architecture.test.ts`

## Watchpoints

- `watchpoint` `cycle_cluster` Files src/app/agent-catalog.ts, src/app/remote-access.ts, src/app/task-attention.ts, src/app/task-close-state.ts, src/app/task-command-dispatch.ts, src/app/task-command-lease-runtime-subscriptions.ts, src/app/task-command-lease-runtime.ts, src/app/task-command-lease-session.ts, src/app/task-command-lease-takeover.ts, src/app/task-command-lease.ts, src/app/task-convergence.ts, src/app/task-lifecycle-workflows.ts, src/app/task-presentation-status.ts, src/app/task-prompt-workflows.ts, src/app/task-review-state.ts, src/app/task-shell-workflows.ts, src/app/task-workflows.ts, src/lib/runtime-client-id.ts, src/store/agent-output-activity.ts, src/store/agents.ts, src/store/auto-trust.ts, src/store/client-session.ts, src/store/completion.ts, src/store/core.ts, src/store/focus.ts, src/store/keyed-snapshot-record.ts, src/store/navigation.ts, src/store/notification.ts, src/store/peer-presence.ts, src/store/persistence-codecs.ts, src/store/persistence-load-context.ts, src/store/persistence-load.ts, src/store/persistence-projects.ts, src/store/persistence-save.ts, src/store/persistence-terminal-restore.ts, src/store/persistence.ts, src/store/projects.ts, src/store/remote.ts, src/store/review.ts, src/store/state.ts, src/store/store.ts, src/store/task-command-controllers.ts, src/store/task-command-takeovers.ts, src/store/task-git-status.ts, src/store/task-state-cleanup.ts, src/store/taskStatus.ts, src/store/tasks.ts, src/store/terminals.ts, src/store/ui.ts form a dependency cycle
- `watchpoint` `clone_family` 4 exact clone groups repeat across 2 files and churn differs by 0 recent commit(s) across siblings; sibling file age spans 1 day(s)
- `watchpoint` `clone_family` 4 exact clone groups repeat across 2 files and churn differs by 3 recent commit(s) across siblings; sibling file age spans 0 day(s)
- `watchpoint` `hotspot` File 'server/browser-channels.ts' is carrying coordination hotspot pressure

## Benchmark Baseline

- cold process total: 16772.8 ms
- warm cached total: 888.7 ms
- warm patch-safety total: 4149.9 ms

## Freshness Check Result

- live commit: `cf21f8733a9d800ec2a41239500e01e01ab8cc4b`
- live dirty paths: `2`
- live dirty-path fingerprint: `f1d370a4de031e83ee00b998f24ac88ff9f4d1338a336da4d92f7dd165c14541`
- live tree fingerprint: `3c54adb61f586d027b231a19a49fe87cc64be8957098769c0447c353883d26fa`
- freshness comparison: goldens matched and report generation was allowed

## Source Documents

- proof snapshot: `<sentrux-root>/docs/v2/examples/parallel-code-proof-snapshot.md`
- golden metadata: `<sentrux-root>/docs/v2/examples/parallel-code-golden/metadata.json`