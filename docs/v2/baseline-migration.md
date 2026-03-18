# Sentrux V2 Baseline Migration

## Purpose

V2 introduces richer patch-safety outputs than v1.

This document defines how v1 and v2 baselines coexist during rollout.

## Design Goals

The migration strategy must:

1. preserve v1 compatibility
2. avoid corrupting existing v1 baseline behavior
3. let v2 compute richer deltas without guessing from v1-only data
4. keep mixed-version sessions predictable

## Decision

Keep v1 and v2 baselines separate.

For rollout:

- keep the existing v1 baseline at its current path
- store the v2 baseline at a separate path

Suggested v2 path:

`.sentrux/baseline.v2.json`

This avoids cross-version ambiguity and lets v1 binaries ignore v2 baseline files safely.

## Baseline Roles

## V1 Baseline

Used for:

- legacy structural health deltas
- existing v1 MCP and CLI workflows

Not used for:

- v2 findings
- v2 obligations
- touched-concept ratchets

## V2 Baseline

Used for:

- session delta
- touched-concept regression gates
- v2 findings and obligation comparisons
- v2 scorecard track deltas

## V2 Baseline Schema

The v2 baseline should include at least:

1. schema version
2. Sentrux version
3. project root fingerprint
4. concept summaries
5. findings snapshot
6. obligation snapshot
7. scorecard snapshot
8. confidence snapshot

It should not try to persist raw TypeScript compiler state.

## Upgrade Behavior

## Case 1: V1 Baseline Exists, No V2 Baseline Exists

Behavior:

- keep using the v1 baseline for legacy outputs
- mark v2 delta as unavailable until a v2 baseline is created
- allow the user or session flow to create the first v2 baseline explicitly

V2 should not fabricate a synthetic v2 baseline from the v1 file.

## Case 2: V2 Baseline Exists, V1 Baseline Does Not

Behavior:

- v2 workflows operate normally
- v1 tools continue without delta or with their current fallback behavior

## Case 3: Both Baselines Exist

Behavior:

- v1 tools read only v1 baseline
- v2 tools read only v2 baseline

This is the normal coexistence state during rollout.

## Mixed-Version Sessions

If a session starts under v1 and ends under v2, or the reverse:

- do not attempt cross-version delta stitching
- report that no compatible baseline exists for that versioned session output
- allow a new baseline to be created for the active version

Predictability matters more than clever migration.

## Reading Rules

V1 binary behavior:

- ignore `.sentrux/baseline.v2.json`

V2 binary behavior:

- read v2 baseline for v2 outputs
- optionally read v1 baseline only to populate legacy structural context, never to synthesize v2 findings or obligations

## Baseline Creation

Recommended behavior:

1. `session_start` records the active v2 baseline if present
2. `session_end` compares against it
3. a successful ratchet update writes a new v2 baseline

For CI:

- the gate should be able to read a checked-in or cached v2 baseline
- updating the baseline should remain an explicit action

## Failure Modes

## Schema Mismatch

If the v2 baseline schema version is unsupported:

- ignore the baseline for v2 delta
- surface a compatibility warning
- do not overwrite the file automatically

## Project Mismatch

If the project fingerprint differs:

- ignore the baseline
- surface mismatch in confidence or delta diagnostics

## Partial Baseline

If fields are missing:

- treat the baseline as invalid for delta computation
- do not silently compute partial patch safety results from incomplete baseline data

## Implementation Tasks

- [ ] define v2 baseline schema and versioning
- [ ] store v2 baseline at a dedicated path
- [ ] keep v1 baseline reader unchanged
- [ ] teach v2 to ignore incompatible or missing baselines cleanly
- [ ] add mixed-version session tests
- [ ] document baseline update workflow for CI and MCP sessions
