import assert from 'node:assert/strict';
import test from 'node:test';

import {
  buildSessionCorpus,
  formatSessionCorpusMarkdown,
} from '../lib/session-corpus.mjs';

test('buildSessionCorpus normalizes live and replay sessions into one review surface', function () {
  const corpus = buildSessionCorpus({
    repoLabel: 'demo-repo',
    sessionTelemetry: {
      summary: {
        session_count: 3,
        thrashing_session_count: 1,
        average_entropy_delta: 0.333,
      },
    },
    codexBatch: {
      repo_label: 'demo-repo',
      results: [
        {
          status: 'completed',
          task_id: 'propagation-fix',
          task_label: 'Propagation fix',
          tags: ['propagation'],
          experiment_arm: 'directive_fix_first',
          expected_signal_kinds: ['incomplete_propagation'],
          outcome: {
            session_count: 1,
            initial_action_kinds: ['large_file'],
            initial_top_action_kind: 'large_file',
            convergence_status: 'stalled',
            entropy_delta: 1,
            final_gate: 'fail',
            final_session_clean: false,
            top_action_cleared: false,
            checks_to_clear_top_action: null,
            followup_regression_introduced: false,
          },
        },
      ],
      failures: [
        {
          status: 'timed_out',
          task_id: 'clone-cleanup',
          task_label: 'Clone cleanup',
          tags: ['clone'],
          experiment_arm: 'report_only',
          expected_signal_kinds: ['clone_propagation_drift'],
          outcome: null,
        },
        {
          status: 'completed',
          task_id: 'governance-fix',
          task_label: 'Governance fix',
          tags: ['governance', 'session'],
          experiment_arm: 'directive_fix_first',
          expected_signal_kinds: ['closed_domain_exhaustiveness'],
          outcome: {
            session_count: 1,
            initial_action_kinds: ['closed_domain_exhaustiveness'],
            initial_top_action_kind: 'closed_domain_exhaustiveness',
            convergence_status: 'converged',
            entropy_delta: -1,
            final_gate: 'pass',
            final_session_clean: true,
            top_action_cleared: true,
            checks_to_clear_top_action: 1,
            followup_regression_introduced: false,
          },
        },
      ],
    },
    replayBatch: {
      repo_label: 'demo-repo',
      results: [
        {
          replay_id: 'commit-123',
          commit: 'abc123',
          tags: ['clone'],
          experiment_arm: 'directive_fix_first',
          expected_signal_kinds: ['session_introduced_clone'],
          outcome: {
            session_count: 1,
            initial_action_kinds: ['session_introduced_clone'],
            initial_top_action_kind: 'session_introduced_clone',
            convergence_status: 'converged',
            entropy_delta: -1,
            final_gate: 'pass',
            final_session_clean: true,
            top_action_cleared: true,
            checks_to_clear_top_action: 1,
            followup_regression_introduced: false,
          },
        },
      ],
    },
  });

  assert.equal(corpus.summary.session_count, 4);
  assert.equal(corpus.summary.live_session_count, 3);
  assert.equal(corpus.summary.replay_session_count, 1);
  assert.equal(corpus.summary.clean_session_count, 2);
  assert.equal(corpus.summary.provider_failure_count, 1);
  assert.equal(corpus.summary.missed_expected_signal_count, 1);
  assert.equal(corpus.summary.propagation_session_count, 2);
  assert.equal(corpus.summary.clone_session_count, 2);
  assert.equal(corpus.summary.focus_area_count, 3);
  assert.equal(corpus.summary.top_action_failure_count, 2);
  assert.equal(corpus.summary.experiment_arm_count, 2);
  assert.equal(corpus.summary.top_action_session_count, 3);
  assert.equal(corpus.summary.top_action_cleared_count, 2);
  assert.equal(corpus.summary.agent_clear_rate, 0.667);
  assert.equal(corpus.summary.regression_after_fix_rate, 0);
  assert.equal(corpus.summary.propagation_escape_rate, 0.5);
  assert.equal(corpus.summary.duplicate_logic_introduced_rate, 0.25);
  assert.equal(corpus.review_queue.length, 1);
  assert.equal(corpus.sessions[0].outcome_bucket, 'provider_failed');
  assert.equal(corpus.sessions[1].outcome_bucket, 'clean');
  assert.equal(corpus.sessions[2].outcome_bucket, 'missed_expected_signal');
  assert.equal(corpus.sessions[3].outcome_bucket, 'clean');
  assert.equal(corpus.focus_area_summaries[0].focus_area, 'clone_followthrough');
  assert.equal(corpus.focus_area_summaries[0].session_count, 2);
  assert.equal(corpus.focus_area_summaries[1].focus_area, 'propagation');
  assert.equal(corpus.focus_area_summaries[2].focus_area, 'session_governance');
  assert.equal(
    corpus.top_action_failure_summary.find((entry) => entry.outcome_bucket === 'missed_expected_signal')
      ?.focus_area_counts[0].focus_area,
    'propagation',
  );
  assert.equal(corpus.experiment_arm_summaries[0].experiment_arm, 'fix_this_first');
  assert.equal(corpus.experiment_arm_summaries[0].session_count, 3);
  assert.equal(corpus.experiment_arm_summaries[0].focus_area_counts[0].focus_area, 'propagation');
  assert.match(
    formatSessionCorpusMarkdown(corpus),
    /duplicate logic introduced rate: 0.25/,
  );
  assert.match(formatSessionCorpusMarkdown(corpus), /top-action sessions: 3/);
  assert.match(formatSessionCorpusMarkdown(corpus), /Focus Areas/);
  assert.match(formatSessionCorpusMarkdown(corpus), /Top Action Failures/);
  assert.match(formatSessionCorpusMarkdown(corpus), /Experiment Arms/);
});

test('buildSessionCorpus keeps clean-but-misranked sessions in the review queue', function () {
  const corpus = buildSessionCorpus({
    repoLabel: 'demo-repo',
    codexBatch: {
      repo_label: 'demo-repo',
      results: [
        {
          status: 'completed',
          task_id: 'propagation-clean-miss',
          task_label: 'Propagation clean miss',
          tags: ['propagation'],
          expected_signal_kinds: ['incomplete_propagation'],
          outcome: {
            session_count: 1,
            initial_action_kinds: ['large_file', 'incomplete_propagation'],
            initial_top_action_kind: 'large_file',
            convergence_status: 'converged',
            entropy_delta: -1,
            final_gate: 'pass',
            final_session_clean: true,
            top_action_cleared: true,
            checks_to_clear_top_action: 1,
            followup_regression_introduced: false,
          },
        },
      ],
    },
  });

  assert.equal(corpus.sessions[0].outcome_bucket, 'clean_but_misranked');
  assert.equal(corpus.review_queue.length, 1);
  assert.equal(corpus.review_queue[0].session_id, 'propagation-clean-miss');
});
