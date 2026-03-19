# Parallel Code: Live Analysis Report For Engineers

Generated on March 19, 2026 from the live checkout at `<parallel-code-root>`.

Historical note:

- this report captures the earlier live-review pass that informed the maintainer verdict loop
- for the current evidence-first structural debt surface, use [parallel-code-proof-snapshot.md](<sentrux-root>/docs/v2/examples/parallel-code-proof-snapshot.md)

## Audience

This report is for an engineer who does not already know `parallel-code` or Sentrux.

It answers two questions:

1. what current code-quality or maintainability issues look worth inspecting
2. which areas do **not** currently look like current debt hotspots

## What Was Analyzed

This was a fresh live analysis of the current `~/parallel-code` checkout using Sentrux v2 on a disposable clone.

Important scope note:

- `parallel-code` does **not** currently carry its own `.sentrux/rules.toml`
- this run used the example rules file shipped with Sentrux:
  [parallel-code.rules.toml](<sentrux-root>/docs/v2/examples/parallel-code.rules.toml)
- that rules file configures three critical concepts:
  - `task_git_status`
  - `task_presentation_status`
  - `server_state_bootstrap`
- repo-wide clone, hotspot, parity, and state checks were also run

Analysis coverage for this run:

- 597 scanned source files
- 136,404 scanned lines
- 1,770 resolved import edges
- scan confidence: `8189 / 10000`
- configured rule coverage: `10000 / 10000`

## What Parallel Code Appears To Be

From the repo README, `parallel-code` is a TypeScript app for running multiple AI coding agents in isolated git worktrees, with:

- Electron and browser/server modes
- a SolidJS frontend
- local and remote/mobile UI surfaces
- task state, session/bootstrap logic, websocket transport, and review/supervision flows

That means the most expensive defects are likely to be:

- UI state drift
- duplicated runtime logic across Electron/browser surfaces
- missing propagation when state or contracts evolve
- orchestration files accumulating too many responsibilities

## Executive Summary

The current live run surfaces **two strong debt signals**, **two hardening/watchpoint signals**, **one coordination watchpoint**, and **two areas that currently do not look like problems**.

### Strong debt signals

1. `AgentGlyph.tsx` and `RemoteAgentGlyph.tsx` contain a real duplicate-maintenance cluster
2. `server/browser-control-plane.ts` is a coordination hotspot worth watching closely

### Hardening and watchpoints

1. `ConnectionBannerState` has a real presentation hardening opportunity
2. `task_presentation_status` has a real canonical-model hardening opportunity
3. `electron/remote/ws-server.ts` and `server/browser-websocket.ts` are a lower-confidence transport watchpoint

### Coordination watchpoint

1. `electron/ipc/hydra-adapter.ts` is a coordination watchpoint, but it is less compelling than `server/browser-control-plane.ts`

### Areas that currently look healthy

1. `task_git_status` does **not** currently look like an ownership problem in the live baseline
2. `server_state_bootstrap` currently looks healthy on parity and configured state-integrity checks

## Inspection Notes

The report does **not** assign final priority. The repo's own architecture docs should be used for that. In particular, the current repo plan points to startup/session/restore alignment as the active architecture workstream.

### 1. Inspect `task_presentation_status` as a hardening opportunity

Files:

- [task-presentation-status.ts](<parallel-code-root>/src/app/task-presentation-status.ts)

Current signal:

- `medium` `closed_domain_exhaustiveness`
- missing required update site for `TaskDotStatus`

Why this matters:

- this is a change-propagation hardening opportunity, not a cosmetic issue
- if `TaskDotStatus` evolves, there is no single explicit exhaustive mapping proving all states are handled
- this is the kind of issue that can cause subtle UI drift later

What Sentrux is saying:

- there is no exhaustive mapping or switch site for `TaskDotStatus`
- the missing site is in [task-presentation-status.ts](<parallel-code-root>/src/app/task-presentation-status.ts)

Recommended action:

- add one explicit total mapping for `TaskDotStatus`
- keep it in [task-presentation-status.ts](<parallel-code-root>/src/app/task-presentation-status.ts) as the canonical presentation surface
- keep the existing tests around that surface strong

Expected payoff:

- easier future edits to task presentation state
- lower chance of “new status added, one display path forgotten”

### 2. Inspect `ConnectionBannerState` as a presentation hardening opportunity

Files:

- [App.tsx](<parallel-code-root>/src/App.tsx)
- [browser-session.ts](<parallel-code-root>/src/runtime/browser-session.ts)

Current signal:

- `high` `closed_domain_exhaustiveness`
- missing coverage for:
  - `connecting`
  - `reconnecting`
  - `restoring`

Why this matters:

- this looks like a genuine runtime/UI state alignment gap
- connection lifecycle code tends to drift if rendering logic is spread across conditions instead of one explicit mapping

Recommended action:

- define one explicit banner-state mapping surface
- make banner text/state rendering exhaustive for `ConnectionBannerState`
- keep runtime state definitions and app rendering aligned around that one surface

Expected payoff:

- cleaner connection lifecycle behavior
- fewer edge-case UI mismatches during reconnect/restore flows

### 3. Collapse the glyph duplication between local and remote UI

Files:

- [AgentGlyph.tsx](<parallel-code-root>/src/components/AgentGlyph.tsx)
- [RemoteAgentGlyph.tsx](<parallel-code-root>/src/remote/RemoteAgentGlyph.tsx)

Current signal:

- clone family score: `86`
- 4 exact clone groups across 2 files

Duplicated functions:

- `CodexGlyph`
- `ClaudeGlyph`
- `HydraGlyph`
- `GenericGlyph`

Why this matters:

- this is real duplicated product logic in two production files
- the cost is not just file size; it is future drift whenever one icon or variant changes and the sibling is forgotten

Recommended action:

- extract shared glyph rendering into one reusable module
- keep only context-specific wrappers where the local and remote surfaces truly differ
- add a small shared rendering test before deduplicating if you want a safer refactor

Expected payoff:

- less duplicated UI logic
- much lower drift risk between local and remote app surfaces

### 4. Treat websocket pause/resume behavior as a watchpoint, not a top refactor target

Files:

- [ws-server.ts](<parallel-code-root>/electron/remote/ws-server.ts)
- [browser-websocket.ts](<parallel-code-root>/server/browser-websocket.ts)

Current signal:

- clone family score: `78`
- 4 exact clone groups across 2 files
- current named duplicated functions include:
  - `pause`
  - `resume`

Why this matters:

- this is transport/control behavior duplicated across Electron and browser/server runtime code
- the surfaces are no longer symmetric shells, so this should be treated carefully

Recommended action:

- decide whether these two files are meant to stay behaviorally aligned
- if yes, extract shared pause/resume framing and sibling helpers
- if no, document and intentionally split the behavior instead of letting them look accidentally duplicated

Expected payoff:

- safer runtime evolution across the two transport surfaces
- less chance of subtle mode-specific behavior skew

### 5. Split the two coordination hotspots before they get worse

Files:

- [browser-control-plane.ts](<parallel-code-root>/server/browser-control-plane.ts)
- [hydra-adapter.ts](<parallel-code-root>/electron/ipc/hydra-adapter.ts)

Current signal:

- both are medium-severity hotspot opportunities
- [browser-control-plane.ts](<parallel-code-root>/server/browser-control-plane.ts)
  - 6 side-effect targets
  - 6 timer/retry coordination signals
  - 3 async/branching control signals
- [hydra-adapter.ts](<parallel-code-root>/electron/ipc/hydra-adapter.ts)
  - 11 side-effect targets
  - 3 timer/retry coordination signals
  - 19 async/branching control signals

Why this matters:

- these files are becoming coordination choke points
- even without a current bug, this is a maintainability warning that future feature work will be harder and riskier here

Recommended action:

- split orchestration from state mutation
- split transport or adapter responsibilities from policy decisions
- avoid adding new behavior here until one extraction pass is done

Expected payoff:

- easier reasoning about side effects
- fewer “god-file” style regressions later

## What Does **Not** Look Like A Current Debt Hotspot

These are useful because they tell the engineer where **not** to spend time first.

### `task_git_status`

Files:

- [task-git-status.ts](<parallel-code-root>/src/store/task-git-status.ts)
- [taskStatus.ts](<parallel-code-root>/src/store/taskStatus.ts)
- [task-presentation-status.ts](<parallel-code-root>/src/app/task-presentation-status.ts)

Current result:

- no current findings
- no current obligations

Interpretation:

- this concept is configured as an authoritative state surface
- on the current baseline, it does **not** look like a current architectural defect

This is useful because an older proof pass had an overstatement here. The live baseline does not support that concern anymore.

### `server_state_bootstrap`

Files:

- [server-state-bootstrap.ts](<parallel-code-root>/src/app/server-state-bootstrap.ts)
- [server-state-bootstrap-registry.ts](<parallel-code-root>/src/app/server-state-bootstrap-registry.ts)
- [browser-session.ts](<parallel-code-root>/src/runtime/browser-session.ts)
- [desktop-session.ts](<parallel-code-root>/src/app/desktop-session.ts)

Current result:

- parity score: `10000 / 10000`
- state-integrity findings: none

Interpretation:

- the configured runtime contract and explicit state model around bootstrap currently look healthy
- this area does not look like the best immediate cleanup target

## Suggested Fix Order

If the team wants one pragmatic order, this is the one I would use:

1. add the explicit `TaskDotStatus` mapping in [task-presentation-status.ts](<parallel-code-root>/src/app/task-presentation-status.ts)
2. centralize exhaustive `ConnectionBannerState` handling
3. deduplicate [AgentGlyph.tsx](<parallel-code-root>/src/components/AgentGlyph.tsx) and [RemoteAgentGlyph.tsx](<parallel-code-root>/src/remote/RemoteAgentGlyph.tsx)
4. review and likely deduplicate the websocket `pause` / `resume` logic
5. split [browser-control-plane.ts](<parallel-code-root>/server/browser-control-plane.ts) and [hydra-adapter.ts](<parallel-code-root>/electron/ipc/hydra-adapter.ts) before adding more behavior

## Feedback Requested From The Engineer

Please answer these directly against the code, not against Sentrux:

1. Is `ConnectionBannerState` intended to have one canonical exhaustive mapping, or is the current split intentional?
2. Is [task-presentation-status.ts](<parallel-code-root>/src/app/task-presentation-status.ts) the right place to own the full `TaskDotStatus` mapping?
3. Are [AgentGlyph.tsx](<parallel-code-root>/src/components/AgentGlyph.tsx) and [RemoteAgentGlyph.tsx](<parallel-code-root>/src/remote/RemoteAgentGlyph.tsx) meant to evolve together, or are they expected to diverge?
4. Are [ws-server.ts](<parallel-code-root>/electron/remote/ws-server.ts) and [browser-websocket.ts](<parallel-code-root>/server/browser-websocket.ts) intentionally duplicated because of platform constraints, or is this accidental?
5. Which of the two hotspot files is already painful to change in practice:
   - [browser-control-plane.ts](<parallel-code-root>/server/browser-control-plane.ts)
   - [hydra-adapter.ts](<parallel-code-root>/electron/ipc/hydra-adapter.ts)

## Caveats

- this is a real live analysis of the current repo state
- it is **not** zero-config analysis; it uses the Sentrux example rules for three important concepts
- that means:
  - the configured concept findings are high-value but intentionally scoped
  - clone/hotspot signals are broader and more heuristic
- there was no current session baseline in the live repo, so this report is baseline analysis, not patch-delta analysis

## Appendix

See the detailed evidence in:

- [parallel-code-live-engineer-report-appendix.md](<sentrux-root>/docs/v2/examples/parallel-code-live-engineer-report-appendix.md)
