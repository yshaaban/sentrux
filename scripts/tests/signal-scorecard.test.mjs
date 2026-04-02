import test from 'node:test';
import assert from 'node:assert/strict';

import {
  buildSignalScorecard,
  formatSignalScorecardMarkdown,
} from '../lib/signal-scorecard.mjs';

test('buildSignalScorecard aggregates seeded, review, and remediation metrics', function () {
  const scorecard = buildSignalScorecard({
    defectReport: {
      repo_label: 'parallel-code',
      defects: [
        {
          id: 'missing_exhaustiveness',
          signal_kind: 'closed_domain_exhaustiveness',
          signal_family: 'obligation',
          promotion_status: 'trusted',
          blocking_intent: 'blocking',
        },
      ],
      results: [
        {
          defect_id: 'missing_exhaustiveness',
          detected: true,
          check: { supported: true, matched: true },
        },
      ],
    },
    reviewVerdicts: {
      verdicts: [
        {
          kind: 'closed_domain_exhaustiveness',
          category: 'useful',
        },
      ],
    },
    remediationReport: {
      results: [
        {
          signal_kind: 'closed_domain_exhaustiveness',
          fixed: true,
          regression_free: true,
        },
      ],
    },
    benchmark: {
      benchmark: {
        warm_cached: {
          check: {
            elapsed_ms: 134.2,
          },
        },
      },
    },
    sessionTelemetry: {
      signals: [
        {
          signal_kind: 'closed_domain_exhaustiveness',
          top_action_presented: 2,
          followup_checks: 1,
          target_cleared: 1,
          followup_regressions: 0,
          sessions_clean: 2,
          total_checks_to_clear: 3,
        },
      ],
    },
  });

  assert.equal(scorecard.signals.length, 1);
  assert.equal(scorecard.signals[0].signal_kind, 'closed_domain_exhaustiveness');
  assert.equal(scorecard.signals[0].primary_lane, 'check');
  assert.equal(scorecard.signals[0].seeded_recall, 1);
  assert.equal(scorecard.signals[0].primary_recall, 1);
  assert.equal(scorecard.signals[0].reviewed_precision, 1);
  assert.equal(scorecard.signals[0].useful_precision, 1);
  assert.equal(scorecard.signals[0].remediation_success_rate, 1);
  assert.equal(scorecard.signals[0].session_resolution_rate, 1);
  assert.equal(scorecard.signals[0].session_clean_rate, 1);
  assert.equal(scorecard.signals[0].average_checks_to_clear, 3);
  assert.equal(scorecard.signals[0].latency_ms, 134.2);
});

test('buildSignalScorecard only assigns check latency to fast-path signals', function () {
  const scorecard = buildSignalScorecard({
    defectReport: {
      repo_label: 'parallel-code',
      defects: [],
      results: [],
    },
    reviewVerdicts: {
      verdicts: [
        {
          kind: 'dead_private_code_cluster',
          category: 'incorrect',
        },
      ],
    },
    benchmark: {
      benchmark: {
        warm_patch_safety: {
          check: {
            elapsed_ms: 66.7,
          },
        },
      },
    },
  });

  assert.equal(scorecard.signals.length, 1);
  assert.equal(scorecard.signals[0].signal_kind, 'dead_private_code_cluster');
  assert.equal(scorecard.signals[0].latency_ms, null);
});

test('buildSignalScorecard tracks check_rules-only seeded defects through the primary lane', function () {
  const scorecard = buildSignalScorecard({
    defectReport: {
      repo_label: 'sentrux',
      defects: [
        {
          id: 'self_boundary_violation',
          signal_kind: 'check_rules',
          signal_family: 'rules',
          promotion_status: 'watchpoint',
          blocking_intent: 'blocking',
        },
      ],
      results: [
        {
          defect_id: 'self_boundary_violation',
          detected: true,
          check: { supported: false, matched: false },
          check_rules: { supported: true, matched: true },
        },
      ],
    },
  });

  assert.equal(scorecard.signals.length, 1);
  assert.equal(scorecard.signals[0].primary_lane, 'check_rules');
  assert.equal(scorecard.signals[0].primary_recall, 1);
  assert.equal(scorecard.signals[0].check_recall, null);
  assert.equal(scorecard.signals[0].check_rules_recall, 1);
  assert.equal(scorecard.signals[0].latency_ms, null);
});

test('buildSignalScorecard respects an explicit repo label override', function () {
  const scorecard = buildSignalScorecard({
    repoLabel: 'telemetry-fixture',
    defectReport: {
      repo_label: 'parallel-code',
      defects: [],
      results: [],
    },
  });

  assert.equal(scorecard.repo_label, 'telemetry-fixture');
});

test('buildSignalScorecard builds a review-and-session-only scorecard without seeded defects', function () {
  const scorecard = buildSignalScorecard({
    repoLabel: 'review-only',
    reviewVerdicts: {
      verdicts: [
        {
          kind: 'missing_test_coverage',
          category: 'acceptable_warning',
        },
      ],
    },
    sessionTelemetry: {
      signals: [
        {
          signal_kind: 'missing_test_coverage',
          top_action_presented: 1,
          followup_checks: 1,
          target_cleared: 1,
          followup_regressions: 0,
          sessions_clean: 1,
          total_checks_to_clear: 2,
        },
      ],
    },
  });

  assert.equal(scorecard.repo_label, 'review-only');
  assert.equal(scorecard.signals.length, 1);
  assert.equal(scorecard.signals[0].signal_kind, 'missing_test_coverage');
  assert.equal(scorecard.signals[0].seeded_total, 0);
  assert.equal(scorecard.signals[0].reviewed_precision, 1);
  assert.equal(scorecard.signals[0].session_clean_rate, 1);
});

test('formatSignalScorecardMarkdown renders the score table', function () {
  const markdown = formatSignalScorecardMarkdown({
    repo_label: 'parallel-code',
    generated_at: '2026-04-02T00:00:00.000Z',
    summary: {
      total_signals: 1,
      trusted_count: 1,
      watchpoint_count: 0,
      needs_review_count: 0,
      degrade_count: 0,
    },
    signals: [
      {
        signal_kind: 'missing_test_coverage',
        signal_family: 'structural',
        promotion_status: 'watchpoint',
        primary_lane: 'check',
        seeded_recall: 1,
        primary_recall: 1,
        reviewed_precision: null,
        useful_precision: null,
        remediation_success_rate: null,
        session_resolution_rate: 0.5,
        session_clean_rate: 0.5,
        average_checks_to_clear: 2,
        latency_ms: 134.2,
        promotion_recommendation: 'keep_watchpoint',
      },
    ],
  });

  assert.match(markdown, /Signal Quality Scorecard/);
  assert.match(markdown, /missing_test_coverage/);
  assert.match(markdown, /keep_watchpoint/);
  assert.match(markdown, /0.5/);
});
