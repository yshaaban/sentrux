import assert from 'node:assert/strict';
import test from 'node:test';

import { summarizeTaskRuns } from '../evals/run-codex-session-batch.mjs';

function buildTelemetrySummary(sessionRunId, signalKind) {
  return {
    schema_version: 1,
    generated_at: '2026-04-12T00:00:00.000Z',
    repo_root: '/tmp/repo',
    source_path: null,
    summary: {
      event_count: 4,
      session_count: 1,
      explicit_session_count: 1,
      implicit_session_count: 0,
      check_run_count: 2,
    },
    sessions: [
      {
        session_run_id: sessionRunId,
        session_mode: 'explicit',
        session_started: true,
        session_ended: true,
        initial_gate: 'warn',
        initial_top_action_kind: signalKind,
        top_action_cleared: false,
        checks_to_clear_top_action: null,
        followup_regression_introduced: false,
        final_decision: 'warn',
        final_gate: 'warn',
        final_session_clean: false,
        check_run_count: 2,
        top_action_kinds: [signalKind],
      },
    ],
    signals: [
      {
        signal_kind: signalKind,
        top_action_presented: 1,
        followup_checks: 1,
        target_cleared: 0,
        followup_regressions: 0,
        sessions_cleared: 0,
        sessions_clean: 0,
        total_checks_to_clear: 0,
        resolution_rate: 0,
        regression_rate: 0,
        session_clear_rate: 0,
        session_clean_rate: 0,
        average_checks_to_clear: null,
      },
    ],
  };
}

test('summarizeTaskRuns keeps failure telemetry in the merged batch summary', function () {
  const taskRuns = [
    {
      type: 'result',
      result: {
        output_dir: '/tmp/success-task',
        telemetry_summary: buildTelemetrySummary('success-run', 'forbidden_raw_read'),
      },
    },
    {
      type: 'failure',
      failure: {
        output_dir: '/tmp/failure-task',
        telemetry_summary: buildTelemetrySummary('failure-run', 'closed_domain_exhaustiveness'),
      },
    },
  ];

  const summary = summarizeTaskRuns(taskRuns, '/tmp/repo');

  assert.equal(summary.taskResults.length, 1);
  assert.equal(summary.taskFailures.length, 1);
  assert.equal(summary.mergedSummary.summary.session_count, 2);
  assert.equal(summary.mergedSummary.summary.explicit_session_count, 2);
  assert.equal(summary.mergedSummary.signals.length, 2);
  assert.deepEqual(summary.mergedSummary.source_paths, [
    '/tmp/success-task/agent-session-events.jsonl',
    '/tmp/failure-task/agent-session-events.jsonl',
  ]);
});
