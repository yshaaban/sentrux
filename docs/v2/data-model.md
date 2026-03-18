# Sentrux V2 Data Model

## Design Constraint

V2 must fit the current codebase.

That means:

- keep the existing structural `Snapshot`
- keep the existing `HealthReport`
- add semantic and concept layers beside them
- do not force a full rewrite of the current scan pipeline

## Existing Anchors

Current types that remain valid:

- `core::snapshot::Snapshot`
- `core::types::*`
- `metrics::HealthReport`
- `metrics::rules::RulesConfig`
- `app::mcp_server::McpState`

## Top-Level V2 Analysis Object

```rust
pub struct V2Analysis {
    pub structural: std::sync::Arc<Snapshot>,
    pub project: ProjectModel,
    pub semantic: SemanticSnapshot,
    pub concepts: ConceptGraph,
    pub findings: Vec<Finding>,
    pub obligations: Vec<Obligation>,
    pub session_delta: Option<SessionDelta>,
    pub scorecard: Scorecard,
    pub confidence: ConfidenceReport,
    pub legacy: Option<crate::metrics::HealthReport>,
}
```

The field order is intentional.

V2 should assemble and expose patch-safety outputs before score summaries.

## Project Model

`ProjectModel` describes the repo as a project, not just a folder tree.

```rust
pub struct ProjectModel {
    pub root: String,
    pub primary_language: Option<String>,
    pub language_roots: Vec<LanguageRoot>,
    pub workspaces: Vec<WorkspaceUnit>,
    pub config_files: Vec<ConfigFile>,
    pub exclusions: ExclusionSet,
    pub rules: Option<RulesV2>,
    pub architecture_tests: Vec<ArchitectureGuardrail>,
}
```

Key roles:

- discover TS project boundaries
- load config and rules
- classify excluded directories
- discover architecture guardrail tests

## Semantic Snapshot

`SemanticSnapshot` holds compiler-backed facts and normalized static observations.

```rust
pub struct SemanticSnapshot {
    pub frontend: FrontendKind,
    pub symbols: Vec<SymbolFact>,
    pub references: Vec<ReferenceFact>,
    pub imports: Vec<ImportFact>,
    pub calls: Vec<CallFact>,
    pub writes: Vec<WriteFact>,
    pub reads: Vec<ReadFact>,
    pub closed_domains: Vec<ClosedDomain>,
    pub mapping_sites: Vec<MappingSite>,
    pub adapter_sites: Vec<AdapterSite>,
    pub protocol_contracts: Vec<ProtocolContract>,
    pub state_models: Vec<StateModel>,
    pub clone_families: Vec<CloneFamily>,
}
```

`clone_families` is a first-class fact set because clone drift is part of the initial wedge, not a later add-on.

## Symbol Facts

Symbols are the stable semantic building blocks.

```rust
pub struct SymbolFact {
    pub id: SymbolId,
    pub file: String,
    pub export_name: Option<String>,
    pub local_name: String,
    pub kind: SymbolKind,
    pub span: Span,
    pub type_summary: Option<String>,
}
```

Examples:

- function
- variable
- type alias
- interface
- union
- enum-like constant array
- object literal registry

## Read And Write Facts

These are the foundation for authority and canonical-access metrics.

```rust
pub struct WriteFact {
    pub file: String,
    pub span: Span,
    pub target: AccessPath,
    pub write_kind: WriteKind,
    pub layer_hint: Option<String>,
    pub concept_hint: Option<String>,
}

pub struct ReadFact {
    pub file: String,
    pub span: Span,
    pub target: AccessPath,
    pub read_kind: ReadKind,
    pub layer_hint: Option<String>,
    pub concept_hint: Option<String>,
}
```

Examples of `WriteKind`:

- store mutation
- reducer write
- persistence hydration
- event application
- backend authority emission
- polling refresh write

Examples of `ReadKind`:

- raw store read
- adapter read
- projection read
- component render read

## Closed Domains

Closed domains are where static obligation completeness becomes possible.

```rust
pub struct ClosedDomain {
    pub id: ClosedDomainId,
    pub symbol_id: Option<SymbolId>,
    pub file: String,
    pub kind: ClosedDomainKind,
    pub variants: Vec<ClosedVariant>,
    pub protection: ExhaustivenessProtection,
}
```

Examples:

- string literal unions
- `as const` arrays
- enum-like object keys
- registry category lists

Protection captures:

- exhaustive `switch`
- `assertNever`
- `Record<Union, ...>`
- `satisfies`

## Protocol Contracts

Protocol contracts represent static cross-boundary agreements.

```rust
pub struct ProtocolContract {
    pub id: String,
    pub concept_id: Option<String>,
    pub categories_symbol: Option<SymbolId>,
    pub payload_map_symbol: Option<SymbolId>,
    pub browser_paths: Vec<CodePath>,
    pub electron_paths: Vec<CodePath>,
    pub backend_paths: Vec<CodePath>,
    pub versioning_paths: Vec<CodePath>,
}
```

Examples:

- bootstrap categories
- event payload maps
- IPC contracts
- browser/Electron runtime listeners

## State Models

State models can be explicit or inferred.

```rust
pub struct StateModel {
    pub id: String,
    pub file: String,
    pub kind: StateModelKind,
    pub state_domain: Option<ClosedDomainId>,
    pub transition_sites: Vec<Span>,
    pub guard_sites: Vec<Span>,
    pub explicitness: ExplicitnessLevel,
}
```

Kinds:

- explicit reducer
- explicit controller
- explicit discriminated-state machine
- inferred implicit lifecycle
- inferred boolean-coordination model

Suggested `ExplicitnessLevel` values:

- `DeclaredExplicit`
- `StructuralExplicit`
- `Mixed`
- `Implicit`

Interpretation:

- `DeclaredExplicit`: a named controller or reducer with a closed domain and explicit transition handling
- `StructuralExplicit`: explicit closed-domain state handling exists, but not as a strongly named controller abstraction
- `Mixed`: explicit state handling exists, but is mixed with flags, timers, or auxiliary coordination paths
- `Implicit`: lifecycle behavior is inferred from flags, timers, maps, or generations without an explicit state model

## Concept Graph

`ConceptGraph` is the core v2 abstraction.

```rust
pub struct ConceptGraph {
    pub concepts: Vec<Concept>,
    pub ownership_edges: Vec<OwnershipEdge>,
    pub derivation_edges: Vec<DerivationEdge>,
    pub boundary_edges: Vec<BoundaryEdge>,
    pub obligation_templates: Vec<ObligationTemplate>,
}
```

Each `Concept` should answer:

- what this concept is
- where it is anchored
- who writes it
- who reads it canonically
- which contracts belong to it
- which closed domains belong to it
- what its expected obligations are

```rust
pub struct Concept {
    pub id: String,
    pub kind: ConceptKind,
    pub summary: String,
    pub anchors: Vec<ConceptAnchor>,
    pub authoritative_writers: Vec<CodePath>,
    pub canonical_accessors: Vec<CodePath>,
    pub raw_state_paths: Vec<AccessPath>,
    pub closed_domains: Vec<ClosedDomainId>,
    pub contracts: Vec<String>,
    pub state_models: Vec<String>,
    pub related_tests: Vec<String>,
    pub source: ConceptSource,
}
```

`ConceptSource` indicates whether the concept came from:

- rules
- inference
- both

## Scorecard Types

```rust
pub struct Scorecard {
    pub core_tracks: Vec<TrackScore>,
    pub context_tracks: Vec<TrackScore>,
    pub future_tracks: Vec<TrackScore>,
}

pub struct TrackScore {
    pub id: String,
    pub label: String,
    pub score_0_10000: u32,
    pub numerator: u32,
    pub denominator: u32,
    pub confidence_0_10000: u32,
    pub excluded_count: u32,
}
```

The grouping matters:

- `core_tracks` support the beta wedge
- `context_tracks` provide prioritization and repo background
- `future_tracks` reserve space for later analyzers without forcing them into beta

## Findings

```rust
pub struct Finding {
    pub id: String,
    pub kind: FindingKind,
    pub severity: Severity,
    pub concept_id: Option<String>,
    pub summary: String,
    pub evidence: Vec<EvidenceRef>,
    pub confidence_0_10000: u32,
    pub fix_cost: FixCost,
    pub introduced_in_patch: bool,
    pub likely_fix_sites: Vec<RequirementSite>,
}
```

Evidence should point to:

- file path
- symbol or access path
- short explanation of why it triggered

## Obligations

```rust
pub struct Obligation {
    pub id: String,
    pub concept_id: String,
    pub trigger: ObligationTrigger,
    pub required_sites: Vec<RequirementSite>,
    pub satisfied_sites: Vec<RequirementSite>,
    pub missing_sites: Vec<RequirementSite>,
    pub confidence_0_10000: u32,
}
```

This structure allows both repo-wide and patch-specific obligation views.

## Session Delta

```rust
pub struct SessionDelta {
    pub changed_files: Vec<String>,
    pub changed_concepts: Vec<String>,
    pub introduced_findings: Vec<String>,
    pub resolved_findings: Vec<String>,
    pub missing_obligations: Vec<String>,
    pub track_deltas: Vec<TrackDelta>,
    pub touched_concept_regressions: Vec<String>,
    pub gate: GateDecision,
}

pub struct TrackDelta {
    pub id: String,
    pub before_0_10000: Option<u32>,
    pub after_0_10000: u32,
}
```

This is the object that should power `session_end` and CI ratchets.

## Confidence Report

```rust
pub struct ConfidenceReport {
    pub analysis_coverage_0_10000: u32,
    pub semantic_coverage_0_10000: u32,
    pub rules_coverage_0_10000: u32,
    pub heuristic_ratio_0_10000: u32,
    pub unresolved_internal_ratio_0_10000: u32,
    pub excluded_scope_ratio_0_10000: u32,
}
```

Confidence is not optional. Every top-level response should include it.

## Caching Model

V2 should use project-local cache files under:

`.sentrux/cache/v2/`

Suggested storage split:

- SQLite for indexes and run metadata
- compressed binary or JSON blobs for per-file semantic facts

Suggested cache keys:

- file hash
- tsconfig hash
- rules hash
- Sentrux version

## Compatibility Strategy

No current v1 type needs to be deleted.

Minimal state additions to MCP server:

```rust
pub struct McpState {
    // existing fields...
    pub cached_v2: Option<V2Analysis>,
}
```

That is the preferred initial integration point.
