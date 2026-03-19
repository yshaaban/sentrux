# Sentrux V2 MCP And CLI Spec

## Design Goal

V2 should be consumable by agents without breaking the current MCP workflow.

That means:

- keep existing tools working
- add patch-safety-first v2 tools
- make `session_end` the primary agent touchpoint
- return findings and obligations before score summaries

## Product Surface Priority

For v2 integrations, the preferred order is:

1. `session_end`
2. `findings`
3. `obligations`
4. `gate`
5. `scorecard`
6. concept and parity inspection tools

This ordering should shape both MCP and CLI design.

## Existing Tools To Preserve

Keep these unchanged during the initial v2 rollout:

- `scan`
- `rescan`
- `health`
- `session_start`
- `session_end`
- `check_rules`
- `evolution`
- `dsm`
- `test_gaps`

## Relationship Between `health` And V2 Tools

`health` remains the v1-compatible summary tool.

For v2:

- `health` should keep its legacy role as a lightweight structural summary
- `findings`, `obligations`, `session_end`, and `gate` should carry the actionable v2 diagnostics
- `health` may eventually link to the presence of v2 data, but it should not duplicate full v2 finding lists

Agents should use v2 tools for patch-safety decisions.

## New Primary Tools

## `findings`

Purpose:

- return concrete patch-safety findings with evidence

Arguments:

- `severity`
- `kind`
- `concept`
- `scope = "all" | "changed"`
- `limit`

Returns:

- findings
- evidence
- likely fix sites
- concept summaries for repeated concept pressure
- ranked quality-improvement opportunities
- confidence

## `obligations`

Purpose:

- return required update sites and what is still missing

Arguments:

- `scope = "all" | "changed"`
- `concept`
- `file`
- `symbol`

Returns:

- obligation summaries
- missing sites
- satisfied sites
- obligation count
- derived context burden

## `gate`

Purpose:

- answer whether the current patch regressed touched concepts enough to warn or fail

Arguments:

- `scope = "changed"`
- `strict = true | false`

Returns:

- decision
- blocking findings
- confidence
- explanation

## `session_end` Upgrade

The upgraded `session_end` response is the primary v2 product surface.

It should add:

- changed files
- changed concepts
- introduced findings
- resolved findings
- missing obligations
- concept summaries for changed concepts
- patch-scoped quality-improvement opportunities
- track deltas
- touched-concept regression verdict
- confidence delta if coverage changed

## Secondary Tools

## `scorecard`

Purpose:

- return grouped v2 tracks as supporting context

Arguments:

- none

Returns:

- core tracks
- context tracks
- future tracks if available
- confidence
- exclusions

## `concepts`

Purpose:

- list known concepts and their ownership or canonical adapters

Arguments:

- `kind`
- `limit`

## `explain_concept`

Purpose:

- drill into one concept

Arguments:

- `id`

Returns:

- anchors
- authoritative writers
- canonical accessors
- contracts
- findings
- obligations

## `trace_symbol`

Purpose:

- trace a symbol to concepts, readers, writers, and obligations

Arguments:

- `symbol`

## `parity`

Purpose:

- return contract parity analysis as supporting context

Arguments:

- `contract`
- `scope`

## Proposed Response Shapes

## `session_end`

```json
{
  "changed_files": [
    "src/app/task-workflows.ts"
  ],
  "changed_concepts": [
    "task_git_status"
  ],
  "concept_summaries": [
    {
      "concept_id": "task_git_status",
      "summary": "Concept 'task_git_status' combines architecture violations with 2 missing update sites",
      "score_0_10000": 7800
    }
  ],
  "quality_opportunities": [
    {
      "kind": "concept",
      "scope": "task_git_status",
      "severity": "high",
      "summary": "Concept 'task_git_status' combines architecture violations with 2 missing update sites",
      "suggested_actions": [
        "centralize writes behind a single owner",
        "complete the propagation chain before extending the concept further"
      ]
    }
  ],
  "introduced_findings": [
    {
      "id": "multi_writer:task_git_status",
      "kind": "multi_writer_concept",
      "severity": "high",
      "summary": "taskGitStatus has more than one durable write path",
      "confidence": 9300
    }
  ],
  "missing_obligations": [
    {
      "concept": "task_git_status",
      "kind": "canonical_projection_update",
      "site": "src/app/task-presentation-status.ts"
    }
  ],
  "gate": {
    "decision": "warn",
    "reason": "high-confidence regression on touched concept"
  }
}
```

## `findings`

```json
{
  "concept_summaries": [
    {
      "concept_id": "task_git_status",
      "dominant_kinds": [
        "multi_writer_concept",
        "forbidden_writer"
      ],
      "summary": "Concept 'task_git_status' has repeated high-severity ownership or access issues"
    }
  ],
  "quality_opportunities": [
    {
      "kind": "concept",
      "scope": "task_git_status",
      "severity": "high",
      "summary": "Concept 'task_git_status' has repeated high-severity ownership or access issues"
    }
  ],
  "findings": [
    {
      "id": "public_api_bypass:task_workflows",
      "kind": "public_api_bypass",
      "severity": "high",
      "concept": "tasks",
      "summary": "task-workflows imports store internals directly instead of the intended public store boundary",
      "evidence": [
        {
          "file": "src/app/task-workflows.ts",
          "detail": "imports setStore from ../store/core"
        }
      ],
      "confidence": 9100,
      "likely_fix_sites": [
        {
          "site": "src/store/store.ts"
        }
      ]
    }
  ]
}
```

## `obligations`

```json
{
  "concept": "server_state_bootstrap",
  "missing": [
    {
      "kind": "registry_mapping",
      "site": "src/app/server-state-bootstrap-registry.ts"
    },
    {
      "kind": "browser_listener",
      "site": "src/runtime/browser-session.ts"
    }
  ],
  "satisfied": [
    {
      "kind": "category_list",
      "site": "src/domain/server-state-bootstrap.ts"
    }
  ],
  "context_burden": {
    "required_sites": 5,
    "affected_concepts": 1,
    "boundaries": 2
  }
}
```

## `scorecard`

```json
{
  "core_tracks": [
    {
      "id": "obligation_completeness",
      "score": 8600,
      "numerator": 43,
      "denominator": 50,
      "confidence": 9300
    }
  ],
  "context_tracks": [
    {
      "id": "analysis_coverage",
      "score": 8800,
      "numerator": 440,
      "denominator": 500,
      "confidence": 10000
    }
  ]
}
```

## CLI Surface

The CLI should mirror MCP priorities.

Suggested commands:

- `sentrux v2 findings`
- `sentrux v2 obligations`
- `sentrux v2 session-end`
- `sentrux v2 gate`
- `sentrux v2 scorecard`
- `sentrux v2 concepts`
- `sentrux v2 parity`

## MCP State Integration

Current MCP state:

- root
- cached snapshot
- cached health
- cached arch
- baseline

Add:

- `cached_v2`
- `cached_v2_baseline`

Do not overload the existing v1 baseline types.

Baseline coexistence and migration rules are defined in `baseline-migration.md`.

## Rollout Strategy

1. improve current `health` and baseline reporting first
2. ship `findings` and `obligations` before broader score tooling
3. upgrade `session_end` as soon as obligation analysis is usable
4. add `gate` when touched-concept regressions are trustworthy
5. add richer inspection tools after the wedge is stable

## Implementation Tasks

- [ ] add `cached_v2` to MCP state
- [ ] add `handlers_v2.rs`
- [ ] add `findings` tool
- [ ] add `obligations` tool
- [ ] upgrade `session_end` for v2 delta and gate data
- [ ] add `gate` tool
- [ ] add `scorecard` tool
- [ ] add `concepts` tool
- [ ] add `explain_concept` tool
- [ ] add `trace_symbol` tool
- [ ] add `parity` tool
- [ ] add CLI wrappers after MCP stabilizes
