# Sentrux V2 TypeScript Bridge

## Purpose

The TypeScript bridge is the critical technical dependency for v2.

Rust needs compiler-backed TypeScript facts, but the TypeScript compiler and language service live in Node.

This document defines the Rust ↔ TypeScript bridge architecture for beta.

## Decision

For beta, the TypeScript frontend should run as a persistent Node subprocess managed by Rust.

Rejected beta options:

- tree-sitter-only heuristics for semantic facts
- Rust-native parsers without type information
- one-shot `tsc` invocations per file or per query
- wasm or embedded alternatives that do not provide language-service-equivalent semantics

## Why This Approach

The beta wedge needs:

- symbol declarations
- reference tracking
- resolved module structure
- union and literal domain extraction
- control-flow-backed exhaustiveness checks
- incremental updates

The persistent Node subprocess is the only practical beta design that delivers all of those with acceptable fidelity.

## Requirements

The bridge must satisfy all of these:

1. persistent process, not one-shot invocations
2. incremental project updates
3. compiler-backed facts, not syntax-only facts
4. crash recovery and restart
5. explicit capability and version handshake
6. graceful fallback when Node is unavailable
7. fact responses serialized into Rust-owned cacheable shapes

The warm-run target in the roadmap depends on this persistence requirement.

## Process Model

Rust owns bridge lifecycle.

Suggested model:

1. Rust discovers one or more TypeScript projects
2. Rust spawns one persistent Node bridge process per repo analysis run
3. The Node bridge manages one or more language-service instances keyed by `tsconfig`
4. Rust sends requests and file updates
5. The Node bridge returns normalized semantic facts

Rust remains the system of record for:

- cache
- baseline
- concept graph
- findings
- obligations
- gating

The Node side should only own TypeScript compiler interactions and local normalization close to the language service.

## Communication Protocol

Use JSON-RPC over stdio for beta.

Reasons:

- cross-platform
- easy process supervision from Rust
- request/response shape fits the workload
- easy to log, debug, and version

Required protocol messages:

1. `initialize`
2. `analyze_projects`
3. `update_files`
4. `analyze_changed_files`
5. `get_semantic_facts`
6. `shutdown`
7. `ping`

The bridge should expose protocol version and capabilities in `initialize`.

## Request And Response Ownership

Rust should not ask the bridge thousands of tiny symbol questions.

The bridge should return coarse fact batches for a project or changed file set:

- `SymbolFact[]`
- `ReferenceFact[]`
- `ReadFact[]`
- `WriteFact[]`
- `ClosedDomain[]`
- `MappingSite[]`
- `ProtocolContract[]`

This keeps IPC coarse-grained enough for performance.

## Project Lifecycle

For each discovered `tsconfig`:

1. create or reuse a language-service instance
2. load source files into the language service
3. keep file snapshots and versions in memory
4. update only changed files on incremental runs
5. rebuild project-local fact batches on demand

`tsconfig` changes should invalidate the affected project instance and trigger rebuild.

## Failure Modes

## Node Missing

If Node is not installed or the bridge cannot start:

- report that TypeScript semantic analysis is unavailable
- keep Tier 0 outputs and clone-drift outputs working
- downgrade concept-level semantic findings to unavailable, not failed

Do not pretend the repo is healthy.

## Bridge Crash

If the bridge crashes:

1. mark semantic coverage unavailable for the affected run
2. restart the bridge once
3. retry the current request if safe
4. surface a warning if the retry also fails

## Version Mismatch

If the bridge protocol version or TypeScript version is unsupported:

- fail initialization cleanly
- surface bridge incompatibility in confidence output

## Cache Boundary

The bridge should not write directly into Sentrux cache files.

Instead:

1. Rust sends project and file-change requests
2. Node returns semantic fact batches
3. Rust serializes normalized results into `.sentrux/cache/v2`

This keeps cache format stable and Rust-owned.

## Performance Constraints

The bridge architecture must support:

- persistent process reuse
- incremental file updates
- coarse-grained fact responses
- pipeline parallelism with structural scan and clone drift

Without these, the warm-run target is not credible.

## Beta Scope

The beta bridge only needs to support the facts required for:

1. clone drift correlation inputs
2. explicit `[[concept]]`-driven authority and access findings
3. obligation completeness on closed-domain changes

It does not need to solve:

- general semantic clone detection
- fully automatic concept inference
- complete state-machine inference

## Implementation Tasks

- [ ] choose JSON-RPC over stdio as the bridge transport
- [ ] define bridge protocol version and capability handshake
- [ ] implement Rust bridge supervisor
- [ ] implement Node bridge bootstrap
- [ ] implement per-`tsconfig` language-service lifecycle
- [ ] implement project-wide semantic fact batch responses
- [ ] implement incremental file update protocol
- [ ] implement crash recovery and restart behavior
- [ ] implement Node-missing fallback behavior
- [ ] document minimum supported Node and TypeScript versions
