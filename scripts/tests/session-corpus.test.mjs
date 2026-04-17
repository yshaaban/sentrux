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
          expected_signal_kinds: ['clone_propagation_drift'],
          outcome: null,
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

  assert.equal(corpus.summary.session_count, 3);
  assert.equal(corpus.summary.live_session_count, 2);
  assert.equal(corpus.summary.replay_session_count, 1);
  assert.equal(corpus.summary.clean_session_count, 1);
  assert.equal(corpus.summary.provider_failure_count, 1);
  assert.equal(corpus.summary.missed_expected_signal_count, 1);
  assert.equal(corpus.summary.propagation_session_count, 1);
  assert.equal(corpus.summary.clone_session_count, 2);
  assert.equal(corpus.summary.top_action_session_count, 2);
  assert.equal(corpus.summary.top_action_cleared_count, 1);
  assert.equal(corpus.summary.agent_clear_rate, 0.5);
  assert.equal(corpus.summary.regression_after_fix_rate, 0);
  assert.equal(corpus.summary.propagation_escape_rate, 1);
  assert.equal(corpus.summary.duplicate_logic_introduced_rate, 0.333);
  assert.equal(corpus.review_queue.length, 1);
  assert.equal(corpus.sessions[0].outcome_bucket, 'provider_failed');
  assert.equal(corpus.sessions[1].outcome_bucket, 'missed_expected_signal');
  assert.equal(corpus.sessions[2].outcome_bucket, 'clean');
  assert.match(
    formatSessionCorpusMarkdown(corpus),
    /duplicate logic introduced rate: 0.333/,
  );
  assert.match(formatSessionCorpusMarkdown(corpus), /top-action sessions: 2/);
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
