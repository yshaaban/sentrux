# Parallel-Code V2 Benchmark Notes

Benchmark artifact:

- [parallel-code-benchmark.json](./parallel-code-benchmark.json)

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

- cold `scan`: `6785.1ms`
- first semantic call (`concepts`): `3234.0ms`
- total first-round session through `state`: `14126.3ms`
- warm cached semantic round: `422.5ms`
- warm `session_start`: `131.4ms`
- warm `gate`: `7695.9ms`
- warm `session_end`: `7239.6ms`
- total warm patch-safety round: `15067.4ms`

## What We Learned

1. the warm semantic path is already in good shape for MCP usage
   - the roadmap target in [analyzer-pipeline.md](../analyzer-pipeline.md) was under `3s`
   - the current cached semantic round is still under `0.5s`

2. the cold path is still dominated by `scan`
   - the first-round cost is still mostly structural scan plus startup overhead
   - the first semantic materialization is meaningful, but not the main bottleneck

3. the TypeScript bridge architecture is not the current interactive bottleneck
   - the bridge and cached semantic facts are fast enough for repeated MCP calls
   - the next ROI is no longer bridge startup or semantic caching

4. the patch-safety tools are still scan-bound
   - warm `gate` and `session_end` each take roughly `7-8s` on the real repo even with cached semantic state
   - the current bottleneck is repeated scan/evolution work inside those tools, not semantic reuse

5. the benchmark harness now needs stable inputs as well as stable timings
   - the artifact is now format-versioned so regression comparison does not compare incompatible benchmark shapes
   - future regression runs should prefer a controlled repo state or temp copy when possible

## Implication

The next implementation step should focus on two things:

1. release-grade validation
   - checked-in `session_end` and gate goldens on the case-study repo
   - false-positive review workflow

2. patch-safety performance
   - reduce redundant scan/evolution work for `gate` and `session_end`
   - preserve the already-good warm semantic path while improving the scan-bound tools
