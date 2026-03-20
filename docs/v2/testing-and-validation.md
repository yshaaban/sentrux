# Sentrux V2 Testing And Validation

## Purpose

V2 is explicitly trust-sensitive.

That means testing is part of the product design, not just an implementation detail.

This document defines the validation strategy for the semantic frontend, analyzers, and patch-safety outputs.

## Testing Goals

The test strategy must prove:

1. semantic facts are extracted correctly
2. analyzers fire on the right conditions
3. patch-scoped obligations are accurate enough to trust
4. false positives stay controlled
5. upgrades do not silently regress case-study findings

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

Required golden targets:

1. `parallel-code`
2. `private-benchmark-repo`
3. `private-frontend`

Golden outputs should include:

- agent briefs
- findings
- obligations
- session delta
- scorecard
- confidence

Current status:

- initial scoped `parallel-code` goldens exist in [examples/parallel-code-golden](./examples/parallel-code-golden/README.md)
- checked-in real-repo pass goldens now include `session_start`, `gate`, and `session_end` captured from a temporary local clone of `parallel-code`
- checked-in real-repo regression goldens now include deterministic fail-path `gate` and `session_end` cases on a temporary local clone of `parallel-code`
- initial benchmark notes exist in [examples/parallel-code-benchmark.md](./examples/parallel-code-benchmark.md)
- scoped `private-benchmark-repo` goldens now exist in [examples/private-benchmark-repo-golden](./examples/private-benchmark-repo-golden/README.md)
- `private-benchmark-repo` now has a checked-in benchmark artifact in [examples/private-benchmark-repo-benchmark.md](./examples/private-benchmark-repo-benchmark.md)
- scoped `private-frontend` goldens now exist in [examples/private-frontend-golden](./examples/private-frontend-golden/README.md)
- `private-frontend` now has a checked-in benchmark artifact in [examples/private-frontend-benchmark.md](./examples/private-frontend-benchmark.md)
- the checked-in benchmark repos now include mode-aware `agent_brief` outputs for repo onboarding, patch guidance, and pre-merge guidance
- the benchmark harness now records warm persisted semantic timings and semantic-cache source attribution
- the external eval harness now includes repo-agnostic scenario schemas plus a focused `dead_private` review loop
- synthetic touched-concept gate and `session_end` regression scenarios now exist in the MCP handler test suite
- migration/coexistence coverage now verifies that `gate` and `session_end` still work when only the v2 session baseline is usable, when the v2 session baseline is missing, and when copied or incompatible baselines are present
- confidence regression coverage now checks incompatible schema and project-mismatch session baselines
- session baseline migration coverage now verifies that cross-project v2 baselines are rejected instead of being treated as compatible
- the benchmark harness now supports versioned artifact comparison and separate warm patch-safety timings
- benchmark comparison now has an explicit policy:
  - fail at `>250ms` and `>20%`
  - warn at `>150ms` and `>10%`
- a release checklist now exists in [release-checklist.md](./release-checklist.md)
- the validation loop now has a dedicated one-command runner for checked-in goldens and benchmark regression checks
- the validation loop now has a multi-repo runner across all checked-in benchmark repos
- the validation loop now has a third benchmark repo for modular Next.js frontend shape and onboarding helpers
- full release-grade validation still needs broader benchmark-repo unhappy-path coverage and stronger analyzer promotion criteria

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

V2 should track at least these validation metrics:

1. semantic extraction accuracy on fixtures
2. analyzer precision on benchmark repos
3. false-positive rate by analyzer family
4. warm and cold runtime on benchmark repos
5. number of stable golden findings across versions

The goal is not perfect recall.

The goal is high trust on the findings we choose to surface and gate on.

## Recommended Loop

For the current `parallel-code` proof loop:

1. refresh checked-in goldens when the expected outputs intentionally change with `./scripts/refresh_parallel_code_goldens.sh`
2. validate checked-in goldens and benchmark behavior with `node scripts/validate_parallel_code_v2.mjs`
3. run performance-only checks with `node scripts/benchmark_parallel_code_v2.mjs`

The validation loop catches two classes of regressions:

- output drift in the real-repo goldens
- warm or cold patch-safety regressions in the benchmark artifact

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

## Implementation Tasks

- [ ] add Rust unit-test coverage for normalization and analyzer helpers
- [ ] add bridge contract tests for the Node subprocess
- [ ] create semantic fixture repos for wedge analyzers
- [x] create initial scoped golden outputs for `parallel-code`
- [x] expand `parallel-code` goldens to include `session_end` and gate-oriented regression cases
- [x] add synthetic gate/session regression scenarios for closed-domain changes
- [x] add second benchmark repo proof loop (`private-benchmark-repo`)
- [x] add analyzer false-positive review checklist
- [x] capture initial `parallel-code` benchmark artifact
- [x] capture initial `private-benchmark-repo` benchmark artifact
- [x] add performance regression benchmarks
- [x] expand baseline migration tests beyond the current schema and project-mismatch cases
- [x] add a one-command validation loop for real-repo goldens and benchmark regression checks
- [x] add a multi-repo validation loop for benchmark repos
- [x] capture a short release checklist for proof artifacts and migration checks
- [ ] define promotion criteria for gating analyzers
