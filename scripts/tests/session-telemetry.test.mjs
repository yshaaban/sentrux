import test from 'node:test';
import assert from 'node:assert/strict';

import {
  buildSessionTelemetrySummary,
  formatSessionTelemetrySummaryMarkdown,
} from '../lib/session-telemetry.mjs';

test('buildSessionTelemetrySummary tracks top-action follow-up resolution', function () {
  const summary = buildSessionTelemetrySummary([
    {
      event_type: 'session_started',
      session_run_id: 'session-1',
      server_run_id: 'mcp-1',
      session_mode: 'explicit',
      event_index: 1,
      repo_root: '/tmp/parallel-code',
    },
    {
      event_type: 'check_run',
      session_run_id: 'session-1',
      server_run_id: 'mcp-1',
      session_mode: 'explicit',
      event_index: 2,
      repo_root: '/tmp/parallel-code',
      top_action_kind: 'forbidden_raw_read',
      action_kinds: ['forbidden_raw_read', 'missing_test_coverage'],
    },
    {
      event_type: 'check_run',
      session_run_id: 'session-1',
      server_run_id: 'mcp-1',
      session_mode: 'explicit',
      event_index: 3,
      repo_root: '/tmp/parallel-code',
      top_action_kind: 'missing_test_coverage',
      action_kinds: ['missing_test_coverage'],
    },
    {
      event_type: 'session_ended',
      session_run_id: 'session-1',
      server_run_id: 'mcp-1',
      session_mode: 'explicit',
      event_index: 4,
      repo_root: '/tmp/parallel-code',
      decision: 'warn',
    },
  ]);

  assert.equal(summary.summary.session_count, 1);
  assert.equal(summary.signals.length, 2);
  assert.equal(summary.signals[0].signal_kind, 'forbidden_raw_read');
  assert.equal(summary.signals[0].top_action_presented, 1);
  assert.equal(summary.signals[0].followup_checks, 1);
  assert.equal(summary.signals[0].target_cleared, 1);
  assert.equal(summary.signals[0].followup_regressions, 0);
  assert.equal(summary.signals[0].resolution_rate, 1);
});

test('formatSessionTelemetrySummaryMarkdown renders the telemetry table', function () {
  const markdown = formatSessionTelemetrySummaryMarkdown({
    repo_root: '/tmp/parallel-code',
    generated_at: '2026-04-02T00:00:00.000Z',
    summary: {
      event_count: 3,
      session_count: 1,
      explicit_session_count: 1,
      implicit_session_count: 0,
      check_run_count: 2,
    },
    signals: [
      {
        signal_kind: 'missing_test_coverage',
        top_action_presented: 1,
        followup_checks: 1,
        target_cleared: 0,
        followup_regressions: 1,
        resolution_rate: 0,
        regression_rate: 1,
      },
    ],
  });

  assert.match(markdown, /Session Telemetry Summary/);
  assert.match(markdown, /missing_test_coverage/);
  assert.match(markdown, /Regression Rate/);
});
