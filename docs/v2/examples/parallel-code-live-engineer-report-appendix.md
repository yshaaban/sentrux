# Parallel Code: Live Analysis Report Appendix

Generated on March 19, 2026 from the live checkout at `<parallel-code-root>`.

This appendix contains the evidence behind the maintainer-facing report in
[parallel-code-live-engineer-report.md](<sentrux-root>/docs/v2/examples/parallel-code-live-engineer-report.md).

## Method

The analysis was run against a disposable clone of the live `~/parallel-code` checkout with:

- Sentrux binary:
  [sentrux](<sentrux-root>/target/debug/sentrux)
- rules file:
  [parallel-code.rules.toml](<sentrux-root>/docs/v2/examples/parallel-code.rules.toml)
- refresh path:
  [refresh_parallel_code_goldens.sh](<sentrux-root>/scripts/refresh_parallel_code_goldens.sh)
- benchmark path:
  [benchmark_parallel_code_v2.mjs](<sentrux-root>/scripts/benchmark_parallel_code_v2.mjs)

This was a **live** run, but the rules came from Sentrux’s example config, not from a project-owned `.sentrux/rules.toml` inside `parallel-code`.

## Scan Scope And Confidence

Current scan:

- scanned files: `597`
- scanned lines: `136,404`
- resolved import edges: `1,770`
- kept files from git candidate set: `597 / 729`
- excluded files: `132`
- scan confidence: `8189 / 10000`
- rule coverage: `10000 / 10000`
- semantic rules loaded: `true`

Interpretation:

- the run had full configured-rule coverage
- scan confidence is lower than rule coverage because the repo includes excluded/generated/vendor/build surfaces outside the kept analysis set
- that is normal for a repo of this shape

## Configured Concepts And Contracts

Configured concepts from
[parallel-code.rules.toml](<sentrux-root>/docs/v2/examples/parallel-code.rules.toml):

### `task_git_status`

- kind: `authoritative_state`
- intended writer surfaces:
  - [git-status-sync.ts](<parallel-code-root>/src/app/git-status-sync.ts)
  - [task-git-status.ts](<parallel-code-root>/src/store/task-git-status.ts)
- intended canonical accessors:
  - [taskStatus.ts](<parallel-code-root>/src/store/taskStatus.ts)
  - [task-presentation-status.ts](<parallel-code-root>/src/app/task-presentation-status.ts)

### `task_presentation_status`

- kind: `projection`
- intended authoritative inputs:
  - `store.agentSupervision`
  - `store.taskGitStatus`
  - `store.taskReview`
- intended canonical surface:
  - [task-presentation-status.ts](<parallel-code-root>/src/app/task-presentation-status.ts)
  - [task-attention.ts](<parallel-code-root>/src/app/task-attention.ts)

### `server_state_bootstrap`

- kind: `runtime_contract`
- anchors:
  - [server-state-bootstrap.ts](<parallel-code-root>/src/app/server-state-bootstrap.ts)
  - [server-state-bootstrap-registry.ts](<parallel-code-root>/src/app/server-state-bootstrap-registry.ts)
  - [server-state-bootstrap.ts domain](<parallel-code-root>/src/domain/server-state-bootstrap.ts)

Configured contract:

- `server_state_bootstrap`
- browser entry:
  [browser-session.ts](<parallel-code-root>/src/runtime/browser-session.ts)
- electron entry:
  [desktop-session.ts](<parallel-code-root>/src/app/desktop-session.ts)

Configured explicit state models:

- [browser-state-sync-controller.ts](<parallel-code-root>/src/runtime/browser-state-sync-controller.ts)
- [server-state-bootstrap.ts](<parallel-code-root>/src/app/server-state-bootstrap.ts)

## Top Findings From The Live Run

### 1. `ConnectionBannerState`

- severity: `high`
- kind: `closed_domain_exhaustiveness`
- files:
  - [App.tsx](<parallel-code-root>/src/App.tsx)
  - [browser-session.ts](<parallel-code-root>/src/runtime/browser-session.ts)
- summary:
  `Closed domain 'ConnectionBannerState' is missing coverage for variants: connecting, reconnecting, restoring`

Supporting concept summary:

- score: `3100 / 10000`
- dominant kind: `closed_domain_exhaustiveness`
- summary:
  `Concept 'ConnectionBannerState' has repeated high-severity ownership or access issues`

Related quality opportunity:

- kind: `concept`
- score: `4305 / 10000`
- hotspot overlap:
  [browser-session.ts](<parallel-code-root>/src/runtime/browser-session.ts)

### 2. `task_presentation_status`

- severity: `medium`
- kind: `closed_domain_exhaustiveness`
- files:
  - [task-presentation-status.ts](<parallel-code-root>/src/app/task-presentation-status.ts)
- summary:
  `Closed domain 'task_presentation_status' is missing required update sites`

Missing site evidence:

- [task-presentation-status.ts](<parallel-code-root>/src/app/task-presentation-status.ts)
  - line `12`
  - `no exhaustive mapping or switch site found for 'TaskDotStatus'`

Related concept summary:

- score: `1680 / 10000`
- missing site count: `1`
- obligation count: `1`
- summary:
  `Concept 'task_presentation_status' has 1 missing update sites to complete`

Related tests already matched by the configured concept:

- [task-attention.test.ts](<parallel-code-root>/src/app/task-attention.test.ts)
- [task-presentation-status.test.ts](<parallel-code-root>/src/app/task-presentation-status.test.ts)
- [SidebarTaskRow.architecture.test.ts](<parallel-code-root>/src/components/SidebarTaskRow.architecture.test.ts)
- [SidebarTaskRow.test.tsx](<parallel-code-root>/src/components/SidebarTaskRow.test.tsx)

### 3. Clone family: `AgentGlyph` / `RemoteAgentGlyph`

Files:

- [AgentGlyph.tsx](<parallel-code-root>/src/components/AgentGlyph.tsx)
- [RemoteAgentGlyph.tsx](<parallel-code-root>/src/remote/RemoteAgentGlyph.tsx)

Clone family summary:

- family score: `86`
- member count: `4`
- recent file age gap: `1 day`
- recent commit gap: `0`

Exact clone groups surfaced:

- `CodexGlyph` (`30` lines)
- `ClaudeGlyph` (`25` lines)
- `HydraGlyph` (`16` lines)
- `GenericGlyph` (`10` lines)

Primary remediation hints:

- synchronize recent divergence explicitly if the files are meant to stay aligned
- extract a shared helper/module
- collapse repeated clone groups behind one named abstraction
- add focused shared behavior tests before extraction

### 4. Clone family: `ws-server` / `browser-websocket`

Files:

- [ws-server.ts](<parallel-code-root>/electron/remote/ws-server.ts)
- [browser-websocket.ts](<parallel-code-root>/server/browser-websocket.ts)

Clone family summary:

- family score: `78`
- member count: `4`
- recent commit gap: `3`
- recent file age gap: `0 days`

Named exact clone groups surfaced in the top findings:

- `resume` (`11` lines)
- `pause` (`11` lines)

Primary remediation hints:

- review whether the two runtime surfaces should still be behaviorally identical
- synchronize or intentionally split
- extract shared helper logic if behavior should stay aligned

### 5. Hotspots

### `server/browser-control-plane.ts`

File:

- [browser-control-plane.ts](<parallel-code-root>/server/browser-control-plane.ts)

Signal:

- hotspot score: `5350 / 10000`
- evidence:
  - `6` side-effect targets
  - `6` timer/retry coordination signals
  - `3` async/branching control signals

### `electron/ipc/hydra-adapter.ts`

File:

- [hydra-adapter.ts](<parallel-code-root>/electron/ipc/hydra-adapter.ts)

Signal:

- hotspot score: `4827 / 10000`
- evidence:
  - `11` side-effect targets
  - `3` timer/retry coordination signals
  - `19` async/branching control signals

## Other Lower-Priority Clone Signals

These are real, but I would rank them behind the items above:

### `client-session` / `persistence-load`

Files:

- [client-session.ts](<parallel-code-root>/src/store/client-session.ts)
- [persistence-load.ts](<parallel-code-root>/src/store/persistence-load.ts)

Current duplicate:

- `isStringNumberRecord` (`9` lines)

### `App` / `remote/App`

Files:

- [App.tsx](<parallel-code-root>/src/App.tsx)
- [remote/App.tsx](<parallel-code-root>/src/remote/App.tsx)

Current duplicates:

- `markBusyTakeoverRequest` (`5` lines)
- `clearBusyTakeoverRequest` (`5` lines)

These may still be worth cleaning up, but they are not my first recommendation for engineering time.

## Current Non-Findings That Matter

### `task_git_status`

Current status:

- no findings
- no obligations

Useful detail:

- configured readers and writers are present
- the live run did **not** reproduce the old ownership overstatement

One note:

- one configured related test pattern did not match:
  `src/app/git-status-sync.test.ts`

That is a test-coverage/config maintenance note, not a current architectural finding.

### `server_state_bootstrap`

Current status:

- parity findings: none
- parity score: `10000 / 10000`
- state findings: none
- state integrity score: `10000 / 10000`

This makes it a poor first cleanup target relative to the findings above.

## Clean-Tree Patch-Safety Result

The live repo was analyzed in a no-change state.

Current `gate` result:

- decision: `pass`
- changed files: `0`
- blocking findings: `0`
- missing obligations: `0`
- obligation completeness: `10000 / 10000`

This means the current baseline is stable enough for patch-delta use, but this report is still about **baseline maintainability** rather than a live patch review.

## Performance Notes For This Live Run

These are tool/runtime notes, not project-health findings.

Current live benchmark:

- cold process total: `20,161.4 ms`
- cold scan: `12,361.4 ms`
- warm cached total: `1,077.6 ms`
- warm patch-safety total: `4,838.3 ms`
- warm gate: `4,505.2 ms`
- warm session_end: `35.1 ms`

Compared with the current blessed benchmark artifact:

- fail-tier regressions:
  - cold process total: `+3388.6 ms` (`+20.2%`)
  - cold scan: `+2585.5 ms` (`+26.4%`)
- warn-tier regressions:
  - warm cached total: `+188.9 ms` (`+21.3%`)
  - warm patch-safety total: `+688.4 ms` (`+16.6%`)
  - warm gate: `+639.6 ms` (`+16.5%`)

Interpretation:

- these are Sentrux performance deltas, not `parallel-code` product defects
- the code-quality findings above remain useful even though the analysis run was slower than the blessed benchmark artifact

## Suggested Engineer Feedback Format

If you want structured feedback from the engineer, ask them to respond inline to these statements:

1. `ConnectionBannerState` coverage issue is:
   - real
   - partly real
   - not useful
2. `TaskDotStatus` should be exhaustively mapped in [task-presentation-status.ts](<parallel-code-root>/src/app/task-presentation-status.ts):
   - yes
   - maybe elsewhere
   - no
3. `AgentGlyph` and `RemoteAgentGlyph` should be deduplicated:
   - yes
   - maybe
   - no
4. `ws-server` and `browser-websocket` should stay synchronized:
   - yes
   - maybe
   - no
5. The most painful current hotspot is:
   - [browser-control-plane.ts](<parallel-code-root>/server/browser-control-plane.ts)
   - [hydra-adapter.ts](<parallel-code-root>/electron/ipc/hydra-adapter.ts)
   - neither
