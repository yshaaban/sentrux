#!/usr/bin/env node

import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

import { nowIso } from '../lib/eval-batch.mjs';
import { parseCliArgs } from '../lib/eval-support.mjs';
import {
  buildExperimentRunPlan,
  buildExperimentTracker,
  collectExperimentRunState,
  formatExperimentTrackerMarkdown,
  loadExperimentRegistry,
} from '../lib/experiment-program.mjs';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, '../..');
const defaultIndexPath = path.join(repoRoot, 'docs', 'v2', 'evals', 'experiments', 'index.json');

function defaultTrackerArtifactPathForRepo(repoRootPath, fileName) {
  return path.join(
    repoRootPath,
    '.sentrux',
    'evals',
    'experiments',
    fileName,
  );
}

function defaultTrackerJsonPathForRepo(repoRootPath) {
  return defaultTrackerArtifactPathForRepo(repoRootPath, 'experiment-tracker.json');
}

function defaultTrackerMarkdownPathForRepo(repoRootPath) {
  return defaultTrackerArtifactPathForRepo(repoRootPath, 'experiment-tracker.md');
}

function setResolvedPathOption(property) {
  return function setResolvedPathValue(target, value) {
    target[property] = path.resolve(value);
  };
}

export function parseArgs(argv) {
  return parseCliArgs(
    argv,
    {
      indexPath: defaultIndexPath,
      repoRootPath: repoRoot,
      trackerJsonPath: defaultTrackerJsonPathForRepo(repoRoot),
      trackerMarkdownPath: defaultTrackerMarkdownPathForRepo(repoRoot),
    },
    {
      values: {
        '--index': setResolvedPathOption('indexPath'),
        '--repo-root': setResolvedPathOption('repoRootPath'),
        '--tracker-json': setResolvedPathOption('trackerJsonPath'),
        '--tracker-md': setResolvedPathOption('trackerMarkdownPath'),
      },
    },
  );
}

function buildIssue({ experimentId, specPath, runId = null, artifact = null, message }) {
  return {
    experiment_id: experimentId,
    spec_path: specPath,
    run_id: runId,
    artifact,
    message,
  };
}

export async function buildExperimentIntegrityReport({
  indexPath = defaultIndexPath,
  repoRootPath = repoRoot,
  trackerJsonPath = null,
  trackerMarkdownPath = null,
} = {}) {
  const resolvedTrackerJsonPath =
    trackerJsonPath ?? defaultTrackerJsonPathForRepo(repoRootPath);
  const resolvedTrackerMarkdownPath =
    trackerMarkdownPath ?? defaultTrackerMarkdownPathForRepo(repoRootPath);
  const registry = await loadExperimentRegistry(indexPath);
  const issues = [];

  for (const experiment of registry.experiments) {
    const plan = await buildExperimentRunPlan({
      specPath: experiment.spec_path,
      repoRootPath,
    });

    for (const run of plan.runs) {
      const state = await collectExperimentRunState(run);
      if (plan.spec.status !== 'completed' || state.artifact_state === 'completed') {
        continue;
      }

      issues.push(
        buildIssue({
          experimentId: plan.spec.experiment_id,
          specPath: experiment.spec_path,
          runId: run.run_id,
          artifact: 'artifact_expectations',
          message: `Completed run is not complete (state=${state.artifact_state}) and is missing expected artifacts: ${state.missing_artifacts.join(', ') || 'n/a'}`,
        }),
      );
    }
  }

  await appendTrackerArtifactIssues({
    issues,
    indexPath,
    repoRootPath,
    trackerJsonPath: resolvedTrackerJsonPath,
    trackerMarkdownPath: resolvedTrackerMarkdownPath,
  });

  return {
    schema_version: 1,
    generated_at: nowIso(),
    index_path: indexPath,
    repo_root_path: repoRootPath,
    experiment_count: registry.experiments.length,
    issue_count: issues.length,
    issues,
  };
}

async function appendTrackerArtifactIssues({
  issues,
  indexPath,
  repoRootPath,
  trackerJsonPath,
  trackerMarkdownPath,
}) {
  const tracker = await buildExperimentTracker({
    indexPath,
    repoRootPath,
  });
  const trackerMarkdown = formatExperimentTrackerMarkdown(tracker);

  await compareTrackerArtifact({
    issues,
    pathLabel: trackerJsonPath,
    expectedText: normalizeTrackerJsonText(`${JSON.stringify(tracker, null, 2)}\n`),
    artifact: 'experiment_tracker_json',
    message: 'Checked-in experiment tracker JSON is missing or stale.',
  });
  await compareTrackerArtifact({
    issues,
    pathLabel: trackerMarkdownPath,
    expectedText: normalizeTrackerMarkdownText(trackerMarkdown),
    artifact: 'experiment_tracker_markdown',
    message: 'Checked-in experiment tracker Markdown is missing or stale.',
  });
}

async function compareTrackerArtifact({ issues, pathLabel, expectedText, artifact, message }) {
  let actualText = null;

  try {
    actualText = await readFile(pathLabel, 'utf8');
  } catch {
    actualText = null;
  }

  const normalizedActualText = normalizeTrackerArtifactText(artifact, actualText);

  if (normalizedActualText === expectedText) {
    return;
  }

  issues.push(
    buildIssue({
      experimentId: 'registry',
      specPath: pathLabel,
      artifact,
      message,
    }),
  );
}

function normalizeTrackerArtifactText(artifact, text) {
  if (artifact === 'experiment_tracker_json') {
    return normalizeTrackerJsonText(text);
  }
  if (artifact === 'experiment_tracker_markdown') {
    return normalizeTrackerMarkdownText(text);
  }

  return text;
}

function normalizeTrackerJsonText(text) {
  if (typeof text !== 'string') {
    return null;
  }

  try {
    const parsed = JSON.parse(text);
    if (parsed && typeof parsed === 'object') {
      parsed.generated_at = '<normalized>';
    }
    return JSON.stringify(parsed, null, 2);
  } catch {
    return text;
  }
}

function normalizeTrackerMarkdownText(text) {
  if (typeof text !== 'string') {
    return null;
  }

  return text.replace(/^Generated: .+$/m, 'Generated: <normalized>');
}

export function formatExperimentIntegrityReport(report) {
  const lines = [
    '# Experiment Registry Integrity',
    '',
    `Generated: ${report.generated_at}`,
    `Index: ${report.index_path}`,
    `Repo root: ${report.repo_root_path}`,
    `Experiments checked: ${report.experiment_count}`,
    `Issues: ${report.issue_count}`,
    '',
  ];

  if (report.issue_count === 0) {
    lines.push('OK: no registry honesty issues found.');
    lines.push('');
    return `${lines.join('\n').trimEnd()}\n`;
  }

  for (const issue of report.issues) {
    lines.push(`- ${issue.experiment_id}`);
    lines.push(`  - spec: ${issue.spec_path}`);
    if (issue.run_id) {
      lines.push(`  - run: ${issue.run_id}`);
    }
    if (issue.artifact) {
      lines.push(`  - artifact: ${issue.artifact}`);
    }
    lines.push(`  - ${issue.message}`);
  }

  lines.push('');
  return `${lines.join('\n').trimEnd()}\n`;
}

async function main(argv = process.argv) {
  const args = parseArgs(argv);
  const report = await buildExperimentIntegrityReport({
    indexPath: args.indexPath,
    repoRootPath: args.repoRootPath,
    trackerJsonPath: args.trackerJsonPath,
    trackerMarkdownPath: args.trackerMarkdownPath,
  });
  process.stdout.write(formatExperimentIntegrityReport(report));
  if (report.issue_count > 0) {
    process.exitCode = 1;
  }
}

const invokedPath = process.argv[1] ? pathToFileURL(path.resolve(process.argv[1])).href : null;

if (invokedPath === import.meta.url) {
  main().catch(function handleMainError(error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  });
}
