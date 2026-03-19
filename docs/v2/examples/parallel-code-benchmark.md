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

## Current Results

Primary captured run:

- cold `scan`: `9284.6ms`
- first semantic call (`concepts`): `2800.3ms`
- total first-round session through `state`: `15739.4ms`
- warm cached semantic round: `369.1ms`
- warm `session_start`: `124.6ms`
- warm `gate`: `3251.0ms`
- warm `session_end`: `3470.8ms`
- total warm patch-safety round: `6846.8ms`

## What We Learned

1. the warm semantic path is in very good shape for MCP usage
   - the roadmap target in [analyzer-pipeline.md](../analyzer-pipeline.md) was under `3s`
   - the current cached semantic round is well under `0.5s`

2. the cold path is still dominated by `scan`
   - the first-round cost is still mostly structural scan plus startup overhead
   - the first semantic materialization is meaningful, but not the main bottleneck

3. the TypeScript bridge architecture is not the current interactive bottleneck
   - the bridge and cached semantic facts are fast enough for repeated MCP calls
   - the next ROI is no longer bridge startup or semantic caching

4. the shared patch-safety analysis reuse materially improved the warm no-change path
   - warm `gate` improved from the previous artifact (`3377.3ms` -> `3251.0ms`)
   - warm `session_end` regressed slightly but stayed within the benchmark threshold (`3362.2ms` -> `3470.8ms`)
   - total warm patch-safety remained slightly better overall (`6863.4ms` -> `6846.8ms`)
   - the dominant remaining cost is now scan-bound structural work and changed-file bookkeeping, not duplicate semantic analysis

5. the benchmark harness is now stable enough for guarded comparison, but the cold path is still noisier than the warm path
   - this run did not trigger any benchmark regressions
   - warm patch-safety comparisons remain the most useful signal for this part of the roadmap

6. the benchmark harness now needs stable inputs as well as stable timings
   - the artifact is now format-versioned so regression comparison does not compare incompatible benchmark shapes
   - future regression runs should prefer a controlled repo state or temp copy when possible

## Validation Flow

The benchmark is part of the broader validation loop:

1. refresh the checked-in goldens when the expected outputs intentionally change
2. run `node scripts/validate_parallel_code_v2.mjs` to compare a fresh temporary run against the checked-in goldens
3. use the benchmark-only command when you want latency data without golden comparison noise

## Implication

The next implementation step should focus on two things:

1. release-grade validation
   - checked-in `session_end` and gate goldens on the case-study repo
   - false-positive review workflow

2. patch-safety performance
   - reduce remaining scan-bound and changed-file bookkeeping work for `gate` and `session_end`
   - preserve the already-good warm semantic path while improving the still-expensive patch-safety tools
