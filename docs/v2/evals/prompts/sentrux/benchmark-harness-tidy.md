Make the smallest safe smoke-lane maintenance change in the benchmark harness flow.

Start with only these surfaces:

- `scripts/lib/benchmark-harness.mjs`
- `scripts/lib/benchmark-summaries.mjs`
- `scripts/tests/benchmark-harness.test.mjs`

Stay inside those files unless a directly imported helper is required to complete one local fix. Do not scan the broader repository for alternate targets. Do not run full builds, full test suites, or Cargo commands unless a touched file cannot be validated any other way.

Prefer one explicit readability, result-shape, or helper-boundary fix that keeps benchmark-harness edits surgical.

If those surfaces already look clean, report a no-op instead of expanding scope.

Endurance note: broader benchmark harness refactors belong in the non-smoke lane, not this task.
