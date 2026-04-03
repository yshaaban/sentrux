import test from 'node:test';
import assert from 'node:assert/strict';

import { buildSignalBacklog, formatSignalBacklogMarkdown } from '../lib/signal-backlog.mjs';

test('buildSignalBacklog highlights weak cohort signals and next candidates', function () {
  const backlog = buildSignalBacklog({
    cohort: {
      cohort_id: 'agent-loop-core',
      signals: [
        { signal_kind: 'closed_domain_exhaustiveness' },
        { signal_kind: 'forbidden_raw_read' },
        { signal_kind: 'session_introduced_clone' },
        { signal_kind: 'incomplete_propagation' },
      ],
    },
    scorecard: {
      signals: [
        {
          signal_kind: 'closed_domain_exhaustiveness',
          promotion_status: 'trusted',
          promotion_recommendation: 'keep_trusted',
        },
        {
          signal_kind: 'forbidden_raw_read',
          promotion_status: 'trusted',
          promotion_recommendation: 'improve_fix_guidance',
          session_clean_rate: 0.4,
          remediation_success_rate: 0.5,
        },
      ],
    },
    codexBatch: {
      results: [
        {
          task_id: 'task-1',
          task_label: 'Fix boundary break',
          expected_signal_kinds: ['incomplete_propagation'],
          outcome: {
            initial_top_action_kind: null,
            initial_action_kinds: [],
            final_gate: 'fail',
            final_session_clean: false,
            followup_regression_introduced: false,
          },
        },
      ],
    },
    replayBatch: {
      results: [
        {
          replay_id: 'commit-1',
          commit: 'abc123',
          expected_signal_kinds: ['incomplete_propagation', 'forbidden_raw_read'],
          outcome: {
            initial_top_action_kind: 'forbidden_raw_read',
            initial_action_kinds: ['forbidden_raw_read'],
            top_action_cleared: true,
            final_gate: 'warn',
            final_session_clean: false,
            followup_regression_introduced: true,
          },
        },
      ],
    },
  });

  assert.equal(backlog.weak_signals.length, 1);
  assert.equal(backlog.weak_signals[0].signal_kind, 'forbidden_raw_read');
  assert.equal(backlog.summary.recommended_next_signal, null);
  assert.equal(backlog.next_signal_candidates.length, 0);
  assert.equal(backlog.active_signal_misses[0].signal_kind, 'incomplete_propagation');
  assert.equal(backlog.active_signal_misses[0].miss_count, 2);
  assert.equal(backlog.active_signal_misses[1].signal_kind, 'forbidden_raw_read');
  assert.equal(backlog.active_signal_misses[1].regression_followup_count, 1);
  assert.equal(backlog.live_misses.length, 1);
  assert.equal(backlog.replay_misses.length, 1);
});

test('buildSignalBacklog ignores clean sessions that simply lacked an expected signal', function () {
  const backlog = buildSignalBacklog({
    cohort: {
      cohort_id: 'agent-loop-core',
      signals: [{ signal_kind: 'missing_test_coverage' }],
    },
    scorecard: { signals: [] },
    codexBatch: {
      results: [
        {
          task_id: 'task-clean',
          task_label: 'Clean task',
          expected_signal_kinds: ['missing_test_coverage'],
          outcome: {
            initial_top_action_kind: null,
            initial_action_kinds: [],
            final_gate: 'pass',
            final_session_clean: true,
            followup_regression_introduced: false,
          },
        },
      ],
    },
  });

  assert.equal(backlog.live_misses.length, 0);
  assert.equal(backlog.summary.live_miss_count, 0);
  assert.equal(backlog.active_signal_misses.length, 0);
});

test('formatSignalBacklogMarkdown renders the backlog summary', function () {
  const markdown = formatSignalBacklogMarkdown({
    cohort_id: 'agent-loop-core',
    generated_at: '2026-04-02T00:00:00.000Z',
    summary: {
      weak_signal_count: 1,
      live_miss_count: 1,
      replay_miss_count: 1,
      recommended_next_signal: null,
    },
    weak_signals: [
      {
        signal_kind: 'forbidden_raw_read',
        recommendation: 'improve_fix_guidance',
        session_clean_rate: 0.4,
        remediation_success_rate: 0.5,
      },
    ],
    next_signal_candidates: [],
    active_signal_misses: [
      {
        signal_kind: 'incomplete_propagation',
        miss_count: 2,
        live_miss_count: 1,
        replay_miss_count: 1,
        regression_followup_count: 0,
      },
    ],
  });

  assert.match(markdown, /Signal Calibration Backlog/);
  assert.match(markdown, /forbidden_raw_read/);
  assert.match(markdown, /Active Signal Misses/);
  assert.match(markdown, /incomplete_propagation/);
  assert.doesNotMatch(markdown, /recommended next signal: `incomplete_propagation`/);
});
