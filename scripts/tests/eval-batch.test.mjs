import assert from 'node:assert/strict';
import test from 'node:test';

import { resolveManifestPath, summarizeBundleOutcome } from '../lib/eval-batch.mjs';

test('summarizeBundleOutcome prefers telemetry-derived initial action kinds', function () {
  const outcome = summarizeBundleOutcome({
    outcome: {
      initial_action_kinds: ['large_file'],
      initial_top_action_kind: 'large_file',
      final_gate: 'warn',
      final_session_clean: false,
    },
    initial_check: {
      actions: [],
    },
  });

  assert.deepEqual(outcome.initial_action_kinds, ['large_file']);
  assert.equal(outcome.initial_top_action_kind, 'large_file');
  assert.deepEqual(outcome.top_action, {
    kind: 'large_file',
    presented: true,
    cleared: false,
    checks_to_clear: null,
    followed: null,
    helped: null,
  });
});

test('summarizeBundleOutcome falls back to initial check actions', function () {
  const outcome = summarizeBundleOutcome({
    outcome: {
      initial_top_action_kind: 'forbidden_raw_read',
      final_gate: 'fail',
      final_session_clean: false,
    },
    initial_check: {
      actions: [{ kind: 'forbidden_raw_read' }, { kind: 'missing_test_coverage' }],
    },
  });

  assert.deepEqual(outcome.initial_action_kinds, [
    'forbidden_raw_read',
    'missing_test_coverage',
  ]);
  assert.equal(outcome.top_action.kind, 'forbidden_raw_read');
  assert.equal(outcome.top_action.presented, true);
});

test('summarizeBundleOutcome normalizes failed sessions away from clean pass outcomes', function () {
  const outcome = summarizeBundleOutcome({
    status: 'provider_failed',
    outcome: {
      initial_action_kinds: [],
      initial_top_action_kind: null,
      convergence_status: 'converged',
      final_gate: 'pass',
      final_session_clean: true,
      checks_to_clear_top_action: 1,
    },
    initial_check: {
      actions: [],
    },
  });

  assert.equal(outcome.final_gate, 'warn');
  assert.equal(outcome.final_session_clean, false);
  assert.equal(outcome.convergence_status, 'provider_failed');
  assert.equal(outcome.checks_to_clear_top_action, null);
  assert.deepEqual(outcome.top_action, {
    kind: null,
    presented: false,
    cleared: false,
    checks_to_clear: null,
    followed: null,
    helped: null,
  });
});

test('resolveManifestPath resolves repo roots relative to the manifest file', function () {
  assert.equal(
    resolveManifestPath(
      '/workspace/sentrux/docs/v2/evals/repos/parallel-code.json',
      '../../../../../parallel-code',
    ),
    '/workspace/parallel-code',
  );
  assert.equal(
    resolveManifestPath('/workspace/sentrux/docs/v2/evals/repos/sentrux.json', '../../../..'),
    '/workspace/sentrux',
  );
  assert.equal(
    resolveManifestPath('/workspace/sentrux/docs/v2/evals/repos/sentrux.json', '/tmp/repo'),
    '/tmp/repo',
  );
  assert.equal(resolveManifestPath('/workspace/sentrux/docs/v2/evals/repos/sentrux.json', null), null);
});
