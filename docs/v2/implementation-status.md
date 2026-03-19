# Sentrux V2 Implementation Status

Last audited: 2026-03-19

## Overall Verdict

The implementation is in a good place relative to the **core v2 wedge**, but it is not done relative to the **full roadmap**.

Current assessment:

- doctrine alignment: strong
- core patch-safety wedge: strongly implemented
- full roadmap: partially implemented
- cross-cutting proof and validation: still behind implementation

Working estimate:

- core wedge completion: about 85-90%
- full roadmap completion: about 65-70%
- validation and proof completion: about 50-55%

## What Is True Today

The product shape in code matches the doctrine in [doctrine.md](./doctrine.md):

- patch safety is primary
- findings, obligations, and session delta are primary MCP outputs
- TypeScript-first deep analysis is real
- concept-level findings are mostly rule-driven
- score summaries are secondary context, not the main narrative

The strongest completed work is:

- trust and scan-scope reporting
- TypeScript semantic substrate
- concept rules and rule coverage
- suppression enforcement with expiry-aware visibility
- authority/access findings
- obligation engine
- upgraded `session_end`
- touched-concept `gate` in MCP and CLI
- shared patch-safety analysis reuse across `gate` and `session_end`
- parity and concentration context
- conservative state-integrity analysis

## Overall Status By Tier

| Tier | Status | Assessment | Main Gap |
| --- | --- | --- | --- |
| Tier 0 | Mostly complete | Trust foundation is real in MCP | health/CLI/GUI productization is still uneven |
| Tier 1A | Mostly complete | Clone drift findings now have stable ids, git-aware risk context, deterministic instance ordering, and cleaner production-first ranking | no divergent-clone lane or family-level prioritization yet |
| Tier 1B | Mostly complete | TS bridge, semantic facts, and initial `parallel-code` benchmark proof are real | no regression benchmark suite and no mature persisted cache story yet |
| Tier 1C | Mostly complete | v2 rules, concept graph, and suppression enforcement now exist | broader policy UX and validation are still incomplete |
| Tier 1D | Mostly complete | authority/access findings work | no full scorecard track and limited generic bypass detection |
| Tier 1E | Mostly complete | obligation engine is one of the strongest pieces | no full contract-driven obligations or scorecard surface |
| Tier 1F | Mostly complete | `session_end` and `gate` now work in MCP and CLI on the same touched-concept model, including suppression-aware decisions and shared patch-safety analysis reuse | release-grade gate validation is still incomplete |
| Tier 2A | Mostly complete | parity analyzer, MCP tool, and real `parallel-code` bootstrap proof now exist | broader contract families still need more than one benchmark repo |
| Tier 2B | Mostly complete | concentration analysis exists and is tested | not yet benchmarked or validated on the real case-study repo |
| Tier 2C | Mostly complete | inspection tools and adoption helpers exist | real-repo validation has started, but the proof loop is not closed |
| Tier 3 | Partial | conservative state-integrity slice is in place and now validated on real `parallel-code` controllers | transition modeling and implicit lifecycle heuristics are not built |
| Validation | Partial | unit tests, synthetic gate/session regression scenarios, scoped real-repo goldens, and a versioned `parallel-code` benchmark comparison loop now exist | no second benchmark repo, release-grade confidence regression suite, or full migration suite |

## Tier-By-Tier Status

## Tier 0: Trust And Output Foundation

Status: mostly complete

Delivered:

- tracked plus untracked scan scope
- exclusion buckets
- internal vs external unresolved split
- scan trust/confidence data in MCP
- bottleneck-first health framing
- baseline/session-v2 persistence

Still missing or partial:

- baseline deltas are not surfaced inline in `health`
- the composite score is demoted in MCP, but CLI and GUI are not yet fully aligned
- confidence exists as MCP payload shape, but not as a fully unified first-class v2 type across product surfaces

## Tier 1A: Clone Drift Fast Lane

Status: mostly complete

Delivered:

- exact clone groups are emitted as findings
- clone findings are integrated into `findings`, `session_end`, and `gate`
- stable clone ids are carried in finding payloads
- clone findings now include git-aware churn and code-age context
- clone reasons now explain recent activity and uneven change patterns
- clone instance ordering is deterministic, which makes baselines and session deltas more stable
- recent-activity and asymmetry counts are now computed at the distinct-file level, not per clone instance

Still missing:

- recently diverged clone family detection
- family-level prioritization and collapse when the same subsystem emits several adjacent clone findings
- a dedicated CLI clone-drift surface beyond the general findings and gate flows

## Tier 1B: TypeScript Semantic Substrate

Status: mostly complete

Delivered:

- `analysis::semantic`
- TypeScript project discovery
- persistent Node bridge
- protocol version and capability handshake
- compiler-backed semantic extraction
- symbols
- reads and writes
- closed domains and exhaustiveness sites
- crash recovery and Node-missing fallback behavior

Still missing:

- first-class generic reference facts are still incomplete compared to the design intent
- semantic fact caching is mostly in-memory, not a mature persisted cache story
- there is no regression benchmark suite yet, even though an initial real-repo benchmark now exists

## Tier 1C: Minimal Concept Graph And Rules

Status: mostly complete

Delivered:

- concept graph types
- explicit concept extraction from rules
- v2 `rules.toml` sections
- rule coverage
- suppression schema
- suppression matching by kind, concept, and file
- suppression-aware findings, gate, session, and concept-inspection outputs
- expiry-aware suppression visibility in MCP and CLI

Still missing:

- broader policy-management ergonomics and release-grade validation around suppressions

## Tier 1D: Authority And Access

Status: mostly complete

Delivered:

- durable write-path analysis
- multi-writer findings
- writer allowlist/forbid findings
- forbidden raw-read findings
- production-only authority findings now ignore test setup writes

Still missing:

- no explicit authority/access scorecard tool or track surface
- authoritative concept detection is still primarily rule-driven
- public-API/barrel bypass detection is still narrower than the roadmap intent

## Tier 1E: Obligation Engine

Status: mostly complete

Delivered:

- obligation model
- missing vs satisfied site computation
- obligation findings
- closed-domain exhaustiveness support
- obligation count and context burden summaries

Still missing:

- obligations are primarily closed-domain driven, not fully contract-driven
- changed-symbol precision is still coarser than the full design ambition
- no full scorecard surface for obligation completeness

## Tier 1F: Session Delta And CI Gate

Status: mostly complete

Delivered:

- `findings`
- `obligations`
- upgraded `session_end`
- touched-concept regression verdicts
- MCP `gate`
- CLI `gate` now uses the touched-concept v2 model when v2 rules are configured
- CLI `gate --save` now writes the v2 session baseline used by touched-concept comparisons
- gate and session outputs now respect active suppressions and surface expired suppression matches
- legacy structural gate remains as the fallback when no v2 rules are configured
- `gate` and `session_end` now reuse one shared patch-safety analysis pass per working tree state
- changed-tree cache reuse now keeps the v2 patch-safety cache alive across the normal `gate -> session_end` flow
- clone-only findings now survive semantic bridge failures instead of disappearing from the patch-safety surface

Still missing:

- release-grade touched-concept gate goldens
- broader CI ergonomics beyond the current CLI parity step
- further reduction in scan-bound work inside the warm patch-safety path

## Tier 2A: Contract Parity

Status: mostly complete

Delivered:

- parity cells
- parity reports and findings
- `parity` MCP tool

Still missing:

- deeper runtime-binding detection and false-positive review against the real `parallel-code` goldens

## Tier 2B: Concentration Risk

Status: mostly complete

Delivered:

- side-effect breadth
- authority breadth
- timer/retry weight
- async branching weight
- churn-aware scoring
- concentration findings

Still missing:

- proof that rankings and thresholds are good on `parallel-code`

## Tier 2C: Concept Inspection And Rule Adoption

Status: mostly complete

Delivered:

- `concepts`
- `explain_concept`
- `trace_symbol`
- guardrail-test evidence
- conservative concept inference
- onboarding docs

Still missing:

- broader proof beyond the initial real-repo validation and scoped golden outputs

## Tier 3: Advanced Static Analysis

Status: partial

Delivered:

- conservative state-integrity reports
- state findings
- `state` MCP tool
- state findings integrated into `findings`, `gate`, `session_end`, `trace_symbol`, and `explain_concept`

Still missing:

- transition modeling
- transition-coverage analysis
- implicit lifecycle inference
- invalid-state-risk findings
- real scorecard track for state integrity

## Cross-Cutting Validation Status

Status: partial

Delivered:

- bridge supervisor tests
- semantic fixture-style tests
- concept tests
- concentration tests
- state-analysis tests
- MCP handler tests
- synthetic touched-concept gate and `session_end` regression scenarios
- v2-only and invalid-v1-baseline coexistence tests
- initial scoped golden outputs for `parallel-code`
- checked-in real-repo `session_start`, `gate`, and `session_end` pass goldens captured from a temporary local clone
- checked-in real-repo regression-path `gate` and `session_end` fail goldens captured from a deterministic temporary mutation on a local clone
- initial cold/warm benchmark artifact for `parallel-code`
- versioned benchmark comparison flow with separate warm patch-safety timings
- no-change patch-safety reuse for cached scan state plus an empty-change semantic short-circuit
- shared patch-safety analysis reuse across `gate` and `session_end`
- false-positive review workflow and promotion checklist

Still missing:

- confidence-report regression tests
- full v1/v2 migration suite, including schema/version mismatch cases
- second-repo benchmark and validation coverage
- remaining patch-safety performance work beyond the shared-analysis reuse work, especially file-hash walk cost and cold-path variance

## Where We Are Relative To The Plan

The current implementation is enough to say:

- the **core patch-safety engine exists**
- the **MCP story is real**
- the **TypeScript-first architecture is viable**

It is not enough to say:

- the full roadmap is delivered
- the case-study proof loop is closed
- the product is equally mature across MCP, CLI, and GUI

## Biggest Remaining Gaps

1. the roadmap document had fallen behind the code and needed an explicit audit
2. the real `parallel-code` validation loop is only partially closed
3. clone drift is still missing divergence-aware prioritization
4. release-grade gate/session proof is still thinner than the analyzer surface
5. Tier 3 is only an initial conservative slice

## Recommended Next Execution Order

1. turn the existing `parallel-code` proof loop into a second-repo validation pass
2. add confidence-report and migration regression coverage
3. expand clone drift with divergence-aware prioritization
4. reduce remaining scan-bound patch-safety cost
5. validate parity and concentration against a second real repo

## Beta Readiness

The implementation is close to a useful **MCP beta**, but not yet to a fully proven **v2 beta release**.

The missing bar is mostly not new analysis code. It is:

- proof on the real case-study repo
- validation artifacts
- surface consistency across product entry points
