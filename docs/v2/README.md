# Sentrux V2

Status: active implementation with a strong core wedge, fast-path `check`, and a growing signal-quality proof loop

Current implementation audit: [Implementation Status](./implementation-status.md)

This folder is the implementation source of truth for Sentrux v2.

V2 does not replace the current structural scan and quality signal immediately. It adds a second lane focused on static architectural conformance, obligation completeness, and concept ownership. The current v1 lane remains available for backward compatibility.

## Why V2 Exists

The current system is strongest at structural topology:

- import graphs
- call graphs
- cycles
- depth
- complexity concentration
- redundancy

That is useful context, but it is not the highest-leverage feedback loop for AI-generated code on modern application codebases such as `parallel-code`.

The next version should answer questions like:

- Which module is the authority for this concept?
- Are components reading canonical projections or raw authoritative state?
- If a closed-domain concept changed, which files were required to change with it?
- Do browser and Electron implement the same restore/bootstrap contract?
- Is stateful logic explicit and exhaustive, or implicit and drifting?
- Which modules have become risky coordination hotspots?
- Which modules are accumulating technical debt through duplication, drift, or boundary erosion?

## V2 Principles

1. Static-only by default.
2. Concepts before files.
3. Obligations before generic scores.
4. Rules over heuristics when rules exist.
5. Confidence is part of the product.
6. V1 compatibility is preserved during rollout.
7. Patch safety comes before repo-wide scoring.
8. Engineers own final prioritization.

## Document Map

- [Product Doctrine](./doctrine.md)
- [Implementation Status](./implementation-status.md)
- [Core Spec](./spec.md)
- [Data Model](./data-model.md)
- [Analyzer Pipeline](./analyzer-pipeline.md)
- [TypeScript Bridge](./typescript-bridge.md)
- [Testing And Validation](./testing-and-validation.md)
- [Release Checklist](./release-checklist.md)
- [Validation Loop](./validation-loop.md)
- [Parallel-Code Proof Board](./parallel-code-proof-board.md)
- [Parallel-Code Proof Review](./parallel-code-proof-review.md)
- [False-Positive Review](./false-positive-review.md)
- [Baseline Migration](./baseline-migration.md)
- [Rules V2](./rules-v2.md)
- [MCP And CLI](./mcp-and-cli.md)
- [Roadmap](./roadmap.md)
- [Next Implementation Batch](./next-implementation-batch.md)
- [Evaluation Synthesis](./evaluation-synthesis.md)
- [Parallel-Code Case Study](./parallel-code-case-study.md)
- [Parallel-Code Scoped Goldens](./examples/parallel-code-golden/README.md)
- [Parallel-Code Benchmark](./examples/parallel-code-benchmark.md)
- [Parallel-Code Proof Snapshot](./examples/parallel-code-proof-snapshot.md)
- [Parallel-Code Proof Runs](./examples/parallel-code-proof-runs/README.md)
- [Private Benchmark Repo Scoped Goldens](./examples/private-benchmark-repo-golden/README.md)
- [Private Benchmark Repo Benchmark](./examples/private-benchmark-repo-benchmark.md)
- [Private Frontend Scoped Goldens](./examples/private-frontend-golden/README.md)
- [Private Frontend Benchmark](./examples/private-frontend-benchmark.md)

## Deliverable Shape

V2 should produce five classes of output:

1. fast patch `check`
2. agent guidance
3. findings and obligations
4. session delta
5. confidence and signal-quality scorecards

The primary product question changes from:

> What is the one quality number?

to:

> What did this patch change, what obligations did that create, and what did the agent fail to update?

V2 now also exposes:

- debt signals
- debt clusters
- watchpoints
- experimental findings and experimental debt signals as quarantined side channels
- normalized finding details
- patch risks
- trust tiers
- leverage classes and leverage reasons for engineering meaning
- candidate split axes and related surfaces for fix-oriented inspection
- project-shape detection, archetype matches, and starter-rule suggestions for repo onboarding
- adoption-ready working rules generated from project shape
- module-contract rules for module public APIs and cross-module deep-import enforcement
- persisted semantic cache reuse across repeated MCP requests and fresh-process reruns when the repo fingerprint still matches
- external evaluator harness scaffolding plus a focused `dead_private` review loop
- defect-injection, review-packet, remediation-eval, and signal-scorecard tooling for proving signal quality

Any compatibility fields that still mention quality opportunities or optimization-style sorting should be treated as legacy aliases for inspection candidates only. Engineers own the final prioritization.

## Core Wedge

The highest-ROI v2 wedge is deliberately narrow:

1. clone drift
2. authority and access
3. obligation completeness

For beta:

- zero-config findings should come from clone drift, conservative closed-domain checks, missing-test watchpoints, and conservative inferred boundary checks
- concept-level authority, access, and obligation findings should rely on explicit critical concept rules

Everything else is either support context or later-stage analysis.

## Relationship To The Current Codebase

V2 is designed to extend the current architecture, not replace it:

- keep `core::snapshot::Snapshot`
- keep `metrics::HealthReport`
- keep the current tree-sitter structural scan
- add a semantic lane beside the structural lane
- add new MCP tools instead of breaking existing ones

The current anchor files are:

- `sentrux-core/src/core/snapshot.rs`
- `sentrux-core/src/metrics/types.rs`
- `sentrux-core/src/metrics/mod.rs`
- `sentrux-core/src/metrics/rules/mod.rs`
- `sentrux-core/src/app/mcp_server/mod.rs`
- `sentrux-core/src/app/mcp_server/handlers.rs`

## Immediate Build Order

The shortest path to usefulness is:

1. trust and reporting foundation
2. clone drift fast lane
3. TypeScript semantic substrate
4. minimal concept graph and rules
5. authority and access findings
6. obligation completeness
7. upgraded `session_end` and CI ratchet
8. parity and concentration as secondary context

Current reality:

1. the patch-safety wedge is working in MCP and CLI
2. `check` is now the default fast-path patch surface for agents
3. proof artifacts now exist for `parallel-code` and `private-benchmark-repo`
4. proof artifacts now also exist for `private-frontend` as the modular Next.js frontend benchmark repo
5. findings and `session_end` now include concept summaries, structural debt signals, debt clusters, normalized finding details, and watchpoints
6. trusted findings, watchpoints, and experimental findings are now separated in the primary MCP surfaces
7. cycle-cluster reports now include concrete cut-candidate evidence instead of only SCC membership
8. v2 now includes a generic archetype/onboarding layer with `project_shape` output, framework-aware role defaults, starter-rule generation, and adoption-ready working rules
9. the quality loop now includes seeded defect injection, review packets, remediation eval scaffolding, and per-signal scorecards
10. the remaining gaps are broader unhappy-path validation, proof-run refresh follow-through, dead-private detector promotion beyond experimental, deeper cache incrementality, and broader Tier 3 analysis beyond the new explicit transition-integrity slice

## Exit Criteria For V2 Beta

V2 is ready for beta when all of the following are true:

- it can analyze `parallel-code` with a TypeScript semantic frontend
- it emits actionable findings and obligations for changed concepts
- it surfaces clone drift, authority/access regressions, and incomplete propagation
- it upgrades `session_end` into a useful patch-safety report
- it exposes confidence and analysis coverage explicitly
- it supports touched-concept CI ratchets with high-confidence findings
- the current v1 MCP workflow still works unchanged
