import assert from 'node:assert/strict';
import test from 'node:test';

import {
  buildTaskSessionOptions,
  summarizeTaskRuns,
} from '../evals/run-codex-session-batch.mjs';
import { applyExperimentArmToPrompt } from '../evals/run-codex-session/intervention-arms.mjs';

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
      converged_session_count: 0,
      converging_session_count: 0,
      stalled_session_count: 1,
      thrashing_session_count: 0,
      average_entropy_delta: 0,
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
        convergence_status: 'stalled',
        entropy_delta: 0,
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
        top_action_sessions: 1,
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
        session_thrash_rate: 0,
        average_entropy_delta: 0,
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

test('buildTaskSessionOptions lets tasks override manifest time budgets', function () {
  const options = buildTaskSessionOptions(
    {
      task_id: 'smoke-task',
      task_label: 'Smoke Task',
      prompt: 'noop',
      timeout_ms: 120000,
      idle_timeout_ms: 45000,
    },
    {
      repo_label: 'sentrux',
      timeout_ms: 300000,
      idle_timeout_ms: 60000,
      poll_ms: 4000,
      codex_bin: 'codex',
    },
    '/tmp/manifests',
    '/tmp/repo',
    '/tmp/output',
  );

  assert.equal(options.timeoutMs, 120000);
  assert.equal(options.idleTimeoutMs, 45000);
  assert.equal(options.outputDir, '/tmp/output/smoke-task');
});

test('buildTaskSessionOptions preserves task-level experiment arms', function () {
  const options = buildTaskSessionOptions(
    {
      task_id: 'arm-task',
      task_label: 'Arm Task',
      prompt: 'noop',
      experiment_arm: 'fix-first',
    },
    {
      repo_label: 'sentrux',
    },
    '/tmp/manifests',
    '/tmp/repo',
    '/tmp/output',
  );

  assert.equal(options.experimentArm, 'fix_this_first');
});

test('applyExperimentArmToPrompt wraps prompts with arm instructions and context', function () {
  const prompt = 'Fix the thing\n\nMore detail.';
  const wrapped = applyExperimentArmToPrompt(prompt, {
    experimentArm: 'stop-and-refactor',
    sessionGoal: 'clean up the shared helper',
    successCriteria: 'the patch stays localized',
    expectedSignalKinds: ['clone_propagation_drift'],
    expectedFixSurface: 'shared helper extraction',
  });

  assert.ok(wrapped.startsWith('Calibration experiment context:'));
  assert.ok(wrapped.includes('experiment arm: stop_and_refactor'));
  assert.ok(wrapped.includes('session goal: clean up the shared helper'));
  assert.ok(wrapped.includes('expected signal kinds: clone_propagation_drift'));
  assert.ok(wrapped.includes('expected fix surface: shared helper extraction'));
  assert.ok(wrapped.includes('Intervention instructions for this run:'));
  assert.ok(wrapped.includes(prompt));
  assert.equal(applyExperimentArmToPrompt(prompt, { experimentArm: null }), prompt);
});
