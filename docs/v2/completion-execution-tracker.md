# Sentrux V2 Completion Execution Tracker

Last audited: 2026-04-24

This slice tracks execution against the current [master plan](./master-plan.md) using repository-backed state only. A phase is marked `Completed` only when checked-in implementation and checked-in evidence together satisfy the phase exit bar. On this pass, no phase is marked complete.

## Status Legend

- `Planned`: supporting inputs may exist, but no shipping workflow satisfies the phase yet.
- `In progress`: implementation and/or evidence exists, but the phase exit bar is not yet met.
- `Completed`: the repo already contains the implementation and evidence needed to satisfy the phase exit criteria.

## Compact Status Table

| Phase | Status | Repository-backed position | Main blocking gap |
| --- | --- | --- | --- |
| Phase 0: Reset the product contract | In progress | doctrine, roadmap, signal cohorts, scorecard policy, session verdict schema, and lane-aware findings metadata are all checked in | downstream surfaces still need one consistently consumed lane and top-action contract without fallback gaps |
| Phase 1: Harden the intervention-grade signal set | In progress | default cohort metadata, lane-aware ranking, trust tiers, clone/raw-read/propagation surfaces, and structural suppression rules are implemented | the default lane is not yet proven to stay dominated by a small causal signal set across repo shapes |
| Phase 2: Expand the semantic obligation graph | In progress | TS semantic substrate, domain obligations, contract obligations, and obligation repair guidance are implemented and tested | richer contract families, finer changed-symbol precision, and broader sibling-surface coverage remain incomplete |
| Phase 3: Add bounded LLM adjudication | In progress | structured evidence bundles, MiniMax-oriented schema/prompting, audit logging, and advisory-only adjudication summaries are implemented | no production reranking or suppression path is justified by paired treatment evidence yet |
| Phase 4: Add checker and pattern synthesis | Planned | verdict and review artifacts exist as inputs for future synthesis work | no synthesis workflow, synthesized checker output, or held-out validation loop is shipping |
| Phase 5: Build the treatment-vs-baseline evidence program | In progress | calibration runners, batch manifests, scorecards, session corpus, evidence review, and repo-local eval artifacts are all checked in | paired baseline/treatment wins are not yet decisive promotion proof; the latest Sentrux repo-local run still reports `top_action_help_rate = 0` and `promotion_candidate_count = 0` |
| Phase 6: Product surface compression | In progress | `check`, lane-aware findings, repair-packet completeness, active phase-6 experiment specs, and registry-backed stage tracking are implemented | the winning default-lane family mix and the exact `large_file` admissibility rule are still unproven across the fixed three-repo matrix |
| Phase 7: Release gate | In progress | public release checklist, hygiene scan, public preflight, and public-safe proof discipline are implemented | release credibility still depends on stronger default-on and treatment-vs-baseline evidence |

## Current Evidence Snapshot

- Latest checked-in repo-local calibration loop inspected for this pass: `.sentrux/evals/2026-04-18T10-43-34-904Z-repo-calibration-loop-sentrux/`
- Repo calibration summary from that run: `session_count = 15`, `top_action_follow_rate = 0.333`, `top_action_help_rate = 0`, `task_success_rate = 0.667`, `evidence_review_promotion_candidates = 0`
- Signal scorecard summary from that run: all major evidence lanes are present, but only `promotion_evidence_complete_count = 4` and the inspected summary does not prove default-on readiness
- Evidence review summary from that run: `promotion_candidate_count = 0`, `demotion_candidate_count = 1`, `ranking_miss_count = 1`, `top_action_failure_count = 4`

## Latest Implementation Checkpoint

- 2026-04-24: the [comprehensive detection upgrade plan](./comprehensive-detection-upgrade-plan.md) is now checked in with phase-level execution status.
- Implemented detector-facing upgrades include richer closed-domain site metadata, fallback-masked TypeScript coverage, `Map` constructor maps, JSX conditional render sites, large-file first-cut evidence, cycle edge-basis evidence, standardized repair-packet fields, and governance readiness watchpoints.
- This does not yet close the evidence gate for default-on promotion. Phase statuses remain `In progress` until paired treatment-vs-baseline runs prove top-action help and acceptable false-positive pressure across the fixed repo matrix.

## Phase Tracker

### Phase 0: Reset the Product Contract

- Status: `In progress`
- Scope: align docs, cohorts, scorecard policy, and product surfaces around the narrower agent-lane versus maintainer-lane contract and a shared top-action outcome model.
- Code artifacts: [docs/v2/doctrine.md](./doctrine.md), [docs/v2/roadmap.md](./roadmap.md), [docs/v2/evals/signal-cohorts.json](./evals/signal-cohorts.json), [scripts/lib/signal-calibration-policy.mjs](../../scripts/lib/signal-calibration-policy.mjs), [docs/v2/evals/session-verdicts.schema.json](./evals/session-verdicts.schema.json), [sentrux-core/src/app/mcp_server/handlers/classification_details.rs](../../sentrux-core/src/app/mcp_server/handlers/classification_details.rs)
- Acceptance criteria: docs, scorecards, and promotion rules describe the same product; treatment and baseline runs use one canonical top-action contract.
- Evidence requirements: fresh generated scorecard, session corpus, and evidence review artifacts must all carry the same lane/default-on metadata without compatibility shims; product surfaces consuming ranking data must expose the same lane split.

### Phase 1: Harden the Intervention-Grade Signal Set

- Status: `In progress`
- Scope: keep the default agent lane small, causal, and repairable while pushing structural pressure into watchpoint roles unless the patch directly worsened it.
- Code artifacts: [docs/v2/evals/signal-cohorts.json](./evals/signal-cohorts.json), [scripts/lib/v2-report-selection/buckets.mjs](../../scripts/lib/v2-report-selection/buckets.mjs), [scripts/lib/v2-report-selection/compare.mjs](../../scripts/lib/v2-report-selection/compare.mjs), [sentrux-core/src/app/mcp_server/handlers/check.rs](../../sentrux-core/src/app/mcp_server/handlers/check.rs), [sentrux-core/src/app/mcp_server/handlers/findings/findings_tool.rs](../../sentrux-core/src/app/mcp_server/handlers/findings/findings_tool.rs), [sentrux-core/src/app/mcp_server/handlers/findings/concentration_tool.rs](../../sentrux-core/src/app/mcp_server/handlers/findings/concentration_tool.rs), [scripts/tests/v2-report-selection.test.mjs](../../scripts/tests/v2-report-selection.test.mjs)
- Acceptance criteria: the default lane is dominated by a small set of causal drift families; non-fixable watchpoints no longer crowd the top of the agent surface.
- Evidence requirements: public-proof and repo-local runs must show the lead surface staying focused on clone, propagation, boundary, library-evolution, and patch-local concentration signals; structural watchpoints should only surface in the lead lane when patch-worsened and repairable.

### Phase 2: Expand the Semantic Obligation Graph

- Status: `In progress`
- Scope: improve missing-followthrough detection for DTOs, config, registries, contracts, commands, public APIs, tests, and docs, with concrete sibling-surface guidance.
- Code artifacts: [sentrux-core/src/analysis/semantic/typescript.rs](../../sentrux-core/src/analysis/semantic/typescript.rs), [sentrux-core/src/metrics/v2/obligations.rs](../../sentrux-core/src/metrics/v2/obligations.rs), [sentrux-core/src/metrics/v2/obligations_domain.rs](../../sentrux-core/src/metrics/v2/obligations_domain.rs), [sentrux-core/src/metrics/v2/obligations_contract.rs](../../sentrux-core/src/metrics/v2/obligations_contract.rs), [sentrux-core/src/app/mcp_server/handlers/agent_guidance_obligation.rs](../../sentrux-core/src/app/mcp_server/handlers/agent_guidance_obligation.rs), [sentrux-core/src/metrics/v2/obligations_contract_tests.rs](../../sentrux-core/src/metrics/v2/obligations_contract_tests.rs)
- Acceptance criteria: missed-followthrough findings become more specific; repair packets can name likely sibling surfaces with evidence.
- Evidence requirements: checked-in proof packets and repo-local eval runs must show concrete sibling surfaces, repair surfaces, and verification surfaces for changed DTO/config/registry/public-API patches; changed-symbol precision needs to improve enough that obligation misses stay patch-local.

### Phase 3: Add Bounded LLM Adjudication

- Status: `In progress`
- Scope: use a bounded LLM path for ambiguity reduction and repair guidance without allowing ungrounded findings, free-form repo scans, or silent ranking changes.
- Code artifacts: [scripts/lib/eval-runtime/provider-task-runner/adjudication.mjs](../../scripts/lib/eval-runtime/provider-task-runner/adjudication.mjs), [scripts/lib/eval-runtime/provider-task-runner/task-schemas.mjs](../../scripts/lib/eval-runtime/provider-task-runner/task-schemas.mjs), [scripts/lib/eval-runtime/provider-task-runner/evaluation.mjs](../../scripts/lib/eval-runtime/provider-task-runner/evaluation.mjs), [scripts/lib/eval-runtime/provider-task-runner/results.mjs](../../scripts/lib/eval-runtime/provider-task-runner/results.mjs), [scripts/evals/providers/minimax-openai.mjs](../../scripts/evals/providers/minimax-openai.mjs), [docs/v2/evals/session-corpus.schema.json](./evals/session-corpus.schema.json), [docs/v2/evals/evidence-review.schema.json](./evals/evidence-review.schema.json), [scripts/tests/provider-task-runner-adjudication.test.mjs](../../scripts/tests/provider-task-runner-adjudication.test.mjs)
- Acceptance criteria: false-positive pressure drops, repair guidance clarity improves, and bounded adjudication stays within product cost and latency budgets.
- Evidence requirements: paired comparisons must show measurable improvement over static-only ranking or repair guidance; audit logs must preserve bundle hashes and cited evidence; live ranking changes stay blocked until signal-matched treatment proof exists.

### Phase 4: Add Checker and Pattern Synthesis

- Status: `Planned`
- Scope: synthesize narrow new detector classes from confirmed incidents and accepted repairs rather than intuition.
- Code artifacts: no shipping synthesis pipeline found; current precursor artifacts are [docs/v2/evals/review-verdicts.schema.json](./evals/review-verdicts.schema.json), [docs/v2/evals/review-verdicts.template.json](./evals/review-verdicts.template.json), [docs/v2/false-positive-review.md](./false-positive-review.md), and repo-local review verdict artifacts under `.sentrux/evals/`.
- Acceptance criteria: new detectors can be grown from evidence instead of intuition.
- Evidence requirements: a checked-in incident clustering workflow, a synthesis step that emits candidate checks, and held-out incident plus false-positive validation proving synthesized checks are useful and safe.

### Phase 5: Build the Treatment-Vs-Baseline Evidence Program

- Status: `In progress`
- Scope: prove that Sentrux changes real outcomes versus baseline through paired runs, not just through artifact-quality metrics.
- Code artifacts: [scripts/evals/run-repo-calibration-loop.mjs](../../scripts/evals/run-repo-calibration-loop.mjs), [scripts/evals/build-session-telemetry-summary.mjs](../../scripts/evals/build-session-telemetry-summary.mjs), [scripts/evals/build-signal-scorecard.mjs](../../scripts/evals/build-signal-scorecard.mjs), [scripts/evals/build-session-corpus.mjs](../../scripts/evals/build-session-corpus.mjs), [scripts/evals/build-evidence-review.mjs](../../scripts/evals/build-evidence-review.mjs), [docs/v2/evals/repos/sentrux.json](./evals/repos/sentrux.json), [docs/v2/evals/repos/parallel-code.json](./evals/repos/parallel-code.json), [docs/v2/evals/repos/one-tool.json](./evals/repos/one-tool.json), [docs/v2/evals/batches/sentrux-codex-session-batch.json](./evals/batches/sentrux-codex-session-batch.json), [docs/v2/evals/batches/sentrux-diff-replay-batch.json](./evals/batches/sentrux-diff-replay-batch.json)
- Acceptance criteria: at least one stable evaluation lane shows treatment beating baseline on the primary outcome metrics.
- Evidence requirements: reproducible paired baseline/treatment runs on fixed task sets; per-task and per-signal effect-size reporting; evidence review outputs that convert those wins into promotion/default-on decisions.
- Current conservative read: implementation and repo-local evidence are real, but the latest checked-in Sentrux calibration output does not clear the exit bar. The inspected `2026-04-18` run reports `top_action_help_rate = 0`, `top_action_follow_rate = 0.333`, `task_success_rate = 0.667`, and `promotion_candidate_count = 0`.

### Phase 6: Product Surface Compression

- Status: `In progress`
- Scope: compress the lead experience to 1-3 repairable actions, keep confidence and actionability visible, and keep maintainer watchpoints out of the default patch lane.
- Code artifacts: [scripts/lib/v2-report-selection/buckets.mjs](../../scripts/lib/v2-report-selection/buckets.mjs), [scripts/lib/v2-report-selection/ranking.mjs](../../scripts/lib/v2-report-selection/ranking.mjs), [sentrux-core/src/app/mcp_server/handlers/check.rs](../../sentrux-core/src/app/mcp_server/handlers/check.rs), [sentrux-core/src/app/mcp_server/handlers/agent_guidance_packets.rs](../../sentrux-core/src/app/mcp_server/handlers/agent_guidance_packets.rs), [sentrux-core/src/app/mcp_server/handlers/evaluation_signals.rs](../../sentrux-core/src/app/mcp_server/handlers/evaluation_signals.rs), [sentrux-core/src/app/mcp_server/handlers/findings/findings_tool.rs](../../sentrux-core/src/app/mcp_server/handlers/findings/findings_tool.rs), [scripts/lib/experiment-program.mjs](../../scripts/lib/experiment-program.mjs), [scripts/tests/experiment-program.test.mjs](../../scripts/tests/experiment-program.test.mjs), [docs/v2/experiment-program.md](./experiment-program.md), [docs/v2/experiments/phase-6-repo-task-matrix.md](./experiments/phase-6-repo-task-matrix.md), [docs/v2/experiments/phase-6-review-rubric.md](./experiments/phase-6-review-rubric.md), [docs/v2/experiments/phase-6-promotion-ledger.md](./experiments/phase-6-promotion-ledger.md), [docs/v2/mcp-and-cli.md](./mcp-and-cli.md)
- Acceptance criteria: repeated users can act on the product without sorting through a warning wall.
- Evidence requirements: public-safe proof artifacts and live sessions must show the lead surface staying within the intended 1-3 primary-action envelope while keeping repair packets complete enough to act on. The active proof questions are now narrower: which families survive in the default lane, and whether `large_file` stays there at all or only under an explicit admissibility guardrail.

### Phase 7: Release Gate

- Status: `In progress`
- Scope: make public release credibility match the product claim with public-safe proof, deterministic preflight, and an explicit launch bar for default-on detectors.
- Code artifacts: [docs/v2/release-checklist.md](./release-checklist.md), [scripts/release_preflight_public.mjs](../../scripts/release_preflight_public.mjs), [scripts/check_public_release_hygiene.mjs](../../scripts/check_public_release_hygiene.mjs), [docs/v2/testing-and-validation.md](./testing-and-validation.md), [docs/v2/validation-loop.md](./validation-loop.md), [docs/v2/examples/parallel-code-golden/README.md](./examples/parallel-code-golden/README.md)
- Acceptance criteria: trusted default-on signals only, public-safe docs and artifacts, reproducible treatment-vs-baseline results, acceptable false-positive pressure, and stable local plus remote validation.
- Evidence requirements: release-candidate runs must pass public preflight and hygiene checks, refresh public-safe proof artifacts cleanly, and show that default-on signals are supported by treatment-vs-baseline evidence instead of only local precision or maintainer preference.
