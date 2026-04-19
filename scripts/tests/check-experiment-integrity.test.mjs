import assert from 'node:assert/strict';
import { execFile as execFileCallback } from 'node:child_process';
import { mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { promisify } from 'node:util';
import { fileURLToPath } from 'node:url';

import { buildExperimentIntegrityReport } from '../evals/check-experiment-integrity.mjs';
import {
  buildExperimentTracker,
  formatExperimentTrackerMarkdown,
} from '../lib/experiment-program.mjs';

const execFile = promisify(execFileCallback);
const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, '..', '..');
const integrityScriptPath = path.join(__dirname, '..', 'evals', 'check-experiment-integrity.mjs');
const COMPLETED_ARTIFACT_EXPECTATIONS = [
  'repo_calibration_loop',
  'evidence_review',
  'session_corpus',
  'scorecard',
];

async function writeJson(targetPath, value) {
  await mkdir(path.dirname(targetPath), { recursive: true });
  await writeFile(targetPath, `${JSON.stringify(value, null, 2)}\n`, 'utf8');
}

function buildTrackerArtifactPaths(repoRootPath) {
  return {
    trackerJsonPath: path.join(
      repoRootPath,
      '.sentrux',
      'evals',
      'experiments',
      'experiment-tracker.json',
    ),
    trackerMarkdownPath: path.join(
      repoRootPath,
      '.sentrux',
      'evals',
      'experiments',
      'experiment-tracker.md',
    ),
  };
}

async function writeTrackerArtifacts(repoRootPath, indexPath, trackerPaths = null) {
  const {
    trackerJsonPath,
    trackerMarkdownPath,
  } = trackerPaths ?? buildTrackerArtifactPaths(repoRootPath);
  const tracker = await buildExperimentTracker({
    indexPath,
    repoRootPath,
  });

  await writeJson(trackerJsonPath, tracker);
  await writeFile(trackerMarkdownPath, formatExperimentTrackerMarkdown(tracker), 'utf8');
}

async function writeStaleTrackerArtifacts(trackerJsonPath, trackerMarkdownPath) {
  await writeJson(trackerJsonPath, { schema_version: 1, stale: true });
  await writeFile(trackerMarkdownPath, '# stale\n', 'utf8');
}

function buildCalibrationManifest() {
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

function buildCompletedExperimentBase(overrides = {}) {
  return {
    schema_version: 1,
    workstream: 'default_lane',
    status: 'completed',
    cycle_id: '2026-04-cycle-1',
    program_id: 'agent-loop-core',
    phase_id: 'phase_6_default_lane_family_ablation',
    owner_doc: '../../experiment-program.md',
    primary_metrics: ['top_action_help_rate'],
    secondary_metrics: ['reviewed_precision'],
    variants: [
      {
        variant_id: 'current_policy',
        name: 'Current policy',
        description: 'Current policy.',
        status: 'active',
      },
    ],
    decision: {
      outcome: 'promote',
      winner_variant_id: 'current_policy',
      decided_at: '2026-04-19T03:00:00.000Z',
      evidence_summary: 'Decision is recorded.',
      policy_change: 'None.',
    },
    ...overrides,
  };
}

function buildCompletedRun(experimentId) {
  return {
    run_id: 'demo-current',
    repo_id: 'demo',
    variant_id: 'current_policy',
    manifest: '../repos/demo.json',
    output_dir: `.sentrux/evals/experiments/${experimentId}/current_policy/demo`,
    artifact_expectations: COMPLETED_ARTIFACT_EXPECTATIONS,
  };
}

async function writeCompletedRunArtifacts(runOutputDir, { includeScorecard = true } = {}) {
  await writeJson(path.join(runOutputDir, 'repo-calibration-loop.json'), {
    schema_version: 1,
    generated_at: '2026-04-19T02:00:00.000Z',
    artifacts: {
      evidence_review_json: path.join(runOutputDir, 'evidence-review.json'),
      session_corpus_json: path.join(runOutputDir, 'session-corpus.json'),
      scorecard_json: path.join(runOutputDir, 'signal-scorecard.json'),
    },
  });
  await writeFile(path.join(runOutputDir, 'evidence-review.json'), '{}\n', 'utf8');
  await writeFile(path.join(runOutputDir, 'session-corpus.json'), '{}\n', 'utf8');
  if (includeScorecard) {
    await writeFile(path.join(runOutputDir, 'signal-scorecard.json'), '{}\n', 'utf8');
  }
}

async function writeCompletedExperimentFixture({
  repoRootPath,
  indexPath,
  specPath,
  decisionRecordPath,
  runOutputDir,
  experimentId,
  title,
  decisionQuestion,
  hypothesis,
  exitBar,
  includeScorecard = true,
}) {
  await writeJson(indexPath, {
    schema_version: 1,
    generated_at: '2026-04-19T00:00:00.000Z',
    experiments: [
      {
        experiment_id: experimentId,
        path: `./${path.basename(specPath)}`,
      },
    ],
  });
  await writeJson(
    path.join(repoRootPath, 'docs', 'v2', 'evals', 'repos', 'demo.json'),
    buildCalibrationManifest(),
  );
  await writeJson(
    specPath,
    buildCompletedExperimentBase({
      experiment_id: experimentId,
      title,
      decision_question: decisionQuestion,
      hypothesis,
      exit_bar: exitBar,
      decision_record_path: `../decisions/${path.basename(decisionRecordPath)}`,
      repo_runs: [buildCompletedRun(experimentId)],
    }),
  );
  await mkdir(path.dirname(decisionRecordPath), { recursive: true });
  await writeFile(decisionRecordPath, '# Decision record\n', 'utf8');
  await writeCompletedRunArtifacts(runOutputDir, { includeScorecard });
}

async function runIntegrityCli(args) {
  return execFile(process.execPath, [integrityScriptPath, ...args], {
    cwd: repoRoot,
  });
}

test('buildExperimentIntegrityReport passes when completed registry entries are honest', async function () {
  const tempRoot = await mkdtemp(path.join(tmpdir(), 'sentrux-experiment-integrity-clean-'));

  try {
    const repoRootPath = path.join(tempRoot, 'workspace');
    const targetRepoRoot = path.join(tempRoot, 'target-repo');
    const experimentsDir = path.join(repoRootPath, 'docs', 'v2', 'evals', 'experiments');
    const decisionsDir = path.join(repoRootPath, 'docs', 'v2', 'evals', 'decisions');
    const indexPath = path.join(experimentsDir, 'index.json');
    const specPath = path.join(experimentsDir, 'honest-completed.json');
    const decisionRecordPath = path.join(decisionsDir, 'honest-completed.md');
    const runOutputDir = path.join(
      targetRepoRoot,
      '.sentrux',
      'evals',
      'experiments',
      'honest-completed',
      'current_policy',
      'demo',
    );

    await writeCompletedExperimentFixture({
      repoRootPath,
      indexPath,
      specPath,
      decisionRecordPath,
      runOutputDir,
      experimentId: 'honest-completed',
      title: 'Honest Completed Experiment',
      decisionQuestion: 'Does the registry stay honest?',
      hypothesis: 'Completed specs should have a real decision record.',
      exitBar: ['Ship only when the decision record exists.'],
    });
    await writeTrackerArtifacts(repoRootPath, indexPath);

    const report = await buildExperimentIntegrityReport({
      indexPath,
      repoRootPath,
    });

    assert.equal(report.issue_count, 0);
    assert.deepEqual(report.issues, []);
  } finally {
    await rm(tempRoot, { recursive: true, force: true });
  }
});

test('buildExperimentIntegrityReport rejects completed specs with missing decision records', async function () {
  const tempRoot = await mkdtemp(path.join(tmpdir(), 'sentrux-experiment-integrity-decision-'));

  try {
    const repoRootPath = path.join(tempRoot, 'workspace');
    const experimentsDir = path.join(repoRootPath, 'docs', 'v2', 'evals', 'experiments');
    const indexPath = path.join(experimentsDir, 'index.json');
    const specPath = path.join(experimentsDir, 'missing-decision-record.json');

    await writeJson(indexPath, {
      schema_version: 1,
      generated_at: '2026-04-19T00:00:00.000Z',
      experiments: [
        {
          experiment_id: 'missing-decision-record',
          path: './missing-decision-record.json',
        },
      ],
    });
    await writeJson(
      specPath,
      buildCompletedExperimentBase({
        experiment_id: 'missing-decision-record',
        title: 'Missing Decision Record',
        decision_question: 'Does the registry lie?',
        hypothesis: 'Completed specs should not point at missing decision records.',
        exit_bar: ['Record the decision.'],
        decision_record_path: '../decisions/missing-decision-record.md',
        repo_runs: [],
        decision: {
          outcome: 'promote',
          winner_variant_id: 'current_policy',
          decided_at: '2026-04-19T03:00:00.000Z',
          evidence_summary: 'Not written yet.',
          policy_change: 'None.',
        },
      }),
    );

    await assert.rejects(
      async function rejectMissingDecisionRecord() {
        await buildExperimentIntegrityReport({
          indexPath,
          repoRootPath,
        });
      },
      /missing decision record file/,
    );
  } finally {
    await rm(tempRoot, { recursive: true, force: true });
  }
});

test('buildExperimentIntegrityReport rejects completed runs that miss expected artifacts', async function () {
  const tempRoot = await mkdtemp(path.join(tmpdir(), 'sentrux-experiment-integrity-run-'));

  try {
    const repoRootPath = path.join(tempRoot, 'workspace');
    const targetRepoRoot = path.join(tempRoot, 'target-repo');
    const experimentsDir = path.join(repoRootPath, 'docs', 'v2', 'evals', 'experiments');
    const decisionsDir = path.join(repoRootPath, 'docs', 'v2', 'evals', 'decisions');
    const indexPath = path.join(experimentsDir, 'index.json');
    const specPath = path.join(experimentsDir, 'run-missing-artifact.json');
    const decisionRecordPath = path.join(decisionsDir, 'run-missing-artifact.md');
    const runOutputDir = path.join(
      targetRepoRoot,
      '.sentrux',
      'evals',
      'experiments',
      'run-missing-artifact',
      'current_policy',
      'demo',
    );

    await writeCompletedExperimentFixture({
      repoRootPath,
      indexPath,
      specPath,
      decisionRecordPath,
      runOutputDir,
      experimentId: 'run-missing-artifact',
      title: 'Run Missing Artifact',
      decisionQuestion: 'Does the integrity checker notice partial completion?',
      hypothesis: 'Completed runs should satisfy their declared artifacts.',
      exitBar: ['Capture all declared artifacts.'],
      includeScorecard: false,
    });
    await writeTrackerArtifacts(repoRootPath, indexPath);

    const report = await buildExperimentIntegrityReport({
      indexPath,
      repoRootPath,
    });

    assert.equal(report.issue_count, 1);
    assert.equal(report.issues[0].run_id, 'demo-current');
    assert.match(report.issues[0].message, /scorecard/);
  } finally {
    await rm(tempRoot, { recursive: true, force: true });
  }
});

test('buildExperimentIntegrityReport flags stale tracker artifacts', async function () {
  const tempRoot = await mkdtemp(path.join(tmpdir(), 'sentrux-experiment-integrity-tracker-'));

  try {
    const repoRootPath = path.join(tempRoot, 'workspace');
    const experimentsDir = path.join(repoRootPath, 'docs', 'v2', 'evals', 'experiments');
    const indexPath = path.join(experimentsDir, 'index.json');
    const { trackerJsonPath, trackerMarkdownPath } = buildTrackerArtifactPaths(repoRootPath);

    await writeJson(indexPath, {
      schema_version: 1,
      generated_at: '2026-04-19T00:00:00.000Z',
      experiments: [],
    });
    await writeStaleTrackerArtifacts(trackerJsonPath, trackerMarkdownPath);

    const report = await buildExperimentIntegrityReport({
      indexPath,
      repoRootPath,
    });

    assert.equal(report.issue_count, 2);
    assert.deepEqual(
      report.issues
        .map(function mapIssue(issue) {
          return issue.artifact;
        })
        .sort(),
      ['experiment_tracker_json', 'experiment_tracker_markdown'],
    );
  } finally {
    await rm(tempRoot, { recursive: true, force: true });
  }
});

test('integrity CLI honors custom tracker artifact paths', async function () {
  const tempRoot = await mkdtemp(path.join(tmpdir(), 'sentrux-experiment-integrity-cli-'));

  try {
    const repoRootPath = path.join(tempRoot, 'workspace');
    const targetRepoRoot = path.join(tempRoot, 'target-repo');
    const experimentsDir = path.join(repoRootPath, 'docs', 'v2', 'evals', 'experiments');
    const decisionsDir = path.join(repoRootPath, 'docs', 'v2', 'evals', 'decisions');
    const indexPath = path.join(experimentsDir, 'index.json');
    const specPath = path.join(experimentsDir, 'custom-tracker.json');
    const decisionRecordPath = path.join(decisionsDir, 'custom-tracker.md');
    const runOutputDir = path.join(
      targetRepoRoot,
      '.sentrux',
      'evals',
      'experiments',
      'custom-tracker',
      'current_policy',
      'demo',
    );
    const defaultTrackerPaths = buildTrackerArtifactPaths(repoRootPath);
    const customTrackerPaths = {
      trackerJsonPath: path.join(tempRoot, 'custom', 'experiment-tracker.json'),
      trackerMarkdownPath: path.join(tempRoot, 'custom', 'experiment-tracker.md'),
    };

    await writeCompletedExperimentFixture({
      repoRootPath,
      indexPath,
      specPath,
      decisionRecordPath,
      runOutputDir,
      experimentId: 'custom-tracker',
      title: 'Custom Tracker Experiment',
      decisionQuestion: 'Does the CLI forward tracker path overrides?',
      hypothesis: 'The CLI should honor explicit tracker artifact paths.',
      exitBar: ['Use the explicitly requested tracker artifacts.'],
    });
    await writeStaleTrackerArtifacts(
      defaultTrackerPaths.trackerJsonPath,
      defaultTrackerPaths.trackerMarkdownPath,
    );
    await writeTrackerArtifacts(repoRootPath, indexPath, customTrackerPaths);

    const { stdout } = await runIntegrityCli([
      '--index',
      indexPath,
      '--repo-root',
      repoRootPath,
      '--tracker-json',
      customTrackerPaths.trackerJsonPath,
      '--tracker-md',
      customTrackerPaths.trackerMarkdownPath,
    ]);

    assert.match(stdout, /OK: no registry honesty issues found\./);
  } finally {
    await rm(tempRoot, { recursive: true, force: true });
  }
});
