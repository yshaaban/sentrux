# Sentrux V2 Implementation Status

Last audited: 2026-04-18

## Overall Verdict

The implementation is in a good place relative to the **core v2 wedge**, but it is not done relative to the **full roadmap** or the stricter product bar of "agents consistently get the right few things to fix first."

Current assessment:

- doctrine alignment: strong
- core patch-safety wedge: strongly implemented
- full roadmap: partially implemented
- cross-cutting proof and validation: materially improved, with fast-path MCP `check`, defect injection, and signal-quality scorecard foundations now landed
- public release readiness: materially improved, with public-safe docs, hygiene enforcement, and deterministic local release preflight now in place
- current product risk: no longer missing engine capability first; now mostly agent-lane selectivity, repair guidance, treatment-vs-baseline proof, and release credibility

Working estimate:

- core wedge completion: about 90-92%
- full roadmap completion: about 80-85%
- validation and proof completion: about 78-82%
- outcome-proof completion: about 45-55%

## Strategic Read

The project has moved out of the "build the engine" phase.

The next critical phase is product-quality compression:

- improve the quality of the first few findings a user sees
- make primary findings more repairable
- promote or demote detectors through public-proof discipline
- keep the agent lane separate from maintainer-lane watchpoints
- make treatment-vs-baseline evidence part of the default-on promotion bar
- keep release credibility aligned with the local product story

This is the right next phase because the core wedge is already real. The biggest remaining risk is not detector count. It is whether the lead surface is selective, fixable, and trustworthy enough for repeated use.

## Program Status By Phase

This status view follows the phase model in [master-plan.md](./master-plan.md) rather than only the tier model below.

| Phase | Status | What is already true | Main remaining gap |
| --- | --- | --- | --- |
| Phase 0: Reset the product contract | In progress | doctrine, roadmap, scorecards, evidence review, session corpus, and findings surfaces now share lane language and outcome-first promotion inputs | some downstream surfaces still need to consume the same lane metadata and top-action contract without compatibility gaps |
| Phase 1: Harden the intervention-grade signal set | In progress | clone drift, authority/access, obligation completeness, trust-tiered findings, and the narrowed intervention-grade cohort are shipping | the default lane still needs stronger suppression of maintainer-style watchpoints and broader proof across repos |
| Phase 2: Expand the semantic obligation graph | In progress | closed-domain and initial contract-driven obligation expansion are real | richer contract families, finer changed-symbol precision, and stronger follow-through linking remain incomplete |
| Phase 3: Add bounded LLM adjudication | Not started | research direction and product guardrails are defined | no production adjudication loop is in the default product or eval pipeline yet |
| Phase 4: Add checker and pattern synthesis | Not started | verdict, remediation, and calibration artifacts exist to seed future synthesis work | no evidence-backed synthesis loop is implemented yet |
| Phase 5: Build the treatment-vs-baseline evidence program | In progress | scorecards, remediation evals, session telemetry, diff replay, canonical top-action telemetry, and experiment-arm comparison artifacts exist | controlled paired baseline/treatment runs are not yet the decisive promotion evidence |
| Phase 6: Product surface compression | In progress | `check`, `agent_brief`, trust tiers, repair-packet tooling, and lane-aware lead selection have improved the lead surface | the product still does not consistently compress to 1-3 intervention-grade actions |
| Phase 7: Release gate | In progress | public-safe hygiene, preflight, release checklist, and promotion scaffolding are in place | release credibility still depends on stronger outcome evidence and benchmark-gate depth |

## What Is True Today

The product shape in code matches the doctrine in [doctrine.md](./doctrine.md):

- MCP `check` is now the primary fast-path structured patch surface
- CLI v2 entry points are `brief` and `gate`; CLI `check` remains the legacy structural rules check
- `agent_brief` is the synthesized guidance surface when more context is needed
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
- multi-repo golden validation runner
- benchmark fail/warn/info classification with explicit thresholds
- release checklist for proof artifacts, benchmark policy, and migration behavior
- public release hygiene scanning against private repo names, internal domains, abandoned upstream links, and workstation-specific paths
- one-command public release preflight for the supported public matrix
- deterministic checked-in benchmark and golden refreshes via frozen disposable clones and fixed analyzed-commit epochs for age-sensitive proof signals
- warm-path rules-config caching across repeated MCP requests
- persisted semantic snapshot reuse across repeated MCP requests and fresh processes when the project fingerprint and working tree still match
- generic archetype detection with `project_shape` output, starter-rule generation, and framework-aware role defaults for modular Next.js and React frontend repos
- broader service-oriented archetype detection with layered Node service boundary-root suggestions and nested module public-API observations
- generic `module_contract` rules for module public APIs and cross-module deep-import enforcement
- explicit transition-table extraction for rule-declared state domains in the TS bridge
- external Claude-backed eval harness scaffolding plus a focused `dead_private` review loop
- mode-aware `agent_brief` composition for repo onboarding, patch guidance, and pre-merge readiness in MCP and CLI
- fast-path MCP `check` with flat issue output, ranked actions, explicit changed-scope availability, missing-test watchpoints, and inferred zero-config boundary findings
- project-shape caching with adoption-ready `working_rules_toml`
- seeded defect-injection harness and dogfood loop for `check`
- review-packet builder, remediation-eval runner, and per-signal scorecard tooling
- repo-local MCP session telemetry with session-summary generation and calibration-run wiring
- disposable-clone Codex task capture and historical diff replay now feed the same session-telemetry and scorecard loop
- normalized session-corpus artifacts and weekly evidence-review synthesis now sit on top of the live/replay calibration outputs
- cohort-driven batch Codex capture, batch diff replay, and signal-backlog synthesis now exist for continuous calibration
- checked-in repo calibration manifests, batch manifests, prompt files, and review-verdict schema/template now exist for `parallel-code`, `one-tool`, and `sentrux`
- one-command repo calibration orchestration now exists through `run-repo-calibration-loop.mjs`
- scorecard, session-corpus, and evidence-review artifacts now carry explicit program/phase evidence-source metadata plus a stricter default-on rollout separation from generic promotion guidance
- shared static ranking and score-band policy now lives in `.sentrux/signal-policy.json` with Rust and JS parity tests consuming the same fixture contract, and representative brief/report ordering now rides shared behavior fixtures as well
- maintainer architecture boundaries for policy ownership and eval runtime composition are now documented in [policy-and-eval-architecture.md](./policy-and-eval-architecture.md)
- legacy MCP and CLI surfaces now frame structural output as context rather than the main v2 story
- desktop structural panels and export flow now frame structural output as supporting context
- `findings` now includes a top-level confidence summary

The strongest product truth today is narrower than the full roadmap:

- Sentrux is already useful as a patch-safety and structural review assistant
- the maintainer/watchpoint lane is richer than the proven default agent lane
- Sentrux is not yet broadly proven as a universal code-quality reviewer across arbitrary repos or as a treatment that reliably beats baseline

## Overall Status By Tier

| Tier | Status | Assessment | Main Gap |
| --- | --- | --- | --- |
| Tier 0 | Mostly complete | Trust foundation is real in MCP and CLI | GUI productization and `health` inline delta surfacing are still uneven |
| Tier 1A | Mostly complete | Clone drift findings now have stable ids, git-aware risk context, divergence-aware family clustering, remediation hints, and cleaner production-first ranking | no history-aware rename/copy tracing and no dedicated clone-drift CLI surface yet |
| Tier 1B | Mostly complete | TS bridge, semantic facts, persisted semantic cache reuse, proof artifacts, and explicit benchmark policy now exist for the public `parallel-code` and `one-tool` proof loops plus Sentrux dogfood artifacts | warm-path structural cost is still higher than desired and the cache story is not yet fully incremental |
| Tier 1C | Mostly complete | v2 rules, concept graph, suppression enforcement, archetype-aware starter rules, and module-contract support now exist | broader policy UX and validation are still incomplete |
| Tier 1D | Mostly complete | authority/access findings now include stronger public-boundary bypass and concept-boundary pressure summaries | limited generic public-entry inference and scorecard calibration still need more real-repo evidence |
| Tier 1E | Mostly complete | obligation engine now handles closed-domain and initial contract-driven triggers | richer contract families and finer changed-symbol precision are still incomplete |
| Tier 1F | Mostly complete | `session_end` and `gate` now work in MCP and CLI on the same touched-concept model, including suppression-aware decisions and shared patch-safety analysis reuse | release-grade gate validation is still incomplete |
| Tier 2A | Mostly complete | parity analyzer, MCP tool, and real proof now exist on more than one repo shape | broader contract families still need more false-positive review and non-happy-path validation |
| Tier 2B | Mostly complete | concentration analysis exists and is tested | not yet benchmarked or validated on the real case-study repo |
| Tier 2C | Mostly complete | inspection tools, adoption helpers, trust-tiered debt signals, structural debt findings, debt clusters, and `project_shape` now exist with a public benchmark corpus and Sentrux dogfood artifacts | broader onboarding proof and evidence-quality validation are still incomplete |
| Tier 3 | Partial | conservative state-integrity slice now includes explicit transition-site modeling and transition-coverage findings for rule-declared state domains | implicit lifecycle heuristics and broader invalid-state-risk inference are still not built |
| Validation | Mostly complete | unit tests, synthetic gate/session regression scenarios, public benchmark goldens, versioned benchmark comparison, explicit benchmark policy, confidence/migration checks, defect injection, review packets, remediation eval scaffolding, signal scorecards, public hygiene checks, and local release preflight now exist | dedicated quiet-runner benchmark gating, no full migration suite, and broader public benchmark-repo unhappy-path coverage are still incomplete |

## Current Product Gaps

These are the main user-facing gaps that still matter even though the wedge is largely built:

- top findings are not yet proven strongly enough across the public proof set
- some structurally true findings are still stronger diagnostically than they are repair-oriented
- detector promotion is improving, but still needs more measured proof from public repos and remediation loops
- local validation is strong, but remote release confidence is still the deciding trust bar for broad public use

## Tier-By-Tier Status

## Tier 0: Trust And Output Foundation

Status: mostly complete

Delivered:

- mode-aware `agent_brief` in MCP and CLI
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
- persisted semantic cache reuse is now real, but it is not yet a fully incremental changed-symbol cache story
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
- broader repo-archetype packs beyond the current modular Next.js, React frontend, and layered Node service defaults

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
- feature-module public-API enforcement still depends on explicit `module_contract` configuration rather than first-class adoption UX

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
- dead-private detection still remains experimental because its reference model is now conservative for same-file callback/JSX usage, but broader reference precision is not reliable enough on real TS/TSX repos yet

## Tier 3: Advanced Static Analysis

Status: partial

Delivered:

- conservative state-integrity reports
- state findings
- `state` MCP tool
- state findings integrated into `findings`, `gate`, `session_end`, `trace_symbol`, and `explain_concept`
- explicit transition-site modeling for rule-declared TypeScript state domains
- transition-coverage-gap findings and missing-transition-site findings
- bridge-level validation for `switch` and `if`/`else if` explicit transition controllers

Still missing:

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
- public release hygiene scan and banned-content test coverage
- one-command public release preflight
- deterministic frozen-clone benchmark and golden refresh discipline for checked-in public artifacts

Still missing:

- full v1/v2 migration suite breadth beyond the current coexistence, malformed-baseline, and schema/version-mismatch cases
- remaining patch-safety performance work beyond the shared-analysis reuse work, especially file-hash walk cost and cold-path variance
- dedicated quiet-runner benchmark-threshold enforcement for release gating

## Where We Are Relative To The Plan

The current implementation is enough to say:

- the **core patch-safety engine exists**
- the **MCP story is real**
- the **TypeScript-first architecture is viable**
- the **agent-lane vs maintainer-lane split now shows up in docs, scorecards, and findings surfaces**
- the **evidence artifacts now carry a canonical top-action object and explicit treatment-vs-baseline comparisons**

It is not enough to say:

- the full roadmap is delivered
- the case-study proof loop is closed
- treatment beats baseline on the primary product metrics
- the product is equally mature across MCP, CLI, and GUI

## Biggest Remaining Gaps

1. remaining warm-path performance work is still dominated by structural scan and changed-file bookkeeping
2. benchmark-repo unhappy-path validation, dedicated quiet-runner regression gating, and analyzer promotion evidence coverage are still thinner than the main happy-path proof loop
3. GUI wording is aligned, but the desktop surface still lacks first-class v2 findings, obligations, and debt-signal panels
4. richer contract-driven obligations are materially better, but true field-diff precision is still incomplete
5. Tier 3 is still only an initial conservative slice

## Recommended Next Execution Order

1. strengthen Phase 5 so paired treatment-vs-baseline runs, not just artifact summaries, become the decisive promotion input
2. keep tightening Phase 1 and Phase 6 until the default patch surface usually compresses to 1-3 intervention-grade actions
3. finish the remaining Phase 0 propagation so every consumer surface uses the same lane metadata and top-action contract
4. deepen Phase 2 only where repo evidence still shows missed follow-through or weak fix-surface precision
5. treat performance, release gate depth, and supporting watchpoint quality as enabling work, not as substitutes for outcome proof

## Beta Readiness

The implementation is past a useful **MCP beta** and close to a broader **MCP/CLI v2 beta**, but not yet to a fully proven **v2 beta release**.

The missing bar is mostly not new analysis code. It is:

- proof on the real case-study repo
- treatment-vs-baseline evidence on agent outcomes
- validation artifacts
- surface consistency across product entry points
- a cleaner separation between default patch actions and maintainer watchpoints
