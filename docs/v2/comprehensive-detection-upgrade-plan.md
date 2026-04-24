# Comprehensive Detection Upgrade Plan

Last audited: 2026-04-24

## Purpose

This document turns the latest field review against `crew-mail` into a concrete Sentrux upgrade plan.

The goal is not to hardcode `crew-mail` behavior into the product. The goal is to use that project as a field sample of issue families Sentrux should catch across similar TypeScript, JavaScript, Python, and Rust codebases:

- incomplete enum, union, DTO, and workflow propagation
- generic fallback branches that hide stale variants
- non-clickable or stale UI/action links caused by partial mapping updates
- broad composition roots and review-hostile concentration
- large-file pressure that needs actionable extraction guidance
- cycle reports that mix import evidence and looser interaction evidence
- duplicated helper logic across quality/tooling scripts
- missing repository governance setup that prevents Sentrux from becoming a repeatable gate

This plan complements:

- [Master Plan](./master-plan.md)
- [Completion Execution Tracker](./completion-execution-tracker.md)
- [Completion Execution Model](./experiments/completion-execution-model.md)
- [Policy And Eval Architecture](./policy-and-eval-architecture.md)
- [Testing And Validation](./testing-and-validation.md)

## Product Bar

Sentrux should detect these issue families comprehensively enough that a coding agent can ask:

1. What did this patch make incomplete?
2. What else must change with it?
3. Which finding is worth fixing first?
4. Where is the smallest safe repair surface?
5. How do I verify the repair?

The default agent lane should stay small. More detector coverage must not produce a larger warning wall. Breadth belongs in candidate generation and maintainer watchpoints; only high-confidence, repairable, causal drift should lead.

## Execution Status

Updated: 2026-04-24

Status in this table is implementation status, not completion status. A phase is complete only after the [completion execution model](./experiments/completion-execution-model.md) gate passes: checked-in implementation, checked-in evidence, outcome lift, regression guard, and a written decision record.

| Phase | Status | Implemented artifacts | Remaining evidence gate |
| --- | --- | --- | --- |
| Phase 0: Field-repro fixture corpus | Partial implementation | TS bridge fixtures now cover fallback-masked switches, if/else chains, inferred object maps, `Map` constructors, and JSX conditionals; structural fixtures cover large-file first cuts and cycle edge basis; governance fixtures cover missing rules and baseline setup | Broader cross-language fixture parity for Python/Rust equivalents and additional public-safe field apps |
| Phase 1: TypeScript semantic coverage | Implemented first pass | Bridge emits `fallback_kind`, `site_expression`, `site_semantic_role`, `site_confidence`, if/else sites, inferred maps, `Map` constructor sites, and JSX conditional render sites | More JSX branch forms, framework-specific route/action maps, and real-repo false-positive calibration |
| Phase 2: Broader obligation graph | Partial implementation | Contract obligations already classify registry, public API, DTO, command-status, config, test, and doc surfaces; closed-domain obligations now consume richer site metadata | Changed member-level precision, inferred sibling discovery, and non-TypeScript DTO/config parity |
| Phase 3: Repair packet standardization | Implemented | Repair packets now expose title, why-it-matters, concrete evidence, likely fix sites, first cut, verification steps, over-refactor guardrails, confidence reason, and lane reason while preserving legacy fields | Empirical proof that each field improves agent follow-through |
| Phase 4: Structural signal precision | Implemented first pass | Large-file reports include suggested split axes, related surfaces, first-cut extraction, and admissibility evidence; cycle reports split import-only, interaction-only, and mixed edge basis with representative edges and cut candidates | Treatment proof that large-file lead eligibility improves repair behavior without crowding causal signals |
| Phase 5: Governance readiness | Implemented | `check` and `findings` surface missing rules, missing baselines, and invalid baselines as non-blocking onboarding watchpoints with starter setup guidance | CI evidence detection and organization-specific setup policies |
| Phase 6: Ranking and lane policy | Implemented and revalidated | Default-lane selection remains capped at 1-3 actions, demotes broad structural pressure unless patch-worsened and repairable, and uses follow/help/remediation/disagreement/repair evidence tie-breakers | Cross-repo experiment results for the final family mix and `large_file` admissibility |
| Phase 7: Bounded LLM adjudication | Scaffold implemented, intentionally parked | Structured evidence bundle schema, advisory-only adjudication, audit logging, and tests already exist | Positive static-plus-LLM effect size before any production reranking or suppression |
| Phase 8: Evaluation program | Infrastructure implemented, experiments pending | Phase-6 experiment registry, fixed repo/task matrix, review rubric, promotion ledger, scorecards, and treatment-vs-baseline metrics are checked in and tests pass | Actual paired runs on Sentrux, parallel-code, one-tool, and field-style apps |

## Non-Goals

- Do not make Sentrux a generic linter.
- Do not promote broad structural pressure just because it is true.
- Do not use LLMs to scan raw repositories end to end.
- Do not treat `crew-mail` paths, repo names, or domains as product rules.
- Do not require every large file to be split immediately.

## Target Coverage Matrix

| Issue family | Current behavior | Needed behavior | Default lane rule |
| --- | --- | --- | --- |
| Closed-domain switch gaps | Detects many `switch`, `Record`, and `satisfies` sites | Detect fallback-masked gaps, if/else variant chains, nullable target maps, object maps, and JSX branch maps | Lead when missing variants affect changed or directly actionable surfaces |
| Evidence/action target drift | Detected indirectly as closed-domain gaps | Explain that specific links become static, generic, or non-clickable | Lead when the repair is one mapping/helper file plus tests |
| Workflow/status label drift | Detects missing switch coverage | Distinguish harmless textual fallback from stale state semantics and require explicit labels for public UI/status surfaces | Lead only when user-facing or exported behavior is affected |
| DTO/config/registry/API propagation | Partly implemented through contract obligations | Add broader symbol precision, sibling surface matching, test/doc follow-through, and likely verification surfaces | Lead when a changed symbol has concrete missing consumers |
| Large file concentration | Detects line count and rough split axes | Produce first-cut extraction packets based on symbol clusters, churn, fan-in/fan-out, complexity, and nearby tests | Lead only when patch-worsened or highly actionable; otherwise maintainer lane |
| Composition-root sprawl | Detects broad fan-out | Name shell/view/runtime/demo seams and first extraction route | Usually watchpoint; lead only when patch directly widened root responsibilities |
| Cycle clusters | Reports graph cycles from combined graph | Split import cycles from interaction/call cycles and show edge basis | Import cycles may lead; interaction cycles are watchpoints unless patch-worsened |
| Clone/helper drift | Detects clone families | Classify exact helper clone, sibling update drift, and tooling-helper centralization opportunity | Lead when session-introduced or one-sided update; otherwise secondary cleanup |
| Governance readiness | Missing rules/baseline shown as command failure | Report setup debt as a normal actionable finding with starter files and CI guidance | Visible as onboarding guidance; do not occupy the primary patch-repair action lane |
| Build/performance artifact drift | Not a core Sentrux signal | Optionally ingest build/test logs or configured artifact reports for bundle-size and gate-drift evidence | Never lead unless supplied artifact proves a patch regression |

## Phase 0: Field-Repro Fixture Corpus

Status: partial implementation

Objective: encode the field issues as minimal synthetic fixtures so Sentrux covers the issue family without coupling to `crew-mail`.

Implementation steps:

1. Add TypeScript fixture repos for:
   - switch over `link.type` returning `null` for omitted variants
   - label helper returning generic `"Open evidence"` for omitted target variants
   - lifecycle label helper using `state.replace(/-/g, " ")`
   - `KnowledgeSubview`-style default branch that intentionally maps several variants to one output
   - object lookup table without `Record<Union, Value>`
   - if/else chain over discriminated union variants
   - JSX branch rendering over status/variant values
2. Add structural fixture repos for:
   - large file with clear symbol clusters and nearby tests
   - large file with no safe first cut
   - composition root that directly grows via a patch
   - composition root that is broad but unchanged
   - import-cycle-free repo where call graph creates an interaction cycle
   - true import cycle with a small cut candidate
3. Add clone/tooling fixtures for:
   - exact helper clone across two scripts
   - one-sided helper update in one sibling script
   - duplicate validation helpers that should centralize
4. Add governance fixtures for:
   - no `.sentrux/rules.toml`
   - no `.sentrux/baseline.json`
   - stale baseline with incompatible git head
   - repo with gate wired in CI

Likely test sites:

- [sentrux-core/src/metrics/v2/obligations_domain_tests.rs](../../sentrux-core/src/metrics/v2/obligations_domain_tests.rs)
- [sentrux-core/src/metrics/v2/structural/tests_overview.rs](../../sentrux-core/src/metrics/v2/structural/tests_overview.rs)
- [ts-bridge/test/bridge.test.mjs](../../ts-bridge/test/bridge.test.mjs)
- `scripts/tests/*v2-report-selection*.mjs`

Acceptance criteria:

- Each field issue family has at least one failing fixture before implementation and one passing fixture after implementation.
- Fixtures use synthetic names and paths.
- No fixture asserts on `crew-mail` strings.

## Phase 1: TypeScript Semantic Coverage Expansion

Status: implemented first pass

Objective: close the largest engine-quality gap: missing follow-through surfaces hidden by fallback logic.

Implementation steps:

1. Extend closed-domain site collection in [analysis-closed-domains.ts](../../ts-bridge/src/analysis-closed-domains.ts):
   - classify fallback behavior for `default` clauses
   - record whether fallback returns `null`, generic string, identity transform, `undefined`, empty array, empty object, or throws/asserts
   - collect `if/else` chains comparing the same expression to literal variants
   - collect object literal maps whose keys are variant literals even without `Record`
   - collect `Map` constructors with literal variant keys
   - collect JSX conditional branches where a variant controls rendered actions or labels
2. Extend the bridge data model in [types.ts](../../ts-bridge/src/types.ts):
   - add `fallback_kind`
   - add `site_expression`
   - add `site_semantic_role` such as `label`, `target`, `status`, `render`, `handler`, `policy`, `serialization`
   - add `site_confidence`
3. Improve domain matching:
   - preserve discriminant property name for discriminated unions
   - preserve defining file and exported symbol when available
   - distinguish alias domains from anonymous inline unions
4. Update [obligations_domain.rs](../../sentrux-core/src/metrics/v2/obligations_domain.rs):
   - treat fallback-masked variants as missing when the site is user-facing, exported, or target-producing
   - downgrade generic textual fallbacks when the site is internal and unchanged
   - emit missing-site details that name both missing variants and behavioral impact
5. Add repair guidance:
   - for target mappings: "add explicit case or intentionally static handling"
   - for label helpers: "add explicit label cases"
   - for lifecycle/status helpers: "add explicit public label mapping"
   - for intentional grouped variants: "replace default with explicit grouped cases"

Acceptance criteria:

- Sentrux identifies fallback-masked missing variants without requiring `Record`.
- It can distinguish "intentional grouped mapping" from "unknown future default" when all variants are explicit.
- Findings point to concrete line numbers and behavioral impact, not just the domain declaration.

## Phase 2: Broader Obligation Graph

Status: partial implementation

Objective: improve "what else must change?" beyond closed domains.

Implementation steps:

1. Add obligation surface types:
   - DTO/interface fields
   - config keys
   - registry entries
   - command/status/event surfaces
   - public exported functions
   - route/action handlers
   - schema fixtures and seed generators
2. Add changed-symbol precision:
   - track changed exported symbol names, not only changed files
   - associate changed type members with consumers
   - distinguish symbol definition changes from test-only changes
3. Add sibling-surface discovery:
   - same basename across `runtime`, `store`, `components`, `tests`, `docs`
   - same exported symbol stem across adjacent modules
   - config/schema/fixture triplets
4. Add test and doc follow-through:
   - likely tests by import path
   - likely docs by path/name overlap
   - likely seed/fixture updates for data-shape changes
5. Improve obligation output:
   - missing fix sites
   - verification surfaces
   - confidence reason
   - whether the obligation is patch-local or repo-onboarding-only

Likely implementation sites:

- [obligations.rs](../../sentrux-core/src/metrics/v2/obligations.rs)
- [obligations_contract.rs](../../sentrux-core/src/metrics/v2/obligations_contract.rs)
- [concept_match.rs](../../sentrux-core/src/metrics/v2/concept_match.rs)
- [typescript.rs](../../sentrux-core/src/analysis/semantic/typescript.rs)

Acceptance criteria:

- A changed DTO/config/registry/public API produces a concrete, bounded list of likely consumers.
- Missing tests and docs are reported only when evidence ties them to the changed concept.
- Obligation findings remain small enough for the default lane.

## Phase 3: Repair Packet Standardization

Status: implemented

Objective: every default-lane finding should be executable by a coding agent.

Required packet fields:

- `title`
- `why_it_matters`
- `concrete_evidence`
- `likely_fix_sites`
- `smallest_safe_first_cut`
- `verification_steps`
- `what_not_to_over_refactor`
- `confidence_reason`
- `lane_reason`

Implementation steps:

1. Add a canonical repair packet model to the v2 output schema.
2. Populate repair packets for:
   - closed-domain exhaustiveness
   - contract obligations
   - clone drift
   - boundary bypass
   - large file first-cut extraction
   - setup/governance readiness
3. Update report selection so default-lane candidates without repair packets are demoted unless they are explicit hard blockers.
4. Update check-review packet tests to enforce field completeness.

Likely implementation sites:

- `sentrux-core/src/app/mcp_server/handlers/agent_guidance_packets.rs`
- `sentrux-core/src/app/mcp_server/handlers/check.rs`
- [buckets.mjs](../../scripts/lib/v2-report-selection/buckets.mjs)
- [check-review-packet-format.mjs](../../scripts/lib/check-review-packet-format.mjs)

Acceptance criteria:

- Top 1-3 actions always include repair packets.
- Repair packet completeness is measured in scorecards.
- A candidate cannot lead solely because it has high structural severity.

## Phase 4: Structural Signal Precision

Status: implemented first pass

Objective: preserve useful structural signals while preventing misleading or non-actionable reports.

### Large File

Implementation steps:

1. Segment files into top-level symbol clusters:
   - exported functions/classes/components
   - local helper groups
   - constants/config blocks
   - render sections
2. Compute per-cluster facts:
   - lines
   - complexity
   - fan-in/fan-out
   - churn
   - nearest tests
   - outgoing dependency categories
3. Generate first-cut extraction candidates:
   - "extract pure helper group"
   - "move adapter/parser phase"
   - "split render subcomponent"
   - "move entry orchestration behind hook/facade"
4. Add admissibility rules:
   - default-lane lead if patch-worsened and first-cut confidence is high
   - default-lane lead if churn, complexity, and tests make the split safe
   - maintainer watchpoint otherwise

Likely implementation sites:

- [large_file_reports.rs](../../sentrux-core/src/metrics/v2/structural/large_file_reports.rs)
- [report_common.rs](../../sentrux-core/src/metrics/v2/structural/report_common.rs)
- [scoring.rs](../../sentrux-core/src/metrics/v2/structural/scoring.rs)

### Composition Root Sprawl

Implementation steps:

1. Detect shell/view/runtime/demo/store categories in fan-out.
2. Identify whether the patch added a new dependency category.
3. Suggest first extraction route:
   - view orchestration hook
   - lazy-loaded panel
   - demo/admin boundary
   - runtime facade
4. Keep broad but unchanged composition roots in the maintainer lane.

Likely implementation sites:

- [dependency_reports.rs](../../sentrux-core/src/metrics/v2/structural/dependency_reports.rs)
- [path_roles.rs](../../sentrux-core/src/metrics/v2/structural/path_roles.rs)

### Cycles

Implementation steps:

1. Split cycle analysis by edge basis:
   - import-only cycle
   - call/interaction cycle
   - mixed cycle
2. Include representative edges and cut candidates for each cycle.
3. Downgrade interaction-only cycles to watchpoints unless patch-worsened.
4. Prefer small high-confidence import cycles over huge mixed clusters.

Likely implementation sites:

- [graph.rs](../../sentrux-core/src/metrics/v2/structural/graph.rs)
- `sentrux-core/src/metrics/v2/structural/cycles.rs`

Acceptance criteria:

- A simple import resolver and Sentrux agree on import-cycle presence for fixture repos.
- Huge interaction clusters no longer crowd default lead actions.
- Large-file findings include a concrete first-cut extraction candidate or stay out of the default lane.

## Phase 5: Governance Readiness Signals

Status: implemented

Objective: make setup gaps visible as actionable product findings instead of terminal command failures.

Implementation steps:

1. Add findings for:
   - missing `.sentrux/rules.toml`
   - missing `.sentrux/baseline.json`
   - incompatible or stale baseline
   - no CI evidence for Sentrux gate
   - no test/build command hints
2. Emit starter repair packets:
   - create starter rules
   - run `sentrux gate --save`
   - add CI command
   - document baseline update policy
3. Keep these in onboarding mode by default.
4. In patch/pre-merge mode, show them as watchpoints unless they block the requested gate.

Likely implementation sites:

- `sentrux-bin/src/main_impl/commands.rs`
- `sentrux-core/src/app/mcp_server/handlers/check.rs`
- `sentrux-core/src/app/mcp_server/handlers/findings/findings_tool.rs`

Acceptance criteria:

- A repo without rules/baseline gets a clear setup action, not just a failed command.
- Onboarding mode can produce a complete "make Sentrux enforceable here" packet.

## Phase 6: Ranking And Lane Policy

Status: implemented and revalidated

Objective: default-lane outputs stay small, causal, and repairable as detector breadth increases.

Implementation steps:

1. Update signal cohort metadata:
   - keep obligation, clone, boundary, and direct rule violations as lead-capable
   - make large-file lead-capable only with an admissibility packet
   - keep cycles and broad sprawl as watchpoints unless patch-worsened
2. Add evidence-based tie-breakers:
   - reviewed precision
   - follow rate
   - help rate
   - remediation success
   - reviewer disagreement
   - repair packet completeness
   - patch expansion cost
3. Add demotion rules:
   - structurally true but non-repairable
   - repeated low follow/help rate
   - no concrete fix site
   - no verification path
4. Enforce default lane invariants:
   - maximum 1-3 primary actions
   - no more than one broad structural lead
   - no structural lead without direct patch worsening or high-confidence extraction packet
   - no candidate with missing repair packet unless hard blocker

Likely implementation sites:

- [signal-cohorts.mjs](../../scripts/lib/signal-cohorts.mjs)
- [buckets.mjs](../../scripts/lib/v2-report-selection/buckets.mjs)
- [ranking.mjs](../../scripts/lib/v2-report-selection/ranking.mjs)
- [compare.mjs](../../scripts/lib/v2-report-selection/compare.mjs)

Acceptance criteria:

- Default lane remains 1-3 actions across Sentrux, parallel-code, one-tool, and field-sample repos.
- Structural signals remain visible but do not dominate unless their repair packets are strong.

## Phase 7: Bounded LLM Adjudication

Status: scaffold implemented, evidence-gated

Objective: use an LLM only after deterministic narrowing, and only where it improves actionability.

Allowed uses:

- rerank ambiguous top candidates
- suppress likely low-value structural findings
- improve repair and verification guidance
- classify whether a fallback is intentional grouping or stale omission

Prohibited uses:

- raw repo-wide scanning
- unsupported architecture judgment
- inventing findings without deterministic evidence
- silent production reranking before treatment proof

Implementation steps:

1. Define structured evidence bundle fields:
   - candidate finding
   - code evidence snippets
   - changed symbols
   - related tests
   - competing candidates
   - deterministic confidence
2. Add audit logs:
   - input hash
   - output hash
   - model
   - latency
   - decision
   - cited evidence IDs
3. Keep LLM output advisory until experiments prove lift.
4. Compare static-only vs static-plus-adjudication on top-action help rate.

Likely implementation sites:

- [adjudication.mjs](../../scripts/lib/eval-runtime/provider-task-runner/adjudication.mjs)
- [task-schemas.mjs](../../scripts/lib/eval-runtime/provider-task-runner/task-schemas.mjs)
- [results.mjs](../../scripts/lib/eval-runtime/provider-task-runner/results.mjs)

Acceptance criteria:

- LLM adjudication improves top-action help rate enough to justify latency and cost.
- Audit logs make every rerank/suppression decision reproducible.
- No default-on LLM decision path ships without paired treatment proof.

## Phase 8: Evaluation Program

Status: infrastructure implemented, evidence-gated

Objective: prove the upgraded signals help agents produce better patches.

Experiment lanes:

1. Default-lane family ablation
   - compare current policy vs obligation-only vs obligation+clone+boundary vs structural-admissible
2. Large-file admissibility
   - compare large-file watchpoint-only vs lead-capable with repair packet
3. Obligation breadth expansion
   - compare current closed-domain/contract obligations vs expanded DTO/config/registry/API obligations
4. Cycle confidence split
   - compare combined-cycle reports vs import/interaction split
5. LLM adjudication advisory
   - compare static repair packets vs static-plus-bounded adjudication

Required metrics:

- task success
- escaped regression count
- top-action follow rate
- top-action help rate
- false-positive rate
- reviewer disagreement
- repair packet completeness
- remediation success
- patch expansion cost

Required repos:

- Sentrux
- parallel-code
- one-tool
- at least one field-sample-style app repo with large UI/runtime/data surfaces

Acceptance criteria:

- Promotions and demotions are made from signal-level effect size, not repo-level anecdotes.
- Default-on candidates show positive top-action help rate and acceptable disagreement.
- Large-file stays lead-capable only if the admissibility rule wins.

## Implementation Order

1. Phase 0 fixtures.
2. Phase 1 fallback-masked closed-domain detection.
3. Phase 3 repair packet schema and completeness enforcement.
4. Phase 4 cycle edge-basis split and large-file first-cut packets.
5. Phase 5 governance readiness findings.
6. Phase 6 ranking/lane policy update.
7. Phase 2 broader obligation graph.
8. Phase 7 bounded LLM adjudication.
9. Phase 8 evaluation and promotion decisions.

Reasoning:

- Fixtures must come first so coverage expansion does not drift into bespoke patches.
- Closed-domain fallback gaps are the sharpest field-proven issue class.
- Repair packets are required before broader signals become more visible.
- Structural precision must improve before `large_file`, cycle, and sprawl can safely remain useful.
- Broader obligations and LLM adjudication should wait until the narrower deterministic loop is stable.

## Definition Of Done

This plan is complete when:

1. Synthetic fixtures cover every issue family in the target matrix.
2. `crew-mail`-style findings are reproduced by generic detectors without repo-specific rules.
3. Default-lane output stays within 1-3 actions across proof repos.
4. Closed-domain fallback gaps include exact missing variants, behavioral impact, fix sites, and verification steps.
5. Large-file findings include actionable first-cut extraction packets or remain in the maintainer lane.
6. Cycle reports clearly distinguish import cycles from interaction cycles.
7. Governance setup gaps are actionable onboarding findings.
8. Signal-level experiments justify every default-lane promotion.
9. Top-action help rate improves versus current policy.

Operationally, each item above must be closed through the rubric in [Completion Execution Model](./experiments/completion-execution-model.md):

- implementation artifacts are checked in
- evidence artifacts are checked in or generated by a documented repeatable run
- the result improves agent repair outcomes rather than only detector coverage
- the relevant keep, constrain, demote, or park decision is recorded under `docs/v2/experiments/decisions/`
- [Completion Execution Tracker](./completion-execution-tracker.md) is updated with the implementation checkpoint, evidence checkpoint, open gate, next proof artifact, and decision record

The current plan state is therefore not complete. Several implementation rows are marked implemented or first-pass implemented, but no active completion decision records are checked in yet.

## Watchpoints

- More semantic coverage can increase false positives unless ranking and repair-packet gates are tightened at the same time.
- Large-file is useful, but only when Sentrux can name a safe first cut. Otherwise it becomes maintainer pressure, not an agent intervention.
- Cycle detection must be honest about edge basis. Mixed graphs are useful context but weak blockers.
- LLM adjudication should be measured as a product intervention, not assumed helpful.

## Do Not Chase First

- Repo-wide architecture criticism without patch-local repair leverage.
- Style-only findings.
- Perfect language parity before TypeScript field gaps are closed.
- Automatic large-file refactors.
- Raw build-log parsing before deterministic code and artifact inputs are stable.
