# Evaluation Synthesis

This document folds the `parallel-code` evaluation work back into the v2 plan.

It is not a replacement for the spec or roadmap. It is the bridge between the ranked directions and the actual build plan.

## Overall Read

The ranked analysis is directionally strong.

Its best contribution is not the exact ordering of all 50 directions. It is the evidence that:

1. output framing needs a fast fix
2. clone drift deserves a first-class lane
3. most of the top-ranked directions collapse into a small number of reusable analyzers
4. the highest-ROI wedge is smaller than the full vision

The main correction is that some of the poster-child examples were stale or overstated on the current `parallel-code` tree, so v2 docs should only anchor on verified examples.

## What To Incorporate

## 1. Separate Product Reframing From New Analysis

Some top-ranked directions are product and reporting fixes, not new analyzers:

- composite family dashboard
- worst-bottleneck headline
- delta diff as primary output

These should ship as a fast lane before or beside the deeper semantic work.

Reason:

- they immediately improve trust
- they do not require a new semantic frontend
- they reduce the damage caused by the current composite score presentation

## 2. Promote Clone Drift

Clone drift should be a first-class v2 lane, not an afterthought.

Why:

- it ranked highly on evidence, impact, and feasibility
- `parallel-code` has plausible clone-drift candidates in diff parsing and HTML escaping helpers
- Sentrux already has body-hash duplicate infrastructure in v1, so exact clone detection is not a greenfield feature

Implication:

- add a fast-lane roadmap section for exact clones, divergent clones, and copy-paste bug risk

## 3. Collapse The 50 Directions Into A Small Number Of Engines

The evaluation surfaced many useful findings, but most of them are not separate subsystems.

They reduce to a few reusable engines:

### A. Reporting And Delta Surface

Outputs:

- family dashboard
- bottleneck headline
- delta-first summaries
- CI ratchet output

### B. Clone Drift

Outputs:

- exact clone groups
- divergent clone candidates
- copy-paste bug risk
- clone genealogy summaries

### C. Concept Authority And Access

Outputs:

- single-writer authority
- multi-layer mutation
- store/workflow entanglement
- public-API bypass
- canonical-access violations

### D. Obligation Engine

Outputs:

- orphan/read-write asymmetry
- field propagation obligations
- DU exhaustiveness gaps
- missing mapping updates
- missing registry updates
- agent context burden

### E. State And Contract Integrity

Outputs:

- transition coverage
- implicit state machine count
- silent default fallthroughs
- cross-runtime protocol parity
- invalid-state risk

### F. Concentration And Change Cost

Outputs:

- fan-in/fan-out concentration
- schema bottlenecks
- mutation hubs
- churn density
- composition-root overload

## 4. Make The Wedge Explicit

The evaluation supports a stricter hierarchy than the earlier docs had.

The core wedge should be:

1. clone drift
2. authority and access
3. obligation completeness

Everything else is either:

- supporting context
- useful later analysis
- or product framing on top of those engines

This is the highest-ROI interpretation of the case study.

For beta, concept-level findings in this wedge should be rule-driven rather than inference-driven.

That means:

- zero-config findings from clone drift and conservative closed-domain checks
- explicit `[[concept]]` rules for authority, access, and obligation findings

## 5. Treat Architecture Guardrail Tests As Rule Inputs

`parallel-code` already encodes architecture intent in source-level tests:

- `src/app/desktop-session.architecture.test.ts`
- `src/components/SidebarTaskRow.architecture.test.ts`
- `src/components/review-surfaces.architecture.test.ts`

These are high-signal, low-noise rule sources.

Implication:

- v2 should ingest architecture guardrail tests as seed evidence for rule coverage and concept constraints
- this belongs in the roadmap as an explicit task, not just a note in the case study

## 6. Derive "Agent Context Burden" From Obligations

The evaluation correctly identified shotgun-surgery risk.

That does not need a separate engine. It should be a derived view from the obligation engine:

- number of required update sites
- number of affected concepts
- number of exhaustive constructs touched
- number of runtimes or boundaries touched

This is a useful agent-facing output and belongs in `session_end`.

## 7. Use Verified Examples Only

At least some high-ranking examples were stale on the current repo state.

Examples that did not hold as written:

- `planContent` is not a ghost field; it exists in `src/store/types.ts` and has a write path in `src/store/tasks.ts`
- `exposedPorts never restored` is too strong; restore logic exists in the Electron task-port path even though persistence hydration is split across layers

Implication:

- case-study docs and demos should only cite examples verified on the current commit
- where the direction is valid but the example is not fully verified, describe it as a candidate or analyzer target, not an established bug

## Verified Current-Repo Signals Worth Keeping

These are stable enough to use as v2 anchors:

- `task-workflows.ts` imports `setStore` from `../store/core` and performs multiple direct task mutations
- `git-status-sync.ts` and `git-status-polling.ts` both write `taskGitStatus`
- many UI surfaces consume the `store/store.ts` barrel, showing an intended public store API exists
- `SidebarTaskRow.architecture.test.ts` explicitly forbids raw reads of `store.agentSupervision`, `store.taskGitStatus`, and `store.taskReview`
- `task-presentation-status.ts` uses explicit exhaustive maps and `assertNever(...)`
- `server_state_bootstrap` remains a strong contract-parity target because categories and registry wiring are explicit

## What Changes In The V2 Plan

The current v2 plan should be adjusted in four ways:

1. make findings, obligations, and session delta the primary product surface
2. add a fast lane for reporting/output reframing
3. add a fast lane for clone drift
4. explicitly ingest architecture guardrail tests as rule sources
5. derive agent-context burden from obligations instead of treating it as a standalone analyzer

## Incorporation Tasks

- [x] add reporting reframe tasks to the roadmap
- [x] add clone-drift fast-lane tasks to the roadmap
- [x] add architecture-guardrail ingestion as a tracked task
- [x] add obligation-count and agent-context-burden outputs to the roadmap
- [x] keep case-study examples limited to verified current-repo evidence
