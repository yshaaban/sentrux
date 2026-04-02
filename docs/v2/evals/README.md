# External Eval Harness

This directory defines the external evaluation harness for Sentrux v2.

Goals:

- keep the harness model-agnostic
- use Claude Code CLI as the first provider
- evaluate the `agent_brief` and `dead_private` surfaces across benchmark repos
- keep the scenario format repo-agnostic so new benchmark repos can be added without code changes

## Layout

- `index.json` - manifest for the checked-in scenarios
- `scenario.schema.json` - task/scenario schema
- `result.schema.json` - result schema emitted by the runner
- `scenarios/parallel-code.json` - initial scenarios for `<parallel-code-root>`
- `scenarios/private-benchmark-repo.json` - initial scenarios for `<private-benchmark-root>`
- `scenarios/private-frontend.json` - initial scenarios for `<private-frontend-root>`

## Runner

The runner lives at `scripts/evals/run.mjs`.

Default usage:

```bash
node scripts/evals/run.mjs
```

Useful flags:

- `--scenario <path>` - run one scenario file, repeatable
- `--output-dir <path>` - write results somewhere other than `docs/v2/evals/runs`
- `--provider claude-code` - explicit provider selection
- `--model <name>` - pass a Claude model alias or full model name
- `--concurrency <n>` - number of tasks to run in parallel
- `--dry-run` - load and validate scenarios without calling any provider

Repo roots can be overridden with the same environment variables used by the benchmark docs:

- `PARALLEL_CODE_ROOT`
- `PRIVATE_BENCHMARK_ROOT`
- `PRIVATE_FRONTEND_ROOT`

If an override is not provided, each scenario falls back to the checked-in default root in its own file.

## Task Types

The current schema supports two task kinds:

- `agent_brief` - asks for a structured onboarding, patch, or pre-merge brief
- `dead_private` - asks for likely dead private code clusters with evidence

Each task carries its own prompt and a data-driven check list so the runner can score structural validity without baking repo-specific rules into code.

## Dead-Private Review Loop

The generic runner asks the external provider to inspect repos directly. The
focused dead-private experiment loop instead feeds Sentrux's own
`dead_private_code_cluster` candidates back into the evaluator.

Use:

```bash
node scripts/evals/review_dead_private.mjs \
  --repo-root /path/to/repo \
  --repo-name my-repo \
  --dry-run
```

Or run it live once Claude Code CLI auth is configured:

```bash
node scripts/evals/review_dead_private.mjs \
  --repo-root /path/to/repo \
  --repo-name my-repo
```

The output includes:

- the extracted Sentrux dead-private candidates
- local file snippets around the sampled dead helpers
- the provider verdicts (`accept`, `acceptable_watchpoint_only`, `reject_false_positive`, `reject_too_ambiguous`)

This is the intended optimization loop for promoting or constraining the
experimental dead-private detector.

## Result Shape

Each task run emits a JSON result with:

- scenario metadata
- task metadata
- provider configuration
- execution timing and exit status
- captured provider output
- parsed response payload
- check results and summary status

The schema is intentionally stable so future providers can slot into the same runner.

## Signal-Quality Extensions

The external provider runner is only one part of the quality loop.

Supporting scripts now cover:

- `node scripts/evals/build-check-review-packet.mjs`
  Build a reusable review packet from `check`, `findings`, or `session_end` for manual false-positive review.
- `node scripts/evals/run-defect-remediation.mjs`
  Seed a defect, let a provider attempt a fix in a disposable clone, rerun `check`, and record whether the signal actually helped the agent repair the issue.
- `node scripts/evals/build-signal-scorecard.mjs`
  Merge defect-injection results, reviewed verdicts, remediation outcomes, and benchmark latency into a per-signal scorecard.
