# Parallel-Code Proof Review

This note records the reviewed `parallel-code` baseline after the latest analyzer and rules cleanup.

It exists to keep the proof board honest:

1. which top outputs are trusted
2. which outputs were overstated
3. what was changed to fix them

## Trusted Current Signals

These are the current `parallel-code` signals worth inspecting:

- `task_presentation_status` still carries a real `closed_domain_exhaustiveness` hardening opportunity on `TaskDotStatus`
- `ConnectionBannerState` is still a real presentation hardening opportunity
- the `AgentGlyph` / `RemoteAgentGlyph` pair is still a real production clone family
- the `ws-server` / `browser-websocket` pair is still a real production clone family, but it should be treated as a lower-confidence watchpoint
- `server/browser-control-plane.ts` and `electron/ipc/hydra-adapter.ts` are still real coordination watchpoints

## Reviewed Overstatements

These were surfaced during the proof review and then corrected:

### 1. `task_git_status` self-writer pressure

Previous issue:

- the example rules treated `src/store/task-git-status.ts` as an out-of-policy writer even though it is the concept's own store implementation

Fix:

- expanded `allowed_writers` in [parallel-code.rules.toml](./examples/parallel-code.rules.toml) to include `src/store/task-git-status.ts::store.taskGitStatus.*`

Result:

- `task_git_status` no longer appears as a current ownership problem in the reviewed baseline

### 2. `server_state_bootstrap` runtime-contract import bypass

Previous issue:

- runtime-contract anchors in the domain layer were being treated as authoritative import boundaries
- that overstated `src/app/runtime-diagnostics.ts` as a boundary bypass

Fix:

- tightened the boundary-bypass analyzer so generic concept anchors are not treated as authoritative import boundaries unless the concept actually defines authoritative inputs or is an `authoritative_state`

Result:

- `server_state_bootstrap` no longer appears as a current ownership or access issue in the reviewed baseline

## Signals To Treat As Hardening, Not Roadmap Truth

These signals are real, but they are better treated as hardening or watchpoint outputs than as top architectural priorities:

- `task_presentation_status`
- `ConnectionBannerState`
- `ws-server` / `browser-websocket`

## What This Means For The Proof Loop

The current `parallel-code` proof targets are now:

1. seeded ownership regression proof for `task_git_status`
2. propagation cleanup proof for `task_presentation_status`
3. clone-family cleanup proof for `AgentGlyph` / `RemoteAgentGlyph`

That is a stronger proof setup than the older case-study wording, because the baseline now reflects current truth instead of stale analyzer noise.
