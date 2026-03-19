# Parallel-Code Case Study For V2

This document anchors the v2 design against a real codebase:

- `~/parallel-code`

It is not the product spec. It is the validation target.

## Why This Repo Matters

`parallel-code` is useful because it is not a random messy repo. It already has:

- explicit architecture docs
- named architectural principles
- architecture guardrail tests
- explicit bootstrap categories
- explicit runtime/state coordination modules

That makes it a good target for static conformance metrics.

The proof-and-improvement workflow for this repo is tracked in [Parallel-Code Proof Board](./parallel-code-proof-board.md).

This case study is evidence-first. It reports objective findings, debt signals, watchpoints, and patch risks. Engineers should use the repo's own architecture docs and tests to decide final prioritization.

## Repo Signals That Matter For V2

The repo's own architecture plan emphasizes:

1. server-owned state should be server-authoritative
2. shared concepts should have one canonical derivation
3. runtime adapters should not quietly own policy
4. restore and startup should be explicit and consistent

That is exactly the surface v2 should measure.

## What This Repo Should Validate First

`parallel-code` should validate the wedge before it validates the full vision.

That means the first successful v2 story on this repo should be:

1. clone drift
2. authority and access regressions
3. obligation completeness for changed concepts

Parity and concentration matter, but they should be treated as secondary context until the wedge is working.

After the wedge is working, the same proof lab should be used to turn the findings into concrete refactors and record the before/after delta. The refactor choice remains an engineering decision, not a v2 decision.

## Verified Current-Repo Signals

These examples have been rechecked on the current `parallel-code` tree and are safe to use as case-study anchors:

- `src/app/task-workflows.ts` imports `setStore` from `../store/core` and performs multiple direct store mutations
- `src/app/git-status-sync.ts` and `src/store/git-status-polling.ts` both write `taskGitStatus`
- `src/store/store.ts` exists as a public store barrel and many component files consume it
- `src/components/SidebarTaskRow.architecture.test.ts` forbids raw reads of `store.agentSupervision`, `store.taskGitStatus`, and `store.taskReview`
- `src/app/task-presentation-status.ts` uses explicit exhaustive records and `assertNever(...)`
- `src/domain/server-state-bootstrap.ts` and `src/app/server-state-bootstrap-registry.ts` make bootstrap categories and registry wiring explicit

## Real-Repo Validation Learnings

The first real MCP validation run against the current `parallel-code` tree taught us more than the draft case-study assumptions did.

What held up:

- the explicit three-concept rules file is viable on the real repo
- resolver confidence is much better than the older draft numbers implied
- disposable-clone proof runs are viable on the real repo

What did not hold up cleanly:

- authority findings are much cleaner once test-only writes are filtered; `task_git_status` drops from a fake 4-writer concept to the 2 real production writer files
- projection concepts now need split read/write semantics rather than all-or-nothing targets; the example rules can keep `authoritative_inputs` again without reintroducing multi-writer noise
- parity now matches the real bootstrap/runtime wiring for `server_state_bootstrap`; the earlier missing live-update findings were analyzer misses, not repo defects
- explicit controller-style state models now map correctly; the earlier `state_model_unmapped` findings were a discriminated-union extraction gap
- zero-config closed-domain findings need beta scoping; filtering test-only sites and oversized domains removes `IPC`-style noise and leaves smaller actionable domains
- exact-clone findings also need beta scoping; filtering test-only/tiny groups, adding git-aware churn/code-age context, and tightening severity moves larger production clones ahead of trivial helpers
- clone findings need distinct-file accounting and deterministic ordering; counting recent activity per file and sorting clone instances stabilizes the case-study goldens and session deltas
- the `task_git_status` self-writer finding was a rules-modeling issue, not a repo defect; the reviewed rules now allow the concept's own store module
- the `server_state_bootstrap` import-bypass finding was an analyzer overstatement; runtime-contract anchors are no longer treated as authoritative boundaries by default
- the remaining clone gap is inspection quality: a few related production clone families can still crowd the top list, which argues for family-level collapse rather than more raw clone rows

This means `parallel-code` is already doing its job as a benchmark repo: it is showing where v2 is useful and where the next analyzer corrections belong.

## Evidence Discipline For This Case Study

Do not anchor v2 messaging on stale or only partially verified examples.

If a direction is valid but the exact example is uncertain:

- keep it as an analyzer target
- do not present it as a confirmed bug

This matters because the repo changes quickly and some early poster-child examples drifted.

## Beta Concepts To Model First

## 1. `task_git_status`

Why:

- authoritative state
- good seeded-regression proof target for wrong-layer writes
- high leverage for CI patch blocking

Likely anchors:

- `src/store/core.ts::store.taskGitStatus`
- `src/store/task-git-status.ts`
- `src/app/task-presentation-status.ts` as the disposable-clone regression seed

Expected v2 value:

- multi-writer detection on a seeded bad change
- writer-layer reporting
- touched-concept gate failure on the seeded regression

## 2. `task_presentation_status`

Why:

- already embodies the desired canonical-projection pattern
- good target for canonical-access checks

Likely anchors:

- `src/app/task-presentation-status.ts::TaskDotStatus`
- `src/app/task-presentation-status.ts::getTaskDotStatus`
- `src/app/task-presentation-status.ts::getTaskAttentionEntry`

Expected v2 value:

- canonical-access checks on consumers
- closed-domain exhaustiveness checks

## 3. `server_state_bootstrap`

Why:

- explicit categories
- explicit registry
- browser/Electron parity is now measurable and satisfied for the scoped bootstrap contract

Likely anchors:

- `src/domain/server-state-bootstrap.ts`
- `src/app/server-state-bootstrap-registry.ts`
- `src/runtime/browser-session.ts`
- `src/app/desktop-session.ts`

Expected v2 value:

- obligation completeness
- contract parity
- startup/restore contract measurement

## Tier 2 Expansion Concepts

## 4. `task_command_controller`

Why:

- cross-runtime control-plane concept
- protocol-heavy
- likely hotspot

Likely anchors:

- `src/store/task-command-controllers.ts`
- `src/app/task-command-lease.ts`
- remote/browser control-plane paths

Expected v2 value:

- authority and parity findings
- concentration risk
- state integrity findings

## 5. `task_convergence`

Why:

- already went through an architecture alignment project
- strong example of moving from scattered derivation to canonical server-owned state

Expected v2 value:

- conformance checks for review surfaces
- authority and obligation tracking

## Architecture Guardrail Tests

This repo already contains source-level architecture guardrails:

- `src/app/desktop-session.architecture.test.ts`
- `src/components/SidebarTaskRow.architecture.test.ts`
- `src/components/review-surfaces.architecture.test.ts`

V2 should use these as evidence that projects often already encode architectural intent in static tests.

## Expected High-Value Findings

Once the first v2 analyzers land, this repo should plausibly produce findings such as:

- `TaskDotStatus` is a real hardening opportunity in the canonical presentation model
- `ConnectionBannerState` is a real presentation hardening opportunity in the runtime/session surface
- clone-drift candidates in `AgentGlyph` / `RemoteAgentGlyph`
- clone-drift candidates in `ws-server` / `browser-websocket`
- `server/browser-control-plane.ts` is a concentrated coordination hotspot
- some lifecycle-heavy modules are explicit state machines and should not be penalized the same way as implicit coordination code

## Evaluation Tasks

- [x] encode the three beta concepts in a draft `rules.toml`
  - current example: [examples/parallel-code.rules.toml](./examples/parallel-code.rules.toml)
- [x] run the first real-repo MCP validation pass with the example rules
- [x] capture initial scoped golden outputs from the current repo state
  - current outputs: [examples/parallel-code-golden](./examples/parallel-code-golden/README.md)
- [x] capture an initial cold/warm MCP benchmark on the real repo
  - current benchmark: [examples/parallel-code-benchmark.md](./examples/parallel-code-benchmark.md)
- [x] review and tighten authority purity on `task_git_status`
  - current result: the reviewed baseline is clean for this concept, and the seeded regression proof now produces `multi_writer_concept` plus `writer_outside_allowlist`
- [ ] verify canonical-access checks on task presentation surfaces
  - current state: projection semantics are now correct enough to express the rule, but the current repo already satisfies the guardrail so there is no violation to prove against yet
- [x] verify direct `setStore` findings in `src/app` with a seeded regression
- [x] verify parity analysis on bootstrap categories
  - current result: the scoped `server_state_bootstrap` contract now scores 10000 and the prior missing live-update findings were confirmed analyzer misses
- [x] verify explicit state-model mapping on bootstrap and browser sync controllers
  - current result: both configured state models now score 10000 on the real repo
- [x] verify git-aware clone-drift findings on meaningful production clone families
  - current result: clone findings now carry stable ids, churn/code-age context, and better production-first ranking on the real repo
- [x] run disposable-clone proof scenarios for ownership, propagation, and clone cleanup
  - current outputs: [examples/parallel-code-proof-runs](./examples/parallel-code-proof-runs/README.md)
- [ ] add divergence detection and family-level collapse for related clone clusters
- [ ] defer `task_command_controller` and `task_convergence` until Tier 2
- [ ] verify concentration analysis on lease and restore controllers
- [ ] verify session delta output on a synthetic closed-domain change

## Proof-And-Improvement Plan

The current case-study work should be used to prove that v2 can improve `parallel-code`, not only validate it.

The three proof targets are:

1. seeded `task_git_status` ownership regression proof
2. `task_presentation_status` propagation cleanup
3. `AgentGlyph` / `RemoteAgentGlyph` clone-family cleanup

For each target:

1. capture the current findings, debt signals, and watchpoints
2. make one disposable-clone change
3. rerun the proof loop
4. record the before/after delta

The proof board in [Parallel-Code Proof Board](./parallel-code-proof-board.md) is the running checklist for that work.

## Success Condition

The case study is successful when v2 tells a coherent patch-safety and technical-debt story about `parallel-code` without relying on v1 depth and cycle penalties as the primary signal.

For beta, that means:

1. it catches clone drift, authority drift, and incomplete propagation in changed concepts
2. it produces a useful `session_end` report for agent patches
3. it uses parity and concentration as supporting context rather than the primary narrative
4. engineers can still choose what to fix first using repo-owned context
