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
2. one smaller TS app fixture
3. one TypeScript library fixture

Golden outputs should include:

- findings
- obligations
- session delta
- scorecard
- confidence

Current status:

- initial scoped `parallel-code` goldens exist in [examples/parallel-code-golden](./examples/parallel-code-golden/README.md)
- initial benchmark notes exist in [examples/parallel-code-benchmark.md](./examples/parallel-code-benchmark.md)
- synthetic touched-concept gate and `session_end` regression scenarios now exist in the MCP handler test suite
- initial migration/coexistence coverage now verifies that v2 gate and `session_end` still work when only the v2 session baseline is usable
- full release-grade goldens still need `session_end`, gate-oriented deltas, and broader regression coverage

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

## Validation Metrics

V2 should track at least these validation metrics:

1. semantic extraction accuracy on fixtures
2. analyzer precision on benchmark repos
3. false-positive rate by analyzer family
4. warm and cold runtime on benchmark repos
5. number of stable golden findings across versions

The goal is not perfect recall.

The goal is high trust on the findings we choose to surface and gate on.

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
- [-] expand `parallel-code` goldens to include `session_end` and gate-oriented regression cases
- [-] add synthetic gate/session regression scenarios for closed-domain changes
- [ ] add analyzer false-positive review checklist
- [-] capture initial `parallel-code` benchmark artifact
- [ ] add performance regression benchmarks
- [-] add baseline migration tests
- [ ] define promotion criteria for gating analyzers
