# Sentrux V2 Validation Loop

This document defines the repeatable proof loop for the v2 wedge across the checked-in benchmark repos.

The goal is not only to generate artifacts. The goal is to make regressions easy to detect, review, and refresh without guessing.

The proof-and-improvement workflow is tracked separately in [Parallel-Code Proof Board](./parallel-code-proof-board.md).

## What The Loop Covers

The loop validates three separate things:

1. scoped real-repo goldens
2. benchmark regression behavior
3. the baseline and migration story around v1 and v2 coexistence
4. proof-and-improvement runs on disposable clones
5. finding-class usefulness and trust-tier calibration
6. mode-specific `agent_brief` coverage for `repo_onboarding`, `patch`, and `pre_merge`
7. fast-path `check` coverage and ranked action quality
8. external evaluator calibration for agent guidance and experimental detectors
9. remediation success on seeded defects

## Commands

### Refresh Goldens

Use this when the checked-in real-repo outputs need to be regenerated.

```bash
./scripts/refresh_parallel_code_goldens.sh
```

This command:

- clones `parallel-code` into a temporary workspace
- installs the example v2 rules file
- generates the checked-in `parallel-code-golden` JSON outputs
- writes deterministic fail-path outputs from a temporary mutation

The live `parallel-code` worktree may be dirty, so every proof refresh must go through a disposable clone.

The other benchmark repos follow the same pattern:

```bash
./scripts/refresh_h1_sdk_goldens.sh
./scripts/refresh_admin_frontend_goldens.sh
```

For stable engineer-review artifacts against committed `HEAD`, use:

```bash
ANALYSIS_MODE=head_clone \
OUTPUT_DIR=docs/v2/examples/parallel-code-head-golden \
./scripts/refresh_parallel_code_goldens.sh
```

### Validate Goldens And Benchmark

Use this for the normal proof loop.

```bash
node scripts/validate_parallel_code_v2.mjs
```

This command:

- regenerates the real-repo goldens into a temporary directory
- compares them against the checked-in goldens
- runs the benchmark harness against the checked-in benchmark artifact
- fails if the benchmark comparison reports a regression

The proof board explains how the outputs from this command should be turned into real refactor targets and before/after proof records.

For the other benchmark repos, use:

```bash
node scripts/validate_h1_sdk_v2.mjs
node scripts/validate_admin_frontend_v2.mjs
```

To validate all benchmark repos together, use:

```bash
node scripts/validate_benchmark_repos_v2.mjs
```

### Run Proof Targets

Use this when the goal is to regenerate the tracked before/after proof artifacts for the current `parallel-code` proof targets.

```bash
node scripts/run_parallel_code_proof_targets.mjs
```

This command:

- creates disposable `parallel-code` clones
- runs the seeded ownership regression proof
- runs the propagation cleanup proof
- runs the clone-family cleanup proof
- writes checked-in proof artifacts under `docs/v2/examples/parallel-code-proof-runs`

### Benchmark Only

Use this when you only want performance data.

```bash
node scripts/benchmark_parallel_code_v2.mjs
```

### Golden Only

Use this when you want to inspect output stability without rerunning the benchmark.

```bash
node scripts/validate_parallel_code_v2.mjs --goldens-only
```

## What The Validation Script Checks

The validation runner compares the checked-in `parallel-code-golden` files against a fresh temporary run:

- `scan.json`
- `concepts.json`
- `findings-top12.json`
- `explain-task_git_status.json`
- `explain-task_presentation_status.json`
- `explain-server_state_bootstrap.json`
- `obligations-task_presentation_status.json`
- `parity-server_state_bootstrap.json`
- `state.json`
- `session-start.json`
- `gate-pass.json`
- `gate-fail.json`
- `session-end-pass.json`
- `session-end-fail.json`
- `agent_brief` outputs for the supported modes when they are part of the checked-in golden set
- `metadata.json`

The `metadata.json` check ignores the timestamp field and verifies the stable payload instead.

## What The Benchmark Harness Checks

The benchmark harness records and compares:

- cold scan latency
- first semantic materialization latency
- warm cached semantic latency
- warm persisted semantic latency from a fresh process
- warm patch-safety latency
- warm `check` latency
- regression thresholds across benchmark runs

The checked-in benchmark artifact is versioned so incompatible benchmark shapes do not get compared accidentally.

## How To Use Failures

If goldens fail:

1. decide whether the output change is intentional
2. if intentional, refresh the checked-in goldens
3. if not intentional, fix the analyzer or the harness

If the benchmark fails:

1. confirm whether the change is a real regression or a noisy run
2. rerun with the same artifact before changing the baseline
3. only update the baseline after the change is understood

## Proof-And-Improvement Loop

Use this when the goal is to improve `parallel-code`, not just validate analyzer stability.

1. freeze the current baseline outputs
2. review the relevant `agent_brief` first, then the top findings, concept summaries, debt signals, and watchpoints
3. select one ownership/boundary target, one propagation/obligation target, and one duplication/hotspot target
4. make one refactor at a time in a disposable clone
5. rerun the proof loop after each refactor
6. record the before/after delta in the proof board and the case study

The proof board in [Parallel-Code Proof Board](./parallel-code-proof-board.md) is the tracking surface for that loop.
The reviewed baseline and resolved overstatements are tracked in [Parallel-Code Proof Review](./parallel-code-proof-review.md).
Maintainer feedback verdicts are tracked in [parallel-code-review-verdicts.json](./examples/parallel-code-review-verdicts.json) and summarized by [summarize_parallel_code_review_feedback.mjs](../../scripts/summarize_parallel_code_review_feedback.mjs).

## Finding-Class Evaluation Loop

Use maintainer review to calibrate finding classes, not only individual reports.

For each reviewed finding class, capture:

- `category`
  - `useful`
  - `useful_watchpoint`
  - `real_but_overstated`
  - `low_value`
  - `incorrect`
- `expected_trust_tier`
  - `trusted`
  - `watchpoint`
  - `experimental`
- `expected_presentation_class`
  - `structural_debt`
  - `guarded_facade`
  - `watchpoint`
  - `hardening_note`
  - `tooling_debt`
  - `experimental`
- `expected_leverage_class`
  - `architecture_signal`
  - `local_refactor_target`
  - `boundary_discipline`
  - `regrowth_watchpoint`
  - `secondary_cleanup`
  - `hardening_note`
  - `tooling_debt`
  - `experimental`
- `expected_summary_presence`
  - `headline`
  - `section_present`
  - `side_channel`
- optional ranking preferences such as:
  - `preferred_over`
    - use this when the class is right but the within-bucket order still matters

This loop should drive product changes such as:

- promoting a detector to trusted
- keeping a detector as a watchpoint
- quarantining a detector as experimental
- adding fixability metadata when a finding is real but not design-actionable yet
- adjusting leverage classification when the raw finding is right but the engineering meaning is wrong
- improving within-bucket ranking when two valid findings should not be treated as peers

## Signal-Quality Loop

Use the seeded defect harness, review packets, and remediation evals together.

1. run `node scripts/defect-injection/run-injection.mjs`
2. generate or refresh a review packet with `node scripts/evals/build-check-review-packet.mjs`
3. classify reviewed findings using the false-positive workflow
4. run remediation evals with `node scripts/evals/run-defect-remediation.mjs`
5. summarize real MCP sessions with `node scripts/evals/build-session-telemetry-summary.mjs --repo-root /path/to/repo`
6. aggregate the result into a per-signal scorecard with `node scripts/evals/build-signal-scorecard.mjs`

Signals should only be promoted when seeded recall, reviewed precision, and remediation success all support the promotion.

To refresh the session summary and scorecard together, use:

```bash
node scripts/evals/run-signal-calibration.mjs \
  --repo-root /path/to/repo \
  --repo-label my-repo \
  --defect-report /path/to/defect-report.json \
  --review-verdicts /path/to/review-verdicts.json \
  --remediation-report /path/to/remediation-report.json \
  --benchmark /path/to/benchmark.json
```

The repo-local MCP event stream lives at `.sentrux/agent-session-events.jsonl`. The calibration loop treats those events as the real-session evidence layer on top of seeded defects and provider remediation runs.

## Repo Calibration Loop

Use the repo calibration manifests to keep the live-session, replay, review, scorecard, and backlog artifacts aligned for a single repo.

The checked-in manifests live under:

- `docs/v2/evals/repos/parallel-code.json`
- `docs/v2/evals/repos/sentrux.json`

Each repo manifest points at:

- one live Codex batch manifest
- one replay batch manifest
- one review-verdict input file
- one review-verdict output file
- one scorecard output path
- one backlog output path

The expected loop is:

1. pick the repo manifest
2. run `node scripts/evals/run-repo-calibration-loop.mjs --manifest docs/v2/evals/repos/<repo>.json`
3. inspect the generated review packet
4. apply the verdict template or the repo-specific review verdict file
5. rerun the same repo-level loop if you want the refreshed scorecard and backlog to incorporate the new verdicts
6. compare the new outputs against the previous calibration snapshot

The repo-level calibration loop is intentionally manifest-driven so each repo keeps its own prompts, batch inputs, and calibration outputs in one place.

## Continuous Experiment Lanes

Weekly rituals are not the point. The point is to collect evidence continuously while real work is happening.

Use three lanes in parallel:

1. real Codex task capture
   - `node scripts/evals/run-codex-session.mjs --source-root /path/to/repo --task-file task.txt`
   - runs Codex CLI inside a disposable clone
   - captures intermediate `check` snapshots whenever the working tree changes
   - writes a bundle under `<repo>/.sentrux/evals/`
2. diff replay
   - `node scripts/evals/run-diff-replay.mjs --source-root /path/to/repo --commit <sha>`
   - reconstructs a session from a real historical commit
   - produces the same telemetry and outcome bundle shape as the live Codex lane
3. seeded defect + remediation
   - `node scripts/evals/run-defect-remediation.mjs --provider codex-cli`
   - keeps deterministic coverage for the promoted signal cohort

The real Codex lane tells us whether ranked actions help during actual work. The replay lane scales that check across realistic historical diffs. The seeded lane keeps detector and fix-hint regressions obvious while the other two accumulate.

To make that repeatable instead of ad hoc:

1. define the active cohort in `docs/v2/evals/signal-cohorts.json`
2. capture a live-task batch with `node scripts/evals/run-codex-session-batch.mjs --manifest batch.json`
3. capture a replay batch with `node scripts/evals/run-diff-replay-batch.mjs --manifest replay-batch.json`
4. merge the results into a refreshed scorecard
5. build the next-work backlog with `node scripts/evals/build-signal-backlog.mjs`

The backlog output is not a substitute for human judgment, but it makes the weak-signal set, the high-friction live sessions, and the repeated out-of-cohort misses explicit enough to guide the next tranche.

Current promotion note:

- `dead_private_code_cluster` remains intentionally `experimental` until broader TS/TSX reference precision is validated beyond the current same-file callback/JSX suppression fix, exported-symbol visibility fix, and external review-loop evidence

## Relationship To Migration

The validation loop is intentionally separate from baseline migration, but it depends on the same rollout assumptions:

- v1 and v2 baselines remain separate
- v2 validation should never synthesize v2 behavior from the v1 baseline
- missing or incompatible baselines should be surfaced clearly rather than hidden

For the detailed migration rules, see [Baseline Migration](./baseline-migration.md).

## Second Repo Cross-Check

Ranking heuristics should be validated on a second repo shape before they are treated as stable.

Use:

```bash
node scripts/validate_h1_sdk_v2.mjs --goldens-only
```

This does not provide an engineer-report corpus like `parallel-code`, but it does verify that generic ranking-oriented analyzer changes do not accidentally destabilize the second checked-in benchmark repo shape.

## Third Repo Cross-Check

Archetype and onboarding changes should also be validated on a modular Next.js frontend shape.

Use:

```bash
node scripts/validate_admin_frontend_v2.mjs --goldens-only
```

This verifies:

- `project_shape` stability
- starter-rule generation
- framework-aware role tagging on route, provider, service, and state surfaces
- generic `module_contract` enforcement through the synthetic benchmark fail path

## External Evaluator Loop

Use the checked-in eval harness when the question is whether the structured feedback is actually useful to another coding agent, not just internally consistent.

Dry-run the harness with:

```bash
node scripts/evals/run.mjs --dry-run
```

Dry-run the focused dead-private loop with:

```bash
node scripts/evals/review_dead_private.mjs \
  --repo-root /path/to/repo \
  --repo-name my-repo \
  --dry-run
```

These runs validate:

- scenario and result schema stability
- provider wrapper wiring
- prompt payload shape for `agent_brief` and `dead_private` tasks
- dead-private candidate quality before a live external review is attempted

Live provider runs should only be treated as pass/fail evidence once the local Claude Code CLI is authenticated and returning stable structured JSON in headless mode.
