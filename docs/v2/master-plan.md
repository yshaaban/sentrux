# Sentrux V2 Master Plan

Last audited: 2026-04-18

## Purpose

This document is the execution plan for turning the current v2 wedge into a product that coding agents can trust during AI-assisted and vibe-coded development.

It is intentionally narrower than "measure all technical debt."

The product goal is:

- catch the highest-value maintainability drift introduced by real patches
- surface only the few actions most worth taking next
- provide enough evidence and repair guidance for an agent to act safely
- prove with controlled evaluation that the intervention improves outcomes over baseline

This plan complements:

- [Implementation Status](./implementation-status.md) for current-state reality
- [Roadmap](./roadmap.md) for implementation task tracking
- [Testing And Validation](./testing-and-validation.md) for the validation stack
- [Policy And Eval Architecture](./policy-and-eval-architecture.md) for ownership boundaries

## Product Thesis

Sentrux should be a hybrid change-intelligence system for AI-assisted patches.

It should combine:

- deterministic static analysis for coverage, cheap filtering, and traceable evidence
- semantic analysis for change-obligation expansion and boundary reasoning
- limited LLM reasoning for ambiguity resolution, ranking, repair guidance, and rule synthesis

It should not depend on raw repo-wide LLM scanning.

The strongest user promise is:

1. given a patch, identify the highest-leverage maintainability drift it introduced or exposed
2. show what else must change together for the patch to remain coherent
3. suppress low-confidence or low-actionability noise
4. help the agent land a smaller, cleaner, more reviewable repair

## Why This Direction

Recent research supports five conclusions:

1. AI-generated code introduces real long-lived quality debt, and code smells dominate that debt.
2. Vibe coding shifts failure modes toward black-box acceptance, weak specification, verification bypass, and comprehension debt.
3. Correctness-only evaluation is inadequate; maintainability and readability need first-class measurement.
4. Hybrid static-plus-LLM systems outperform pure static or pure LLM approaches when the LLM receives structured evidence instead of raw codebase text.
5. Developer trust depends primarily on low false-positive rates, strong context, and clear next actions.

That means Sentrux should optimize for intervention quality, not detector count.

## Current State

The repo is already strong on engine foundations:

- MCP `check` exists as the fast patch-safety lane
- the TypeScript semantic substrate is real
- authority/access, obligation, clone, and structural-debt signals are implemented
- scorecard, session corpus, review packet, and calibration loops exist
- release hygiene and public-safe proof discipline are materially better than before

The current gap is not "can the engine detect things?"

The gap is:

- are the first few surfaced actions consistently worth taking
- do the metrics correspond to real agent help, not just offline ranking quality
- can trusted signals stay precise in live, mixed-quality repos

## Program Tracking

This plan is tracked as a concurrent program, not a strict waterfall. Several phases are already underway at the same time. The real control variable is evidence: if a phase does not improve the agent lane or the treatment-vs-baseline loop, it should not expand.

| Phase | Status | Current position | Main remaining exit gap |
| --- | --- | --- | --- |
| Phase 0: Reset the product contract | In progress | doctrine, roadmap, scorecards, evidence review, session corpus, and findings surfaces now share the agent-lane vs maintainer-lane split plus an outcome-first contract | some downstream product surfaces still need to consume the same lane metadata and top-action contract without fallback shims |
| Phase 1: Harden the intervention-grade signal set | In progress | clone drift, authority/access, obligation completeness, trust-tiered findings, and the narrowed default cohort are live; structural findings are increasingly framed as supporting context | the default lane is not yet consistently limited to a small set of intervention-grade actions across repos |
| Phase 2: Expand the semantic obligation graph | In progress | closed-domain and contract-driven obligation expansion now classify DTO, registry, public-API, config, command-status, and test/doc follow-through with surface-aware repair guidance | richer contract families, better changed-symbol precision, and broader sibling-surface coverage remain incomplete |
| Phase 3: Add bounded LLM adjudication | In progress | structured-evidence adjudication schemas, deterministic bundle prompts, audit records, and conservative progress-tracking fields now exist for the eval/runtime surfaces | the scaffold is still advisory-only; no production reranking loop or signal-matched treatment proof is wired into the default evaluation path yet |
| Phase 4: Add checker and pattern synthesis | Planned | incident and verdict artifacts exist to support future synthesis | no evidence-backed synthesis workflow is shipping yet |
| Phase 5: Build the treatment-vs-baseline evidence program | In progress | scorecards, remediation evals, session telemetry, diff replay, signal-matched experiment comparisons, effect-size reporting, and default-rollout qualification now exist in the evidence artifacts | controlled paired baseline/treatment runs still need to become the decisive promotion evidence |
| Phase 6: Product surface compression | In progress | `check`, `agent_brief`, trust tiers, repair-packet work, lane-aware cohorts, and lane-aware findings surfaces now enforce a 1-3 action default lane with structural pressure suppressed unless the patch directly worsened it and repair surfaces are concrete | the remaining gap is demonstrating that the same compression quality holds across more public repos and live usage |
| Phase 7: Release gate | In progress | public-safe hygiene, deterministic preflight, and release checklists are real | release credibility still depends on stronger promotion proof and treatment-vs-baseline evidence |

Current program emphasis:

- keep the agent lane narrow, intervention-grade, and fix-oriented
- keep maintainer-lane watchpoints visible but out of the default patch surface
- treat treatment-vs-baseline evidence as the decisive proof bar for default-on promotion
- delay analyzer breadth expansion until the narrow intervention wedge wins on outcomes

## Product Scope

### In Scope

- patch-induced maintainability drift
- obligation and follow-through gaps
- ownership and boundary erosion created or exposed by a patch
- duplicate-path drift
- library-evolution drift when the changed surface triggers it
- repair guidance and verification guidance for high-confidence findings
- evidence-backed promotion and demotion of detectors

### Out of Scope

- broad repo-health grading as the primary user story
- generic architecture critique on every run
- style policing without repair leverage
- fully autonomous large-scale refactoring as the core product
- vague "spaghetti code" judgments without patch-local evidence

## Definition Of Done

The product should be considered complete for the current strategic wedge only when all of the following are true:

1. The default agent lane surfaces at most 1-3 primary actions for typical patches.
2. Trusted default-on signals have seeded, reviewed, remediation, and session evidence.
3. Treatment beats baseline in controlled evaluations on task success and regression avoidance.
4. Top-action follow rate and top-action help rate are both materially positive and stable.
5. False-positive pressure is low enough that repeated users do not route around the product.
6. Repair packets provide clear fix surfaces and verification steps for primary findings.
7. The product distinguishes agent-lane intervention signals from maintainer-lane watchpoints.

## Strategic Decisions

### 1. Split The Product Into Two Lanes

Agent lane:

- intervention-grade
- patch-local
- fix-oriented
- low-noise

Maintainer lane:

- broader structural watchpoints
- backlog and trend analysis
- slower-moving quality governance

The maintainer lane is useful, but it must not contaminate the default agent surface.

### 2. Prioritize Causal Drift Families

The highest-ROI signal families are the causes of maintainability drift that agents frequently introduce:

- incomplete propagation and exhaustiveness gaps
- boundary and ownership bypass
- clone follow-through drift
- library-evolution drift
- patch-local concentration and reviewability growth

Static inventory signals such as cycles, hotspot shape, or generic large-file warnings should usually stay in the maintainer lane unless the current patch directly worsens them.

### 3. Use LLMs Only Where They Add Unique Value

Use an analysis model such as MiniMax M2.7 for:

- ambiguous-candidate adjudication
- semantic obligation expansion from structured evidence
- ranking among a small candidate set
- repair guidance and verification guidance
- synthesizing new rules from confirmed historical incidents

Do not use an LLM for:

- raw repo-wide scanning
- unsupported architecture judgments
- default-on findings without deterministic evidence

### 4. Optimize For Outcome Change, Not Just Ranking Quality

Offline reviewed precision matters, but the primary product question is:

"Did the surfaced action help the agent produce a cleaner patch with less follow-up drift?"

## Target Architecture

## Layer 1: Static Candidate Generation

Purpose:

- cheap coverage
- deterministic evidence
- candidate narrowing

Responsibilities:

- detect patch-local rule violations and warning deltas
- identify changed public surfaces, schema surfaces, clone families, and boundary edges
- collect file, symbol, import, and structural context

Outputs:

- candidate findings with provenance
- supporting facts and path evidence
- changed-surface anchors for semantic expansion

## Layer 2: Semantic Obligation Expansion

Purpose:

- externalize what else should change together

Responsibilities:

- expand from changed enums, unions, DTOs, config keys, registries, payload maps, command/status surfaces, public APIs, and sibling clones
- infer likely dependent tests, docs, and verification surfaces
- model concept ownership and canonical access paths

Outputs:

- obligation graph
- missing-followthrough candidates
- fix-surface and verification-surface hints

## Layer 3: LLM Adjudication And Ranking

Purpose:

- resolve ambiguity after static and semantic narrowing

Responsibilities:

- suppress likely false positives
- judge actionability and review leverage
- compare candidate findings within the current patch context
- generate compact repair packets

Guardrails:

- LLM input must be structured and bounded
- no default-on finding is emitted from LLM reasoning alone
- evidence shown to the user must remain traceable to static or semantic facts

## Layer 4: Validation Loop

Purpose:

- make trust measurable

Responsibilities:

- rerun analyzers after repair
- verify warning disappearance and no-warning-regression behavior
- run tests and build checks where available
- record whether the agent followed the top action and whether it helped

Outputs:

- scorecards
- session corpus
- evidence review
- promotion and demotion decisions

## Priority Signal Portfolio

## Tier A: Default Agent-Lane Signals

These should define the current product wedge.

1. Incomplete propagation
2. Closed-domain exhaustiveness gaps
3. Forbidden raw reads and writer-outside-owner violations
4. Clone propagation drift and session-introduced clone drift
5. Deprecated API or library-evolution drift when the patch touches affected libraries
6. Patch-local concentration growth when directly caused by the patch

## Tier B: Watchpoint Signals

These can remain visible outside the default top-action lane.

1. Large-file pressure
2. Dependency sprawl
3. Unstable hotspot growth
4. Cycle clusters
5. Dead private code and dead islands

## Tier C: Experimental Signals

These stay quarantined until they prove intervention value.

1. broad readability scoring
2. generic architecture critiques
3. semantic smell classification without clear repair leverage
4. any LLM-only detector with no deterministic anchor

## Metrics And Evidence Model

## North-Star Metrics

These should become the main product KPIs.

1. top-action follow rate
2. top-action help rate
3. task success rate under treatment vs baseline
4. regression-after-fix rate
5. patch expansion rate caused by intervention
6. reviewer acceptance or disagreement rate
7. survival rate of introduced maintainability issues over time

## Supporting Metrics

These are necessary, but not sufficient.

1. reviewed precision
2. review noise rate
3. seeded recall
4. remediation success rate
5. top-1 / top-3 / top-10 actionable precision
6. latency and LLM cost per intervention

## Promotion Stages

### Experimental

- candidate detector
- visible only in proof and review workflows

### Watchpoint

- evidence shows the problem is real
- intervention value is not yet stable

### Trusted

- detector is precise
- repair guidance is usable
- remediation and session evidence are positive

### Default-On

- trusted
- repeatedly helps the top action
- stays low-noise across more than one public repo shape

## Master Program

The phases below are tracked against the status table above. They overlap in execution, but they should not blur together in governance: each phase has its own exit criteria and should be advanced only when its evidence bar is met.

## Phase 0: Reset The Product Contract

Goal:

- align the whole repo around the narrower product promise

Deliverables:

- explicit agent-lane vs maintainer-lane split in docs and scorecards
- updated promotion policy that includes agent-outcome metrics
- default surfacing rules that suppress non-intervention-grade watchpoints

Key tasks:

- update v2 doctrine and roadmap language around intervention-grade signals
- update scorecard policy to weigh follow/help/session outcomes more heavily
- define one canonical "top action" event schema for treatment and baseline runs

Exit criteria:

- docs, scorecards, and promotion rules describe the same product

## Phase 1: Harden The Intervention-Grade Signal Set

Goal:

- make the current default set small, sharp, and trustworthy

Deliverables:

- finalized Tier A signal portfolio
- stronger boundary and propagation precision
- library-evolution drift support
- patch-local concentration-growth detector

Key tasks:

- finish promoting or demoting existing clone, propagation, and authority signals based on proof
- add change-triggered deprecated API drift detection
- restrict large-file and structural signals to patch-worsened cases before they can influence top actions

Exit criteria:

- default lane is dominated by a small set of causal drift families
- non-fixable watchpoints no longer crowd the top of the agent surface

## Phase 2: Expand The Semantic Obligation Graph

Goal:

- improve "what else should change?" accuracy

Deliverables:

- stronger changed-surface expansion for DTOs, config, registry maps, state transitions, commands, public APIs, and docs/tests
- explicit concept ownership and canonical-access resolution where confidence is sufficient

Key tasks:

- improve changed-symbol precision and dependent-surface linking
- add richer contract families beyond current closed-domain and contract-driven triggers
- add test and documentation follow-through modeling for patch-local change surfaces

Exit criteria:

- missed-followthrough findings become more specific
- repair packets can name likely sibling surfaces with evidence

## Phase 3: Add Bounded LLM Adjudication

Goal:

- reduce noise and improve fix guidance without losing traceability

Deliverables:

- structured adjudication prompts for MiniMax M2.7
- bounded evidence packages for candidate ranking
- clearer repair-surface and verification-surface packets

Current position:

- a conservative scaffold now exists in the eval/runtime surfaces with deterministic structured-evidence prompts, schema-checked JSON outputs, stored bundle hashes, cited-evidence auditing, and explicit "no auto-apply" guardrails
- the scaffold is intentionally advisory-only until paired outcome evidence justifies any live reranking or suppression behavior

Key tasks:

- define input contract for adjudication: finding kind, evidence, diff slice, dependent surfaces, and candidate fix sites
- implement confidence-aware suppression and reranking
- store adjudication decisions for audit and offline review

Guardrails:

- no ungrounded detector creation
- no free-form repo scanning
- no ranking changes without evidence bundle logging

Exit criteria:

- false-positive pressure drops
- repair guidance clarity improves
- LLM cost and latency stay within the product budget

## Phase 4: Add Checker And Pattern Synthesis

Goal:

- expand coverage without turning the core product into a prompt farm

Deliverables:

- workflow for mining confirmed incidents and historical patches
- pattern-to-checker synthesis pipeline for narrow, high-value detector classes

Key tasks:

- cluster confirmed findings and accepted repairs by signal family
- use the LLM to propose new narrow rules or static checks
- validate synthesized checks against held-out incidents and false-positive review sets

Exit criteria:

- new detectors can be grown from evidence instead of intuition

## Phase 5: Build The Treatment-Vs-Baseline Evidence Program

Goal:

- prove that Sentrux changes outcomes, not just artifact scores

Deliverables:

- controlled benchmark protocol
- external public repo task set
- baseline agent runs versus Sentrux-assisted runs

Evaluation dimensions:

- task success
- escaped regressions
- patch expansion
- review acceptance
- top-action follow/help
- issue survival after merge when longitudinal data exists

Key tasks:

- define fixed task sets for `parallel-code`, `one-tool`, and additional public-safe repos
- run paired baseline/treatment experiments
- measure per-signal and per-task effect size

Exit criteria:

- at least one stable evaluation lane shows treatment beating baseline on the primary outcome metrics

## Phase 6: Product Surface Compression

Goal:

- make the product usable under real coding-agent constraints

Deliverables:

- compact lead surface with 1-3 primary actions
- concise repair packets
- explicit "why this matters now" framing
- maintainer-lane watchpoint view that does not pollute the default patch lane

Key tasks:

- tighten lead selection and tie-breaking
- demote supporting context behind expandable evidence
- make confidence and actionability visible without overwhelming the user

Exit criteria:

- repeated users can act on the product without sorting through a warning wall

## Phase 7: Release Gate

Goal:

- make public release credibility match the product claim

Deliverables:

- promotion review workflow
- public-safe proof refresh workflow
- explicit launch bar for default-on detectors

Launch bar:

- trusted default-on signals only
- public-safe docs and artifacts
- reproducible treatment-vs-baseline results
- acceptable false-positive pressure
- stable local and remote validation

## Research And Experiment Backlog

These experiments are high ROI and should be prioritized before broad analyzer expansion.

1. Does bounded LLM adjudication improve top-action help rate more than static-only ranking?
2. Which signal families have the strongest treatment effect on agent outcomes?
3. Does deprecated API drift materially predict future maintenance cost in the benchmark repos we care about?
4. Are patch-local concentration metrics predictive of reviewer rejection or patch expansion?
5. Which repair packet fields most improve agent follow-through?
6. Can synthesized narrow checkers outperform prompt-only semantic detectors on precision and cost?

## Decision Gates

Use these gates to avoid slow drift away from the product thesis.

### Gate 1: If The Default Lane Still Contains Too Many Non-Fixable Findings

Action:

- narrow the surfaced signal portfolio further

### Gate 2: If LLM Adjudication Does Not Improve Top-Action Help Rate

Action:

- keep LLM usage limited to repair guidance and rule synthesis

### Gate 3: If Treatment Does Not Beat Baseline On Real Tasks

Action:

- stop expanding analyzers
- focus on ranking, guidance, and product framing until outcome improvement is real

### Gate 4: If A Detector Is Structurally Correct But Rarely Followed

Action:

- demote it from the agent lane even if its reviewed precision is high

## Near-Term Execution Order

Recommended order of work:

1. Phase 0: reset contract and promotion policy
2. Phase 1: harden the narrow signal set
3. Phase 2: expand semantic obligation coverage
4. Phase 3: add bounded LLM adjudication and guidance
5. Phase 5: run paired treatment-vs-baseline evaluations
6. Phase 6: compress the product surface
7. Phase 4: checker synthesis for proven families
8. Phase 7: release gate

This ordering is deliberate.

Do not invest heavily in rule synthesis or broad analyzer expansion before proving that the existing wedge changes agent outcomes.

## Notes On MiniMax M2.7 Usage

MiniMax M2.7 is a good fit for the bounded semantic tasks in this plan if it is:

- fed structured evidence rather than whole repositories
- evaluated on adjudication precision and actionability, not eloquence
- kept off the critical path for cheap deterministic filtering
- audited with stored prompts, evidence bundles, and decisions

The success condition is not "the model sounds smart."

The success condition is:

- fewer false positives
- better top-action ranking
- clearer repair packets
- measurable improvement in treatment-vs-baseline runs

## References

This plan is informed in part by the following recent research:

- Debt Behind the AI Boom: A Large-Scale Empirical Study of AI-Generated Code in the Wild
- Quality Assurance of LLM-generated Code: Addressing Non-Functional Quality Characteristics
- Investigating The Smells of LLM Generated Code
- Comprehension Debt in GenAI-Assisted Software Engineering Projects
- Good Vibrations? A Qualitative Study of Co-Creation, Communication, Flow, and Trust in Vibe Coding
- Vibe Coding as a Reconfiguration of Intent Mediation in Software Development
- A Survey of Vibe Coding with Large Language Models
- Beyond Correctness: Benchmarking Multi-dimensional Code Generation for Large Language Models
- SWE-Refactor: A Repository-Level Benchmark for Real-World LLM-Based Code Refactoring
- ZeroFalse: Improving Precision in Static Analysis with LLMs
- Minimizing False Positives in Static Bug Detection via LLM-Enhanced Path Feasibility Analysis
- KNighter: Transforming Static Analysis with LLM-Synthesized Checkers
- Together We Go Further: LLMs and IDE Static Analysis for Extract Method Refactoring
- Static Analysis as a Feedback Loop: Enhancing LLM-Generated Code Beyond Correctness
- CodeCureAgent: Automatic Classification and Repair of Static Analysis Warnings
- Rethinking Code Review Workflows with LLM Assistance: An Empirical Study
- LLMs are Bug Replicators
- LLMs Meet Library Evolution: Evaluating Deprecated API Usage in LLM-based Code Completion
