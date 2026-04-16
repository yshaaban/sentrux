# One-Tool Benchmark

This benchmark records the public mixed-language proof repo configured by `ONE_TOOL_ROOT`.

Checked-in artifact:

- [`one-tool-benchmark.json`](./one-tool-benchmark.json)

What it measures:

- cold onboarding path: `scan`, `project_shape`, `concepts`, `findings`, `agent_brief`
- warm cached path for repeated maintainer inspection
- warm persisted path after process restart
- warm patch-safety path: `session_start`, `check`, `gate`, `session_end`

Use this artifact to compare mixed-language public-repo latency against `parallel-code` and Sentrux dogfood runs. Unlike `parallel-code`, this repo currently ships benchmark evidence and onboarding evidence, not checked-in scoped goldens.
