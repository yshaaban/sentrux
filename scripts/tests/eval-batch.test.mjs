import assert from 'node:assert/strict';
import test from 'node:test';

import { summarizeBundleOutcome } from '../lib/eval-batch.mjs';

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
});
