# Sentrux V2 Testing And Validation

## Purpose

V2 is explicitly trust-sensitive.

That means testing is part of the product design, not just an implementation detail.

This document defines the validation strategy for the semantic frontend, analyzers, and patch-safety outputs.

Current release note:

- MCP `check` is the fast-path v2 validation target for ranked action quality
- CLI `brief` and `gate` are the main v2 CLI validation targets
- CLI `check` remains the legacy structural rules check and should be validated as supporting context, not as the primary v2 surface
- public release validation must keep checked-in artifacts and docs free of private repo names, internal infrastructure links, and workstation-specific paths

## Validation Contract

Validation now has to serve two product lanes with different bars.

Agent lane:

- validates the default patch surface
- cares about intervention-grade signals only
- must prove ranked action quality, fixability, and treatment-vs-baseline outcome improvement

Maintainer lane:

- validates broader watchpoints and structural context
- cares about evidence quality, stability, and governance usefulness
- does not need the same top-action or in-session intervention bar

This means a structurally correct watchpoint is not automatically good enough for default-on agent-lane promotion.

## Testing Goals

The test strategy must prove:

1. semantic facts are extracted correctly
2. analyzers fire on the right conditions
3. patch-scoped obligations are accurate enough to trust
4. false positives stay controlled
5. upgrades do not silently regress case-study findings
6. shared Rust and JS policy consumers do not drift on score bands or ordering
7. default agent-lane findings improve outcomes over baseline, not just offline artifact quality

## Test Layers

## Layer 1: Unit Tests

Scope:

- Rust normalizers
- rule parsing
- suppression logic
- analyzer utilities
- bridge protocol encoding and decoding

Purpose:

- keep small logic deterministic

## Layer 2: Bridge Contract Tests

Scope:

- Rust bridge supervisor
- Node bridge lifecycle
- request and response protocol
- incremental update behavior

Purpose:

- verify Rust ↔ TypeScript transport and restart behavior

Required fixture cases:

- small single-`tsconfig` repo
- multi-`tsconfig` workspace
- missing Node
- bridge crash and restart
- `tsconfig` change invalidation

## Layer 3: Semantic Fixture Repos

Scope:

- known TypeScript examples with expected facts

Purpose:

- validate semantic extraction accuracy

Fixture categories:

1. explicit `Record<Union, ...>` exhaustiveness
2. exhaustive and non-exhaustive `switch`
3. direct store mutations
4. public-API versus internal-path imports
5. registry and payload-map patterns
6. clone families

The fixture repos should be small and purpose-built.

## Layer 4: Analyzer Golden Outputs

Scope:

- full v2 outputs on selected real repos

Purpose:

- regression-test user-visible findings and obligations

Current checked-in public proof targets:

1. `parallel-code`
2. `one-tool`

For commands that validate checked-in public proof artifacts, keep public `parallel-code` and `one-tool` checkouts available at `../parallel-code` and `../one-tool`, or set `PARALLEL_CODE_ROOT` and `ONE_TOOL_ROOT` explicitly.

Internal or non-public benchmark repos must not be checked into the public tree. Additional public-safe benchmark repos can be added later once their artifacts are generated from repos that are safe to publish.

Golden outputs should include:

- check
- agent briefs
- findings
- obligations
- session delta
- scorecard
- confidence

Current status:

- initial scoped `parallel-code` goldens exist in [examples/parallel-code-golden](./examples/parallel-code-golden/README.md)
- checked-in mixed-language onboarding and benchmark evidence now exist for `one-tool` in [examples/one-tool-onboarding.json](./examples/one-tool-onboarding.json) and [examples/one-tool-benchmark.json](./examples/one-tool-benchmark.json)
- checked-in real-repo pass goldens now include `session_start`, `gate`, and `session_end` captured from a temporary local clone of `parallel-code`
- checked-in real-repo regression goldens now include deterministic fail-path `gate` and `session_end` cases on a temporary local clone of `parallel-code`
- initial benchmark notes exist in [examples/parallel-code-benchmark.md](./examples/parallel-code-benchmark.md)
- the checked-in benchmark repos now include mode-aware `agent_brief` outputs for repo onboarding, patch guidance, and pre-merge guidance
- the benchmark harness now records warm persisted semantic timings and semantic-cache source attribution
- the external eval harness now includes repo-agnostic scenario schemas plus a focused `dead_private` review loop
- the quality loop now also includes review packets, defect remediation evals, and per-signal scorecard generation
- synthetic touched-concept gate and `session_end` regression scenarios now exist in the MCP handler test suite
- migration/coexistence coverage now verifies that `gate` and `session_end` still work when only the v2 session baseline is usable, when the v2 session baseline is missing, and when copied or incompatible baselines are present
- confidence regression coverage now checks incompatible schema and project-mismatch session baselines
- session baseline migration coverage now verifies that cross-project v2 baselines are rejected instead of being treated as compatible
- the benchmark harness now supports versioned artifact comparison and separate warm patch-safety timings
- benchmark comparison now has an explicit policy:
  - fail at `>250ms` and `>20%`
  - warn at `>150ms` and `>10%`
- a release checklist now exists in [release-checklist.md](./release-checklist.md)
- the validation loop now has a dedicated local public release preflight in `scripts/release_preflight_public.mjs`, including `ts-bridge` dependency installation, pinned `tree-sitter` CLI setup when needed, current-platform grammar-bundle generation, bundle-aware installer smoke on supported hosts, and an explicit tracked-tree cleanliness check without regenerating benchmark artifacts
- the public tree now has a hygiene scanner that blocks abandoned upstream links, private repo names, internal domains, and maintainer workstation paths
- the validation loop now has a runner for checked-in public benchmark repos that are intended to gate release decisions; self-benchmark docs remain informational until they are promoted into that lane explicitly
- repeated-sample benchmark and golden refreshes now freeze the analyzed input in a disposable clone, and age-sensitive proof runs pin "now" to the analyzed commit epoch for deterministic public artifacts
- fail-tier benchmark regression decisions are now documented as a dedicated quiet-runner step rather than a noisy local laptop gate
- the repo now has a dedicated benchmark-gate workflow for that quiet-runner lane plus installer smoke automation in local preflight, CI, and release builds
- grammar bundle refreshes now fail closed on unreachable pinned refs instead of silently falling back to a repo default branch
- full release-grade validation still needs broader public benchmark-repo unhappy-path coverage and stronger analyzer promotion criteria

## Layer 5: False-Positive Review

Scope:

- every new heuristic analyzer or heuristic rule change

Purpose:

- keep the trust bar high

Process:

1. run on benchmark repos
2. inspect top findings manually
3. classify true positive, acceptable warning, or false positive
4. block promotion of the analyzer if false positives are too high

Reference workflow:

- [False-Positive Review](./false-positive-review.md)

## Layer 6: Baseline And Migration Tests

Scope:

- v1 baseline coexistence
- v2 baseline read and write
- session behavior across version boundaries

Purpose:

- prevent baseline and ratchet regressions during rollout

Current learning:

- `gate` already operates correctly from the v2 session baseline alone
- `session_end` needed an explicit fallback path so missing or unreadable v1 structural baselines do not break the primary v2 patch-safety output
- v2 session baselines now carry project identity, so confidence can reject copied or cross-project baselines explicitly instead of treating them as compatible

## Validation Metrics

V2 should track at least these validation metrics.

Primary agent-lane metrics:

1. top-action follow rate
2. top-action help rate
3. task success rate under treatment vs baseline
4. regression-after-fix rate
5. patch expansion caused by intervention
6. reviewer disagreement rate on surfaced primary actions

Supporting validation metrics:

1. semantic extraction accuracy on fixtures
2. analyzer precision on benchmark repos
3. false-positive rate by analyzer family
4. warm and cold runtime on benchmark repos
5. number of stable golden findings across versions

The goal is not perfect recall.

The goal is high trust on the findings we choose to surface and gate on.

For the default patch surface, generic reviewed precision is necessary but not sufficient. The decisive question is whether the surfaced action helped the agent land a cleaner patch than baseline.

Shared-policy parity is now part of that trust bar:

- [.sentrux/signal-policy.json](../../.sentrux/signal-policy.json) is the shared static source
- [signal-policy.test.mjs](../../scripts/tests/signal-policy.test.mjs) and [signal_policy.rs](../../sentrux-core/src/app/mcp_server/handlers/signal_policy.rs) consume the same fixture in `scripts/tests/fixtures/policy-parity/`

For the lead surface, the quality bar is stricter than generic reviewed precision. The scorecard should now answer:

- are the first 1, 3, and 10 reviewed findings still actionable enough to deserve their rank
- do reviewer ranking preferences agree with the presented order
- does remediation and session telemetry show that the surfaced findings were actually fixable
- does treatment beat baseline when those findings are surfaced as the lead action

Current support boundary:

- top-1 / top-3 / top-10 actionable precision is scorecard-grade when the curated review verdict file preserves the reviewed order
- ranking-preference satisfaction is scorecard-grade when verdicts use `preferred_over`
- repair-packet completeness is scorecard-grade supporting evidence when verdicts preserve the structured repair fields; the current bar is the `REVIEW_PACKET_COMPLETENESS_POLICY` in [`../../scripts/lib/signal-calibration-policy.mjs`](../../scripts/lib/signal-calibration-policy.mjs) (`scope`, `summary`, `evidence`, and `repair_surface` required, `fix_hint` and `likely_fix_sites` preferred, top-3 complete rate at least `0.8`, top-10 complete rate at least `0.7`)
- repair-packet completeness does not replace ranking quality, remediation success, or session outcomes; it strengthens fixability evidence after the detector is already showing useful ranked behavior
- scorecards now separate broad promotion guidance from stricter default-on readiness by carrying per-signal `default_rollout_recommendation`; evidence reviews then summarize `default_on_candidates`, repo-level treatment evidence, and final `default_on_promotion` readiness separately from generic promotion candidates
- session corpus, scorecard, and evidence review artifacts now carry `evidence_sources` so paired baseline/treatment outputs can be traced back to their program, phase, batch, cohort, and analysis mode without relying on filename conventions

## Treatment-Vs-Baseline Evidence

Treatment-vs-baseline runs are now part of the validation stack, not a later optional study.

Minimum bar for the agent lane:

1. run paired baseline and Sentrux-assisted tasks on the same scoped repo/task set
2. record one canonical top-action event shape across both runs
3. compare task success, escaped regressions, patch expansion, review acceptance, top-action follow, and top-action help
4. use those results in detector promotion and demotion decisions

This bar is stronger than the maintainer-lane bar. A watchpoint can remain useful with weaker in-session evidence; a default-on agent-lane signal cannot.

## Recommended Loop

For the current checked-in public proof loop:

1. refresh checked-in goldens when the expected outputs intentionally change with `./scripts/refresh_parallel_code_goldens.sh`
2. validate checked-in goldens deterministically with `node scripts/validate_parallel_code_v2.mjs --goldens-only`
3. run the local public release preflight with `node scripts/release_preflight_public.mjs`
4. run fail-tier benchmark regression review on a quiet machine or dedicated CI runner with `node scripts/validate_benchmark_repos_v2.mjs`

The validation loop catches two classes of regressions:

- output drift in the real-repo goldens
- warm or cold patch-safety regressions in the benchmark artifacts once they are measured on comparable quiet-runner inputs

That split is intentional. Local preflight should be deterministic and cheap enough to run often while still proving the current-platform binary and grammar bundle install path; final benchmark regression decisions need quieter hardware and more comparable inputs than a normal maintainer workstation can guarantee.

The checked-in `one-tool` benchmark artifact is part of that dedicated benchmark lane. Use `node scripts/validate_one_tool_v2.mjs` only when you are intentionally validating that one repo on comparable quiet hardware or the benchmark-runner CI environment.

## Beta Validation Scope

For beta, validation must focus on the wedge:

1. clone drift
2. authority and access
3. obligation completeness

Parity, concentration, and later state analysis can have lighter validation initially.

## `parallel-code` Validation Plan

`parallel-code` should be the primary real-world golden target.

For beta, golden validation should focus on:

1. clone-drift findings in duplicated helper/parser patterns
2. authority and access findings on explicitly-declared concepts
3. obligation findings on closed-domain and registry changes
4. `session_end` output quality on synthetic patch scenarios

Initial real-repo goldens already showed four concrete analyzer issues to fix next:

1. test setup writes pollute authority findings
2. projection concepts need different semantics than owned-state concepts
3. parity runtime-binding detection is too shallow
4. explicit controller-style state models are not being mapped yet

## Release Bar For A New Analyzer

Before a new analyzer becomes:

- visible in `session_end`
- used in `gate`
- used in CI ratchets

it should have:

1. fixture coverage
2. benchmark-repo validation
3. reviewed false-positive samples
4. documented confidence behavior
5. scorecard evidence that meets the current promotion policy in [`../../scripts/lib/signal-calibration-policy.mjs`](../../scripts/lib/signal-calibration-policy.mjs)

Before a signal becomes default-on in the agent lane, it should also have:

1. positive remediation and session evidence
2. treatment-vs-baseline evidence that the surfaced action improves outcomes
3. repair-packet quality strong enough to shorten the next edit

Signals that are real but do not meet that bar should stay maintainer-lane watchpoints or `experimental`.

Current promotion policy thresholds:

- seeded recall at least `0.95`
- reviewed precision at least `0.8`
- review noise rate at most `0.2`
- top-1 actionable precision at least `1.0` when at least one curated reviewed sample exists
- top-3 actionable precision at least `0.67` when at least three curated reviewed samples exist
- top-10 actionable precision at least `0.6` when at least ten curated reviewed samples exist
- ranking-preference satisfaction at least `0.8` when ranked comparison verdicts exist
- remediation success rate at least `0.6`
- top-action clear rate at least `0.6`
- session clean rate at least `0.6`
- follow-up regression rate at most `0.4`

Signals that fail those thresholds should stay `watchpoint` or `experimental` until the scorecard evidence improves.

Do not treat high reviewed precision alone as promotion-grade evidence. A signal that is often "technically true" but still weak in the first few ranked slots is still failing the primary-target bar.
Do not treat strong repair-packet completeness alone as promotion-grade evidence either. It is useful supporting evidence for fixability, not a substitute for ranked usefulness or clean session outcomes.
Do not treat maintainer-lane usefulness as a license to promote a signal into the default patch lane. The agent lane has the stricter bar.

## Implementation Tasks

- [ ] add Rust unit-test coverage for normalization and analyzer helpers
- [ ] add bridge contract tests for the Node subprocess
- [ ] create semantic fixture repos for wedge analyzers
- [x] create initial scoped golden outputs for `parallel-code`
- [x] expand `parallel-code` goldens to include `session_end` and gate-oriented regression cases
- [x] add synthetic gate/session regression scenarios for closed-domain changes
- [x] add analyzer false-positive review checklist
- [x] capture initial `parallel-code` benchmark artifact
- [x] add performance regression benchmarks
- [x] expand baseline migration tests beyond the current schema and project-mismatch cases
- [x] add a one-command validation loop for real-repo goldens and benchmark regression checks
- [x] add a multi-repo validation loop for benchmark repos
- [x] capture a short release checklist for proof artifacts and migration checks
- [x] add public release hygiene scanning for banned public-tree content
- [x] add a one-command public release preflight for the supported public matrix
- [x] define promotion criteria for gating analyzers
