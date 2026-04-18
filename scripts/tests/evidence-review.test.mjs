import assert from 'node:assert/strict';
import test from 'node:test';

import {
  buildEvidenceReview,
  formatEvidenceReviewMarkdown,
} from '../lib/evidence-review.mjs';

test('buildEvidenceReview summarizes promotion, demotion, ranking, and experiment evidence', function () {
  const review = buildEvidenceReview({
    scorecard: {
      repo_label: 'demo-repo',
      signals: [
        {
          signal_kind: 'incomplete_propagation',
          promotion_status: 'watchpoint',
          reviewed_precision: 0.9,
          top_1_actionable_precision: 1,
          top_3_actionable_precision: 1,
          remediation_success_rate: 1,
          session_clean_rate: 1,
          session_trial_miss_rate: 0,
        },
        {
          signal_kind: 'forbidden_raw_read',
          promotion_status: 'trusted',
          review_noise_rate: 0.4,
          top_1_actionable_precision: 0.2,
          top_3_actionable_precision: 0.4,
          session_clean_rate: 0.3,
          session_trial_miss_rate: 0.5,
        },
      ],
    },
    backlog: {
      weak_signals: [
        {
          signal_kind: 'zero_config_boundary_violation',
          recommendation: 'needs_review',
          expected_missing_count: 2,
          expected_present_not_top_count: 1,
          crowded_out_expected_count: 1,
          unexpected_top_action_count: 0,
          session_trial_miss_rate: 0.5,
        },
      ],
    },
    sessionCorpus: {
      sessions: [
        {
          session_id: 'live-1',
          lane: 'live',
          focus_areas: ['propagation'],
          experiment_arm: 'directive_fix_first',
          expected_signal_kinds: ['incomplete_propagation'],
          outcome_bucket: 'missed_expected_signal',
          outcome: {
            initial_top_action_kind: 'large_file',
            convergence_status: 'stalled',
            entropy_delta: 1,
            final_session_clean: false,
            followup_regression_introduced: false,
          },
          session_verdict: {
            session_id: 'live-1',
            top_action_followed: false,
            top_action_helped: null,
            task_completed_successfully: false,
            patch_expanded_unnecessarily: true,
            intervention_cost_checks: 2,
            reviewer_confidence: 'high',
            notes: 'Missed the expected propagation repair.',
          },
        },
        {
          session_id: 'live-2',
          lane: 'live',
          focus_areas: ['clone_followthrough'],
          experiment_arm: 'directive_fix_first',
          expected_signal_kinds: ['clone_propagation_drift'],
          outcome_bucket: 'regressed',
          outcome: {
            initial_top_action_kind: 'clone_propagation_drift',
            convergence_status: 'thrashing',
            entropy_delta: 2,
            final_session_clean: false,
            followup_regression_introduced: true,
          },
          session_verdict: {
            session_id: 'live-2',
            top_action_followed: true,
            top_action_helped: false,
            task_completed_successfully: false,
            patch_expanded_unnecessarily: false,
            intervention_cost_checks: 3,
            reviewer_confidence: 'medium',
            notes: 'Followed the clone finding but did not converge.',
          },
        },
        {
          session_id: 'live-3',
          lane: 'live',
          focus_areas: ['session_governance'],
          experiment_arm: 'report_only',
          expected_signal_kinds: ['closed_domain_exhaustiveness'],
          outcome_bucket: 'thrashing',
          outcome: {
            initial_top_action_kind: 'closed_domain_exhaustiveness',
            convergence_status: 'thrashing',
            entropy_delta: 3,
            final_session_clean: false,
            followup_regression_introduced: false,
          },
          session_verdict: {
            session_id: 'live-3',
            top_action_followed: true,
            top_action_helped: true,
            task_completed_successfully: true,
            patch_expanded_unnecessarily: false,
            intervention_cost_checks: 1,
            reviewer_confidence: 'high',
            notes: 'The intervention was eventually useful.',
          },
        },
      ],
      review_queue: [{ session_id: 'live-1' }, { session_id: 'live-2' }, { session_id: 'live-3' }],
    },
    reviewPacket: {
      summary: {
        sample_count: 4,
      },
    },
  });

  assert.equal(review.summary.promotion_candidate_count, 1);
  assert.equal(review.summary.demotion_candidate_count, 1);
  assert.equal(review.summary.default_on_candidate_count, 0);
  assert.equal(review.summary.ranking_miss_count, 1);
  assert.equal(review.summary.review_queue_count, 3);
  assert.equal(review.summary.focus_area_count, 3);
  assert.equal(review.summary.top_action_failure_count, 3);
  assert.equal(review.summary.experiment_arm_count, 2);
  assert.equal(review.summary.experiment_arm_comparison_count, 0);
  assert.equal(review.summary.session_verdict_count, 3);
  assert.equal(review.promotion_candidates[0].signal_kind, 'incomplete_propagation');
  assert.equal(review.demotion_candidates[0].signal_kind, 'forbidden_raw_read');
  assert.equal(review.ranking_misses[0].signal_kind, 'zero_config_boundary_violation');
  assert.equal(review.focus_area_summaries[0].focus_area, 'clone_followthrough');
  assert.equal(review.focus_area_summaries[1].focus_area, 'propagation');
  assert.equal(review.focus_area_summaries[2].focus_area, 'session_governance');
  assert.equal(
    review.top_action_failure_summary.find((entry) => entry.outcome_bucket === 'regressed')
      ?.session_count,
    1,
  );
  assert.equal(
    review.top_action_failure_summary.find((entry) => entry.outcome_bucket === 'missed_expected_signal')
      ?.session_count,
    1,
  );
  assert.equal(
    review.top_action_failure_summary.find((entry) => entry.outcome_bucket === 'thrashing')
      ?.session_count,
    1,
  );
  assert.equal(review.propagation_examples[0].session_id, 'live-1');
  assert.equal(review.clone_examples[0].session_id, 'live-2');
  assert.equal(review.thrashing_examples[0].session_id, 'live-2');
  assert.equal(review.experiment_arms[0].experiment_arm, 'fix_this_first');
  assert.equal(review.experiment_arms[0].focus_area_counts[0].focus_area, 'clone_followthrough');
  assert.equal(review.experiment_arms[0].clean_rate, 0);
  assert.equal(review.experiment_arms[0].regression_rate, 0.5);
  assert.deepEqual(review.experiment_arm_comparisons, []);
  assert.equal(review.product_value?.top_action_follow_rate, 0.667);
  assert.equal(review.product_value?.top_action_help_rate, 0.5);
  assert.equal(review.product_value?.task_success_rate, 0.333);
  assert.equal(review.product_value?.patch_expansion_rate, 0.333);
  assert.equal(review.default_on_promotion.ready, false);
  assert.equal(review.default_on_promotion.evidence_complete, false);
  assert.equal(review.default_on_promotion.repo_treatment_ready, false);
  assert.equal(review.default_on_promotion.signal_matched_treatment_evidence, false);
  assert(review.default_on_promotion.blockers.includes('no_signal_candidates'));
  assert.match(formatEvidenceReviewMarkdown(review), /Promotion Candidates/);
  assert.match(formatEvidenceReviewMarkdown(review), /Default-On Promotion/);
  assert.match(formatEvidenceReviewMarkdown(review), /ready for default-on: false/);
  assert.match(formatEvidenceReviewMarkdown(review), /top-action help rate: 0.5/);
  assert.match(formatEvidenceReviewMarkdown(review), /Focus Area Rollups/);
  assert.match(formatEvidenceReviewMarkdown(review), /Top Action Failures/);
  assert.match(formatEvidenceReviewMarkdown(review), /Experiment Arms/);
});

test('buildEvidenceReview recomputes corpus rollups from sessions when both are present', function () {
  const review = buildEvidenceReview({
    sessionCorpus: {
      sessions: [
        {
          session_id: 'live-1',
          lane: 'live',
          focus_areas: ['propagation'],
          experiment_arm: 'directive_fix_first',
          expected_signal_kinds: ['incomplete_propagation'],
          outcome_bucket: 'missed_expected_signal',
          outcome: {
            initial_top_action_kind: 'large_file',
            top_action_cleared: false,
            final_session_clean: false,
            followup_regression_introduced: false,
          },
        },
      ],
      focus_area_summaries: [{ focus_area: 'stale', session_count: 99 }],
      top_action_failure_summary: [{ outcome_bucket: 'stale_bucket', session_count: 99 }],
      experiment_arm_summaries: [{ experiment_arm: 'directive_fix_first', session_count: 99 }],
      review_queue: [],
    },
  });

  assert.equal(review.summary.review_queue_count, 1);
  assert.equal(review.focus_area_summaries[0].focus_area, 'propagation');
  assert.equal(review.top_action_failure_summary[0].outcome_bucket, 'missed_expected_signal');
  assert.equal(review.experiment_arms[0].experiment_arm, 'fix_this_first');
  assert.equal(review.experiment_arms[0].session_count, 1);
});

test('buildEvidenceReview summarizes treatment-vs-baseline comparisons when a baseline arm exists', function () {
  const review = buildEvidenceReview({
    scorecard: {
      repo_label: 'demo-repo',
      summary: {
        kpis: {
          session_verdict_count: 3,
        },
      },
      signals: [
        {
          signal_kind: 'incomplete_propagation',
          signal_family: 'obligation',
          promotion_status: 'trusted',
          product_primary_lane: 'agent_default',
          default_surface_role: 'lead',
          session_verdict_count: 3,
          top_action_follow_rate: 1,
          top_action_help_rate: 1,
          task_success_rate: 1,
          patch_expansion_rate: 0,
          intervention_net_value_score: 1,
          promotion_recommendation: 'keep_trusted',
          default_rollout_recommendation: 'await_treatment_proof',
        },
      ],
    },
    sessionCorpus: {
      sessions: [
        {
          session_id: 'baseline-1',
          lane: 'live',
          focus_areas: ['propagation'],
          experiment_arm: 'no_intervention',
          expected_signal_kinds: ['incomplete_propagation'],
          outcome_bucket: 'missed_expected_signal',
          outcome: {
            initial_top_action_kind: 'large_file',
            top_action_cleared: false,
            checks_to_clear_top_action: null,
            convergence_status: 'stalled',
            entropy_delta: 1,
            final_gate: 'fail',
            final_session_clean: false,
            followup_regression_introduced: false,
          },
          session_verdict: {
            session_id: 'baseline-1',
            top_action_followed: false,
            top_action_helped: false,
            task_completed_successfully: false,
            patch_expanded_unnecessarily: false,
            intervention_cost_checks: 0,
            reviewer_confidence: 'high',
            notes: 'Baseline missed the repair.',
          },
        },
        {
          session_id: 'treatment-1',
          lane: 'live',
          focus_areas: ['propagation'],
          experiment_arm: 'fix_this_first',
          expected_signal_kinds: ['incomplete_propagation'],
          outcome_bucket: 'clean',
          outcome: {
            initial_top_action_kind: 'incomplete_propagation',
            top_action_cleared: true,
            checks_to_clear_top_action: 1,
            convergence_status: 'converged',
            entropy_delta: -1,
            final_gate: 'pass',
            final_session_clean: true,
            followup_regression_introduced: false,
          },
          session_verdict: {
            session_id: 'treatment-1',
            top_action_followed: true,
            top_action_helped: true,
            task_completed_successfully: true,
            patch_expanded_unnecessarily: false,
            intervention_cost_checks: 1,
            reviewer_confidence: 'high',
            notes: 'Treatment converged.',
          },
        },
      ],
      review_queue: [],
    },
  });

  assert.equal(review.summary.experiment_arm_comparison_count, 1);
  assert.equal(review.summary.default_on_candidate_count, 1);
  assert.equal(review.default_on_candidates[0].signal_kind, 'incomplete_propagation');
  assert.equal(review.default_on_promotion.ready, false);
  assert.equal(review.default_on_promotion.evidence_complete, false);
  assert.equal(review.default_on_promotion.evidence_scope, 'repo_level');
  assert.equal(review.default_on_promotion.repo_treatment_ready, true);
  assert.equal(review.default_on_promotion.signal_matched_treatment_evidence, false);
  assert.equal(review.default_on_promotion.best_treatment_arm, 'fix_this_first');
  assert(review.default_on_promotion.blockers.includes('missing_signal_matched_treatment_evidence'));
  assert.equal(review.experiment_arm_comparisons[0].experiment_arm, 'fix_this_first');
  assert.equal(review.experiment_arm_comparisons[0].baseline_experiment_arm, 'no_intervention');
  assert.equal(review.experiment_arm_comparisons[0].top_action_help_rate_delta, 1);
  assert.equal(review.experiment_arm_comparisons[0].task_success_rate_delta, 1);
  assert.equal(review.experiment_arm_comparisons[0].intervention_net_value_score_delta, 1);
  assert.match(formatEvidenceReviewMarkdown(review), /Experiment Arm Comparisons/);
  assert.match(formatEvidenceReviewMarkdown(review), /Default-On Candidates/);
  assert.match(formatEvidenceReviewMarkdown(review), /repo treatment ready: true/);
  assert.match(
    formatEvidenceReviewMarkdown(review),
    /signal-matched treatment evidence: false/,
  );
});
