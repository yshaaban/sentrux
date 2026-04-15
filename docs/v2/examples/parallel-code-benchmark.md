# Parallel-Code V2 Benchmark Notes

Benchmark artifact:

- [parallel-code-benchmark.json](./parallel-code-benchmark.json)

Validation runner:

- `node scripts/validate_parallel_code_v2.mjs`

## Method

The benchmark uses a real MCP session against the configured `PARALLEL_CODE_ROOT` checkout with
the example v2 rules file installed temporarily.

It measures:

1. fresh-process `scan`
2. the first semantic tool call in that process
3. the rest of the first semantic round
4. a second semantic round in the same process with the cached semantic state
5. a warm persisted round in a second process using the saved session state
6. a warm patch-safety round in the same process:
   - `session_start`
   - `gate`
   - `session_end`
7. optional comparison against the previous benchmark artifact using version and comparability guards

The checked-in artifact captures `5` repeated samples and writes the median aggregate into
the top-level `benchmark` object along with per-metric sample statistics and representative-sample
metadata.

That is the relevant distinction for agent workflows:

- cold scan cost
- first semantic-materialization cost
- warm cached tool latency
- warm patch-safety latency
- regression visibility across benchmark runs

## Benchmark Policy

Comparison policy defaults:

- fail when a metric regresses by more than `250ms` and `20%`
- warn when a metric regresses by more than `150ms` and `10%`
- only fail-tier regressions set a failing exit code when `FAIL_ON_REGRESSION=1`
- non-comparable runs stay informational unless `FAIL_ON_NONCOMPARABLE=1`

## Current Results

Current checked-in median aggregate (`5` samples, clean `working_tree` baseline):

- cold `scan`: `3805.9ms`
- first semantic call (`concepts`): `4395.1ms`
- cold `findings`: `7890.6ms`
- total cold process: `19048.5ms`
- warm cached semantic round: `4967.6ms`
- warm `session_start`: `683.1ms`
- warm `check`: `70.4ms`
- warm `gate`: `758.7ms`
- warm `session_end`: `1010.4ms`
- total warm patch-safety round: `4354.9ms`

## What We Learned

1. the fast-path `check` loop is in very good shape for MCP usage
   - the current warm `check` call is `70.4ms`
   - that keeps the primary agent feedback surface comfortably inside the patch-loop budget

2. the cold path is still dominated by `scan` and first semantic materialization
   - the first-round cost is still mostly structural scan plus the first semantic and findings passes
   - this is acceptable for onboarding and repo-wide inspection, but not the interactive loop

3. the warm patch-safety loop is now substantially tighter than the historical baseline
   - warm `gate` is `758.7ms`
   - warm `session_end` is `1010.4ms`
   - total warm patch-safety is `4354.9ms`

4. the remaining interactive bottleneck is no longer `check`
   - `check` is the fast path and should stay narrow
   - the dominant remaining costs are still `agent_brief`, `gate`, and `session_end`
   - those paths still carry broader patch-safety work than the minimal `check` surface

5. comparison quality now depends on matching the full benchmark input identity
   - the checked-in artifact is a clean `working_tree` benchmark against the configured repo root
   - changing commit, dirty-path set, rules file, binary, or analysis mode will produce an informational-only comparison instead of a regression verdict

6. the benchmark harness now needs stable inputs as well as stable timings
   - the artifact is format-versioned and identity-aware so regression comparison does not compare incompatible benchmark shapes
   - future regression runs should prefer the same repo state and analysis mode when the goal is a true apples-to-apples latency comparison

## Validation Flow

The benchmark is part of the broader validation loop:

1. refresh the checked-in goldens when the expected outputs intentionally change
2. run `node scripts/validate_parallel_code_v2.mjs` to compare a fresh temporary run against the checked-in goldens
3. use the benchmark-only command when you want latency data without golden comparison noise

## Implication

The next implementation step should focus on two things:

1. patch-safety performance beyond `check`
   - keep `check` narrow and fast
   - reduce remaining changed-file bookkeeping and report assembly cost for `gate`, `agent_brief`, and `session_end`

2. analyzer promotion criteria
   - define when warning-tier benchmark drift is acceptable
   - define when a new analyzer is trusted enough to gate
