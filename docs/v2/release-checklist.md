# Sentrux V2 Release Checklist

Use this checklist before blessing a new v2 beta or release candidate.

## Proof Artifacts

- refresh any intentionally changed scoped goldens
- refresh any intentionally changed benchmark artifacts
- confirm the checked-in benchmark artifacts still match the current benchmark format version

## Validation Commands

Run:

1. `node scripts/validate_parallel_code_v2.mjs`
2. `node scripts/validate_h1_sdk_v2.mjs`
3. `node scripts/validate_benchmark_repos_v2.mjs`
4. `cargo test -p sentrux-core app::mcp_server::handlers::tests -- --nocapture`
5. `cargo test -p sentrux-core metrics::v2::obligations::tests -- --nocapture`
6. `cargo build -p sentrux`

## Benchmark Policy

- check the benchmark comparison output for each repo
- fail-tier regressions are:
  - more than `250ms`
  - and more than `20%`
- warn-tier regressions are:
  - more than `150ms`
  - and more than `10%`
- only fail-tier regressions should block a release by default
- warning-tier regressions should be reviewed and explained before blessing the release

## Migration And Baselines

- verify incompatible v2 session baselines are rejected with confidence errors
- verify copied or cross-project session baselines are rejected
- verify missing v2 baselines degrade cleanly to legacy structural context where supported
- verify malformed baseline files do not silently produce trusted output

## Product Surfaces

- MCP and CLI should both lead with findings, obligations, and touched-concept decisions
- legacy structural context should remain clearly labeled as supporting context
- confidence and suppression information should be visible in the main v2 surfaces

## Exit Bar

Do not bless the release unless:

- proof artifacts are current
- benchmark policy results are understood
- migration and handler validation passes
- the top actionable findings on `parallel-code` still look trustworthy
