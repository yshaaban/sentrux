# Parallel Code: Live Analysis Report For Engineers

Generated on March 19, 2026 from the live checkout at `<parallel-code-root>`.

This report uses the current evidence-first v2 surface. It is meant for an engineer who does not already know either `parallel-code` or Sentrux.

## What Was Analyzed

The run used a disposable clone of the live repo and the current Sentrux v2 analysis surfaces:

- live source checkout: [<parallel-code-root>](<parallel-code-root>)
- rules file used for the run: [parallel-code.rules.toml](<sentrux-root>/docs/v2/examples/parallel-code.rules.toml)
- current proof snapshot for comparison: [parallel-code-proof-snapshot.md](<sentrux-root>/docs/v2/examples/parallel-code-proof-snapshot.md)

Important scope note:

- the live repo currently has a `.sentrux/baseline.json`, but it does **not** carry its own `.sentrux/rules.toml`
- this live rerun therefore still uses the bundled example rules for the three configured critical concepts:
  - `task_git_status`
  - `task_presentation_status`
  - `server_state_bootstrap`

Scan coverage for this run:

- scanned source files: `604`
- scanned lines: `137,347`
- resolved import edges: `1,797`
- git candidate files kept: `604 / 738`
- excluded files: `134`
- scan confidence: `8184 / 10000`
- rule coverage: `10000 / 10000`
- semantic rules loaded: `true`

## Executive Summary

The current live repo is now surfacing **structural debt** much more strongly than the earlier memo did. The dominant current signals are:

1. a very large, coordination-heavy terminal session surface in [terminal-session.ts](<parallel-code-root>/src/components/terminal-view/terminal-session.ts)
2. a very large control-plane seam in [browser-control-plane.ts](<parallel-code-root>/server/browser-control-plane.ts)
3. broad dependency sprawl in [App.tsx](<parallel-code-root>/src/App.tsx) and [TaskPanel.tsx](<parallel-code-root>/src/components/TaskPanel.tsx)
4. a large store/app dependency cycle anchored around [store.ts](<parallel-code-root>/src/store/store.ts)
5. several high-confidence stale private code clusters in UI and store files

The configured concept surfaces are mixed:

- [task_git_status](<parallel-code-root>/src/store/task-git-status.ts) currently looks healthy in the configured rules
- [server_state_bootstrap](<parallel-code-root>/src/app/server-state-bootstrap.ts) currently looks healthy on parity and state integrity
- [ConnectionBannerState](<parallel-code-root>/src/App.tsx) and [task_presentation_status](<parallel-code-root>/src/app/task-presentation-status.ts) still show targeted hardening gaps, but they are no longer the dominant repo-wide story

Clone risk still exists and is useful, but it is now secondary to the structural debt surfaces above.

## Strongest Current Debt Signals

### 1. [terminal-session.ts](<parallel-code-root>/src/components/terminal-view/terminal-session.ts) is carrying stacked debt, not just size

Current evidence:

- large file: `1856` lines, `92` functions, peak complexity `350`
- dependency sprawl: `25` real outward dependencies
- hotspot pressure: `75` side-effect targets, `30` async/branching signals, `8` timer/retry signals
- debt cluster: File 'src/components/terminal-view/terminal-session.ts' intersects 4 debt signals: large_file, dead_private_code_cluster, dependency_sprawl, hotspot

Why this matters:

- this file is simultaneously large, highly coupled, coordination-heavy, and already carrying stale private paths
- if it keeps growing in place, changes will stay expensive to review and harder to isolate when regressions appear

Inspection focus from the live run:

- split orchestration from rendering/data-shaping concerns
- narrow direct dependencies where possible
- remove or reconnect dead private helpers so the file reflects the supported control flow

### 2. [browser-control-plane.ts](<parallel-code-root>/server/browser-control-plane.ts) is a large control seam and part of a transport/control cluster

Current evidence:

- large file: `1054` lines, `74` functions, peak complexity `102`
- current top cluster overlap: Files server/browser-control-plane.ts, electron/remote/server.ts, electron/remote/ws-transport.test.ts, and 1 more intersect 4 debt signals: large_file, clone_family, dead_private_code_cluster, hotspot
- that cluster also overlaps a transport/server clone family touching:
  - [server.ts](<parallel-code-root>/electron/remote/server.ts)
  - [ws-transport.test.ts](<parallel-code-root>/electron/remote/ws-transport.test.ts)
  - [websocket-contract-harness.ts](<parallel-code-root>/tests/harness/websocket-contract-harness.ts)

Why this matters:

- this is not just a big file problem
- the file sits inside a coordination-heavy seam that also overlaps clone and stale-code signals
- that combination is where change drift becomes expensive and subtle

Inspection focus from the live run:

- split control-plane orchestration from narrower protocol or adapter helpers
- inspect whether the transport/server sibling logic is meant to stay aligned or be intentionally separated

### 3. [App.tsx](<parallel-code-root>/src/App.tsx) and [TaskPanel.tsx](<parallel-code-root>/src/components/TaskPanel.tsx) both show real dependency sprawl

Current evidence:

- [App.tsx](<parallel-code-root>/src/App.tsx): fan-out `28`, instability `9655 / 10000`, line count `530`
- [TaskPanel.tsx](<parallel-code-root>/src/components/TaskPanel.tsx): fan-out `28`, instability `9333 / 10000`, line count `637`
- both also participate in debt clusters:
  - Files src/App.tsx, src/remote/App.tsx, src/runtime/browser-session.ts intersect 6 debt signals: dependency_sprawl, clone_family, dead_private_code_cluster, large_file, concept
  - File 'src/components/TaskPanel.tsx' intersects 4 debt signals: dependency_sprawl, large_file, dead_private_code_cluster, hotspot

Why this matters:

- these files are carrying too many direct dependencies for their role
- that makes feature changes spread across orchestration, policy, and view concerns in one place
- the App cluster also overlaps the current `ConnectionBannerState` hardening finding and an App/remote-App clone family

Inspection focus from the live run:

- reduce direct dependency breadth before adding more policy to these files
- inspect whether view, action wiring, and runtime/session coordination can be separated more clearly

### 4. The store/app subsystem cycle is now one of the clearest architecture-level debt signals

Current evidence:

- cycle size: `49` files
- total lines inside the cycle: `6750`
- peak function complexity inside the cycle: `71`
- unstable hotspot inside the cycle: [store.ts](<parallel-code-root>/src/store/store.ts) with fan-in `56` and fan-out `20`
- overlapping debt cluster: Files src/store/review.ts, src/app/agent-catalog.ts, src/app/remote-access.ts, and 46 more intersect 16 debt signals: dead_private_code_cluster, cycle_cluster, hotspot, dependency_sprawl, unstable_hotspot

Why this matters:

- this is not one bad file; it is a subsystem-level layering problem
- a cycle at this size makes refactors and initialization-order changes harder to isolate
- the cluster also overlaps dead private code and hotspot pressure, which suggests the subsystem is paying both coupling cost and maintenance noise at the same time

Inspection focus from the live run:

- break one back-edge rather than trying to “fix the whole store” at once
- look for contract/type extraction seams or runtime-client/state boundaries that can move to a lower-dependency layer
- treat [store.ts](<parallel-code-root>/src/store/store.ts) as a blast-radius surface because of the current inbound reference count

## Lower-Confidence Signal: Dead Private Code Clusters

This finding class is currently **not review-grade** for `parallel-code`.

The live run surfaced dead-private-code clusters in files such as:

- [ScrollingDiffView.tsx](<parallel-code-root>/src/components/ScrollingDiffView.tsx)
- [review.ts](<parallel-code-root>/src/store/review.ts)
- [PreviewPanel.tsx](<parallel-code-root>/src/components/PreviewPanel.tsx)
- [SidebarTaskRow.tsx](<parallel-code-root>/src/components/SidebarTaskRow.tsx)

But the sampled symbol names include helpers that are plainly referenced in the live repo. That means this detector needs correction before engineers should treat it as trusted maintainer-facing evidence.

For now:

- do not use this section as a cleanup queue
- treat it as analyzer follow-up, not repo guidance

## Targeted Concept Hardening Signals

These are still real, but they should be read as targeted completeness hardening rather than the main repo-wide debt story.

### [ConnectionBannerState](<parallel-code-root>/src/App.tsx)

Current evidence:

- severity: `high`
- finding: Closed domain 'ConnectionBannerState' is missing coverage for variants: connecting, reconnecting, restoring
- files: [App.tsx](<parallel-code-root>/src/App.tsx) and [browser-session.ts](<parallel-code-root>/src/runtime/browser-session.ts)
- related watchpoint: Concept 'ConnectionBannerState' intersects clone overlap, coordination hotspot overlap

What this means:

- the runtime/browser-session surface and app rendering surface still do not give the analyzer one explicit total coverage point for the connection banner states
- this is a localized state-completeness hardening signal, not the strongest repo-wide architectural debt signal

### [task_presentation_status](<parallel-code-root>/src/app/task-presentation-status.ts)

Current evidence:

- severity: `medium`
- finding: Closed domain `task_presentation_status` is missing required update sites
- obligation burden: `1` missing site
- exact missing-site detail: `no exhaustive mapping or switch site found for 'TaskDotStatus'` at line `12`
- related tests already exist for this concept in the configured rules

What this means:

- the analyzer still does not see one explicit exhaustive mapping site for `TaskDotStatus`
- this is useful completeness hardening, but it is narrower than the structural debt clusters above

## Clone And Drift Signals That Still Matter

Clone families are no longer the whole story, but two families are still worth an engineer’s attention.

### [AgentGlyph.tsx](<parallel-code-root>/src/components/AgentGlyph.tsx) / [RemoteAgentGlyph.tsx](<parallel-code-root>/src/remote/RemoteAgentGlyph.tsx)

- family score: `86`
- exact clone groups: `4`
- current meaning: repeated UI logic across local and remote surfaces still creates avoidable duplicate maintenance

### [ws-server.ts](<parallel-code-root>/electron/remote/ws-server.ts) / [browser-websocket.ts](<parallel-code-root>/server/browser-websocket.ts)

- family score: `78`
- exact clone groups: `4`
- churn gap across siblings: `3` recent commits
- current meaning: the two transport/runtime surfaces still share repeated behavior and need either deliberate synchronization or a clearer intentional split

A lower-priority but still real family also remains between [App.tsx](<parallel-code-root>/src/App.tsx) and [App.tsx](<parallel-code-root>/src/remote/App.tsx).

## Configured Surfaces That Currently Look Healthy

### [task_git_status](<parallel-code-root>/src/store/task-git-status.ts)

Current state:

- no current findings
- no current obligations
- no current parity/state findings tied to this configured concept

Useful note:

- one related test pattern in the bundled rules did not match a file: `src/app/git-status-sync.test.ts`
- that is a rules/test-coverage detail, not a current concept failure

### [server_state_bootstrap](<parallel-code-root>/src/app/server-state-bootstrap.ts)

Current state:

- no current findings
- no current obligations
- runtime parity score: `10000 / 10000`
- state integrity score: `10000 / 10000`

Why this matters:

- this configured runtime-contract surface currently looks aligned across browser, electron, registry, and explicit state-model checks
- that makes it a good example of the kind of surface the rules are already helping keep honest

## Suggested Engineer Feedback Prompts

If you send this report to an engineer on `parallel-code`, the most useful feedback questions are:

1. Do the terminal-session, App, TaskPanel, and store-cycle signals match where the team already feels change pain?
2. Which dead private code clusters are obviously stale versus intentionally dormant?
3. Is the large store cycle describing a real layering problem, or is it overstating expected store wiring?
4. For the websocket and glyph clone families, which ones should stay aligned and which ones are intentionally separate?
5. Are the `ConnectionBannerState` and `task_presentation_status` findings still useful as hardening prompts even if they are not top priority?
