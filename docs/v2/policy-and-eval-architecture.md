# Policy And Eval Architecture

Last audited: 2026-04-17

## Purpose

This document is the maintainer contract for the current v2 policy and evaluation stack.

Its job is to answer four questions clearly:

1. which runtime owns live product behavior
2. where shared policy data lives
3. how eval entrypoints should be structured
4. what must change together when ranking or report behavior changes

## Source Of Truth

Rust owns the live product policy.

That includes:

- MCP issue ordering
- `agent_brief` target selection
- repair-packet completeness and rendering semantics
- gate-blocking semantics

The authoritative Rust surfaces are:

- [agent_ranking.rs](../../sentrux-core/src/app/mcp_server/handlers/agent_ranking.rs)
- [signal_policy.rs](../../sentrux-core/src/app/mcp_server/handlers/signal_policy.rs)
- [agent_brief](../../sentrux-core/src/app/mcp_server/agent_brief)

JS owns proof, reporting, and eval orchestration.

That includes:

- public proof selection and formatting
- benchmark comparison and scorecard assembly
- eval orchestration and artifact generation

JS must not invent a competing live-product ranking policy.

## Shared Static Policy

Shared static policy lives in:

- [.sentrux/signal-policy.json](../../.sentrux/signal-policy.json)

Both runtimes consume it through thin loaders:

- [signal_policy.rs](../../sentrux-core/src/app/mcp_server/handlers/signal_policy.rs)
- [signal-policy.mjs](../../scripts/lib/signal-policy.mjs)

This file is the source of truth for:

- action-kind weights
- leverage weights
- presentation weights
- report leverage ordering
- report presentation ordering
- score-band thresholds

If a ranking or score-band change can be represented in this file, it should be made here first.

## MCP Surface Boundaries

The old monoliths were split on purpose.

Current boundaries:

- [agent_brief](../../sentrux-core/src/app/mcp_server/agent_brief)
  - `mod.rs` is the entrypoint and shared types
  - `policy.rs` owns decision and deferred-item rules
  - `select.rs` owns primary-target eligibility and visibility
  - `render.rs` owns target rendering and inspection guidance
  - `tests.rs` owns brief-specific regressions
- [findings](../../sentrux-core/src/app/mcp_server/handlers/findings)
  - each tool-specific file owns only one findings surface
  - shared response and context helpers stay out of the tool entrypoint files

When a change touches more than one of these modules, maintainers should first verify that the change is crossing a real boundary rather than reintroducing a monolith.

## JS Report Selection Boundaries

The public report-selection stack is intentionally split.

Current boundaries:

- [v2-report-selection.mjs](../../scripts/lib/v2-report-selection.mjs)
  - public facade only
- [normalization.mjs](../../scripts/lib/v2-report-selection/normalization.mjs)
  - candidate normalization and defaulting
- [ranking.mjs](../../scripts/lib/v2-report-selection/ranking.mjs)
  - within-bucket ranking profiles
- [compare.mjs](../../scripts/lib/v2-report-selection/compare.mjs)
  - deterministic ordering and dedupe behavior
- [buckets.mjs](../../scripts/lib/v2-report-selection/buckets.mjs)
  - leverage and presentation bucket selection

The facade file should stay small. If new logic grows there, it usually belongs in one of the internal modules instead.

## Eval Runtime Boundaries

Eval entrypoints should be thin composition roots.

Current shared runtime helpers live under:

- [common.mjs](../../scripts/lib/eval-runtime/common.mjs)
- [provider-task-runner.mjs](../../scripts/lib/eval-runtime/provider-task-runner.mjs)
- [scenarios.mjs](../../scripts/lib/eval-runtime/scenarios.mjs)

Shared benchmark and session-health helpers live under:

- [benchmark-harness.mjs](../../scripts/lib/benchmark-harness.mjs)
- [session-health-schema.mjs](../../scripts/lib/session-health-schema.mjs)
- [signal-scorecard-evidence.mjs](../../scripts/lib/signal-scorecard-evidence.mjs)
- [session-corpus.mjs](../../scripts/lib/session-corpus.mjs)
- [evidence-review.mjs](../../scripts/lib/evidence-review.mjs)

Entry scripts should only do these jobs:

- parse arguments
- load manifests or repo context
- call shared helpers
- assemble final outputs
- exit with the correct status

They should not keep large blocks of reusable artifact or summary logic inline.

The evaluation ownership stack now has four distinct layers:

- session telemetry: repo-local MCP event log summarized into generic convergence and follow-up metrics
- signal scorecard: per-signal calibration evidence across seeded, reviewed, remediation, and session surfaces
- session corpus: normalized live/replay session outcomes with propagation and clone follow-through interpretation
- evidence review: weekly promotion/demotion/ranking review built from scorecard, backlog, and session corpus

Do not collapse those layers back into one file. The intent is:

- telemetry stays generic and family-agnostic
- scorecard stays signal-centric
- session corpus owns per-session outcome interpretation
- evidence review owns weekly prioritization summaries

## Defect Injection Boundaries

The defect catalog is now modular by source.

Current structure:

- [catalog.mjs](../../scripts/defect-injection/catalog.mjs)
  - public catalog facade
- [catalog-core.mjs](../../scripts/defect-injection/catalog-core.mjs)
  - shared defect model and core helpers
- [catalog-dogfood.mjs](../../scripts/defect-injection/catalog-dogfood.mjs)
  - Sentrux-specific catalog entries
- [catalog-parallel-code.mjs](../../scripts/defect-injection/catalog-parallel-code.mjs)
  - `parallel-code` catalog entries

New repo-specific defects should go into a repo-specific module, not back into the facade file.

## Parity Contract

Shared-policy parity is enforced through fixture-driven tests. The contract has two layers:

- static policy parity for score bands, action weights, and report ordering inputs
- representative behavior parity for brief and report-selection outcomes that are supposed to stay aligned across Rust and JS

Current fixture location:

- [shared-policy.json](../../scripts/tests/fixtures/policy-parity/shared-policy.json)
- [behavior-parity.json](../../scripts/tests/fixtures/policy-parity/behavior-parity.json)

Current test consumers:

- [signal-policy.test.mjs](../../scripts/tests/signal-policy.test.mjs)
- [signal_policy.rs](../../sentrux-core/src/app/mcp_server/handlers/signal_policy.rs)
- [v2-report-selection.test.mjs](../../scripts/tests/v2-report-selection.test.mjs)
- [tests.rs](../../sentrux-core/src/app/mcp_server/agent_brief/tests.rs)

If you change:

- score-band thresholds
- action-weight defaults
- report ordering
- representative brief/report ordering behavior

you must update the shared policy file and the shared parity fixtures together.

## Maintainer Rules

When changing policy or eval behavior:

1. change shared static policy in `.sentrux/signal-policy.json` when the policy is data
2. keep Rust as the owner of live product semantics
3. keep JS report tooling as a consumer, not a competitor
4. add or update parity fixtures when shared policy behavior changes
5. keep entry scripts thin and move reusable logic into support modules

## Completion Bar

This architecture is considered healthy only when:

- self-onboarding no longer leads with unfinished refactor hotspots
- remaining eval-runner duplication is below the surfaced watchpoint threshold
- shared policy and representative behavior parity tests are green in Rust and JS
- docs still match the live module layout
