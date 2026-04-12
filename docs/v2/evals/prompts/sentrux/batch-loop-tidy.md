Make the smallest safe smoke-lane maintenance change in the repo calibration loop.

Start with only these surfaces:

- `scripts/evals/run-repo-calibration-loop.mjs`
- `scripts/evals/run-codex-session-batch.mjs`
- `scripts/lib/signal-backlog.mjs`
- `docs/v2/evals/repos/sentrux.json`

Stay inside those files unless a directly imported helper is required to complete one local fix. Do not scan the broader repository for alternate targets. Do not run full builds, full test suites, or Cargo commands unless a touched file cannot be validated any other way.

Prefer one explicit readability, manifest-consistency, or loop-wiring fix that keeps the smoke lane deterministic.

If those surfaces already look clean, report a no-op instead of expanding scope.

Endurance note: broader calibration-loop cleanup belongs in the non-smoke lane, not this task.
