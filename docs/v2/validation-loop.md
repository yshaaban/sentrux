# Sentrux V2 Validation Loop

This document defines the repeatable proof loop for the v2 wedge on `parallel-code`.

The goal is not only to generate artifacts. The goal is to make regressions easy to detect, review, and refresh without guessing.

The proof-and-improvement workflow is tracked separately in [Parallel-Code Proof Board](./parallel-code-proof-board.md).

## What The Loop Covers

The loop validates three separate things:

1. scoped real-repo goldens
2. benchmark regression behavior
3. the baseline and migration story around v1 and v2 coexistence
4. proof-and-improvement runs on disposable clones

## Commands

### Refresh Goldens

Use this when the checked-in real-repo outputs need to be regenerated.

```bash
./scripts/refresh_parallel_code_goldens.sh
```

This command:

- clones `parallel-code` into a temporary workspace
- installs the example v2 rules file
- generates the checked-in `parallel-code-golden` JSON outputs
- writes deterministic fail-path outputs from a temporary mutation

The live `parallel-code` worktree may be dirty, so every proof refresh must go through a disposable clone.

### Validate Goldens And Benchmark

Use this for the normal proof loop.

```bash
node scripts/validate_parallel_code_v2.mjs
```

This command:

- regenerates the real-repo goldens into a temporary directory
- compares them against the checked-in goldens
- runs the benchmark harness against the checked-in benchmark artifact
- fails if the benchmark comparison reports a regression

The proof board explains how the outputs from this command should be turned into real refactor targets and before/after proof records.

### Benchmark Only

Use this when you only want performance data.

```bash
node scripts/benchmark_parallel_code_v2.mjs
```

### Golden Only

Use this when you want to inspect output stability without rerunning the benchmark.

```bash
node scripts/validate_parallel_code_v2.mjs --goldens-only
```

## What The Validation Script Checks

The validation runner compares the checked-in `parallel-code-golden` files against a fresh temporary run:

- `scan.json`
- `concepts.json`
- `findings-top12.json`
- `explain-task_git_status.json`
- `explain-task_presentation_status.json`
- `explain-server_state_bootstrap.json`
- `obligations-task_presentation_status.json`
- `parity-server_state_bootstrap.json`
- `state.json`
- `session-start.json`
- `gate-pass.json`
- `gate-fail.json`
- `session-end-pass.json`
- `session-end-fail.json`
- `metadata.json`

The `metadata.json` check ignores the timestamp field and verifies the stable payload instead.

## What The Benchmark Harness Checks

The benchmark harness records and compares:

- cold scan latency
- first semantic materialization latency
- warm cached semantic latency
- warm patch-safety latency
- regression thresholds across benchmark runs

The checked-in benchmark artifact is versioned so incompatible benchmark shapes do not get compared accidentally.

## How To Use Failures

If goldens fail:

1. decide whether the output change is intentional
2. if intentional, refresh the checked-in goldens
3. if not intentional, fix the analyzer or the harness

If the benchmark fails:

1. confirm whether the change is a real regression or a noisy run
2. rerun with the same artifact before changing the baseline
3. only update the baseline after the change is understood

## Proof-And-Improvement Loop

Use this when the goal is to improve `parallel-code`, not just validate analyzer stability.

1. freeze the current baseline outputs
2. review the top findings, concept summaries, and optimization priorities
3. select one ownership/boundary target, one propagation/obligation target, and one duplication/hotspot target
4. make one refactor at a time in a disposable clone
5. rerun the proof loop after each refactor
6. record the before/after delta in the proof board and the case study

The proof board in [Parallel-Code Proof Board](./parallel-code-proof-board.md) is the tracking surface for that loop.

## Relationship To Migration

The validation loop is intentionally separate from baseline migration, but it depends on the same rollout assumptions:

- v1 and v2 baselines remain separate
- v2 validation should never synthesize v2 behavior from the v1 baseline
- missing or incompatible baselines should be surfaced clearly rather than hidden

For the detailed migration rules, see [Baseline Migration](./baseline-migration.md).
