# Sentrux V2 Implementation Status

Last audited: 2026-03-20

## Overall Verdict

The implementation is in a good place relative to the **core v2 wedge**, but it is not done relative to the **full roadmap**.

Current assessment:

- doctrine alignment: strong
- core patch-safety wedge: strongly implemented
- full roadmap: partially implemented
- cross-cutting proof and validation: materially improved, still behind the full roadmap

Working estimate:

- core wedge completion: about 90-92%
- full roadmap completion: about 80-85%
- validation and proof completion: about 78-82%

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
- broader contract-driven obligation triggers beyond closed-domain changes
- stronger boundary-bypass and concept-boundary pressure findings
- concept-scoped quality summaries and debt signals in `findings` and `session_end`
- first-class structural debt findings for large files, dependency sprawl, unstable hotspots, cycle clusters, dead private code clusters, and dead islands
- overlap-aware debt clusters that connect related structural and concept signals
- normalized finding-detail rows with explicit impact and inspection focus
- explicit trust tiers across findings, debt signals, debt clusters, and watchpoints
- generic leverage classes and leverage reasons across findings, debt signals, debt clusters, and watchpoints
- cycle-cluster cut-candidate evidence with candidate seam hints and estimated cycle reduction
- quarantine of experimental finding classes from default `findings`, `session_end`, and `gate` outputs
- leverage-aware report selection now lives in a generic shared selector rather than the `parallel-code` renderer
- inspection candidates that combine concept pressure, clone families, and hotspots
- parity and concentration context
- conservative state-integrity analysis
- session baseline project-fingerprint validation and confidence regression coverage
- second benchmark repo proof loop on `private-benchmark-repo`
- multi-repo golden validation runner
- benchmark fail/warn/info classification with explicit thresholds
- release checklist for proof artifacts, benchmark policy, and migration behavior
- warm-path rules-config caching across repeated MCP requests
- legacy MCP and CLI surfaces now frame structural output as context rather than the main v2 story
- desktop structural panels and export flow now frame structural output as supporting context
- `findings` now includes a top-level confidence summary

## Overall Status By Tier

| Tier | Status | Assessment | Main Gap |
| --- | --- | --- | --- |
| Tier 0 | Mostly complete | Trust foundation is real in MCP and CLI | GUI productization and `health` inline delta surfacing are still uneven |
| Tier 1A | Mostly complete | Clone drift findings now have stable ids, git-aware risk context, divergence-aware family clustering, remediation hints, and cleaner production-first ranking | no history-aware rename/copy tracing and no dedicated clone-drift CLI surface yet |
| Tier 1B | Mostly complete | TS bridge, semantic facts, proof artifacts, and explicit benchmark policy now exist across `parallel-code` and `private-benchmark-repo` | no mature persisted cache story and warm-path structural cost is still higher than desired |
| Tier 1C | Mostly complete | v2 rules, concept graph, and suppression enforcement now exist | broader policy UX and validation are still incomplete |
| Tier 1D | Mostly complete | authority/access findings now include stronger public-boundary bypass and concept-boundary pressure summaries | no full scorecard track and limited generic public-entry inference |
| Tier 1E | Mostly complete | obligation engine now handles closed-domain and initial contract-driven triggers | richer contract families and finer changed-symbol precision are still incomplete |
| Tier 1F | Mostly complete | `session_end` and `gate` now work in MCP and CLI on the same touched-concept model, including suppression-aware decisions and shared patch-safety analysis reuse | release-grade gate validation is still incomplete |
| Tier 2A | Mostly complete | parity analyzer, MCP tool, and real proof now exist on more than one repo shape | broader contract families still need more false-positive review and non-happy-path validation |
| Tier 2B | Mostly complete | concentration analysis exists and is tested | not yet benchmarked or validated on the real case-study repo |
| Tier 2C | Mostly complete | inspection tools, adoption helpers, trust-tiered debt signals, structural debt findings, debt clusters, and two real benchmark repos now exist | broader onboarding proof and evidence-quality validation are still incomplete |
| Tier 3 | Partial | conservative state-integrity slice is in place and now validated on real `parallel-code` controllers | transition modeling and implicit lifecycle heuristics are not built |
| Validation | Mostly complete | unit tests, synthetic gate/session regression scenarios, two real benchmark repos, multi-repo goldens, versioned benchmark comparison, explicit benchmark policy, and confidence/migration checks now exist | no full migration suite and benchmark-repo unhappy-path coverage is still incomplete |

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
- normalized finding details and evidence-first debt reporting in MCP responses
- trust-tier separation for trusted findings, watchpoints, and experimental detectors in MCP responses

Still missing or partial:

- baseline deltas are not surfaced inline in `health`
- the composite score is demoted in MCP, but CLI and GUI are not yet fully aligned
- confidence exists as MCP payload shape, but not as a fully unified first-class v2 type across product surfaces
- proof-run markdown and GUI surfaces still lag the newer debt-cluster and finding-detail shapes
- proof-run markdown and evaluator artifacts still need regular refresh against the newer trust-tier and leverage schema

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

- history-aware rename/copy tracing for clone families
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
- the remaining warm patch-safety cost is still dominated by structural scan and changed-file bookkeeping

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
- contract-driven trigger symbols and file surfaces
- semantically related contract trigger paths
- obligation count and context burden summaries

Still missing:

- richer contract families beyond the current trigger surface
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

- deeper benchmark-repo unhappy-path validation around proof artifacts and analyzer promotion
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
- concept-scoped quality summaries in `findings`
- debt signals in `findings` and `session_end`
- structural debt findings in `findings` and `session_end`
- debt clusters in `findings` and `session_end`
- normalized finding details in `findings` and `session_end`
- trust-tiered separation of trusted findings, watchpoints, and experimental detector output
- cycle-cluster cut-candidate evidence in structural finding details
- concentration-backed inspection scoring for concept-level refactor candidates
- inspection candidates that combine boundary pressure, clone families, hotspots, and missing-site pressure

Still missing:

- broader proof beyond the initial real-repo validation and scoped golden outputs
- dead-private detection remains experimental because its reference model is not reliable enough on real TS/TSX repos yet

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

- full v1/v2 migration suite, including schema/version mismatch cases
- remaining patch-safety performance work beyond the shared-analysis reuse work, especially file-hash walk cost and cold-path variance
- broader benchmark-threshold enforcement for release gating

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

1. remaining warm-path performance work is still dominated by structural scan and changed-file bookkeeping
2. benchmark-repo unhappy-path validation and analyzer promotion criteria are still thinner than the main happy-path proof loop
3. GUI wording is aligned, but the desktop surface still lacks first-class v2 findings, obligations, and debt-signal panels
4. richer contract-driven obligations are materially better, but true field-diff precision is still incomplete
5. Tier 3 is still only an initial conservative slice

## Recommended Next Execution Order

1. reduce the remaining scan-bound warm patch-safety cost
2. expand benchmark-repo unhappy-path validation and formal analyzer promotion criteria
3. decide whether the desktop product needs first-class v2 panels instead of doctrinal alignment only
4. deepen contract-field precision only where real repo feedback still shows misses
5. validate parity, concentration, and debt-signal quality against more non-happy-path repo scenarios

## Beta Readiness

The implementation is past a useful **MCP beta** and close to a broader **MCP/CLI v2 beta**, but not yet to a fully proven **v2 beta release**.

The missing bar is mostly not new analysis code. It is:

- proof on the real case-study repo
- validation artifacts
- surface consistency across product entry points
