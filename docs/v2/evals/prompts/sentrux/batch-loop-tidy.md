Make the smallest safe smoke-lane maintenance change in the repo calibration loop.

Start with only these surfaces:

- `docs/v2/evals/batches/sentrux-codex-session-batch.json`
- `docs/v2/evals/batches/sentrux-diff-replay-batch.json`
- `docs/v2/evals/signal-cohorts.json`
- `scripts/lib/signal-cohorts.mjs`
- `docs/v2/evals/repos/sentrux.json`

Stay inside those files unless a directly related manifest helper is required to complete one local fix. Do not scan the broader repository for alternate targets. Do not run full builds, full test suites, or Cargo commands unless a touched file cannot be validated any other way.

Prefer one explicit readability, manifest-consistency, or loop-wiring fix that keeps the smoke lane deterministic.

If those surfaces already look clean, report a no-op instead of expanding scope.

Endurance note: broader calibration-loop cleanup belongs in the non-smoke lane, not this task.
