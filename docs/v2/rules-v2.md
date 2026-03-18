# Sentrux V2 Rules Model

## Goal

The v2 rules model should let projects state their architectural intent in machine-checkable form.

V1 rules are mostly threshold and layer rules. V2 rules must cover:

- concept ownership
- canonical access paths
- contract parity
- state-model expectations
- suppressions and approved exceptions

The rules model should optimize for a small number of critical concepts, not require full repo modeling before value appears.

For beta:

- zero-config findings should come from clone drift and conservative closed-domain checks
- concept-level authority, access, and obligation findings should be driven by explicit critical concept rules

## Compatibility

Keep existing v1 rules intact:

- constraints
- layers
- boundaries

Add new sections instead of breaking the current file format.

## Proposed File

`.sentrux/rules.toml`

## Core Sections

## Project

```toml
[project]
primary_language = "typescript"
exclude = ["vendor/**", "dist/**", "coverage/**"]
```

## Concept

`concept` is the main new unit.

```toml
[[concept]]
id = "task_presentation_status"
kind = "projection"
priority = "critical"
anchors = [
  "src/app/task-presentation-status.ts::TaskDotStatus",
  "src/app/task-presentation-status.ts::getTaskDotStatus",
  "src/app/task-presentation-status.ts::getTaskAttentionEntry",
]
authoritative_inputs = [
  "src/domain/server-state.ts::AgentSupervisionSnapshot",
  "src/store/core.ts::store.taskGitStatus",
]
canonical_accessors = [
  "src/app/task-presentation-status.ts::getTaskDotStatus",
  "src/app/task-presentation-status.ts::getTaskAttentionEntry",
]
forbid_raw_reads = [
  "src/components/**::store.agentSupervision",
  "src/components/**::store.taskGitStatus",
]
```

Supported fields should include:

- `id`
- `kind`
- `priority`
- `anchors`
- `authoritative_inputs`
- `allowed_writers`
- `forbid_writers`
- `canonical_accessors`
- `forbid_raw_reads`
- `related_tests`

## Contract

Use `contract` for cross-runtime and cross-boundary parity expectations.

```toml
[[contract]]
id = "server_state_bootstrap"
kind = "runtime_parity"
priority = "critical"
categories_symbol = "src/domain/server-state-bootstrap.ts::SERVER_STATE_BOOTSTRAP_CATEGORIES"
payload_map_symbol = "src/domain/server-state-bootstrap.ts::ServerStateBootstrapPayloadMap"
registry_symbol = "src/app/server-state-bootstrap-registry.ts::SERVER_STATE_BOOTSTRAP_REGISTRY"
browser_entry = "src/runtime/browser-session.ts"
electron_entry = "src/app/desktop-session.ts"
required_capabilities = ["snapshot", "live_updates", "versioning"]
```

## State Model

Use `state_model` when a module is expected to be explicit and exhaustive.

```toml
[[state_model]]
id = "browser_state_sync"
roots = ["src/runtime/browser-state-sync-controller.ts"]
kind = "explicit"
require_exhaustive_switch = true
require_assert_never = true
```

## Suppression

Suppressions are necessary for approved exceptions.

```toml
[[suppress]]
kind = "multi_writer_concept"
concept = "legacy_browser_state"
reason = "Temporary migration bridge"
expires = "2026-06-30"
```

Suppression fields:

- `kind`
- `concept` or `file`
- `reason`
- `expires`

Expired suppressions should become findings.

## Derived Rules From Architecture Tests

Some repos already encode architecture expectations in test files such as:

- `.architecture.test.ts`

V2 should not depend on these tests, and test-ingestion sophistication should not block the wedge.

But it should be able to:

1. discover them
2. use them as rule-coverage evidence
3. optionally bind them to explicit concepts and contracts

This is particularly relevant for the `parallel-code` case study.

## Rule Resolution Order

When both heuristic inference and explicit rules exist:

1. explicit rule wins
2. explicit suppression wins over heuristic finding
3. heuristic result remains visible, but lower priority

## Rule Coverage

Every v2 run should compute rule coverage:

- declared concepts with at least one machine-checked condition
- declared contracts with parity cells checked
- declared state models with explicit structural checks

This score must be exposed directly.

## Validation Semantics

V2 rules should be static-only.

Allowed evidence sources:

- compiler-backed symbol graph
- normalized access paths
- registry tables
- imports and references
- guardrail tests as static source

Not allowed for v2 rule compliance:

- runtime traces
- executed UI flows
- behavioral logs

## Adoption Guidance

Teams should be able to start with:

1. zero-config heuristic findings
2. 3 to 5 critical concept rules
3. one or two critical contracts

The rules model should reward this small-config path rather than assuming a complete architecture specification.

## Implementation Tasks

- [ ] extend `RulesConfig` with v2 sections
- [ ] add parser structs for `project`, `concept`, `contract`, `state_model`, and `suppress`
- [ ] add normalization for symbol anchors and access-path patterns
- [ ] implement suppression matching and expiry
- [ ] compute rule coverage metrics
- [ ] expose rule coverage in MCP
- [x] provide example `rules.toml` for `parallel-code`
  - current example: [examples/parallel-code.rules.toml](./examples/parallel-code.rules.toml)
