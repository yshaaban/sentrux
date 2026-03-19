# Parallel Code: Live Analysis Report For Engineers

Generated on March 19, 2026 from the live checkout at `<parallel-code-root>`.

This report is for an engineer who does not already know `parallel-code` or Sentrux.

Important location note:

- the analyzed repo is [<parallel-code-root>](<parallel-code-root>)
- this report lives in the Sentrux repo, not in `parallel-code`:
  [parallel-code-live-engineer-report.md](<sentrux-root>/docs/v2/examples/parallel-code-live-engineer-report.md)

## What Was Analyzed

The run used a disposable clone of the live repo plus the current Sentrux v2 surfaces:

- live source checkout: [<parallel-code-root>](<parallel-code-root>)
- rules file used for the run: [parallel-code.rules.toml](<sentrux-root>/docs/v2/examples/parallel-code.rules.toml)
- comparison snapshot: [parallel-code-proof-snapshot.md](<sentrux-root>/docs/v2/examples/parallel-code-proof-snapshot.md)

Important scope note:

- the live repo still does not have a repo-owned `.sentrux/rules.toml`
- this rerun therefore still uses the bundled example rules for:
  - `task_git_status`
  - `task_presentation_status`
  - `server_state_bootstrap`

Scan coverage for this run:

- scanned source files: `604`
- scanned lines: `136,558`
- git candidate files kept: `604 / 738`
- excluded files: `134`
- resolved import edges: `1,798`
- unresolved internal imports: `1`
- unresolved external imports: `491`
- unresolved unknown imports: `75`
- scan confidence: `8184 / 10000`
- rule coverage: `10000 / 10000`
- semantic rules loaded: `true`

## How To Read This Report

Sentrux v2 now separates results by trust tier:

- `trusted`: strong enough to use as normal engineer-facing debt evidence
- `watchpoint`: real pressure signal, but still needs seam-aware interpretation
- `experimental`: visible for analyzer follow-up, not for cleanup prioritization

That matters here:

- the top structural signals below are mostly `trusted`
- the large store/app cycle is a `watchpoint`, but it now includes cut candidates
- dead-private-code results are `experimental` and should not drive refactor work

## Executive Summary

The current live repo surfaces five meaningful engineering signals:

1. [terminal-session.ts](<parallel-code-root>/src/components/terminal-view/terminal-session.ts) is a stacked trusted debt surface: very large, highly coupled, and coordination-heavy.
2. [browser-control-plane.ts](<parallel-code-root>/server/browser-control-plane.ts) is still a large control seam, and it overlaps a transport/control cluster with clone and hotspot pressure.
3. [App.tsx](<parallel-code-root>/src/App.tsx) and [TaskPanel.tsx](<parallel-code-root>/src/components/TaskPanel.tsx) both have strong dependency-sprawl signals.
4. the store/app subsystem cycle is still one of the clearest architecture watchpoints, and v2 now gives concrete candidate back-edges to inspect first.
5. configured concept health is mostly stable:
   - [task-git-status.ts](<parallel-code-root>/src/store/task-git-status.ts) looks healthy
   - [server-state-bootstrap.ts](<parallel-code-root>/src/app/server-state-bootstrap.ts) looks healthy
   - [ConnectionBannerState](<parallel-code-root>/src/App.tsx) and [task-presentation-status.ts](<parallel-code-root>/src/app/task-presentation-status.ts) remain targeted hardening findings

The main improvement over the earlier reports is that weak stale-private-code signals are now quarantined as experimental instead of being mixed into the default debt story.

## Strongest Trusted Debt Signals

### 1. [terminal-session.ts](<parallel-code-root>/src/components/terminal-view/terminal-session.ts)

Current trusted evidence:

- large file: `1856` lines
- function count: `92`
- peak complexity: `350`
- dependency sprawl: `25` real outward dependencies
- hotspot metrics:
  - side-effect breadth: `75`
  - async/branch weight: `30`
  - timer/retry weight: `8`
- trusted cluster:
  - `large_file`
  - `dependency_sprawl`
  - `hotspot`

Impact if left alone:

- review cost stays high
- changes keep crossing orchestration, UI, and helper concerns in one file
- regressions will remain expensive to isolate

Inspection focus:

- split orchestration from rendering and data shaping
- reduce dependency breadth before adding more behavior
- use the reported split axes as likely first-cut seams:
  - `lib dependency boundary`
  - `store dependency boundary`
  - `app dependency boundary`

### 2. [browser-control-plane.ts](<parallel-code-root>/server/browser-control-plane.ts)

Current trusted evidence:

- large file: `1054` lines
- function count: `74`
- peak complexity: `102`
- related surfaces:
  - [channels.ts](<parallel-code-root>/electron/ipc/channels.ts)
  - [runtime-diagnostics.ts](<parallel-code-root>/electron/ipc/runtime-diagnostics.ts)
  - [task-command-leases.ts](<parallel-code-root>/electron/ipc/task-command-leases.ts)
  - [protocol.ts](<parallel-code-root>/electron/remote/protocol.ts)
  - [ws-transport.ts](<parallel-code-root>/electron/remote/ws-transport.ts)
- trusted cluster overlap:
  - `large_file`
  - `clone_family`
  - `hotspot`

Impact if left alone:

- this seam keeps accumulating coordination responsibility
- fixes in transport/control behavior are more likely to drift across related surfaces

Inspection focus:

- split control-plane orchestration from narrower adapter/protocol helpers
- treat the cluster as a pressure point to inspect, not an automatic dedupe mandate

### 3. [App.tsx](<parallel-code-root>/src/App.tsx) and [TaskPanel.tsx](<parallel-code-root>/src/components/TaskPanel.tsx)

Current trusted evidence:

- [App.tsx](<parallel-code-root>/src/App.tsx)
  - fan-out: `28`
  - instability: `9655 / 10000`
  - dominant dependency categories: `components(12), lib(5), app(3)`
- [TaskPanel.tsx](<parallel-code-root>/src/components/TaskPanel.tsx)
  - fan-out: `28`
  - instability: `9333 / 10000`
  - dominant dependency categories: `components(17), lib(4), store(3)`
- both are also in trusted structural clusters

Impact if left alone:

- orchestration, action wiring, and view concerns stay entangled
- future changes will keep spreading across too many direct dependencies

Inspection focus:

- reduce direct dependency breadth before adding more feature logic
- inspect whether view composition can be separated from runtime/session wiring

### 4. [store.ts](<parallel-code-root>/src/store/store.ts) plus the store/app cycle

This is the strongest current `watchpoint`, not the strongest current `trusted` finding.

Current evidence:

- unstable hotspot:
  - inbound references: `56`
  - fan-out: `20`
  - instability: `0.26`
- cycle size: `49` files
- total lines inside cycle: `6750`
- peak complexity inside cycle: `71`

Why this still matters:

- it is subsystem-level layering pressure, not just one bad file
- the cycle increases refactor friction and blast radius
- but it still needs seam-aware interpretation, so it stays a `watchpoint`

Concrete cycle cut candidates from the live run:

1. [tasks.ts](<parallel-code-root>/src/store/tasks.ts) -> [task-workflows.ts](<parallel-code-root>/src/app/task-workflows.ts)
   - seam kind: `app_store_boundary`
   - estimated cyclic-file reduction: `10`
2. [core.ts](<parallel-code-root>/src/store/core.ts) -> [store.ts](<parallel-code-root>/src/store/store.ts)
   - seam kind: `local_module_split`
   - estimated cyclic-file reduction: `13`
3. [state.ts](<parallel-code-root>/src/store/state.ts) -> [core.ts](<parallel-code-root>/src/store/core.ts)
   - seam kind: `contract_or_type_extraction`
   - estimated cyclic-file reduction: `7`

That is not the same as “this is the next thing to refactor,” but it is now a much better design starting point than a bare SCC listing.

## Clone And Drift Watchpoints

These are real, but they should still be read as watchpoints unless the team decides they are worth cleanup now.

### [AgentGlyph.tsx](<parallel-code-root>/src/components/AgentGlyph.tsx) / [RemoteAgentGlyph.tsx](<parallel-code-root>/src/remote/RemoteAgentGlyph.tsx)

- trust tier: `watchpoint`
- exact clone groups: `4`
- current interpretation: real duplicate maintenance risk across local and remote glyph rendering

### [ws-server.ts](<parallel-code-root>/electron/remote/ws-server.ts) / [browser-websocket.ts](<parallel-code-root>/server/browser-websocket.ts)

- trust tier: `watchpoint`
- exact clone groups: `4`
- churn gap across siblings: `3`
- current interpretation: shared behavior still exists, but this is better treated as drift risk than as an automatic extraction target

## Targeted Concept Hardening Signals

These are trusted, but they are narrower than the structural debt signals above.

### [ConnectionBannerState](<parallel-code-root>/src/App.tsx)

Current finding:

- `trusted`
- `high`
- `closed_domain_exhaustiveness`
- missing variants: `connecting`, `reconnecting`, `restoring`

Related surfaces:

- [App.tsx](<parallel-code-root>/src/App.tsx)
- [browser-session.ts](<parallel-code-root>/src/runtime/browser-session.ts)

Current interpretation:

- still useful as a completeness-hardening finding
- not the dominant repo-wide debt story

### [task-presentation-status.ts](<parallel-code-root>/src/app/task-presentation-status.ts)

Current finding:

- `trusted`
- `medium`
- one obligation
- one missing site
- missing-site path:
  [task-presentation-status.ts](<parallel-code-root>/src/app/task-presentation-status.ts)

Current interpretation:

- still useful hardening evidence
- narrower than the main structural pressure points

## Configured Surfaces That Currently Look Healthy

### [task-git-status.ts](<parallel-code-root>/src/store/task-git-status.ts)

Current state:

- findings: `0`
- obligations: `0`

### [server-state-bootstrap.ts](<parallel-code-root>/src/app/server-state-bootstrap.ts)

Current state:

- findings: `0`
- obligations: `0`
- parity score: `10000 / 10000`
- state integrity score: `10000 / 10000`

This remains a good example of a configured surface that currently looks aligned.

## Experimental Findings: Visible, But Not Actionable Yet

The live run still surfaces `dead_private_code_cluster` results, but they are now explicitly quarantined as experimental.

Current experimental count:

- experimental findings: `12`
- experimental debt signals: `5`

Examples:

- [ScrollingDiffView.tsx](<parallel-code-root>/src/components/ScrollingDiffView.tsx)
- [review.ts](<parallel-code-root>/src/store/review.ts)
- [PreviewPanel.tsx](<parallel-code-root>/src/components/PreviewPanel.tsx)
- [SidebarTaskRow.tsx](<parallel-code-root>/src/components/SidebarTaskRow.tsx)

How to use this section:

- do not treat it as a cleanup queue
- do treat it as analyzer follow-up
- if a human confirms a specific case is truly dead, that is useful, but the detector is not review-grade yet

## Session-End Baseline State

The untouched baseline session is clean:

- `pass: true`
- introduced findings: `0`
- experimental findings: `0`
- debt signals in pass-state `session_end`: `5`
- watchpoints in pass-state `session_end`: `1`

That means the structural signals above are baseline inspection signals, not patch-specific regressions from the pass-state session.

## Suggested Engineer Feedback Prompts

1. Do the trusted structural signals around terminal session, App, TaskPanel, and browser control plane match where the team already feels change pain?
2. For the store/app cycle, which of the three cut candidates looks most plausible as the first seam to inspect?
3. Are the clone watchpoints better handled by shared contract tests, helper extraction, or intentional divergence?
4. Are the `ConnectionBannerState` and `task_presentation_status` findings still useful hardening prompts even if they are not top priorities?
5. Which experimental dead-private findings are obviously wrong, and are any obviously real?
