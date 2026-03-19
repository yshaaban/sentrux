# Parallel-Code Proof Board

This board tracks the single proof lab for v2.

The goal is to prove two things on `parallel-code`:

1. v2 catches meaningful code issues
2. v2 helps drive real maintainability improvements

The proof lab must use disposable clones for every regeneration, benchmark, and refactor proof run.
The live `parallel-code` worktree may be dirty, and the proof loop should not depend on it being clean.

## Working Rules

1. Use disposable clones for all proof runs.
2. Treat checked-in goldens as the baseline.
3. Treat the proof board as the source of truth for current proof targets.
4. Do not expand beta scope while proof targets remain unresolved.
5. Record before/after evidence for every refactor target.

## Proof Phases

### Phase 1: Freeze Baseline

Status: complete in artifacts, refreshable on demand

Goal:

- capture the current `parallel-code` state as a stable baseline

Checklist:

- [x] scoped goldens exist for the current beta concepts
- [x] benchmark artifacts exist
- [x] proof commands run against disposable clones
- [x] top findings and concept summaries are reproducible
- [ ] refresh the board whenever the repo or analyzer output changes materially

Expected proof output:

- stable `findings`
- stable `session_end`
- stable `gate`
- stable `obligations`
- stable benchmark artifact

### Phase 2: Validate The Top Issues

Status: in progress

Goal:

- make sure the current top outputs are the right things to optimize

Checklist:

- [x] review the top ownership and boundary findings
- [x] review the top obligation and propagation findings
- [x] review the top clone and hotspot findings
- [ ] keep a short false-positive / overstatement log
- [ ] tune or suppress anything that is noisy but not useful

Expected proof output:

- a small set of trusted findings
- a small set of trusted concept summaries
- a small set of trusted optimization priorities

### Phase 3: Select Proof Targets

Status: in progress

Goal:

- choose three maintainability improvements that best demonstrate v2 value

Current proof targets:

1. `task_git_status` ownership and boundary purity
2. `task_presentation_status` propagation and obligations
3. `task_command_controller` / `task-command-lease.ts` plus the diff-parsing/shared-escaping clone family

Checklist:

- [x] target 1 chosen
- [x] target 2 chosen
- [x] target 3 chosen
- [ ] capture expected before-state proof signals for each target
- [ ] record the refactor outcome we want from each target

Expected proof output:

- one ownership/boundary cleanup
- one propagation/obligation cleanup
- one duplication/hotspot cleanup

### Phase 4: Refactor And Measure

Status: pending

Goal:

- use v2 to drive actual cleanups, then prove the repo improved

Checklist:

- [ ] refactor target 1 in a disposable clone
- [ ] rerun v2 and compare before/after
- [ ] refactor target 2 in a disposable clone
- [ ] rerun v2 and compare before/after
- [ ] refactor target 3 in a disposable clone
- [ ] rerun v2 and compare before/after

Expected proof output:

- fewer or cleaner findings for the target concept
- clearer ownership or propagation boundaries
- lower clone or hotspot pressure

### Phase 5: Publish The Case Study

Status: pending

Goal:

- turn the proof loop into a durable case study

Checklist:

- [ ] document the before state for each target
- [ ] document the refactor made
- [ ] document the after state and the v2 delta
- [ ] keep the case study anchored to verified examples only

Expected proof output:

- a readable proof narrative
- explicit before/after value
- a reliable target list for future tuning

## Proof Targets

### 1. `task_git_status`

Why it matters:

- it is the clearest ownership and boundary target in the current proof set
- the current outputs show repeated `writer_outside_allowlist` and `authoritative_import_bypass` pressure

What a successful refactor should do:

- reduce duplicate ownership pressure
- centralize mutation through the intended owner
- shrink bypass evidence around the concept

### 2. `task_presentation_status`

Why it matters:

- it is the cleanest propagation and obligation target
- the current obligations output shows a missing exhaustive mapping site for `TaskDotStatus`

What a successful refactor should do:

- close the missing update site
- reduce obligation burden
- keep canonical access explicit

### 3. `task_command_controller` / clone family

Why it matters:

- it combines hotspot pressure with duplication pressure
- the case study already points to coordination complexity and clone families that matter for maintainability

What a successful refactor should do:

- split or simplify the hotspot
- reduce related clone-family pressure
- make the coordination path easier to reason about

## Proof Loop Output

Every proof run should record:

- current top findings
- current top concept summaries
- current optimization priorities
- the proof target selected
- the before-state evidence
- the after-state evidence
- the resulting maintainability takeaway

## Guardrails

- Do not use the live `parallel-code` worktree as the benchmark source of truth.
- Do not treat runtime stability as proof of maintainability improvement.
- Do not add new beta concepts until the current proof targets are closed or explicitly re-scoped.

