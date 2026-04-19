import assert from 'node:assert/strict';
import { mkdtemp, mkdir, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import test from 'node:test';

import {
  buildExperimentRunPlan,
  buildExperimentTracker,
  formatExperimentRunMarkdown,
  formatExperimentTrackerMarkdown,
  loadExperimentRegistry,
} from '../lib/experiment-program.mjs';

async function writeJson(targetPath, value) {
  await mkdir(path.dirname(targetPath), { recursive: true });
  await writeFile(targetPath, `${JSON.stringify(value, null, 2)}\n`, 'utf8');
}

async function writeLoopRunner(repoRootPath) {
  await mkdir(path.join(repoRootPath, 'scripts', 'evals'), { recursive: true });
  await writeFile(
    path.join(repoRootPath, 'scripts', 'evals', 'run-repo-calibration-loop.mjs'),
    '#!/usr/bin/env node\n',
    'utf8',
  );
}

function buildDemoCalibrationManifest() {
  return {
    schema_version: 1,
    repo_id: 'demo',
    repo_label: 'demo',
    repo_root: '../../../../../target-repo',
    cohort_manifest: '../signal-cohorts.json',
    live_batch_manifest: '../batches/demo-live.json',
    replay_batch_manifest: '../batches/demo-replay.json',
    artifacts: {
      review_packet_output: '.sentrux/evals/demo/check-review-packet.md',
      review_verdicts_input: '../../examples/review-verdicts.json',
      review_verdicts_output: '.sentrux/evals/demo/review-verdicts.json',
      scorecard_output: '.sentrux/evals/demo/signal-scorecard.json',
      backlog_output: '.sentrux/evals/demo/signal-backlog.json',
    },
  };
}

function buildExperimentSpec(overrides = {}) {
  return {
    schema_version: 1,
    cycle_id: '2026-04-cycle-1',
    program_id: 'agent-loop-core',
    question_id: 'default_lane_family_selection',
    owner_doc: '../../experiment-program.md',
    primary_metrics: ['top_action_help_rate'],
    secondary_metrics: ['reviewed_precision'],
    control_variant_id: 'current_policy',
    repo_scope: ['demo'],
    stages: [
      {
        stage_id: 'screen',
        title: 'Screen variants',
        status: 'in_progress',
        exit_bar: ['Collect one repo run.'],
      },
      {
        stage_id: 'confirm',
        title: 'Confirm shortlist',
        status: 'planned',
        exit_bar: ['Shortlist confirmation-ready variants.'],
      },
      {
        stage_id: 'decide',
        title: 'Record decision',
        status: 'planned',
        exit_bar: ['Record the final default-lane decision.'],
      },
    ],
    variants: [
      {
        variant_id: 'current_policy',
        name: 'Current policy',
        status: 'active',
        description: 'Current policy.',
      },
    ],
    repo_runs: [
      {
        run_id: 'demo-current',
        repo_id: 'demo',
        variant_id: 'current_policy',
        manifest: '../repos/demo.json',
        output_dir: '.sentrux/evals/experiments/demo/current/demo',
      },
    ],
    decision: null,
    ...overrides,
  };
}

test('buildExperimentRunPlan resolves repo roots, outputs, and loop flags', async function () {
  const tempRoot = await mkdtemp(path.join(tmpdir(), 'sentrux-experiment-plan-'));

  try {
    const repoRootPath = path.join(tempRoot, 'workspace');
    const targetRepoRoot = path.join(tempRoot, 'target-repo');
    const experimentSpecPath = path.join(
      repoRootPath,
      'docs',
      'v2',
      'evals',
      'experiments',
      'default-lane.json',
    );
    const calibrationManifestPath = path.join(
      repoRootPath,
      'docs',
      'v2',
      'evals',
      'repos',
      'demo.json',
    );

    await mkdir(targetRepoRoot, { recursive: true });
    await writeLoopRunner(repoRootPath);
    await writeJson(calibrationManifestPath, buildDemoCalibrationManifest());

    await writeJson(experimentSpecPath, buildExperimentSpec({
      experiment_id: 'default-lane-family-ablation',
      title: 'Default Lane Family Ablation',
      workstream: 'default_lane',
      status: 'in_progress',
      phase_id: 'phase_6_default_lane_family_ablation',
      decision_question: 'Which families belong in the default lane?',
      hypothesis: 'Patch-local causal signals should outperform structural pressure.',
      exit_bar: ['Select the top 2 variants for confirmation.'],
      repo_runs: [
        {
          run_id: 'demo-current',
          repo_id: 'demo',
          variant_id: 'current_policy',
          manifest: '../repos/demo.json',
          output_dir: '.sentrux/evals/experiments/default-lane/current/demo',
          flags: {
            skip_backlog: true,
            skip_review: true,
          },
        },
      ],
      variants: [
        {
          variant_id: 'current_policy',
          name: 'Current policy',
          status: 'active',
          description: 'Current ranking and gating behavior.',
        },
      ],
    }));

    const plan = await buildExperimentRunPlan({
      specPath: experimentSpecPath,
      repoRootPath,
    });

    assert.equal(plan.runs.length, 1);
    assert.equal(plan.runs[0].repo_root_path, targetRepoRoot);
    assert.equal(
      plan.runs[0].output_dir,
      path.join(
        targetRepoRoot,
        '.sentrux',
        'evals',
        'experiments',
        'default-lane',
        'current',
        'demo',
      ),
    );
    assert.deepEqual(plan.runs[0].args.slice(-2), ['--skip-review', '--skip-backlog']);
    assert.match(plan.runs[0].command, /run-repo-calibration-loop\.mjs/);
  } finally {
    await rm(tempRoot, { recursive: true, force: true });
  }
});

test('buildExperimentTracker reports completed evidence and next gates', async function () {
  const tempRoot = await mkdtemp(path.join(tmpdir(), 'sentrux-experiment-tracker-'));

  try {
    const repoRootPath = path.join(tempRoot, 'workspace');
    const targetRepoRoot = path.join(tempRoot, 'target-repo');
    const experimentsDir = path.join(repoRootPath, 'docs', 'v2', 'evals', 'experiments');
    const reposDir = path.join(repoRootPath, 'docs', 'v2', 'evals', 'repos');
    const indexPath = path.join(experimentsDir, 'index.json');
    const experimentSpecPath = path.join(
      experimentsDir,
      'large-file-default-lane-admissibility.json',
    );
    const calibrationManifestPath = path.join(reposDir, 'demo.json');
    const runOutputDir = path.join(
      targetRepoRoot,
      '.sentrux',
      'evals',
      'experiments',
      'large-file-default-lane-admissibility',
      'current_policy',
      'demo',
    );

    await mkdir(targetRepoRoot, { recursive: true });
    await writeLoopRunner(repoRootPath);

    await writeJson(indexPath, {
      schema_version: 1,
      generated_at: '2026-04-19T00:00:00.000Z',
      schema: '../experiment.schema.json',
      experiments: [
        {
          experiment_id: 'large-file-default-lane-admissibility',
          path: './large-file-default-lane-admissibility.json',
        },
      ],
    });
    await writeJson(calibrationManifestPath, buildDemoCalibrationManifest());
    await writeJson(experimentSpecPath, buildExperimentSpec({
      experiment_id: 'large-file-default-lane-admissibility',
      title: 'Large File Default-Lane Admissibility',
      workstream: 'structural_pressure',
      status: 'in_progress',
      question_id: 'large_file_default_lane_admissibility',
      phase_id: 'phase_6_large_file_admissibility',
      decision_question: 'Should large_file stay in the default lane?',
      hypothesis: 'Large-file pressure should stay default-lane eligible only when it helps repair outcomes.',
      repo_scope: ['demo'],
      exit_bar: ['Retain, constrain, or demote the signal with explicit restrictions.'],
      variants: [
        {
          variant_id: 'current_policy',
          name: 'Current policy',
          status: 'active',
          description: 'Current treatment for structural pressure.',
        },
      ],
      repo_runs: [
        {
          run_id: 'demo-current',
          repo_id: 'demo',
          variant_id: 'current_policy',
          manifest: '../repos/demo.json',
          output_dir: '.sentrux/evals/experiments/large-file-default-lane-admissibility/current_policy/demo',
        },
      ],
    }));
    await writeJson(path.join(runOutputDir, 'repo-calibration-loop.json'), {
      schema_version: 1,
      generated_at: '2026-04-19T02:00:00.000Z',
      artifacts: {
        evidence_review_json: path.join(runOutputDir, 'evidence-review.json'),
        session_corpus_json: path.join(runOutputDir, 'session-corpus.json'),
        scorecard_json: path.join(runOutputDir, 'signal-scorecard.json'),
      },
      summary: {
        top_action_follow_rate: 0.4,
        top_action_help_rate: 0.2,
        task_success_rate: 0.6,
        intervention_net_value_score: 0.1,
        ranking_miss_count: 1,
        demotion_candidate_count: 1,
        default_on_ready: false,
      },
    });

    const tracker = await buildExperimentTracker({
      indexPath,
      repoRootPath,
    });
    const markdown = formatExperimentTrackerMarkdown(tracker);

    assert.equal(tracker.summary.experiment_count, 1);
    assert.equal(tracker.experiments[0].run_status_counts.completed, 1);
    assert.equal(tracker.experiments[0].next_gate, 'decision_review_required');
    assert.equal(tracker.experiments[0].control_variant_id, 'current_policy');
    assert.equal(tracker.experiments[0].repo_scope[0], 'demo');
    assert.equal(tracker.experiments[0].stages[0].stage_id, 'screen');
    assert.equal(
      tracker.experiments[0].runs[0].metrics.top_action_help_rate,
      0.2,
    );
    assert.match(markdown, /Large File Default-Lane Admissibility/);
    assert.match(markdown, /control variant: current_policy/);
    assert.match(markdown, /active stage: screen \(in_progress\)/);
    assert.match(markdown, /top_action_help_rate=0.200/);
    assert.match(markdown, /next gate: decision_review_required/);
  } finally {
    await rm(tempRoot, { recursive: true, force: true });
  }
});

test('buildExperimentRunPlan rejects repo runs that reference the wrong repo manifest', async function () {
  const tempRoot = await mkdtemp(path.join(tmpdir(), 'sentrux-experiment-plan-mismatch-'));

  try {
    const repoRootPath = path.join(tempRoot, 'workspace');
    const targetRepoRoot = path.join(tempRoot, 'target-repo');
    const experimentSpecPath = path.join(
      repoRootPath,
      'docs',
      'v2',
      'evals',
      'experiments',
      'mismatch.json',
    );
    const calibrationManifestPath = path.join(
      repoRootPath,
      'docs',
      'v2',
      'evals',
      'repos',
      'demo.json',
    );

    await mkdir(targetRepoRoot, { recursive: true });
    await writeLoopRunner(repoRootPath);
    await writeJson(calibrationManifestPath, buildDemoCalibrationManifest());
    await writeJson(experimentSpecPath, buildExperimentSpec({
      experiment_id: 'repo-id-mismatch',
      title: 'Repo Id Mismatch',
      workstream: 'default_lane',
      status: 'planned',
      phase_id: 'phase_6_default_lane_family_ablation',
      decision_question: 'Does repo validation fail fast?',
      hypothesis: 'Runs should not be allowed to point at the wrong repo manifest.',
      exit_bar: ['Reject mismatched repo ids.'],
      repo_runs: [
        {
          run_id: 'demo-current',
          repo_id: 'different-repo',
          variant_id: 'current_policy',
          manifest: '../repos/demo.json',
          output_dir: '.sentrux/evals/experiments/default-lane/current/demo',
        },
      ],
    }));

    await assert.rejects(
      async function rejectMismatchedRepoRun() {
        await buildExperimentRunPlan({
          specPath: experimentSpecPath,
          repoRootPath,
        });
      },
      /expects repo_id "different-repo" but manifest .* is "demo"/,
    );
  } finally {
    await rm(tempRoot, { recursive: true, force: true });
  }
});

test('loadExperimentRegistry rejects index entries that drift from spec ids', async function () {
  const tempRoot = await mkdtemp(path.join(tmpdir(), 'sentrux-experiment-registry-'));

  try {
    const experimentsDir = path.join(tempRoot, 'docs', 'v2', 'evals', 'experiments');
    const indexPath = path.join(experimentsDir, 'index.json');
    const experimentSpecPath = path.join(experimentsDir, 'actual-spec.json');

    await writeJson(indexPath, {
      schema_version: 1,
      generated_at: '2026-04-19T00:00:00.000Z',
      experiments: [
        {
          experiment_id: 'reference-id',
          path: './actual-spec.json',
        },
      ],
    });
    await writeJson(experimentSpecPath, buildExperimentSpec({
      experiment_id: 'different-spec-id',
      title: 'Different Spec Id',
      workstream: 'default_lane',
      status: 'planned',
      phase_id: 'phase_6_default_lane_family_ablation',
      decision_question: 'Does the loader reject drift?',
      hypothesis: 'The reference id should match the spec id.',
      exit_bar: ['Reject drift.'],
    }));

    await assert.rejects(
      function rejectDriftedRegistry() {
        return loadExperimentRegistry(indexPath);
      },
      /does not match spec id/,
    );
  } finally {
    await rm(tempRoot, { recursive: true, force: true });
  }
});

test('formatExperimentRunMarkdown surfaces planned commands for dry runs', function () {
  const markdown = formatExperimentRunMarkdown(
    {
      generated_at: '2026-04-19T03:00:00.000Z',
      status: 'dry_run',
      run_results: [
        {
          run_id: 'demo-current',
          repo_id: 'demo',
          variant_id: 'current_policy',
          execution_mode: 'repo_calibration_loop',
          status: 'planned',
          output_dir: '/tmp/demo-output',
          command: 'node scripts/evals/run-repo-calibration-loop.mjs --manifest demo.json',
        },
      ],
    },
    {
      spec: {
        title: 'Default Lane Family Ablation',
        experiment_id: 'default-lane-family-ablation',
      },
    },
  );

  assert.match(markdown, /output dir: \/tmp\/demo-output/);
  assert.match(markdown, /command: `node scripts\/evals\/run-repo-calibration-loop\.mjs --manifest demo\.json`/);
});
