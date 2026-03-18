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

That is the relevant distinction for agent workflows:

- cold scan cost
- first semantic-materialization cost
- warm cached tool latency

## Current Results

Primary captured run:

- cold `scan`: `9085.6ms`
- first semantic call (`concepts`): `2895.5ms`
- total first-round session through `state`: `12416.3ms`
- warm cached semantic round: `465.7ms`

Immediate repeat run for sanity checking:

- cold `scan`: `10520.2ms`
- first semantic call (`concepts`): `2597.5ms`
- warm cached semantic round: `478.6ms`

## What We Learned

1. the warm semantic path is already in good shape for MCP usage
   - the roadmap target in [analyzer-pipeline.md](../analyzer-pipeline.md) was under `3s`
   - the current cached semantic round is under `0.5s`

2. the cold path is still dominated by `scan`
   - roughly `9-10.5s` of the first-round cost is the structural scan plus startup overhead
   - the first semantic materialization is a smaller but still meaningful `2.6-2.9s`

3. the TypeScript bridge architecture is not the current interactive bottleneck
   - the bridge and cached semantic facts are fast enough for repeated MCP calls
   - the next ROI is analyzer correctness, not emergency performance work

4. fresh-process logs still include grammar-initialization noise
   - benchmark narratives should separate first-process startup chatter from steady-state analysis timings

## Implication

The next implementation step should focus on correctness:

- filter or classify test-only writes in authority findings
- distinguish projection concepts from owned-state concepts in authority analysis
- improve parity runtime-binding detection
- improve state-model matching for explicit controller-style modules
