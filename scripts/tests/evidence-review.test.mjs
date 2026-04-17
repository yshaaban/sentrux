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
        },
      ],
      review_queue: [{ session_id: 'live-1' }, { session_id: 'live-2' }],
    },
    reviewPacket: {
      summary: {
        sample_count: 4,
      },
    },
  });

  assert.equal(review.summary.promotion_candidate_count, 1);
  assert.equal(review.summary.demotion_candidate_count, 1);
  assert.equal(review.summary.ranking_miss_count, 1);
  assert.equal(review.summary.review_queue_count, 2);
  assert.equal(review.promotion_candidates[0].signal_kind, 'incomplete_propagation');
  assert.equal(review.demotion_candidates[0].signal_kind, 'forbidden_raw_read');
  assert.equal(review.ranking_misses[0].signal_kind, 'zero_config_boundary_violation');
  assert.equal(review.propagation_examples[0].session_id, 'live-1');
  assert.equal(review.clone_examples[0].session_id, 'live-2');
  assert.equal(review.thrashing_examples[0].session_id, 'live-2');
  assert.equal(review.experiment_arms[0].experiment_arm, 'directive_fix_first');
  assert.equal(review.experiment_arms[0].clean_rate, 0);
  assert.equal(review.experiment_arms[0].regression_rate, 0.5);
  assert.match(formatEvidenceReviewMarkdown(review), /Promotion Candidates/);
});
