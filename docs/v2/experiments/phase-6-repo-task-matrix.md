# Phase 6 Repo Task Matrix

Last audited: 2026-04-19

This matrix freezes the repo and task battery for the active phase-6 questions.

The machine-readable companion lives at [../evals/phase-6-repo-task-matrix.json](../evals/phase-6-repo-task-matrix.json).

The matrix intentionally reuses the existing live and replay batch manifests that still carry the older `phase_5_treatment_baseline` tag. That tag reflects the shared evidence-collection lane, while this document defines the narrower phase-6 product questions that consume the same task battery.

## Active Questions

1. which signal families belong in the default lane
2. whether `large_file` belongs in the default lane

## Repo Matrix

### `sentrux`

Purpose:

- dogfood ranking quality
- repair-packet quality on the product repo
- clone and obligation followthrough under real maintainer pressure

Live batch:

- [../evals/batches/sentrux-codex-session-batch.json](../evals/batches/sentrux-codex-session-batch.json)

Replay batch:

- [../evals/batches/sentrux-diff-replay-batch.json](../evals/batches/sentrux-diff-replay-batch.json)

Most relevant phase-6 tasks:

- [../evals/prompts/sentrux/clone-cleanup.md](../evals/prompts/sentrux/clone-cleanup.md)
- [../evals/prompts/sentrux/clone-followthrough.md](../evals/prompts/sentrux/clone-followthrough.md)
- [../evals/prompts/sentrux/contract-surface-followthrough.md](../evals/prompts/sentrux/contract-surface-followthrough.md)
- [../evals/prompts/sentrux/benchmark-harness-tidy.md](../evals/prompts/sentrux/benchmark-harness-tidy.md)

Phase-6 role:

- test whether clone, obligation, and patch-local concentration consistently beat structural pressure in the default lane

### `parallel-code`

Purpose:

- strongest benchmark repo for clone, boundary, and replay stress
- best existing replay lane for structural pressure and `large_file`

Live batch:

- [../evals/batches/parallel-code-codex-session-batch.json](../evals/batches/parallel-code-codex-session-batch.json)

Replay batch:

- [../evals/batches/parallel-code-diff-replay-batch.json](../evals/batches/parallel-code-diff-replay-batch.json)

Most relevant phase-6 tasks:

- [../evals/prompts/parallel-code/raw-read-guardrail.md](../evals/prompts/parallel-code/raw-read-guardrail.md)
- [../evals/prompts/parallel-code/exhaustiveness-hardening.md](../evals/prompts/parallel-code/exhaustiveness-hardening.md)
- [../evals/prompts/parallel-code/propagation-hardening.md](../evals/prompts/parallel-code/propagation-hardening.md)
- [../evals/prompts/parallel-code/clone-followthrough.md](../evals/prompts/parallel-code/clone-followthrough.md)
- replay commits already tagged with `large_file` in [../evals/batches/parallel-code-diff-replay-batch.json](../evals/batches/parallel-code-diff-replay-batch.json)

Phase-6 role:

- test whether `large_file` is helpful enough to survive next to stronger causal signals

### `one-tool`

Purpose:

- mixed JavaScript and Python repo
- public-safe repo with command-surface and export-followthrough pressure
- best current repo for testing whether `large_file` remains useful outside the first-party codebase

Live batch:

- [../evals/batches/one-tool-codex-session-batch.json](../evals/batches/one-tool-codex-session-batch.json)

Replay batch:

- [../evals/batches/one-tool-diff-replay-batch.json](../evals/batches/one-tool-diff-replay-batch.json)

Most relevant phase-6 tasks:

- [../evals/prompts/one-tool/command-surface-split.md](../evals/prompts/one-tool/command-surface-split.md)
- [../evals/prompts/one-tool/public-entrypoint-followthrough.md](../evals/prompts/one-tool/public-entrypoint-followthrough.md)
- [../evals/prompts/one-tool/mcp-cycle-cut.md](../evals/prompts/one-tool/mcp-cycle-cut.md)

Phase-6 role:

- test whether `large_file` retains value on a simpler public repo where command-surface containment is the main structural question

## Question Coverage

### Default-Lane Family Selection

Use all three repos.

Most important surfaces:

- clone and clone-followthrough
- incomplete propagation and contract followthrough
- boundary and rule violations
- patch-local concentration and reviewability pressure
- `large_file` as the only structural pressure family under active comparison

### `large_file` Admissibility

Use all three repos.

Most important surfaces:

- `parallel-code` replay cases explicitly tagged `large_file`
- `one-tool` command-surface split live task
- `sentrux` benchmark-harness and structural containment pressure

## Freeze Rule

Do not add repo-specific ad hoc tasks mid-cycle unless:

- the task is checked in
- it is tied to one of the two active questions
- the same task becomes part of the fixed matrix for every future comparison of that question
