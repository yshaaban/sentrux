# Parallel-Code V2 Benchmark Notes

Benchmark artifact:

- [parallel-code-benchmark.json](./parallel-code-benchmark.json)

Validation runner:

- `node scripts/validate_parallel_code_v2.mjs`

## Method

The benchmark uses a real MCP session against `<parallel-code-root>` with the example v2 rules file installed temporarily.

It measures:

1. fresh-process `scan`
2. the first semantic tool call in that process
3. the rest of the first semantic round
4. a second semantic round in the same process with the cached semantic state
5. a warm patch-safety round in the same process:
   - `session_start`
   - `gate`
   - `session_end`
6. optional comparison against the previous benchmark artifact using a versioned format guard

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

## Current Results

Primary captured run:

- cold `scan`: `14031.7ms`
- first semantic call (`concepts`): `5440.1ms`
- total first-round session through `state`: `24880.8ms`
- warm cached semantic round: `7512.9ms`
- warm `session_start`: `1003.1ms`
- warm `check`: `66.7ms`
- warm `gate`: `1032.1ms`
- warm `session_end`: `1489.3ms`
- total warm patch-safety round: `5383.9ms`

## What We Learned

1. the fast-path `check` loop is in very good shape for MCP usage
   - the current warm `check` call is `66.7ms`
   - that keeps the primary agent feedback surface comfortably inside the patch-loop budget

2. the cold path is still dominated by `scan` and first semantic materialization
   - the first-round cost is still mostly structural scan, startup overhead, and the first semantic bridge pass
   - this is acceptable for onboarding and repo-wide inspection, but not the interactive loop

3. the warm patch-safety path improved materially in `head_clone` mode
   - warm `gate` is down to `1032.1ms`
   - warm `session_end` is down to `1489.3ms`
   - total warm patch-safety is `5383.9ms`

4. the remaining interactive bottleneck is no longer `check`
   - `check` is the fast path and should stay narrow
   - the dominant remaining costs are still `agent_brief`, `gate`, and `session_end`
   - those paths still carry broader patch-safety work than the minimal `check` surface

5. comparison quality depends on using a controlled repo identity
   - `working_tree` runs on a dirty repo can look catastrophically worse than the checked-in head-clone artifact
   - use `ANALYSIS_MODE=head_clone` for apples-to-apples regression checks

6. the benchmark harness now needs stable inputs as well as stable timings
   - the artifact is format-versioned so regression comparison does not compare incompatible benchmark shapes
   - future regression runs should prefer a controlled repo state or temp copy when possible

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
