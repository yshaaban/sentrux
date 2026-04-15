# Sentrux V2 Next Implementation Batch

Last updated: 2026-03-19

This document defines the next execution block for v2 after the current wedge is working in MCP and CLI, the second benchmark repo proof loop on `private-benchmark-repo`, the boundary and contract expansions, and the current evidence-first debt-signal surface.

The goal of this batch is not to add broad new analyzer scope. It is to finish the remaining work that makes v2 dependable enough to surface objective technical-debt evidence on repos like `parallel-code`, while leaving final prioritization to engineers.

## Why This Batch Exists

The current v2 implementation can already:

- catch touched-concept regressions
- surface clone drift, authority/access violations, and incomplete propagation
- surface concept-level quality opportunities
- surface debt signals that combine boundary pressure, clone families, hotspots, and missing-site pressure
- prove the core workflow on `parallel-code` and `private-benchmark-repo`

That is enough for a strong beta wedge.

The remaining gaps were narrower and more operational:

1. benchmark-threshold policy and warm-path performance are not yet strong enough for release-grade confidence
2. migration and non-happy-path validation still trail the analyzer surface
3. GUI and legacy surfaces still lag the v2 doctrine
4. richer contract-driven obligations and changed-symbol precision are still incomplete

This batch is now largely delivered. The remaining open work is mostly deeper follow-through, not missing first-pass implementation.

## Scope

Included:

1. benchmark-threshold policy and warm-path performance work
2. migration and release-grade validation hardening
3. GUI and remaining legacy-surface alignment
4. richer contract-driven obligation precision

Explicitly excluded from this batch:

- broad new Tier 3 implicit-state heuristics
- third benchmark repo onboarding unless current proof breaks down
- new dashboard work unrelated to v2 patch safety or evidence-first debt output

## Current Baseline

At the start of this batch:

- the core wedge is working in MCP and CLI
- the proof loop spans `parallel-code` and `private-benchmark-repo`
- `findings` and `session_end` include:
  - concept summaries
  - quality opportunities
  - debt signals
- clone findings are git-aware and family-clustered
- obligations cover closed-domain changes plus initial contract-driven triggers

Relevant references:

- [Implementation Status](../../v2/implementation-status.md)
- [Roadmap](../../v2/roadmap.md)
- [Testing And Validation](../../v2/testing-and-validation.md)
- [MCP And CLI](../../v2/mcp-and-cli.md)
- [Parallel-Code Case Study](../../v2/parallel-code-case-study.md)
- [Parallel-Code Scoped Goldens](../../v2/examples/parallel-code-golden/README.md)
- [Parallel-Code Benchmark](../../v2/examples/parallel-code-benchmark.md)
- [Private Benchmark Repo Scoped Goldens](../../v2/examples/private-benchmark-repo-golden/README.md)
- [Private Benchmark Repo Benchmark](../../v2/examples/private-benchmark-repo-benchmark.md)

## Success Criteria

This batch is successful when all of the following are true:

1. warm patch-safety runs have an explicit threshold policy and stable comparison flow
2. migration and non-happy-path validation cover the remaining release-risk edges
3. GUI and legacy surfaces no longer tell a different product story than MCP and CLI
4. contract-driven obligation triggers cover more real propagation failures without adding noisy overreach
5. the debt signals remain useful on real repos after the added precision and validation work

## Work Package A: Benchmark Policy And Warm-Path Performance

Status: mostly complete

Goal:

- make performance confidence good enough for release gating and habitual use

Deliverables:

- explicit cold and warm benchmark-threshold policy
- benchmark regression classification:
  - fail
  - warn
  - informational
- targeted reduction in remaining scan-bound warm-path cost

Tasks:

- [x] define benchmark-threshold policy for `parallel-code` and `private-benchmark-repo`
- [x] classify which benchmark regressions should fail CI versus warn only
- [x] profile remaining warm-path scan/evolution cost in `gate` and `session_end`
- [x] remove one warm-path overhead source by caching rules config across repeated MCP requests
- [x] document the expected warm-path budget in the validation docs

Acceptance criteria:

- performance regressions are explicit and reviewable
- warm patch-safety latency improves without correctness regressions

Open gap:

- structural scan and changed-file bookkeeping still dominate the remaining warm patch-safety cost

## Work Package B: Migration And Release-Grade Validation

Status: mostly complete

Goal:

- close the remaining validation gap between “good beta wedge” and “release-grade proof”

Deliverables:

- fuller v1/v2 migration suite
- broader schema/version mismatch coverage
- broader non-happy-path proof for benchmark repos

Tasks:

- [x] add explicit schema/version mismatch migration tests
- [x] add more malformed or stale session-baseline scenarios
- [-] add more non-happy-path golden validation for benchmark repos
- [x] capture a short release checklist for proof artifacts and migration checks

Acceptance criteria:

- migration and baseline compatibility failures are predictable and tested
- the proof loop covers both happy and unhappy paths

Open gap:

- benchmark-repo validation still needs deeper unhappy-path coverage beyond the current baseline and format mismatch checks

## Work Package C: GUI And Legacy Surface Alignment

Status: mostly complete

Goal:

- make the product story consistent across the remaining surfaces

Deliverables:

- legacy structural surfaces clearly framed as supporting context
- GUI wording aligned with findings, obligations, and debt signals
- confidence and suppression state visible where it matters

Tasks:

- [x] audit remaining GUI and legacy CLI surfaces for score-first framing
- [x] align wording with MCP and CLI patch-safety surfaces
- [-] surface confidence and suppression context in the remaining high-traffic views

Acceptance criteria:

- users do not get a different quality narrative depending on which surface they open

Open gap:

- the desktop GUI now labels structural context honestly, but it still does not expose native v2 findings, obligations, or debt-signal panels

## Work Package D: Richer Contract-Driven Obligation Precision

Status: mostly complete

Goal:

- catch more real incomplete propagation failures without widening into noisy heuristics

Deliverables:

- richer contract trigger families
- better changed-symbol precision
- stronger surfacing of contract-related missing sites

Tasks:

- [x] extend contract triggers beyond the current symbol/file surface set
- [-] improve changed-symbol precision for field-level contract changes
- [x] surface contract-related missing sites by boundary crossing and runtime risk
- [-] validate the new triggers against the benchmark repos before broadening further

Acceptance criteria:

- more real contract or field changes produce useful missing-site output
- obligation noise stays low enough for continued gating

Open gap:

- changed-symbol precision is better for member-level declarations and semantically related surfaces, but the substrate still does not provide true AST diffing for contract-field edits

## Recommended Execution Order

1. Work Package A: Benchmark Policy And Warm-Path Performance
2. Work Package B: Migration And Release-Grade Validation
3. Work Package C: GUI And Legacy Surface Alignment
4. Work Package D: Richer Contract-Driven Obligation Precision

## Batch Exit Criteria

Batch result:

1. performance regressions now have a stable fail/warn/info policy
2. migration and baseline compatibility coverage are materially broader
3. the remaining user-facing structural surfaces now match the v2 doctrine more closely
4. contract-driven obligations catch more real propagation misses without degrading trust

What remains after this batch:

1. reduce the remaining scan-bound warm patch-safety cost
2. deepen benchmark-repo unhappy-path validation and analyzer promotion criteria
3. decide whether the GUI needs first-class v2 panels instead of only doctrinal alignment
4. keep tightening contract-field precision if real repo feedback demands it
