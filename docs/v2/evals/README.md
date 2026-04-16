# External Eval Harness

This directory defines the external evaluation harness for Sentrux v2.

Goals:

- keep the harness model-agnostic
- use Claude Code CLI as the first provider
- evaluate the `agent_brief` and `dead_private` surfaces across benchmark repos
- keep the scenario format repo-agnostic so new benchmark repos can be added without code changes

## Layout

- `index.json` - manifest for the checked-in scenarios
- `signal-cohorts.json` - active signal cohort definitions for calibration
- `repo-calibration.schema.json` - schema for per-repo calibration manifests
- `review-verdicts.schema.json` - schema for human review verdict input
- `review-verdicts.template.json` - starter verdict file for new repos
- `codex-session-batch.schema.json` - schema for batch Codex task capture manifests
- `diff-replay-batch.schema.json` - schema for batch replay manifests
- `scenario.schema.json` - task/scenario schema
- `result.schema.json` - result schema emitted by the runner
- `repos/parallel-code.json` - checked-in calibration manifest for the repo configured by `PARALLEL_CODE_ROOT`
- `repos/one-tool.json` - checked-in calibration manifest for the repo configured by `ONE_TOOL_ROOT`
- `repos/sentrux.json` - checked-in calibration manifest for the current Sentrux checkout
- `batches/parallel-code-codex-session-batch.json` - live Codex batch manifest for `parallel-code`
- `batches/parallel-code-diff-replay-batch.json` - replay batch manifest for `parallel-code`
- `batches/one-tool-codex-session-batch.json` - live Codex batch manifest for `one-tool`
- `batches/one-tool-diff-replay-batch.json` - replay batch manifest for `one-tool`
- `batches/sentrux-codex-session-batch.json` - live Codex batch manifest for `sentrux`
- `batches/sentrux-diff-replay-batch.json` - replay batch manifest for `sentrux`
- `prompts/parallel-code/*.md` - checked-in prompt files for the `parallel-code` calibration tasks
- `prompts/one-tool/*.md` - checked-in prompt files for the `one-tool` calibration tasks
- `prompts/sentrux/*.md` - checked-in prompt files for the `sentrux` calibration tasks
- `scenarios/parallel-code.json` - initial scenarios for the repo configured by `PARALLEL_CODE_ROOT`
- `scenarios/one-tool.json` - initial scenarios for the repo configured by `ONE_TOOL_ROOT`

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

Repo roots can be overridden with the same environment variables used by the public benchmark docs:

- `PARALLEL_CODE_ROOT`
- `ONE_TOOL_ROOT`

If an override is not provided, each scenario falls back to its checked-in default root. Public documentation should prefer the environment variables above instead of assuming a workstation-specific path layout.

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

- `node scripts/evals/run-repo-calibration-loop.mjs --manifest docs/v2/evals/repos/<repo>.json`
  Run the full per-repo calibration loop from one checked-in manifest. The loop now snapshots the previous repo-local scorecard/backlog/review packet when available, emits delta summaries, warns when seeded-defect, remediation, or benchmark inputs are missing, and can bootstrap provisional review verdicts from the latest review packet when a repo only has the generic template.
- `node scripts/evals/build-check-review-packet.mjs`
  Build a reusable review packet from `check`, `findings`, or `session_end` for manual false-positive review. Artifact mode can read a single bundle, a live Codex batch, a replay batch, or a combined live+replay set without rescanning repo HEAD. For `check`, the builder samples the first non-empty ranked payload across recorded snapshots instead of assuming `initial_check` is representative, and it emits a companion verdict-template JSON matching the review-verdict schema. The packet summary now also records repair-packet completeness for the full sample set plus the top 3 and top 10 ranked samples so fix-guidance gaps stay visible even before remediation evals exist.
- `node scripts/evals/run-external-repo-validation.mjs --repo-root /path/to/repo`
  Run a full external-repo validation pass, capture the raw MCP payloads plus reusable review packets, and emit both a Sentrux-facing `REPORT.md` and a repo-engineer-facing `ENGINEERING_REPORT.md` in one output directory.
- `node scripts/evals/build-session-telemetry-summary.mjs --repo-root /path/to/repo`
  Summarize the repo-local `.sentrux/agent-session-events.jsonl` stream into per-session and per-signal resolution metrics.
- `node scripts/evals/run-codex-session.mjs --source-root /path/to/repo --task-file task.txt`
  Run a real Codex CLI task inside a disposable clone, capture intermediate `check` snapshots, and bundle the resulting session telemetry and outcome summary.
- `node scripts/evals/run-codex-session-batch.mjs --manifest batch.json`
  Run a cohort-oriented batch of real Codex tasks, keep failed or timed-out task metadata visible in the batch index, and merge any captured telemetry from those runs into the shared session summary.
- `node scripts/evals/run-diff-replay.mjs --source-root /path/to/repo --commit <sha>`
  Reconstruct a before/after session from a real git commit by checking out the parent revision in a disposable clone, applying the patch, and recording the resulting `check` / `session_end` artifacts.
- `node scripts/evals/run-diff-replay-batch.mjs --manifest replay-batch.json`
  Replay a selected set of commits, emit one batch index, and merge the resulting telemetry into one summary.
- `node scripts/evals/run-defect-remediation.mjs`
  Seed a defect, let a provider attempt a fix in a disposable clone, rerun `check`, and record whether the signal actually helped the agent repair the issue. Both `claude-code` and `codex-cli` are supported.
- `node scripts/evals/build-signal-scorecard.mjs`
  Merge defect-injection results, reviewed verdicts, remediation outcomes, session telemetry, and benchmark latency into a per-signal scorecard. The scorecard now keeps explicit review-noise, top-action-clear, follow-up regression, evidence-coverage, top-1/top-3/top-10 actionable precision, and ranking-preference-satisfaction metrics so weak signals can be distinguished from under-instrumented ones and from findings that are true but misranked. When the benchmark artifact includes repeated samples, the scorecard consumes the authoritative top-level `benchmark` timings, so median-aggregated latency evidence flows through without extra wiring.
- `node scripts/evals/build-signal-backlog.mjs`
  Combine the active cohort, scorecard, and live/replay batch outputs into a weak-signal and false-negative backlog. Candidate ordering now exposes an explicit priority score that weights live misses above replay misses and keeps regression follow-through pressure visible. Configured next candidates stay queued, but the recommended next signal now requires positive evidence instead of defaulting to a zero-score placeholder.
- `node scripts/evals/run-signal-calibration.mjs`
  Build the session telemetry summary and the refreshed scorecard together for the current repo or benchmark artifact set. When explicit live/replay batch paths are omitted, the script now reuses the latest repo-calibration-loop batch artifacts for the same repo so stable self-eval scorecards do not silently lose session-trial evidence; when a cohort is available it also refreshes the backlog in the same pass.

## Real Session Instrumentation

MCP `session_start`, `check`, and `session_end` now append best-effort JSONL events to:

```text
<repo>/.sentrux/agent-session-events.jsonl
```

Those events are repo-local and intended for product calibration, not network telemetry.

The current event stream records:

- explicit vs implicit session mode
- `check` gate result, changed-file count, and top action kinds
- whether the run was partial or reused cached scan state
- `session_end` decisions, introduced finding kinds, and missing-obligation counts

Use that log as the input to the session-telemetry summary script before rebuilding the scorecard.

## Real-Work And Replay Lanes

The fastest signal-calibration loop now uses three complementary lanes:

- real Codex task capture via `run-codex-session.mjs`
- diff replay over historical commits via `run-diff-replay.mjs`
- seeded defect and remediation evals via `run-defect-remediation.mjs`

All three lanes should feed the same scorecard inputs instead of creating separate review systems. The default local artifact root is:

```text
<repo>/.sentrux/evals/
```

Use checked-in `docs/v2/examples/` artifacts only for intentionally promoted reference runs.

The checked-in live Codex batch manifests default to `analysis_mode: "working_tree"` so the real-work lane includes local uncommitted changes. Use `head_clone` only when you intentionally want a committed-HEAD calibration run.

The checked-in replay batch manifests use explicit commit lists rather than broad `HEAD~N..HEAD` ranges. That keeps the default replay lane focused on code-rich commits and avoids letting docs-only churn dominate the backlog with low-value `large_file` noise.

## Cohort-Driven Calibration

The recommended operating model is:

1. define the active signal cohort in `docs/v2/evals/signal-cohorts.json`
2. load a per-repo calibration manifest from `docs/v2/evals/repos/`
3. run a live Codex batch with `run-codex-session-batch.mjs` using the checked-in live batch manifest
4. run a replay batch with `run-diff-replay-batch.mjs` using the checked-in replay batch manifest
5. generate a review packet from the captured artifact bundle when you want human verdicts
6. record verdicts with `review-verdicts.template.json` or the repo-specific verdict file
   When a repo only has the generic template, the loop can bootstrap a provisional repo-local verdict file from the latest review packet so scorecard precision coverage is not empty by default. Keep verdict order rank-preserving: top-1/top-3/top-10 actionable precision is computed from verdict order, not from scope-name sorting.
7. build a refreshed scorecard
8. build a backlog with `build-signal-backlog.mjs`

That keeps the current trusted/watchpoint candidates, the real-session evidence, and the “what should we build next?” report on one shared set of artifacts.

For the duplication family, treat the checked-in signals as distinct surfaces:

- `session_introduced_clone` for fresh duplicates introduced in the current task
- `clone_propagation_drift` for followthrough misses where one side of an existing clone family changed and the sibling did not
- `touched_clone_family` as low-priority clone context, not a primary fast-path blocker

Current operating stance after the 2026-04-12 calibration refresh:

- use `parallel-code` as the primary duplication and structural replay repo because it still surfaces the clearest clone, boundary, and exhaustiveness pressure
- use `one-tool` as the mixed TypeScript/Python public proof repo for onboarding ranking, command-surface pressure, and small-boundary-cycle evidence
- use Sentrux as the dogfood calibration repo with checked-in seeded-defect, remediation, benchmark, and curated-review artifacts; its replay lane is still most useful for ranking and watchpoint pressure rather than clone promotion
- keep `touched_clone_family` as contextual clone pressure unless a future pass shows it consistently outranking more important actions
- treat `zero_config_boundary_violation` as the current next out-of-cohort candidate because the latest Sentrux replay backlog gave it the highest positive priority score
- keep `multi_writer_concept`, `forbidden_writer`, and `writer_outside_allowlist` queued as configured candidates until they pick up real live or replay misses
- keep the live smoke interpretation strict: the Sentrux smoke lane now passes structurally, while the `parallel-code` exhaustiveness smoke still fails on a real signal rather than on provider timeout

Promotion guidance for the current loop:

- promoted signals need curated reviewed precision plus actionable ranking evidence; reviewed precision by itself is not enough
- if a signal is repeatedly real but lands as low-value in top-1 or top-3 slots, downgrade the lead-surface presentation before promoting the detector
- treat repair-packet completeness as packet-local evidence for now; until the review-verdict schema carries structured repair fields, do not treat it as scorecard-grade promotion input

Current replay-expansion stance after the 2026-04-12 structural pass:

- use `parallel-code` as the primary structural watchpoint replay repo because the checked-in additions now include explicit `large_file` and `missing_test_coverage` stress commits and the live smoke lane still surfaces a real exhaustiveness failure
- use Sentrux for dogfood artifact coverage, backlog ranking, and clone-followthrough or contract-obligation replay evidence, not as the main structural-watchpoint corpus

For the current checked-in repo manifests, start with:

- `docs/v2/evals/repos/parallel-code.json`
- `docs/v2/evals/repos/one-tool.json`
- `docs/v2/evals/repos/sentrux.json`

End-to-end examples:

```bash
node scripts/evals/run-repo-calibration-loop.mjs \
  --manifest docs/v2/evals/repos/parallel-code.json

node scripts/evals/run-repo-calibration-loop.mjs \
  --manifest docs/v2/evals/repos/sentrux.json
```
