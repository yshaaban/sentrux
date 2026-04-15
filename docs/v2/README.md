# Sentrux V2

Status: public docs refreshed on 2026-04-15

V2 is the patch-safety and structured-guidance lane for Sentrux. It sits beside the older structural lane rather than replacing it outright.

Current surface reality:

- MCP `check` is the fast-path v2 patch surface
- MCP `agent_brief`, `findings`, `obligations`, `gate`, and `session_end` carry the broader structured evidence
- CLI `sentrux brief` and `sentrux gate` are the current v2 CLI entry points
- CLI `sentrux check` is still the legacy structural rules check

This folder is the maintained source of truth for the current v2 shape.

## Start Here

- [Implementation Status](./implementation-status.md)
- [MCP And CLI](./mcp-and-cli.md)
- [Testing And Validation](./testing-and-validation.md)
- [Release Checklist](./release-checklist.md)
- [Roadmap](./roadmap.md)

## Core Design

- [Doctrine](./doctrine.md)
- [Spec](./spec.md)
- [Data Model](./data-model.md)
- [Analyzer Pipeline](./analyzer-pipeline.md)
- [TypeScript Bridge](./typescript-bridge.md)
- [Rules V2](./rules-v2.md)
- [Baseline Migration](./baseline-migration.md)

## Validation And Evidence

- [Validation Loop](./validation-loop.md)
- [False-Positive Review](./false-positive-review.md)
- [Parallel-Code Case Study](./parallel-code-case-study.md)
- [Eval Harness](./evals/README.md)

## Checked-In Reference Artifacts

- [Parallel-Code Goldens](./examples/parallel-code-golden/README.md)
- [Parallel-Code Benchmark](./examples/parallel-code-benchmark.md)
- [Parallel-Code Proof Snapshot](./examples/parallel-code-proof-snapshot.md)
- [Parallel-Code Proof Runs](./examples/parallel-code-proof-runs/README.md)
- [Private Benchmark Repo Goldens](./examples/private-benchmark-repo-golden/README.md)
- [Private Benchmark Repo Benchmark](./examples/private-benchmark-repo-benchmark.md)
- [Private Frontend Goldens](./examples/private-frontend-golden/README.md)
- [Private Frontend Benchmark](./examples/private-frontend-benchmark.md)

## Historical Material

Superseded planning notes and prototype tracking docs were moved out of the public index into [`../archive/`](../archive/README.md).
