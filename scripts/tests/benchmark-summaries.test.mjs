import test from 'node:test';
import assert from 'node:assert/strict';
import {
  summarizeFindings,
  summarizeProjectShape,
  summarizeCheck,
} from '../lib/benchmark-summaries.mjs';

test('summarizeFindings reads canonical debt and watchpoint fields only', function () {
  const summary = summarizeFindings({
    debt_signal_count: 4,
    watchpoint_count: 2,
    quality_opportunity_count: 99,
    optimization_priority_count: 88,
    findings: [{}, {}],
  });

  assert.equal(summary.debt_signal_count, 4);
  assert.equal(summary.watchpoint_count, 2);
  assert.equal(summary.finding_count, 2);
});

test('summarizeProjectShape reads the canonical project_shape field', function () {
  const summary = summarizeProjectShape({
    project_shape: {
      primary_archetype: 'modular_frontend',
      effective_archetypes: ['modular_frontend', 'nextjs_app_router'],
      capabilities: ['app_router'],
      boundary_roots: ['src/features'],
      module_contracts: ['src/features/index.ts'],
    },
    shape: {
      primary_archetype: 'stale_alias',
    },
  });

  assert.equal(summary.primary_archetype, 'modular_frontend');
  assert.equal(summary.effective_archetype_count, 2);
  assert.equal(summary.capability_count, 1);
  assert.equal(summary.boundary_root_count, 1);
  assert.equal(summary.module_contract_count, 1);
});

test('summarizeCheck reports gate, issue counts, and availability', function () {
  const summary = summarizeCheck({
    gate: 'warn',
    issues: [{}, {}],
    changed_files: ['src/app.ts'],
    diagnostics: {
      partial_results: true,
      availability: {
        changed_scope: false,
      },
    },
  });

  assert.equal(summary.gate, 'warn');
  assert.equal(summary.issue_count, 2);
  assert.equal(summary.changed_file_count, 1);
  assert.equal(summary.partial_results, true);
  assert.equal(summary.changed_scope_available, false);
});
