import test from 'node:test';
import assert from 'node:assert/strict';

import {
  buildSignalScorecard,
  formatSignalScorecardMarkdown,
} from '../lib/signal-scorecard.mjs';
import { enrichReviewVerdictsFromPacket } from '../lib/review-verdict-enrichment.mjs';

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
          rank_observed: 1,
          rank_preserved: true,
          repair_packet_complete: true,
          repair_packet_missing_fields: [],
          repair_packet_fix_surface_clear: true,
          repair_packet_verification_clear: true,
          sample_helpfulness: 3,
          sample_distraction_cost: 0,
        },
      ],
    },
    sessionVerdicts: {
      repo_label: 'parallel-code',
      verdicts: [
        {
          session_id: 'live-exhaustiveness',
          lane: 'live',
          top_action_followed: true,
          top_action_helped: true,
          task_completed_successfully: true,
          patch_expanded_unnecessarily: false,
          intervention_cost_checks: 1,
          reviewer_confidence: 'high',
          notes: 'Followed the expected exhaustiveness fix.',
        },
        {
          session_id: 'replay-exhaustiveness',
          lane: 'replay',
          top_action_followed: false,
          top_action_helped: null,
          task_completed_successfully: false,
          patch_expanded_unnecessarily: true,
          intervention_cost_checks: 2,
          reviewer_confidence: 'medium',
          notes: 'Missed the replay signal and expanded the patch.',
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
          top_action_sessions: 2,
          followup_checks: 1,
          target_cleared: 1,
          followup_regressions: 0,
          sessions_cleared: 1,
          sessions_clean: 2,
          total_checks_to_clear: 3,
          sessions_thrashing: 0,
          sessions_stalled: 0,
          reopened_top_actions: 0,
          repeated_top_action_carries: 0,
          total_entropy_delta: -1,
          sessions_with_entropy_increase: 0,
        },
      ],
      summary: {
        session_count: 2,
        converged_session_count: 1,
        converging_session_count: 1,
        stalled_session_count: 0,
        thrashing_session_count: 0,
        average_entropy_delta: -0.5,
      },
    },
    codexBatch: {
      results: [
        {
          task_id: 'live-exhaustiveness',
          expected_signal_kinds: ['closed_domain_exhaustiveness'],
          outcome: {
            initial_top_action_kind: 'closed_domain_exhaustiveness',
            initial_action_kinds: ['closed_domain_exhaustiveness'],
          },
        },
      ],
    },
    replayBatch: {
      results: [
        {
          replay_id: 'replay-exhaustiveness',
          expected_signal_kinds: ['closed_domain_exhaustiveness'],
          outcome: {
            initial_top_action_kind: 'large_file',
            initial_action_kinds: ['large_file'],
          },
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
  assert.equal(scorecard.signals[0].review_noise_rate, 0);
  assert.equal(scorecard.signals[0].remediation_success_rate, 1);
  assert.equal(scorecard.signals[0].session_trial_count, 2);
  assert.equal(scorecard.signals[0].live_session_trial_count, 1);
  assert.equal(scorecard.signals[0].replay_session_trial_count, 1);
  assert.equal(scorecard.signals[0].session_expectation_hit_rate, 0.5);
  assert.equal(scorecard.signals[0].session_expectation_top_action_rate, 0.5);
  assert.equal(scorecard.signals[0].session_trial_miss_rate, 0.5);
  assert.equal(scorecard.signals[0].session_resolution_rate, 1);
  assert.equal(scorecard.signals[0].top_action_clear_rate, 0.5);
  assert.equal(scorecard.signals[0].agent_clear_rate, 0.5);
  assert.equal(scorecard.signals[0].regression_after_fix_rate, 0);
  assert.equal(scorecard.signals[0].session_clean_rate, 1);
  assert.equal(scorecard.signals[0].session_thrash_rate, 0);
  assert.equal(scorecard.signals[0].average_entropy_delta, -0.5);
  assert.equal(scorecard.signals[0].entropy_increase_rate, 0);
  assert.equal(scorecard.signals[0].average_checks_to_clear, 3);
  assert.equal(scorecard.signals[0].promotion_evidence_complete, true);
  assert.equal(scorecard.signals[0].latency_ms, 134.2);
  assert.equal(scorecard.summary.kpis.session_trial_count, 2);
  assert.equal(scorecard.summary.kpis.session_verdict_count, 2);
  assert.equal(scorecard.summary.coverage.has_session_trials, true);
  assert.equal(scorecard.summary.coverage.has_session_verdicts, true);
  assert.equal(scorecard.summary.kpis.session_count, 2);
  assert.equal(scorecard.summary.product_value.session_verdict_count, 2);
  assert.equal(scorecard.summary.product_value.top_action_follow_rate, 0.5);
  assert.equal(scorecard.summary.product_value.top_action_help_rate, 1);
  assert.equal(scorecard.summary.product_value.task_success_rate, 0.5);
  assert.equal(scorecard.summary.product_value.patch_expansion_rate, 0.5);
  assert.equal(scorecard.summary.product_value.intervention_cost_checks_mean, 1.5);
  assert.equal(scorecard.summary.product_value.intervention_net_value_score, 0.333);
  assert.equal(scorecard.summary.ranking_quality.rank_preserved_rate, 1);
  assert.equal(scorecard.summary.ranking_quality.repair_packet_complete_rate, 1);
  assert.equal(scorecard.summary.ranking_quality.sample_helpfulness_mean, 3);
  assert.equal(scorecard.summary.ranking_quality.sample_distraction_cost_mean, 0);
  assert.equal(scorecard.summary.session_health.converged_session_count, 1);
  assert.equal(scorecard.summary.session_health.top_action_session_count, 2);
  assert.equal(scorecard.summary.session_health.top_action_cleared_count, 1);
  assert.equal(scorecard.summary.session_health.agent_clear_rate, 0.5);
  assert.equal(scorecard.summary.session_health.followup_regression_session_rate, 0);
  assert.equal(scorecard.summary.session_health.regression_after_fix_rate, 0);
  assert.equal(scorecard.summary.session_health.session_clean_rate, 1);
  assert.equal(scorecard.summary.session_health.average_checks_to_clear, 3);
  assert.equal(scorecard.summary.session_health.average_entropy_delta, -0.5);
});

test('buildSignalScorecard treats missed expected trials as valid session evidence', function () {
  const scorecard = buildSignalScorecard({
    repoLabel: 'trial-evidence-only',
    defectReport: {
      repo_label: 'trial-evidence-only',
      defects: [
        {
          id: 'raw-read',
          signal_kind: 'forbidden_raw_read',
          signal_family: 'rules',
          promotion_status: 'trusted',
          blocking_intent: 'blocking',
        },
      ],
      results: [
        {
          defect_id: 'raw-read',
          detected: true,
          check: { supported: true, matched: true },
        },
      ],
    },
    reviewVerdicts: {
      verdicts: [
        {
          kind: 'forbidden_raw_read',
          category: 'useful',
        },
      ],
    },
    remediationReport: {
      results: [
        {
          signal_kind: 'forbidden_raw_read',
          fixed: true,
          regression_free: true,
        },
      ],
    },
    replayBatch: {
      results: [
        {
          replay_id: 'replay-raw-read',
          expected_signal_kinds: ['forbidden_raw_read'],
          outcome: {
            initial_top_action_kind: 'large_file',
            initial_action_kinds: ['large_file'],
          },
        },
      ],
    },
  });

  assert.equal(scorecard.signals.length, 1);
  assert.equal(scorecard.signals[0].signal_kind, 'forbidden_raw_read');
  assert.equal(scorecard.signals[0].has_session_action_evidence, false);
  assert.equal(scorecard.signals[0].has_session_trial_evidence, true);
  assert.equal(scorecard.signals[0].has_session_evidence, true);
  assert.equal(scorecard.signals[0].promotion_evidence_complete, true);
  assert.equal(scorecard.signals[0].session_trial_count, 1);
  assert.equal(scorecard.signals[0].session_expectation_misses, 1);
  assert.equal(scorecard.signals[0].session_trial_miss_rate, 1);
});

test('buildSignalScorecard requires top-action sessions before marking session action evidence complete', function () {
  const scorecard = buildSignalScorecard({
    repoLabel: 'session-evidence-gap',
    defectReport: {
      repo_label: 'session-evidence-gap',
      defects: [
        {
          id: 'raw-read',
          signal_kind: 'forbidden_raw_read',
          signal_family: 'rules',
          promotion_status: 'trusted',
          blocking_intent: 'blocking',
        },
      ],
      results: [
        {
          defect_id: 'raw-read',
          detected: true,
          check: { supported: true, matched: true },
        },
      ],
    },
    reviewVerdicts: {
      verdicts: [
        {
          kind: 'forbidden_raw_read',
          category: 'useful',
        },
      ],
    },
    remediationReport: {
      results: [
        {
          signal_kind: 'forbidden_raw_read',
          fixed: true,
          regression_free: true,
        },
      ],
    },
    sessionTelemetry: {
      signals: [
        {
          signal_kind: 'forbidden_raw_read',
          top_action_presented: 1,
          top_action_sessions: 0,
          followup_checks: 0,
          target_cleared: 0,
          followup_regressions: 0,
          sessions_cleared: 0,
          sessions_clean: 0,
          total_checks_to_clear: 0,
        },
      ],
      summary: {
        session_count: 1,
      },
    },
  });

  assert.equal(scorecard.signals.length, 1);
  assert.equal(scorecard.signals[0].has_session_action_evidence, false);
  assert.equal(scorecard.signals[0].has_session_evidence, false);
  assert.equal(scorecard.signals[0].promotion_evidence_complete, false);
  assert.equal(scorecard.signals[0].top_action_clear_rate, null);
  assert.equal(scorecard.summary.coverage.has_session_telemetry, true);
});

test('buildSignalScorecard backfills missing structured review fields from a matching packet without overriding curated values', function () {
  const reviewVerdicts = {
    repo: 'packet-backed-review',
    verdicts: [
      {
        kind: 'forbidden_raw_read',
        scope: 'src/b.ts',
        report_bucket: 'actions',
        category: 'useful',
      },
      {
        kind: 'large_file',
        scope: 'src/a.ts',
        report_bucket: 'actions',
        category: 'useful_watchpoint',
        repair_packet_complete: true,
        repair_packet_missing_fields: ['already_curated'],
        repair_packet_fix_surface_clear: true,
        repair_packet_verification_clear: false,
      },
    ],
  };
  const reviewPacket = {
    repo_root: '/tmp/packet-backed-review',
    samples: [
      {
        rank: 1,
        kind: 'large_file',
        scope: 'src/a.ts',
        report_bucket: 'actions',
        repair_packet: {
          complete: false,
          missing_fields: ['fix_hint'],
          fix_surface_clear: false,
          verification_clear: true,
        },
      },
      {
        rank: 2,
        kind: 'forbidden_raw_read',
        scope: 'src/b.ts',
        report_bucket: 'actions',
        repair_packet: {
          complete: true,
          missing_fields: [],
          fix_surface_clear: true,
          verification_clear: true,
        },
      },
    ],
  };

  const scorecard = buildSignalScorecard({
    reviewVerdicts,
    reviewPacket,
  });
  const forbiddenRawRead = scorecard.signals.find(
    (signal) => signal.signal_kind === 'forbidden_raw_read',
  );
  const largeFile = scorecard.signals.find((signal) => signal.signal_kind === 'large_file');

  assert.equal(scorecard.repo_label, 'packet-backed-review');
  assert.equal(forbiddenRawRead.review_rank_observed_total, 1);
  assert.equal(forbiddenRawRead.review_rank_preserved_count, 0);
  assert.equal(forbiddenRawRead.review_repair_packet_complete_count, 1);
  assert.equal(forbiddenRawRead.review_repair_packet_fix_surface_clear_count, 1);
  assert.equal(forbiddenRawRead.review_repair_packet_verification_clear_count, 1);
  assert.equal(largeFile.review_rank_observed_total, 1);
  assert.equal(largeFile.review_rank_preserved_count, 0);
  assert.equal(largeFile.review_repair_packet_complete_count, 1);
  assert.equal(largeFile.review_repair_packet_fix_surface_clear_count, 1);
  assert.equal(largeFile.review_repair_packet_verification_clear_count, 0);
  assert.deepEqual(reviewVerdicts.verdicts[0].repair_packet_missing_fields, undefined);
  assert.equal(reviewVerdicts.verdicts[0].rank_observed, undefined);
  assert.equal(reviewVerdicts.verdicts[1].repair_packet_complete, true);
  assert.equal(scorecard.summary.ranking_quality.rank_preserved_rate, 0);
  assert.equal(scorecard.summary.ranking_quality.repair_packet_complete_rate, 1);
  assert.equal(scorecard.summary.ranking_quality.repair_packet_fix_surface_clear_rate, 1);
  assert.equal(scorecard.summary.ranking_quality.repair_packet_verification_clear_rate, 0.5);
});

test('enrichReviewVerdictsFromPacket matches duplicate review findings by source identity when available', function () {
  const enriched = enrichReviewVerdictsFromPacket(
    {
      repo: 'duplicate-review-sources',
      verdicts: [
        {
          kind: 'forbidden_raw_read',
          scope: 'src/a.ts',
          report_bucket: 'actions',
          source_label: 'Task one',
          task_id: 'task-one',
          category: 'useful',
        },
        {
          kind: 'forbidden_raw_read',
          scope: 'src/a.ts',
          report_bucket: 'actions',
          source_label: 'Task two',
          task_id: 'task-two',
          category: 'useful',
        },
      ],
    },
    {
      repo_root: '/tmp/duplicate-review-sources',
      samples: [
        {
          rank: 1,
          kind: 'forbidden_raw_read',
          scope: 'src/a.ts',
          report_bucket: 'actions',
          source_label: 'Task two',
          task_id: 'task-two',
          repair_packet: {
            complete: true,
            missing_fields: [],
            fix_surface_clear: true,
            verification_clear: true,
          },
        },
        {
          rank: 2,
          kind: 'forbidden_raw_read',
          scope: 'src/a.ts',
          report_bucket: 'actions',
          source_label: 'Task one',
          task_id: 'task-one',
          repair_packet: {
            complete: false,
            missing_fields: ['fix_hint'],
            fix_surface_clear: true,
            verification_clear: false,
          },
        },
      ],
    },
  );

  assert.equal(enriched.verdicts[0].rank_observed, 2);
  assert.equal(enriched.verdicts[0].rank_preserved, false);
  assert.equal(enriched.verdicts[0].repair_packet_complete, false);
  assert.deepEqual(enriched.verdicts[0].repair_packet_missing_fields, ['fix_hint']);
  assert.equal(enriched.verdicts[1].rank_observed, 1);
  assert.equal(enriched.verdicts[1].rank_preserved, false);
  assert.equal(enriched.verdicts[1].repair_packet_complete, true);
  assert.deepEqual(enriched.verdicts[1].repair_packet_missing_fields, []);
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
    repoLabel: 'one-tool',
    defectReport: {
      repo_label: 'parallel-code',
      defects: [],
      results: [],
    },
  });

  assert.equal(scorecard.repo_label, 'one-tool');
});

test('buildSignalScorecard derives a repo label without a defect report', function () {
  const scorecard = buildSignalScorecard({
    reviewVerdicts: {
      repo: 'review-only-repo',
      verdicts: [
        {
          kind: 'large_file',
          category: 'useful',
        },
      ],
    },
  });

  assert.equal(scorecard.repo_label, 'review-only-repo');
  assert.equal(scorecard.signals.length, 1);
});

test('buildSignalScorecard counts session trials per expected signal kind', function () {
  const scorecard = buildSignalScorecard({
    replayBatch: {
      repo_label: 'multi-signal-replay',
      results: [
        {
          replay_id: 'multi-signal',
          expected_signal_kinds: ['forbidden_raw_read', 'incomplete_propagation'],
          outcome: {
            initial_top_action_kind: 'large_file',
            initial_action_kinds: ['large_file'],
          },
        },
      ],
    },
  });

  assert.equal(scorecard.repo_label, 'multi-signal-replay');
  assert.equal(scorecard.summary.kpis.session_trial_count, 2);
  assert.deepEqual(
    scorecard.signals.map((signal) => signal.signal_kind).sort(),
    ['forbidden_raw_read', 'incomplete_propagation'],
  );
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
          top_action_sessions: 1,
          followup_checks: 1,
          target_cleared: 1,
          followup_regressions: 0,
          sessions_cleared: 1,
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
  assert.equal(scorecard.signals[0].top_action_clear_rate, 1);
  assert.equal(scorecard.signals[0].session_clean_rate, 1);
});

test('buildSignalScorecard keeps provisional review verdicts out of curated coverage', function () {
  const scorecard = buildSignalScorecard({
    repoLabel: 'one-tool',
    reviewVerdicts: {
      provisional: true,
      verdicts: [
        {
          kind: 'large_file',
          category: 'useful_watchpoint',
        },
      ],
    },
  });

  assert.equal(scorecard.signals.length, 1);
  assert.equal(scorecard.signals[0].reviewed_total, 0);
  assert.equal(scorecard.signals[0].reviewed_precision, null);
  assert.equal(scorecard.signals[0].provisional_reviewed_total, 1);
  assert.equal(scorecard.signals[0].provisional_reviewed_precision, 1);
  assert.equal(scorecard.signals[0].has_review_evidence, false);
  assert.equal(scorecard.signals[0].has_provisional_review_evidence, true);
  assert.equal(scorecard.summary.kpis.review_sample_count, 0);
  assert.equal(scorecard.summary.kpis.provisional_review_sample_count, 1);
  assert.equal(scorecard.summary.coverage.has_review_verdicts, false);
  assert.equal(scorecard.summary.coverage.has_provisional_review_verdicts, true);
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
      ranking_quality: {
        top_1_actionable_precision: 1,
        top_1_actionable_count: 1,
        top_1_reviewed_count: 1,
        top_3_actionable_precision: 1,
        top_3_actionable_count: 1,
        top_3_reviewed_count: 1,
        top_10_actionable_precision: 1,
        top_10_actionable_count: 1,
        top_10_reviewed_count: 1,
        ranking_preference_satisfaction_rate: 1,
        meets_primary_target_policy: true,
      },
      session_health: {
        thrashing_session_count: 0,
        top_action_session_count: 2,
        top_action_cleared_count: 1,
        followup_regression_count: 0,
        reopened_top_action_count: 0,
        session_clean_count: 1,
        agent_clear_rate: 0.5,
        followup_regression_session_rate: 0,
        regression_after_fix_rate: 0,
        session_clean_rate: 0.5,
        session_thrash_rate: 0,
        average_checks_to_clear: 2,
        average_entropy_delta: -0.5,
      },
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
        review_noise_rate: null,
        remediation_success_rate: null,
        session_trial_count: 2,
        top_action_sessions: 1,
        session_trial_miss_rate: 0.5,
        top_action_clear_rate: 0.5,
        followup_regression_rate: 0,
        session_clean_rate: 0.5,
        session_thrash_rate: 0,
        average_entropy_delta: -0.5,
        average_checks_to_clear: 2,
        latency_ms: 134.2,
        promotion_recommendation: 'keep_watchpoint',
      },
    ],
  });

  assert.match(markdown, /Signal Quality Scorecard/);
  assert.match(markdown, /missing_test_coverage/);
  assert.match(markdown, /keep_watchpoint/);
  assert.match(markdown, /Trials/);
  assert.match(markdown, /Top Action Sessions/);
  assert.match(markdown, /Thrash Rate/);
  assert.match(markdown, /top-action sessions: 2/);
  assert.match(markdown, /agent clear rate: 0\.5 \(1\/2\)/);
  assert.match(markdown, /top-1 actionable precision/);
  assert.match(markdown, /0.5/);
});
