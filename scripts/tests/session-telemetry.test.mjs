import test from 'node:test';
import assert from 'node:assert/strict';

import {
  buildSessionTelemetrySummary,
  formatSessionTelemetrySummaryMarkdown,
  mergeSessionTelemetrySummaries,
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
      gate: 'fail',
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
      gate: 'pass',
      top_action_kind: 'missing_test_coverage',
      action_kinds: [],
    },
    {
      event_type: 'session_ended',
      session_run_id: 'session-1',
      server_run_id: 'mcp-1',
      session_mode: 'explicit',
      event_index: 4,
      repo_root: '/tmp/parallel-code',
      decision: 'pass',
      action_count: 0,
    },
  ]);

  assert.equal(summary.summary.session_count, 1);
  assert.equal(summary.sessions[0].initial_gate, 'fail');
  assert.deepEqual(summary.sessions[0].initial_action_kinds, [
    'forbidden_raw_read',
    'missing_test_coverage',
  ]);
  assert.equal(summary.sessions[0].top_action_cleared, true);
  assert.equal(summary.sessions[0].checks_to_clear_top_action, 1);
  assert.equal(summary.sessions[0].followup_regression_introduced, false);
  assert.equal(summary.sessions[0].final_gate, 'pass');
  assert.equal(summary.sessions[0].final_session_clean, true);
  assert.equal(summary.signals.length, 2);
  assert.equal(summary.signals[0].signal_kind, 'forbidden_raw_read');
  assert.equal(summary.signals[0].top_action_presented, 1);
  assert.equal(summary.signals[0].followup_checks, 1);
  assert.equal(summary.signals[0].target_cleared, 1);
  assert.equal(summary.signals[0].followup_regressions, 0);
  assert.equal(summary.signals[0].resolution_rate, 1);
  assert.equal(summary.signals[0].session_clean_rate, 1);
  assert.equal(summary.signals[0].average_checks_to_clear, 1);
});

test('buildSessionTelemetrySummary tracks the first surfaced action when a session starts clean', function () {
  const summary = buildSessionTelemetrySummary([
    {
      event_type: 'session_started',
      session_run_id: 'session-2',
      server_run_id: 'mcp-2',
      session_mode: 'explicit',
      event_index: 1,
      repo_root: '/tmp/parallel-code',
    },
    {
      event_type: 'check_run',
      session_run_id: 'session-2',
      server_run_id: 'mcp-2',
      session_mode: 'explicit',
      event_index: 2,
      repo_root: '/tmp/parallel-code',
      gate: 'pass',
      top_action_kind: null,
      action_kinds: [],
    },
    {
      event_type: 'check_run',
      session_run_id: 'session-2',
      server_run_id: 'mcp-2',
      session_mode: 'explicit',
      event_index: 3,
      repo_root: '/tmp/parallel-code',
      gate: 'fail',
      top_action_kind: 'closed_domain_exhaustiveness',
      action_kinds: ['closed_domain_exhaustiveness'],
    },
    {
      event_type: 'check_run',
      session_run_id: 'session-2',
      server_run_id: 'mcp-2',
      session_mode: 'explicit',
      event_index: 4,
      repo_root: '/tmp/parallel-code',
      gate: 'pass',
      top_action_kind: null,
      action_kinds: [],
    },
    {
      event_type: 'session_ended',
      session_run_id: 'session-2',
      server_run_id: 'mcp-2',
      session_mode: 'explicit',
      event_index: 5,
      repo_root: '/tmp/parallel-code',
      decision: 'pass',
      action_count: 0,
    },
  ]);

  assert.equal(summary.sessions[0].initial_gate, 'pass');
  assert.deepEqual(summary.sessions[0].initial_action_kinds, ['closed_domain_exhaustiveness']);
  assert.equal(summary.sessions[0].initial_top_action_kind, 'closed_domain_exhaustiveness');
  assert.equal(summary.sessions[0].top_action_cleared, true);
  assert.equal(summary.sessions[0].checks_to_clear_top_action, 1);
  assert.equal(summary.signals[0].signal_kind, 'closed_domain_exhaustiveness');
  assert.equal(summary.signals[0].sessions_cleared, 1);
  assert.equal(summary.signals[0].sessions_clean, 1);
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
        session_clean_rate: 0,
        average_checks_to_clear: null,
      },
    ],
  });

  assert.match(markdown, /Session Telemetry Summary/);
  assert.match(markdown, /missing_test_coverage/);
  assert.match(markdown, /Regression Rate/);
  assert.match(markdown, /Session Clean Rate/);
});

test('mergeSessionTelemetrySummaries combines per-signal counts across summaries', function () {
  const merged = mergeSessionTelemetrySummaries([
    {
      repo_root: '/tmp/repo',
      summary: {
        event_count: 4,
        session_count: 1,
        explicit_session_count: 1,
        implicit_session_count: 0,
        check_run_count: 2,
      },
      sessions: [{ session_run_id: 'one' }],
      signals: [
        {
          signal_kind: 'missing_test_coverage',
          top_action_presented: 1,
          followup_checks: 1,
          target_cleared: 1,
          followup_regressions: 0,
          sessions_cleared: 1,
          sessions_clean: 1,
          total_checks_to_clear: 1,
        },
      ],
    },
    {
      repo_root: '/tmp/repo',
      summary: {
        event_count: 3,
        session_count: 1,
        explicit_session_count: 0,
        implicit_session_count: 1,
        check_run_count: 1,
      },
      sessions: [{ session_run_id: 'two' }],
      signals: [
        {
          signal_kind: 'missing_test_coverage',
          top_action_presented: 1,
          followup_checks: 0,
          target_cleared: 0,
          followup_regressions: 0,
          sessions_cleared: 0,
          sessions_clean: 0,
          total_checks_to_clear: 0,
        },
      ],
    },
  ]);

  assert.equal(merged.summary.event_count, 7);
  assert.equal(merged.summary.session_count, 2);
  assert.equal(merged.summary.explicit_session_count, 1);
  assert.equal(merged.summary.implicit_session_count, 1);
  assert.equal(merged.signals[0].signal_kind, 'missing_test_coverage');
  assert.equal(merged.signals[0].top_action_presented, 2);
  assert.equal(merged.signals[0].session_clean_rate, 0.5);
});
