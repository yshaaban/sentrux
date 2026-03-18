# Sentrux V2 Roadmap

Last audited: 2026-03-19

Status legend:

- `[ ]` not started
- `[-]` in progress or partially complete
- `[x]` complete

This roadmap tracks implementation work, not just design ideas.

It is organized by ROI and dependency, not by the full analyzer wish list.

For a detailed current-state assessment, see [Implementation Status](./implementation-status.md).

## Strategy

Build order for v2:

1. trust and output framing
2. patch-safety wedge
3. context and adoption
4. advanced static analysis

If work does not improve one of the first two tiers, it is not on the critical path.

## Tier 0: Trust And Output Foundation

Status: mostly complete

Goal:

- make the current product honest and useful before deep semantics land

Deliverables:

- tracked plus untracked scan scope
- vendor/generated/build classification
- internal versus external unresolved split
- confidence reporting
- bottleneck-first and delta-first output framing

Tasks:

- [x] include untracked files in scan scope
- [x] classify excluded files into vendor/generated/build/fixture/cache buckets
- [x] split unresolved imports into internal failures versus external expected references
- [x] expose exclusions and unresolved-internal count in MCP
- [-] add confidence report type
- [x] add confidence fields to scan responses
- [x] promote the worst bottleneck to the lead field in health responses
- [x] expose root-cause family scores ahead of the composite score
- [x] auto-load baseline data when available during scan and health flows
- [-] surface baseline deltas inline in health and session summaries
- [-] demote the single composite score in CLI, MCP, and GUI output

Exit criteria:

- scan reports what was excluded and why
- scan reports confidence-related scope gaps directly
- agents see bottleneck and delta before they see a single composite number

Open gap:

- MCP is ahead of CLI and GUI here

This tier should ship independently of the semantic frontend.

## Tier 1: Patch-Safety Wedge

Status: mostly complete at the MCP layer

Goal:

- catch the highest-value static regressions in agent-generated patches

This tier is the core of v2.

Deliverables:

- clone-drift findings
- TypeScript semantic substrate
- minimal concept graph
- authority and access findings
- obligation engine
- upgraded `session_end`
- touched-concept CI ratchet

## Tier 1A: Clone Drift Fast Lane

Status: mostly complete

Tasks:

- [x] emit exact clone groups as findings, not just aggregate counts
- [x] filter test-only and tiny clone groups from the beta findings surface
- [x] add stable clone identifiers and evidence payloads
- [x] correlate clone groups with git recency and churn
- [ ] detect recently diverged clone families
- [-] expose clone-drift findings via MCP and CLI
- [ ] collapse repeated clone-family findings into higher-level prioritization where appropriate

Exit criteria:

- plausible clone-drift risks in `parallel-code` are surfaced without the semantic frontend

Open gap:

- current clone drift is now git-aware, but it is still exact-clone-first and not yet divergence-aware or family-collapsed

## Tier 1B: TypeScript Semantic Substrate

Status: mostly complete

Tasks:

- [x] build a TypeScript bridge spike: one Rust request, one Node subprocess, one file worth of `SymbolFact[]`
- [x] add `analysis::semantic` module
- [x] add TypeScript project discovery from `tsconfig.json`
- [x] build persistent Node bridge over stdio
- [x] define bridge protocol version and capability handshake
- [x] build compiler-backed language-service host behind the bridge
- [-] extract symbols and references
- [x] extract reads and writes
- [x] extract unions and literal domains
- [x] extract `switch`, `assertNever`, `satisfies`, and `Record<Union, ...>` patterns
- [-] persist semantic facts in cache
- [x] implement crash recovery and Node-missing fallback behavior
- [x] benchmark cold and warm runs on `parallel-code`

Exit criteria:

- TypeScript semantic analysis runs successfully on `parallel-code`
- semantic reruns are incremental
- warm runs rely on a persistent bridge process, not one-shot invocations

Open gap:

- initial benchmark proof exists, but long-lived cache maturity and a regression benchmark suite are still missing

## Tier 1C: Minimal Concept Graph And Rules

Status: mostly complete

Tasks:

- [x] add `ConceptGraph` types
- [x] add concept extraction from explicit rules
- [x] extend `rules.toml` schema with v2 sections
- [x] add suppression support
- [x] compute rule coverage

Exit criteria:

- `parallel-code` beta concepts can be represented explicitly from rules
- rule coverage is visible for configured concepts

Open gap:

- suppression matching and expiry are implemented, but still need broader validation and policy ergonomics

## Tier 1D: Authority And Access

Status: mostly complete

Tasks:

- [-] detect authoritative concepts
- [x] detect durable write paths
- [x] detect multi-writer concepts
- [x] detect writer-layer violations
- [x] filter test-only writes out of authority findings
- [-] detect canonical accessors
- [x] detect raw authoritative reads in forbidden layers
- [-] detect public-API and barrel bypasses where confidence is sufficient
- [ ] implement authority and access scorecard track
- [x] emit authority and access findings

Exit criteria:

- `parallel-code` surfaces authority drift, direct internal mutation, and canonicalization bypasses statically

Open gap:

- the analyzer works best with explicit critical concept rules and is not yet a full scorecard lane

## Tier 1E: Obligation Engine

Status: mostly complete

Tasks:

- [-] define obligation template model
- [-] derive obligation templates from closed domains and contracts
- [-] map changed files to changed symbols and changed concepts
- [x] compute satisfied versus missing required sites
- [x] derive obligation count and agent-context-burden summaries from required sites
- [x] emit non-exhaustive closed-domain findings through the obligation engine
- [ ] implement obligation completeness scorecard track

Exit criteria:

- adding or changing a closed-domain concept produces a required static update set
- obligation output is useful on `parallel-code`

Open gap:

- obligations are strong for closed domains, but not yet as complete for contracts and richer change triggers

## Tier 1F: Session Delta And CI Gate

Status: mostly complete in MCP, partial overall

Tasks:

- [x] implement `findings` MCP tool
- [x] implement `obligations` MCP tool
- [x] upgrade `session_end` with v2 delta data
- [x] add touched-concept regression verdicts to `session_end`
- [x] add `gate` MCP and CLI surface for high-confidence regressions
- [-] make CI ratchets operate on touched-concept regressions before repo-wide debt

Exit criteria:

- `session_end` tells the agent what changed, what is missing, and whether to fix before merge
- CI can fail new high-confidence regressions on touched concepts

Open gap:

- CLI parity and suppression-aware gate behavior are in place, but release-grade gate validation is still incomplete

## Tier 2: Context And Adoption

Status: mostly complete in code, partially complete in real-world adoption

Goal:

- add prioritization context and make the wedge easier to adopt across repos

Deliverables:

- contract parity
- concentration risk
- concept inspection tools
- rule and guardrail integration
- conservative concept inference

## Tier 2A: Contract Parity

Status: mostly complete

Tasks:

- [-] add protocol contract extraction
- [x] define parity cells
- [x] compare browser, Electron, backend, and snapshot/live-update paths
- [x] add contract parity score as context
- [x] emit parity findings and missing cells
- [x] implement `parity` MCP tool
- [x] validate bootstrap/runtime parity against the scoped `parallel-code` contract

Exit criteria:

- `parallel-code` bootstrap and runtime parity gaps are visible from source only

Open gap:

- the scoped bootstrap contract is now correct on `parallel-code`, but parity still needs another benchmark repo before the heuristic can be called broadly proven

## Tier 2B: Concentration Risk

Status: mostly complete

Tasks:

- [x] compute side-effect breadth
- [x] compute authority breadth
- [x] compute timer and retry weight
- [x] compute async branching weight
- [x] integrate churn weighting
- [x] emit concentration findings

Exit criteria:

- risky hubs like lease and restore controllers rank plausibly

Open gap:

- thresholds and rankings still need case-study validation on `parallel-code`

## Tier 2C: Concept Inspection And Rule Adoption

Status: mostly complete

Tasks:

- [x] implement `concepts` MCP tool
- [x] implement `explain_concept` MCP tool
- [x] implement `trace_symbol` MCP tool
- [x] ingest architecture guardrail tests as rule-coverage evidence or optional rule seeds
- [x] add conservative concept inference from anchors, contracts, and naming convergence
- [x] provide example `rules.toml` for `parallel-code`
- [x] document the small-config onboarding path

Exit criteria:

- users can inspect critical concepts without reading internal implementation details
- architecture tests help coverage without blocking the wedge
- conservative concept inference is additive, not required for beta findings

Open gap:

- the example rules file now works on the real repo, but the proof loop is not closed until goldens, benchmarks, and follow-up analyzer fixes land

## Tier 3: Advanced Static Analysis

Status: partial

Goal:

- extend v2 beyond the wedge once the core product is already useful

Deliverables:

- richer state-integrity analysis
- transition modeling
- implicit lifecycle heuristics

Tasks:

- [x] detect explicit stateful controllers and reducers across files
- [x] recognize discriminated object unions and trailing `assertNever(...)` proofs in the TS bridge
- [ ] model transition sites for explicit state domains
- [ ] detect transition-coverage gaps for explicit state domains
- [ ] infer implicit lifecycle modules from booleans, timers, maps, and generations
- [ ] emit invalid-state-risk findings
- [ ] add `state_integrity` as a future scorecard track

Exit criteria:

- explicit state models are rewarded
- implicit stateful hotspots are visible with acceptable false-positive rates

Open gap:

- the current state analyzer now validates the scoped `parallel-code` controllers, but it is still only a conservative slice of the full Tier 3 plan

## Cross-Cutting Validation Tasks

Status: partial

- [x] create initial scoped golden outputs for `parallel-code`
- [ ] expand `parallel-code` goldens to cover `session_end` and touched-concept gate scenarios
- [-] add synthetic touched-concept gate and `session_end` regression scenarios
- [-] add fixture repos for semantic frontends
- [-] add bridge contract tests for the persistent Node subprocess
- [-] capture initial `parallel-code` benchmark artifact
- [-] add performance regression benchmarks
- [ ] add false-positive review process for new heuristics
- [-] add confidence-report regression tests
- [-] add v1/v2 baseline migration tests
- [x] verify case-study examples against the current target repo before docs or demos cite them

Open gap:

- implementation is ahead of proof, benchmarking, and release-grade validation

## Target Outcome On `parallel-code`

By the end of Tier 1, v2 should be able to show:

- clone-drift risks in copied helpers and parser logic
- dual-writer or cross-layer ownership drift on important concepts
- raw bypasses of canonical access or public store boundaries
- missing required updates when closed-domain concepts change
- a useful `session_end` report for agent patches

Beta concept scope should be limited to:

- `task_git_status`
- `task_presentation_status`
- `server_state_bootstrap`

By the end of Tier 2, v2 should additionally show:

- bootstrap and runtime parity gaps
- concentration hotspots worth architectural attention
- inspectable concept models and rule coverage
- expansion into `task_command_controller` and `task_convergence`

Tier 3 should improve the repo story, but it is not required for the wedge to be useful.

Current reality:

- the codebase is close to a useful MCP beta
- the real `parallel-code` proof loop is partially closed, and the first scoped goldens have already identified the next analyzer corrections
