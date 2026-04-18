# Sentrux V2 Product Doctrine

## Purpose

This document records the product decisions that should constrain the rest of v2.

If a roadmap item, metric, or analyzer conflicts with this doctrine, the doctrine wins unless it is explicitly revised.

## Primary Job

Sentrux v2 exists to reduce agentic entropy by catching patch-level architectural regressions and maintainability drift before they land.

The core product question is:

> What did this patch change, what architectural obligations did that create, what intervention-grade signals does the agent need to act on now, and what maintainer watchpoints should stay out of the default patch lane?

That question should be answered first through fast-path `check`, then through a mode-aware `agent_brief` when the agent needs more context. Agents should never have to stitch together raw tool output to find the next fix.

## Priority Order

Priority order for v2:

1. patch safety
2. touched-concept regression ratchet
3. repo-level context
4. repo-level score summaries

This means:

- `check` is the primary fast-path patch surface
- `agent_brief` is the synthesized guidance surface
- a patch-scoped missing obligation matters more than a repo-wide depth penalty
- a new multi-writer regression matters more than a low modularity score
- objective findings matter more than elegant composite math
- engineers own final prioritization; v2 supplies evidence, debt signals, and watchpoints

## Primary User

The primary user is the coding agent during a session.

Secondary users:

- human reviewer
- CI gate
- repo owner looking at trend summaries

## Product Lanes

V2 has two product lanes with different jobs and different evidence bars.

Agent lane:

- default patch surface
- intervention-grade only
- patch-local
- fix-oriented
- low-noise
- optimized for top-action follow rate, top-action help rate, and treatment-vs-baseline outcomes

Maintainer lane:

- broader structural watchpoints
- backlog shaping and trend review
- slower-moving quality governance
- useful even when the next patch action is not obvious

The agent lane is the primary product claim.

The maintainer lane can be rich, but it must not crowd out the first 1-3 patch actions. If a signal is informative but not reliably fixable in-session, it belongs in the maintainer lane or a supporting watchpoint surface.

## Primary Outputs

The product surface must be ordered like this:

1. check
2. agent_brief
3. findings
4. obligations
5. session delta
6. scorecard
7. confidence

The scorecard is useful, but it is not the core wedge. Any optimization-like output is a sorting aid, not a roadmap decision. `check`, `agent_brief`, and `session_end` should all lead with the same ranked action model.

Default agent-lane outputs should usually show at most 1-3 primary actions. Broader debt summaries, structural watchpoints, and trend context belong behind that lead surface.

## Primary-Target Contract

The user experiences Sentrux through the first few findings, not through detector coverage totals.

That means the product should optimize for:

- fewer, stronger primary targets
- trustworthy ordering
- fixability
- explicit confidence and trust-tier separation

A finding should not become a primary target just because it is technically true. It should usually meet all of these bars:

- high enough trust for the current surface
- worth fixing now
- local enough that the next repair step is clear
- supported by enough evidence that the user does not have to reverse-engineer the claim

If only one or two findings meet that bar, the product should show one or two. A thin but trustworthy lead surface is better than a long, noisy one.

## Repair Guidance Contract

The product should prefer fixable findings over merely accurate findings.

Primary findings should increasingly behave like repair packets:

- what is risky or incomplete
- why now
- likely fix sites
- smallest safe first cut
- what to verify after the fix

If a finding cannot support that level of guidance, it should usually be demoted to a watchpoint unless it is severe enough to justify a broad blocker.

## Agent Brief Modes

`agent_brief` should support exactly three mode families:

- `repo_onboarding`: explain repo shape, critical concepts, rules, exclusions, and first steps
- `patch`: explain what changed, what it touched, what obligations were created, and what is still missing
- `pre_merge`: explain merge readiness, remaining blockers, and confidence before land

The brief should synthesize findings, obligations, session delta, and confidence. It should not replace those outputs.

## Core Wedge

The foundational highest-ROI v2 wedge is still three analyzer families:

1. clone drift
2. authority and access
3. obligation completeness

These directly address the most expensive static failure modes in agentic coding:

- copy-paste divergence
- ownership drift across layers
- incomplete propagation of closed-domain changes
- boundary erosion and brittle coordination hotspots

For beta, concept-level findings in this wedge should rely on explicit critical concept rules.

The default intervention-grade portfolio can expand beyond that foundation only when the added signal family improves the agent lane without degrading trust. The current candidate expansion families are:

1. change-triggered library-evolution drift
2. patch-worsened concentration growth

These are agent-lane candidates only when they stay local, fix-oriented, and evidence-backed. Otherwise they remain maintainer-lane watchpoints.

Zero-config beta findings should be limited to:

- clone drift
- conservative closed-domain and exhaustiveness checks
- missing-test watchpoints
- conservative inferred boundary violations

## Secondary Context

These analyzers are valuable, but they are not the first wedge:

1. contract parity
2. concentration risk

They help with inspection, architecture review, and longer-term repo quality, but they should not delay the patch-safety engine or become the primary source of prioritization.

## Later Analysis

These are later-stage analyzers because they are more heuristic or more expensive:

1. explicit state-model synthesis across files
2. transition-coverage modeling
3. implicit state-machine inference
4. richer protocol/state integrity heuristics

They are aligned with the vision, but they are not the highest-ROI starting point.

## Language Strategy

V2 should be TypeScript/JavaScript-first.

Reason:

- that is where the case study value is strongest
- that is where agentic entropy is currently most visible
- high-quality deep analysis in one ecosystem is better than shallow cross-language coverage

## Rules Strategy

Rule priority order:

1. explicit repo rules
2. existing architecture guardrail tests
3. conservative inference

Teams should not have to model their entire repo to get value.

The intended onboarding path is:

1. zero-config findings
2. add 3-5 critical concepts
3. bind critical rules to CI ratchets

In beta, that means:

- zero-config findings do not need concept inference
- authority, access, and obligation findings for important domains should come from explicit `[[concept]]` rules

## Trust Rules

V2 must always expose:

- what was analyzed
- what was excluded
- what required heuristics
- what had deep semantic coverage
- what confidence the result deserves
- what trust tier each finding belongs to: `trusted`, `watchpoint`, or `experimental`
- what engineering leverage class each finding belongs to: `architecture_signal`, `local_refactor_target`, `boundary_discipline`, `regrowth_watchpoint`, `secondary_cleanup`, `hardening_note`, or `tooling_debt`

Do not hide uncertainty behind precise scores.

Engineer-facing defaults should:

- lead with trusted findings
- keep default agent-lane actions separate from maintainer-lane watchpoints
- separate watchpoints from trusted debt signals
- quarantine experimental detectors from default ratchets and top finding lists
- separate trust from leverage: reliability is not the same as improvement leverage
- preserve fixability metadata such as impact, inspection focus, and candidate split axes

Trust is not only about whether a detector is correct. It is also about whether the lead surface is disciplined. A top-ranked finding that is true but not worth acting on still damages product trust.

## Public-Proof Discipline

Signals should be promoted by evidence, not by engineering intuition alone.

The promotion loop should be grounded in public-safe proof through:

- seeded defects
- false-positive review
- remediation evals
- public proof repos
- real session telemetry

Promotion standard:

- detectors that consistently produce actionable, fixable, high-value findings can graduate into primary surfaces
- detectors that are informative but not reliably worth acting on should remain watchpoints or supporting context

Default-on agent-lane promotion needs a stricter bar than generic detector promotion. It should require:

- reviewed precision and false-positive review strong enough to trust the finding
- repair guidance that is specific enough to shorten the next edit
- positive remediation and session evidence
- treatment-vs-baseline evidence that the surfaced action improves outcomes, not just artifact quality

## Anti-Goals

Do not optimize v2 around:

- a better single quality number
- graph-theory elegance over actionability
- runtime tracing as a dependency
- full semantic clone detection in the initial wedge
- broad implicit-FSM inference before obligation and ownership work is strong
- broad analyzer expansion before primary-target quality and repair guidance are strong

## Success Definition

V2 is succeeding when:

1. agents get fixable findings at `check`
2. touched-concept regressions can fail CI with high trust
3. important architectural rules become machine-checkable
4. the default agent lane usually shows only 1-3 intervention-grade actions
5. maintainer watchpoints stay available without polluting the default patch lane
6. `parallel-code` gets meaningful debt signals and watchpoints that match its own architecture docs and tests
7. the score is no longer the primary product narrative
8. seeded defects, false-positive review, remediation evals, and treatment-vs-baseline runs all support the signals we promote
9. real session telemetry shows the top-ranked `check` actions are actually getting cleared without creating new regressions

The next chapter after the core wedge is not "more analyzers first." It is quality compression:

- better top-finding selection
- stronger repair guidance
- stricter promotion discipline
- release trust that matches the local product story
