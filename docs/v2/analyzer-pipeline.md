# Sentrux V2 Analyzer Pipeline

## Pipeline Goals

The pipeline must be:

- static-only
- incremental
- patch-aware
- confidence-producing
- compatible with the current structural scan

The pipeline should optimize first for high-trust patch-safety outputs, not for the widest possible static inference.

## Output Priority

The pipeline should naturally assemble results in this order:

1. findings
2. obligations
3. session delta
4. scorecard
5. confidence

If a stage improves the scorecard but does not improve findings, obligations, or delta, it is not on the critical path.

## Stage 0: Scope, Trust, And Reporting Foundation

Purpose:

- define what is in scope
- define what is excluded
- define how much the result can be trusted
- improve current output framing before deeper semantics land

Required work:

1. include tracked and untracked files
2. classify generated, vendor, build, fixture, and cache directories
3. split unresolved imports into:
   - internal resolution failures
   - external expected references
4. expose confidence and exclusions in every run
5. promote bottleneck-first and delta-first reporting in current outputs

This stage fixes the trust and presentation gap before deeper semantics are added.

Stages 0, 1, and 3 should be parallelized where practical because they do not depend on one another's semantic outputs.

## Stage 1: Structural Scan

Reuse the current structural scan:

- scanner
- parser
- import graph
- call graph
- inherit graph
- file metrics
- duplicate body-hash groups

Output:

- `Snapshot`

This remains the broad language-agnostic fact base and supports the clone-drift fast lane.

## Stage 2: Project Discovery

Build `ProjectModel`.

Responsibilities:

- locate config roots
- locate TypeScript project boundaries
- load `.sentrux/rules.toml`
- discover architecture guardrail tests
- discover workspaces and packages
- classify exclusions

Artifacts:

- `ProjectModel`
- frontend plans

This stage should remain conservative. The goal is a reliable project model, not maximum inference.

## Stage 3: Early Clone-Drift Pass

Before the semantic frontend is complete, v2 should already surface:

1. exact clone groups
2. git-recency and churn around clone groups
3. divergent clone candidates from recently split histories

Inputs:

- `Snapshot`
- git history metadata

Outputs:

- clone findings
- clone evidence payloads

This is part of the initial wedge because it has high ROI and low implementation risk.

## Stage 4: TypeScript Semantic Frontend

Introduce a new abstraction:

```rust
pub trait SemanticFrontend {
    fn discover(&self, project: &ProjectModel) -> Result<FrontendPlan, String>;
    fn analyze(&self, plan: &FrontendPlan, structural: &Snapshot) -> Result<SemanticSnapshot, String>;
}
```

Frontend strategy:

1. TypeScript
2. Rust
3. fallback heuristics for unsupported ecosystems

V2 usefulness on `parallel-code` depends on the TypeScript frontend landing first.

For beta, the TypeScript frontend should be implemented through a persistent Node subprocess bridge.

This is an architectural requirement because the warm-run performance target depends on:

- a persistent process
- incremental updates
- compiler-backed facts

Bridge design details are specified in `typescript-bridge.md`.

## TypeScript Frontend

Use the TypeScript compiler or language service rather than tree-sitter-only facts.

Why:

- symbol graph
- reference tracking
- union and literal domain extraction
- compiler-backed project structure
- incremental updates

Implementation notes:

- one language service or program per discovered `tsconfig`
- hash-backed file snapshots
- reuse document registry and incremental parsing

Core extraction targets for the wedge:

1. declarations
2. references
3. imports and resolved module structure
4. writes and reads to known access paths
5. unions and literal domains
6. `switch`, `assertNever`, `satisfies`, `Record<Union, ...>`
7. registry objects and payload maps
8. test references to concepts

## Stage 5: Normalization

Normalize structural and semantic facts into shared engine facts.

Responsibilities:

- stable symbol IDs
- stable access-path IDs
- normalize alias chains
- normalize registry tables
- normalize union protections
- assign files to inferred or rule-defined layers

This stage turns compiler-backed facts into reusable analyzer inputs.

## Stage 6: Minimal Concept Graph

Build a minimal `ConceptGraph` sufficient for the wedge.

For beta, strong concept-level findings should come from explicit rules.

Concept graph sources for beta, in priority order:

1. explicit rules
2. protocol registries attached to explicit rules
3. architecture guardrail tests as supporting evidence

For beta:

- do not require general concept inference to make the wedge useful
- do not emit high-severity concept findings from inferred concepts alone

Conservative concept inference can be added later as a Tier 2 capability.

## Stage 7: Wedge Analyzers

These analyzers are the beta critical path.

## 7A. Clone Drift

Required outputs:

- exact clone findings
- divergent clone candidates
- copy-paste drift risk findings

## 7B. Authority And Access

Required outputs:

- authoritative concept detection
- durable writer detection
- multi-writer findings
- writer-layer violations
- raw authoritative reads
- public-API or barrel bypass findings

For beta, these findings should be scoped to explicit critical concepts.

## 7C. Obligation Engine

Required outputs:

- obligation templates
- changed-file to changed-concept mapping
- satisfied vs missing required sites
- obligation count
- agent-context-burden summaries derived from obligations

The obligation engine should absorb:

- closed-domain exhaustiveness gaps
- field and registry propagation gaps
- missing mapping updates
- missing test obligations where confidence is sufficient

Zero-config beta findings can still come from conservative closed-domain exhaustiveness analysis even when no explicit concept rules exist.

## Stage 8: Session Delta And Gate

Purpose:

- compare the current working tree against a baseline
- decide whether touched concepts regressed
- produce the main agent-facing output

Inputs:

- current analysis
- baseline analysis
- changed files from git diff and untracked-file detection

Outputs:

- changed concepts
- introduced findings
- resolved findings
- missing obligations
- track deltas
- gate decision for touched-concept regressions

This is the primary agent loop feature.

## Stage 9: Context Analyzers

These analyzers are important, but they are not required before the wedge is useful.

## 9A. Contract Parity

Purpose:

- measure cross-runtime and cross-boundary symmetry

Examples:

- bootstrap categories
- browser/Electron listener paths
- snapshot versus live-update support

## 9B. Concentration Risk

Purpose:

- identify coordination hotspots that deserve refactoring attention

Examples:

- concept breadth
- side-effect fan-out
- timer and retry usage
- churn-weighted protocol hubs

## Stage 10: Later State-Integrity Analysis

These analyzers should land only after the wedge is delivering value:

1. explicit state-model synthesis across files
2. transition-coverage modeling
3. implicit lifecycle heuristics
4. richer invalid-state findings

The beta wedge should reuse closed-domain exhaustiveness data without requiring full state-machine inference.

## Stage 11: Result Assembly

Assemble:

- findings
- obligations
- session delta
- scorecard
- confidence
- optional legacy structural context

Return `V2Analysis`.

## Cache And Invalidation

V2 must be incremental from day one.

Cache layers:

1. structural scan cache
2. project discovery cache
3. semantic frontend cache
4. concept extraction cache
5. analyzer result cache
6. session-baseline cache

Invalidation rules:

- file hash change invalidates structural and semantic data for that file
- tsconfig change invalidates the affected TS project
- rules change invalidates concept extraction and analyzers
- exclusions change invalidates scope and coverage
- frontend version change invalidates semantic cache

## Performance Targets

Initial targets for `parallel-code` scale:

- cold structural scan: similar to current v1 scan
- cold semantic analysis: under 20 seconds
- warm semantic analysis after small edit: under 3 seconds
- patch-scoped session delta after small edit: under 2 seconds

These are product targets, not hard guarantees.

Meeting the warm-run target requires:

1. persistent Node bridge process
2. incremental TypeScript project updates
3. coarse-grained fact responses
4. parallel execution of non-dependent stages where possible

## Failure Modes

The pipeline must degrade gracefully.

Examples:

- if TS semantic analysis fails, return clone findings, structural context, and low-confidence notices instead of pretending the repo is fine
- if a project has no rules, emit conservative heuristic findings with lower confidence
- if concept inference is ambiguous, avoid a strong finding and surface the confidence gap
- if parity or state analyzers are unavailable, do not block patch-safety outputs

## Beta-Critical Implementation Tasks

- [ ] add reporting and confidence foundation to current outputs
- [ ] include untracked files and exclusion classification
- [ ] expose unresolved internal versus external resolution gaps
- [ ] emit exact clone findings from existing duplicate infrastructure
- [ ] add `analysis::semantic` module
- [ ] add `SemanticFrontend` trait
- [ ] implement TypeScript project discovery
- [ ] implement TypeScript symbol extraction
- [ ] implement read/write extraction
- [ ] implement closed-domain extraction
- [ ] implement normalization layer
- [ ] implement minimal concept extraction
- [ ] implement authority and access analyzers
- [ ] implement obligation templates and missing-site detection
- [ ] implement `session_end` patch delta and gate decision
- [ ] benchmark warm and cold runs on `parallel-code`

## Later Implementation Tasks

- [ ] implement contract parity analyzer
- [ ] implement concentration analyzer
- [ ] ingest architecture guardrail tests as rule evidence
- [ ] implement explicit state-domain synthesis
- [ ] implement transition-coverage analysis
- [ ] implement implicit stateful heuristics
