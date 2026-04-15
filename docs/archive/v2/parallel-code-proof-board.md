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

Status: complete

Goal:

- capture the current `parallel-code` state as a stable baseline

Checklist:

- [x] scoped goldens exist for the current beta concepts
- [x] benchmark artifacts exist
- [x] proof commands run against disposable clones
- [x] top findings and concept summaries are reproducible
- [x] refresh the board whenever the repo or analyzer output changes materially
- [x] checked-in proof snapshot exists

Expected proof output:

- stable `findings`
- stable `session_end`
- stable `gate`
- stable `obligations`
- stable benchmark artifact

### Phase 2: Validate The Top Issues

Status: complete

Goal:

- make sure the current top outputs are the right things to optimize

Checklist:

- [x] review the top ownership and boundary findings
- [x] review the top obligation and propagation findings
- [x] review the top clone and hotspot findings
- [x] keep a short false-positive / overstatement log
- [x] tune or suppress anything that is noisy but not useful

Expected proof output:

- a small set of trusted findings
- a small set of trusted concept summaries
- a small set of trusted debt signals and watchpoints

Current review artifact:

- [Parallel-Code Proof Review](./parallel-code-proof-review.md)

### Phase 3: Select Proof Targets

Status: complete

Goal:

- choose three maintainability improvements that best demonstrate v2 value

Current proof targets:

1. seeded `task_git_status` ownership regression proof
2. `task_presentation_status` propagation cleanup
3. `AgentGlyph` / `RemoteAgentGlyph` clone-family cleanup

Checklist:

- [x] target 1 chosen
- [x] target 2 chosen
- [x] target 3 chosen
- [x] capture expected before-state proof signals for each target
- [x] record the refactor outcome we want from each target

Expected proof output:

- one ownership/boundary cleanup
- one propagation/obligation cleanup
- one duplication/hotspot cleanup

### Phase 4: Refactor And Measure

Status: complete

Goal:

- use v2 to drive actual cleanups, then prove the repo improved

Checklist:

- [x] prove target 1 with a disposable-clone seeded ownership regression
- [x] rerun v2 and compare before/after
- [x] refactor target 2 in a disposable clone
- [x] rerun v2 and compare before/after
- [x] refactor target 3 in a disposable clone
- [x] rerun v2 and compare before/after

Expected proof output:

- fewer or cleaner findings for the target concept
- clearer ownership or propagation boundaries
- lower clone or hotspot pressure

Current proof outputs:

- [Proof Snapshot](./examples/parallel-code-proof-snapshot.md)
- [Disposable Proof Runs](./examples/parallel-code-proof-runs/README.md)

### Phase 5: Publish The Case Study

Status: complete

Goal:

- turn the proof loop into a durable case study

Checklist:

- [x] document the before state for each target
- [x] document the refactor made
- [x] document the after state and the v2 delta
- [x] keep the case study anchored to verified examples only

Expected proof output:

- a readable proof narrative
- explicit before/after value
- a reliable target list for future tuning

## Proof Targets

### 1. `task_git_status`

Why it matters:

- the reviewed baseline is now clean for this concept
- that makes it a better seeded-regression proof than a current-repo cleanup target

What a successful proof should do:

- introduce an out-of-policy write from an app-layer file
- show `multi_writer_concept` and `writer_outside_allowlist`
- show that `gate` fails on the touched concept

### 2. `task_presentation_status`

Why it matters:

- it is the cleanest propagation and obligation target
- the current obligations output shows a missing exhaustive mapping site for `TaskDotStatus`

What a successful refactor should do:

- close the missing update site
- reduce obligation burden
- keep canonical access explicit

### 3. `AgentGlyph` / `RemoteAgentGlyph`

Why it matters:

- it is the highest-ranked current production clone family in the reviewed baseline
- it is a clean, deterministic deduplication proof target

What a successful refactor should do:

- extract the shared glyph rendering into one helper module
- remove the current clone family from the findings surface
- leave the remaining hotspot list cleaner and more reviewable

## Proof Loop Output

Every proof run should record:

- current top findings
- current top concept summaries
- current debt signals and watchpoints
- the proof target selected
- the before-state evidence
- the after-state evidence
- the resulting maintainability takeaway

## Guardrails

- Do not use the live `parallel-code` worktree as the benchmark source of truth.
- Do not treat runtime stability as proof of maintainability improvement.
- Do not add new beta concepts until the current proof targets are closed or explicitly re-scoped.
