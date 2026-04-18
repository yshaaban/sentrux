import assert from 'node:assert/strict';
import test from 'node:test';

import { buildTaskSessionOptions } from '../evals/run-codex-session-batch.mjs';
import {
  normalizeExperimentArm,
  applyExperimentArmToPrompt,
} from '../evals/run-codex-session/intervention-arms.mjs';
import { buildCodexBundle } from '../evals/run-codex-session/resources.mjs';

test('normalizeExperimentArm accepts legacy aliases', function () {
  assert.equal(normalizeExperimentArm('baseline'), 'no_intervention');
  assert.equal(normalizeExperimentArm('baseline/no-intervention'), 'no_intervention');
  assert.equal(normalizeExperimentArm('no-intervention'), 'no_intervention');
  assert.equal(normalizeExperimentArm('report-only'), 'report_only');
  assert.equal(normalizeExperimentArm('fix-first'), 'fix_this_first');
  assert.equal(normalizeExperimentArm('directive_fix_first'), 'fix_this_first');
  assert.equal(
    normalizeExperimentArm('directive_stop_and_refactor'),
    'stop_and_refactor',
  );
  assert.equal(normalizeExperimentArm('stop-and-refactor'), 'stop_and_refactor');
  assert.equal(normalizeExperimentArm('report_only'), 'report_only');
  assert.equal(normalizeExperimentArm(null), null);
});

test('applyExperimentArmToPrompt preserves baseline prompts', function () {
  const prompt = 'Make the smallest safe change.';
  assert.equal(
    applyExperimentArmToPrompt(prompt, { experimentArm: 'no_intervention' }),
    prompt,
  );
  assert.equal(applyExperimentArmToPrompt(prompt, { experimentArm: null }), prompt);
});

test('applyExperimentArmToPrompt prepends directive context for active arms', function () {
  const prompt = applyExperimentArmToPrompt('Fix the propagation issue.', {
    experimentArm: 'fix_this_first',
    sessionGoal: 'clear the top propagation action',
    successCriteria: 'the followthrough path is complete',
    expectedSignalKinds: ['incomplete_propagation'],
    expectedFixSurface: 'consumer followthrough',
  });

  assert.match(prompt, /Calibration experiment context:/);
  assert.match(prompt, /experiment arm: fix_this_first/);
  assert.match(prompt, /session goal: clear the top propagation action/);
  assert.match(prompt, /expected signal kinds: incomplete_propagation/);
  assert.match(prompt, /Original task:/);
  assert.match(prompt, /Fix the propagation issue\./);
});

test('buildTaskSessionOptions normalizes task experiment arms', function () {
  const options = buildTaskSessionOptions(
    {
      task_id: 'smoke-task',
      task_label: 'Smoke Task',
      prompt: 'noop',
      experiment_arm: 'directive_fix_first',
      session_goal: 'clear the top issue first',
      success_criteria: 'the task remains contained',
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

  assert.equal(options.experimentArm, 'fix_this_first');
  assert.equal(options.sessionGoal, 'clear the top issue first');
  assert.equal(options.successCriteria, 'the task remains contained');
});

test('buildCodexBundle persists experiment metadata', function () {
  const bundle = buildCodexBundle({
    args: {
      analysisMode: 'working_tree',
      tags: ['calibration'],
      expectedSignalKinds: ['incomplete_propagation'],
      expectedFixSurface: 'followthrough',
      experimentArm: 'fix_this_first',
      sessionGoal: 'clear the top propagation action',
      successCriteria: 'the followthrough path is complete',
    },
    repoLabel: 'sentrux',
    taskId: 'task-1',
    sourceRoot: '/tmp/repo',
    clone: { workRoot: '/tmp/repo-clone' },
    taskLabel: 'Task',
    paths: { promptPath: '/tmp/prompt.md' },
    startedAt: '2026-04-18T00:00:00.000Z',
    providerRun: {
      timeout_phase: null,
      exit_code: 0,
      timed_out: false,
      idle_timed_out: false,
    },
    executionStatus: 'completed',
    snapshots: [{ check: { kind: 'check' } }],
    finalSnapshot: { check: { kind: 'check' } },
    finalGate: { payload: { decision: 'pass' } },
    sessionEnd: { payload: { decision: 'pass' } },
    sessionTelemetry: {
      summary: { session_count: 1 },
      sessions: [],
      signals: [],
    },
  });

  assert.equal(bundle.experiment_arm, 'fix_this_first');
  assert.equal(bundle.session_goal, 'clear the top propagation action');
  assert.equal(bundle.success_criteria, 'the followthrough path is complete');
});
