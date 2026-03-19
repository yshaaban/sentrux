# Sentrux V2 Next Implementation Batch

Last updated: 2026-03-19

This document defines the next execution block for v2 after the `parallel-code` proof loop, the `private-benchmark-repo` second benchmark proof loop, and the first output-coherence pass.

The goal of this batch is not to broaden v2. It is to make the current wedge reliably useful for improving code quality in real workflows.

## Why This Batch Exists

The current v2 implementation is already good enough to surface useful quality findings on `parallel-code`:

- authority drift on scoped concepts
- obligation completeness for closed domains
- scoped contract parity
- explicit state-model validation
- cleaner clone findings than before

But there are still three gaps between “useful diagnostics” and “reliable quality improvement loop”:

1. clone drift is git-aware now, but it still lacks divergence detection and family-level prioritization
2. validation still trails the breadth of the analyzer surface
3. the new prioritization lane still needs broader real-repo tuning

This batch closes those gaps in ROI order.

## Scope

Included:

1. git-aware clone drift completion
2. validation hardening
3. quality-guidance validation and tuning

Explicitly excluded from this batch:

- broader Tier 3 implicit-state heuristics
- more parity generalization beyond the current scoped proof
- new dashboard or GUI work
- a third benchmark repo before the operational wedge is closed

## Current Baseline

At the start of this batch:

- the core wedge is mostly implemented in MCP
- suppressions are enforced across findings, gate, session, and concept-inspection outputs
- clone findings now have stable ids, git-aware risk context, and deterministic ordering
- `parallel-code` now has real scoped pass/fail goldens and a cold/warm benchmark

Recent learning:

- the real-repo proof loop is stronger now because a deterministic fail-path mutation exists in the golden harness
- the warm no-change patch-safety path improved once cached scan reuse and the empty-change semantic short-circuit landed
- the remaining performance uncertainty is now mostly cold-path noise and residual file-hash/structural work

Relevant references:

- [Implementation Status](./implementation-status.md)
- [Roadmap](./roadmap.md)
- [Parallel-Code Case Study](./parallel-code-case-study.md)
- [Parallel-Code Scoped Goldens](./examples/parallel-code-golden/README.md)
- [Parallel-Code Benchmark](./examples/parallel-code-benchmark.md)

## Success Criteria

This batch is successful when all of the following are true:

1. suppressions can hide known findings, and expired suppressions become visible again
2. the top findings surface contains fewer repeated/noisy entries and more actionable ones
3. clone findings can prioritize risky copied code using git recency/churn
4. `session_end` and `gate` have golden scenarios and benchmark regression coverage
5. clone-family clustering and concept pressure summaries are stable enough that the next prioritization pass has a clear target

## Work Package A: CLI Gate Parity

Status: complete

Goal:

- make the v2 patch-safety wedge usable in CI and non-MCP workflows

Why now:

- this is the highest-leverage product gap
- MCP already has the right model
- the current CLI gate still enforces v1 structural deltas

Current references:

- MCP gate: [handlers.rs](<sentrux-root>/sentrux-core/src/app/mcp_server/handlers.rs#L2277)
- CLI gate: [main_impl.rs](<sentrux-root>/sentrux-bin/src/main_impl.rs#L357)

Deliverables:

- shared touched-concept gate computation reusable by MCP and CLI
- CLI `gate` path that uses v2 findings, obligations, and session/baseline context
- CLI output that explains:
  - changed files
  - introduced findings
  - missing obligations
  - pass/fail decision
- CLI exit codes consistent with MCP gate semantics

Tasks:

- [x] extract gate decision logic into shared v2 helper used by MCP and CLI
- [x] add CLI path for v2 gate computation
- [x] preserve existing structural gate as compatibility mode or fallback
- [x] add CLI output formatting for introduced findings and missing obligations
- [-] add focused CLI tests for pass/fail behavior

Acceptance criteria:

- the same synthetic patch should produce the same gate verdict in MCP and CLI
- `sentrux gate` can be used in CI without MCP

What we expect to learn:

- whether the v2 gate is stable enough to be used outside exploratory MCP sessions

## Work Package B: Suppressions And Trust Controls

Status: mostly complete

Goal:

- make v2 findings governable enough for adoption

Why now:

- findings are now strong enough that teams will want to ratchet them
- without suppressions, CI rollout will stall

Current references:

- schema: [v2.rs](<sentrux-root>/sentrux-core/src/metrics/rules/v2.rs#L70)
- docs: [Rules V2](./rules-v2.md)

Deliverables:

- finding suppression matching by:
  - kind
  - concept
  - file
- expiry handling
- visibility for active and expired suppressions
- finding dedupe for repeated same-file/same-kind evidence

Tasks:

- [x] implement suppression matcher shared across analyzer outputs
- [x] apply suppressions to findings before gate/session presentation
- [x] treat expired suppressions as findings or explicit warnings
- [x] dedupe repeated authority findings from the same file/concept/kind
- [x] expose suppression hits and expiry state in MCP/CLI responses

Acceptance criteria:

- a configured suppression can hide a matching finding
- an expired suppression becomes visible automatically
- repeated same-file forbidden-writer evidence collapses into a cleaner top-level finding

Open gap:

- the enforcement layer is in place, but it still needs broader golden coverage and real-repo adoption feedback

What we expect to learn:

- whether the remaining noise is primarily a policy issue or still an analyzer-quality issue

## Work Package C: Clone Drift Depth

Status: mostly complete

Goal:

- turn clone drift from a useful exact-clone fast lane into a real entropy sensor

Why now:

- clone drift is one of the three core wedge lanes
- current clone support is still shallower than the plan intended

Current references:

- current exact clone findings: [handlers.rs](<sentrux-root>/sentrux-core/src/app/mcp_server/handlers.rs#L1231)
- roadmap status: [Roadmap](./roadmap.md)

Deliverables:

- stable clone ids
- git recency/churn correlation
- divergent-clone candidate ranking
- better distinction between:
  - harmless duplication
  - risky copied logic
  - likely copy-paste drift

Tasks:

- [x] assign stable clone ids in finding payloads
- [x] correlate clone groups with commit recency and churn
- [x] rank clones by production presence, size, recency, and asymmetry
- [ ] add divergent-clone candidate detection using git/file-history signals
- [-] expose clone-drift detail in MCP and CLI surfaces
- [ ] collapse repeated same-family clone findings into higher-level prioritization

Acceptance criteria:

- clone findings can explain why a group is risky, not just that it exists
- `parallel-code` clone results prioritize meaningful production clones over trivia

What we learned:

- git-aware clone context materially improves the `parallel-code` findings surface
- the remaining clone gap is prioritization: repeated clone families can still dominate the top list even when the per-finding ranking is better

## Work Package D: Validation Hardening

Status: in progress

Goal:

- turn the current proof artifacts into a release-grade validation loop

Why now:

- implementation is ahead of proof
- the wedge should become harder to regress as we operationalize it

Current references:

- benchmark script: [benchmark_parallel_code_v2.mjs](<sentrux-root>/scripts/benchmark_parallel_code_v2.mjs)
- golden script: [refresh_parallel_code_goldens.sh](<sentrux-root>/scripts/refresh_parallel_code_goldens.sh)
- validation doc: [Testing And Validation](./testing-and-validation.md)

Deliverables:

- `session_end` goldens
- `gate` goldens
- synthetic patch scenarios for obligation and gate behavior
- regression benchmark suite
- false-positive review checklist
- v1/v2 migration tests

Tasks:

- [-] add `session_end` golden scenarios
- [-] add touched-concept gate golden scenarios
- [-] add synthetic patch fixtures for closed-domain propagation
- [-] turn one-off benchmark script into a comparable regression benchmark flow
- [x] add false-positive review checklist and sample set
- [-] add baseline migration coexistence tests

Acceptance criteria:

- new analyzer changes can be checked against stable gate/session expectations
- cold/warm regressions are visible over time

What we learned so far:

- the current touched-concept gate is stable on a real closed-domain regression fixture
- `session_end` was too tightly coupled to the legacy structural baseline and now needs to degrade gracefully for v2-only workflows
- synthetic regression fixtures are the right first layer, but they do not replace checked-in real-repo goldens
- the warm semantic path is already fast, but `gate` and `session_end` are still scan-bound on the real repo and need their own performance attention
- checked-in real-repo no-change `gate` and `session_end` goldens now exist; the remaining golden gap is real-repo regression scenarios

## Work Package E: Quality Improvement Prioritization

Status: mostly complete

Goal:

- make v2 better at pointing to the highest-leverage code-quality work, not just reporting raw findings

Why this is after A-D:

- prioritization only matters once outputs are operational and trusted

Deliverables:

- concept-scoped summaries for repeated findings
- clearer “top refactor opportunities” from:
  - multi-writer concepts
  - repeated raw-access violations
  - high context-burden obligations
  - concentrated hotspots combined with policy violations

Tasks:

- [x] add concept-level grouping for repeated findings
- [x] rank top-quality-improvement opportunities from current findings and obligations
- [x] surface suggested structural improvements in `findings`
- [x] surface patch-scoped quality opportunities in `session_end`
- [-] validate and tune the opportunity ranking on more than one real repo

Acceptance criteria:

- the tool can point to a small set of high-value quality improvements on an existing repo

What we learned:

- grouping repeated concept pressure makes the `findings` surface materially easier to scan than a flat list of violations
- concentration context is useful as a score boost, but it should stay supporting evidence instead of becoming a separate top-level queue for already-covered concepts
- the next gap is proof quality, not another round of raw ranking logic

## Recommended Execution Order

1. Work Package A: CLI Gate Parity
2. Work Package B: Suppressions And Trust Controls
3. Work Package C: Clone Drift Depth
4. Work Package D: Validation Hardening
5. Work Package E: Quality Improvement Prioritization

## Why This Order

This order matches the current product bottlenecks:

- first operationalize the wedge
- then make it trustworthy
- then deepen a still-underbuilt core lane
- then harden the proof loop
- then validate and tune prioritization for repo-improvement work

## Batch Exit Criteria

The batch is done when:

1. the CLI can enforce the v2 gate
2. suppressions and expiry work
3. clone drift is git-aware
4. `session_end` and `gate` have stable goldens
5. the top findings and opportunities on `parallel-code` are clean enough that a human would use them to drive quality work without heavy manual filtering
