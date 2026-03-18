# Sentrux V2 Core Spec

## Problem Statement

Sentrux v1 measures structural topology well, but it does not yet measure the most expensive static failure modes in agentic coding on application repos:

1. clone drift from copy-paste edits
2. ownership drift across layers
3. incomplete propagation of closed-domain changes
4. contract drift across runtimes and boundaries
5. concentration of risky coordination logic
6. hidden or weakly-protected state transitions

The biggest gap is not the lack of one more graph metric.

The biggest gap is that v1 does not reliably answer whether a patch left the architecture more coherent or more incomplete.

## Product Goal

Sentrux v2 is a static patch-safety and architectural conformance engine.

It should tell an AI agent:

- which concepts its patch touched
- which architecture rules apply to those concepts
- which findings the patch introduced
- which required updates were missed
- whether the patch is safe to continue, fix, or block

## Primary Product Question

The core product question for v2 is:

> What did this patch change, what architectural obligations did that create, and what did the agent fail to update?

That question has priority over repo-wide aesthetic scoring.

## Product Hierarchy

Priority order for v2:

1. patch safety
2. touched-concept regression ratchet
3. repo-level context
4. repo-level score summaries

Output order for v2:

1. findings
2. obligations
3. session delta
4. scorecard
5. confidence

The scorecard remains useful, but it is secondary.

## Core Wedge

The highest-ROI v2 wedge is intentionally narrow.

Beta must win on three analyzer families:

1. clone drift
2. authority and access
3. obligation completeness

These map directly to the static agentic failure modes we care about most:

- duplicate logic drifting apart
- multiple layers owning the same concept
- a changed variant, field, registry, or contract not being fully propagated

For beta:

- zero-config findings should come from clone drift and conservative closed-domain checks
- concept-level authority, access, and obligation findings should rely on explicit critical concept rules

## Secondary Context

These analyzers are valuable, but they are not the initial wedge:

1. contract parity
2. concentration risk

They provide prioritization and architecture context once the patch-safety engine is working.

## Later Analysis

These analyzers are aligned with the vision but should land later because they are more heuristic or more expensive:

1. explicit state-model synthesis across files
2. transition-coverage modeling
3. implicit state-machine inference
4. broader state-integrity heuristics

V2 should not delay the wedge on these.

## Implementation Compression

The `parallel-code` evaluation produced many useful directions, but v2 should not be implemented as dozens of independent analyzers.

Most of the work compresses into a small number of reusable engines:

1. reporting and delta surface
2. clone-drift engine
3. concept authority and access graph
4. obligation engine
5. context analyzers for parity and concentration
6. later state-integrity analyzers

Examples:

- single-writer authority, multi-layer mutation, store/workflow entanglement, and public-API bypass come from the same concept read/write graph
- orphan fields, field propagation obligations, DU exhaustiveness gaps, and shotgun-surgery burden come from the same obligation engine
- parity gaps and startup/restore symmetry come from the same contract model

## Evidence Discipline

Case-study examples in docs, demos, and golden outputs must be verified against the current target repo state.

If a direction is valid but an example is stale or only partially verified:

- keep the direction
- downgrade the example to a candidate
- do not use it as a headline proof point

This is a trust requirement, not a documentation preference.

## Non-Goals

V2 is not trying to:

- prove runtime correctness
- replace tests
- execute app behavior
- depend on runtime traces
- optimize primarily for a single composite quality number
- delay the patch-safety wedge in order to chase broad heuristic state analysis

## Output Model

V2 emits five first-class outputs.

## 1. Findings

Findings are the primary output.

They are concrete static problems with evidence, severity, and likely fix sites.

Examples:

- divergent clone family
- multi-writer concept
- canonicalization bypass
- missing registry update
- missing exhaustive mapping
- missing parity cell
- coordination hotspot

Each finding must include:

- id
- kind
- severity
- summary
- concept
- evidence
- confidence
- estimated fix cost
- whether it is new in the current patch

## 2. Obligations

Obligations are the required update set implied by a changed concept.

Examples:

- adding a bootstrap category requires category list, registry, browser path, Electron path, and tests
- adding a status variant requires exhaustive mappings and switch sites to update
- changing a canonical state source requires projection and adapter updates

This is the core agent-facing feature.

## 3. Session Delta

Session delta compares a changed working tree against a baseline and answers:

- which concepts changed
- which findings were introduced or resolved
- which obligations remain unsatisfied
- whether touched concepts regressed
- whether the patch should warn or fail in CI

## 4. Scorecard

The scorecard is supporting context.

It is not a single number. It is a grouped set of tracks with numerators, denominators, and confidence.

### Core Tracks For Beta

1. Clone Drift
2. Authority And Access
3. Obligation Completeness

### Context Tracks For Beta

1. Contract Parity
2. Concentration Risk
3. Rule Coverage
4. Analysis Coverage
5. Legacy Structural Context

### Future Track

1. State Integrity

`State Integrity` should not become a required beta track until the underlying analyzers are mature enough to avoid noisy guidance.

## 5. Confidence

Confidence is emitted with every top-level output.

It must answer:

- how much of the repo had deep semantic coverage
- how much was structural-only
- how much was excluded
- how many rules were machine-checkable
- how much of the result depends on heuristics

## Additional Product Requirements

## TypeScript Bridge

The TypeScript semantic frontend is the critical technical dependency for v2.

For beta, this should be implemented as a persistent Node subprocess that exposes compiler-backed facts to Rust.

This is an architectural requirement, not just one possible implementation.

The bridge design is specified separately in `typescript-bridge.md`.

## Architecture Guardrail Tests

V2 should treat architecture guardrail tests as first-class evidence when projects have them.

Examples from `parallel-code`:

- `desktop-session.architecture.test.ts`
- `SidebarTaskRow.architecture.test.ts`
- `review-surfaces.architecture.test.ts`

These tests already encode real architecture rules. V2 should be able to ingest them as rule seeds or rule-coverage evidence, but this should not block the wedge.

## Patch-Safety Gate

V2 should support a touched-concept ratchet.

High-confidence regressions introduced by the current patch should be gateable in CI even if the repo still carries older debt.

This is more important than a repo-wide passing score.

## Beta Track Definitions

## Clone Drift

Goal:

- prevent copied logic from silently diverging

Signals:

- exact clone groups
- recently modified clone families
- asymmetric edits in clone families
- duplicate helpers in risky domains

Typical finding kinds:

- `exact_clone_group`
- `divergent_clone_candidate`
- `copy_paste_drift_risk`

## Authority And Access

Goal:

- keep one durable writer and approved access paths for important concepts

Signals:

- writer count per concept
- writer layers per concept
- raw reads of authoritative state
- bypasses of approved adapter or store boundaries

Typical finding kinds:

- `multi_writer_concept`
- `writer_layer_violation`
- `raw_authoritative_read`
- `public_api_bypass`

## Obligation Completeness

Goal:

- ensure changed closed-domain concepts are fully propagated to all required static sites

Signals:

- registry updates
- exhaustive mappings
- protocol payload maps
- startup and hydration wiring
- tests that should move with the concept

Typical finding kinds:

- `missing_registry_update`
- `missing_mapping_update`
- `missing_test_obligation`
- `incomplete_boundary_propagation`
- `non_exhaustive_closed_domain`

## Contract Parity

Goal:

- keep equivalent contracts aligned across runtimes and boundaries

Signals:

- category lists
- payload maps
- snapshot support
- live event support
- listener registration
- versioning

Typical finding kinds:

- `missing_browser_path`
- `missing_electron_path`
- `snapshot_without_live_update`
- `parity_version_gap`

## Concentration Risk

Goal:

- identify oversized coordination hubs before they dominate future changes

Signals:

- write authority breadth
- timer and retry usage
- async branching
- owned concepts
- side-effect fan-out
- churn

Typical finding kinds:

- `coordination_hotspot`
- `protocol_hub`
- `multi_concept_owner`

## State Integrity

Goal:

- later-stage static analysis for whether stateful logic is explicit, closed, and well-protected

Signals:

- discriminated unions
- exhaustive handling
- transition modeling
- inferred implicit lifecycle logic

Typical later finding kinds:

- `implicit_stateful_module`
- `transition_gap`
- `cross_layer_state_mismatch`
- `boolean_soup_lifecycle`
