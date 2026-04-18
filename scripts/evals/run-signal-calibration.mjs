#!/usr/bin/env node

import { mkdir, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  formatSessionTelemetrySummaryMarkdown,
  loadSessionTelemetrySummary,
} from '../lib/session-telemetry.mjs';
import {
  buildSignalScorecard,
  formatSignalScorecardMarkdown,
} from '../lib/signal-scorecard.mjs';
import { buildSessionCorpus, formatSessionCorpusMarkdown } from '../lib/session-corpus.mjs';
import { buildEvidenceReview, formatEvidenceReviewMarkdown } from '../lib/evidence-review.mjs';
import { buildSignalBacklog, formatSignalBacklogMarkdown } from '../lib/signal-backlog.mjs';
import { getSignalCohort, loadSignalCohortContext } from '../lib/signal-cohorts.mjs';
import { resolveLatestRepoCalibrationArtifacts } from '../lib/repo-calibration-artifacts.mjs';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, '../..');

function parseArgs(argv) {
  const result = {
    repoRoot: null,
    repoLabel: null,
    defectReportPath: null,
    reviewVerdictsPath: null,
    sessionVerdictsPath: null,
    remediationReportPath: null,
    benchmarkPath: null,
    sessionEventsPath: null,
    codexBatchPath: null,
    replayBatchPath: null,
    latestCalibrationPath: null,
    cohortManifestPath: path.join(repoRoot, 'docs/v2/evals', 'signal-cohorts.json'),
    cohortId: null,
    outputDir: path.join(repoRoot, 'docs/v2/examples'),
  };

  for (let index = 2; index < argv.length; index += 1) {
    const value = argv[index];
    if (value === '--repo-root') {
      index += 1;
      result.repoRoot = argv[index];
      continue;
    }
    if (value === '--repo-label') {
      index += 1;
      result.repoLabel = argv[index];
      continue;
    }
    if (value === '--defect-report') {
      index += 1;
      result.defectReportPath = argv[index];
      continue;
    }
    if (value === '--review-verdicts') {
      index += 1;
      result.reviewVerdictsPath = argv[index];
      continue;
    }
    if (value === '--session-verdicts') {
      index += 1;
      result.sessionVerdictsPath = argv[index];
      continue;
    }
    if (value === '--remediation-report') {
      index += 1;
      result.remediationReportPath = argv[index];
      continue;
    }
    if (value === '--benchmark') {
      index += 1;
      result.benchmarkPath = argv[index];
      continue;
    }
    if (value === '--session-events') {
      index += 1;
      result.sessionEventsPath = argv[index];
      continue;
    }
    if (value === '--codex-batch') {
      index += 1;
      result.codexBatchPath = argv[index];
      continue;
    }
    if (value === '--replay-batch') {
      index += 1;
      result.replayBatchPath = argv[index];
      continue;
    }
    if (value === '--latest-calibration') {
      index += 1;
      result.latestCalibrationPath = argv[index];
      continue;
    }
    if (value === '--cohort-manifest') {
      index += 1;
      result.cohortManifestPath = argv[index];
      continue;
    }
    if (value === '--cohort-id') {
      index += 1;
      result.cohortId = argv[index];
      continue;
    }
    if (value === '--output-dir') {
      index += 1;
      result.outputDir = argv[index];
      continue;
    }
    throw new Error(`Unknown argument: ${value}`);
  }

  if (!result.sessionEventsPath && !result.repoRoot) {
    throw new Error('Provide either --session-events or --repo-root');
  }

  if (!result.defectReportPath) {
    throw new Error('Missing required --defect-report path');
  }

  return result;
}

function defaultSessionEventsPath(repoRootPath) {
  return path.join(repoRootPath, '.sentrux', 'agent-session-events.jsonl');
}

async function readJson(targetPath) {
  if (!targetPath) {
    return null;
  }

  const source = await readFile(targetPath, 'utf8');
  return JSON.parse(source);
}

async function writeArtifact(targetPath, content) {
  await mkdir(path.dirname(targetPath), { recursive: true });
  await writeFile(targetPath, content, 'utf8');
}

async function writeArtifactWithStableCompanion(outputDir, repoLabel, fileName, content) {
  const targetPath = path.join(outputDir, `${repoLabel}-${fileName}`);
  await writeArtifact(targetPath, content);

  if (path.basename(outputDir) === repoLabel) {
    await writeArtifact(path.join(outputDir, fileName), content);
  }
}

function resolveRepoLabel(args, inputs = {}) {
  return (
    args.repoLabel ??
    inputs.defectReport?.repo_label ??
    inputs.reviewVerdicts?.repo ??
    inputs.sessionVerdicts?.repo_label ??
    inputs.sessionVerdicts?.repo ??
    inputs.remediationReport?.repo_label ??
    inputs.sessionTelemetry?.repo_label ??
    inputs.sessionTelemetry?.repo_root ??
    inputs.benchmark?.repo ??
    inputs.benchmark?.repo_root ??
    (args.repoRoot ? path.basename(args.repoRoot) : null) ??
    (args.outputDir ? path.basename(args.outputDir) : null) ??
    'repo'
  );
}

async function main() {
  const args = parseArgs(process.argv);
  const sessionEventsPath = args.sessionEventsPath ?? defaultSessionEventsPath(args.repoRoot);
  const inputs = await loadCalibrationInputs(args, sessionEventsPath);
  const resolvedRepoLabel = resolveRepoLabel(args, {
    defectReport: inputs.defectReport,
    reviewVerdicts: inputs.reviewVerdicts,
    sessionVerdicts: inputs.sessionVerdicts,
    remediationReport: inputs.remediationReport,
    benchmark: inputs.benchmark,
    sessionTelemetry: inputs.sessionTelemetry,
  });
  const { latestCalibration, codexBatch, replayBatch } = await loadCalibrationBatches(
    args,
    resolvedRepoLabel,
  );
  const cohortContext = await loadSignalCohortContext({
    cohortManifestPath: args.cohortManifestPath,
    cohortId: args.cohortId,
    fallbackCohortId: latestCalibration?.cohortId ?? null,
    codexBatch,
    replayBatch,
  });

  const scorecard = buildSignalScorecard({
    repoLabel: resolvedRepoLabel,
    defectReport: inputs.defectReport,
    reviewVerdicts: inputs.reviewVerdicts,
    remediationReport: inputs.remediationReport,
    benchmark: inputs.benchmark,
    sessionTelemetry: inputs.sessionTelemetry,
    codexBatch,
    replayBatch,
    sessionVerdicts: inputs.sessionVerdicts,
    cohortManifest: cohortContext.cohortManifest,
    cohortId: cohortContext.cohortId,
  });
  const sessionCorpus = buildSessionCorpus({
    repoLabel: resolvedRepoLabel,
    repoRoot: args.repoRoot,
    sessionTelemetry: inputs.sessionTelemetry,
    codexBatch,
    replayBatch,
    sessionVerdicts: inputs.sessionVerdicts,
  });

  await writeCoreCalibrationArtifacts(
    args.outputDir,
    resolvedRepoLabel,
    inputs.sessionTelemetry,
    scorecard,
    sessionCorpus,
  );
  if (cohortContext.cohortId) {
    const cohort = getSignalCohort(
      cohortContext.cohortManifest,
      cohortContext.cohortId,
    );
    const backlog = buildSignalBacklog({
      cohort,
      scorecard,
      codexBatch,
      replayBatch,
    });
    await writeBacklogArtifacts(args.outputDir, resolvedRepoLabel, backlog);
    await writeEvidenceReviewArtifacts(
      args.outputDir,
      resolvedRepoLabel,
      scorecard,
      backlog,
      sessionCorpus,
    );
  }

  console.log(
    `Wrote calibration artifacts for ${resolvedRepoLabel}: ${inputs.sessionTelemetry.summary.session_count} session(s), ${scorecard.summary.total_signals} signal(s).`,
  );
}

async function loadCalibrationInputs(args, sessionEventsPath) {
  const sessionTelemetry = await loadSessionTelemetrySummary(sessionEventsPath, {
    repoRoot: args.repoRoot,
  });

  return {
    sessionTelemetry,
    defectReport: await readJson(args.defectReportPath),
    reviewVerdicts: await readJson(args.reviewVerdictsPath),
    sessionVerdicts: await readJson(args.sessionVerdictsPath),
    remediationReport: await readJson(args.remediationReportPath),
    benchmark: await readJson(args.benchmarkPath),
  };
}

async function loadCalibrationBatches(args, repoLabel) {
  const latestCalibration =
    (!args.codexBatchPath || !args.replayBatchPath) && repoLabel
      ? await resolveLatestRepoCalibrationArtifacts({
          repoRootPath: args.repoRoot,
          repoLabel,
          latestCalibrationPath: args.latestCalibrationPath,
        })
      : null;
  const codexBatchPath =
    args.codexBatchPath ?? latestCalibration?.artifacts?.codex_batch_json ?? null;
  const replayBatchPath =
    args.replayBatchPath ?? latestCalibration?.artifacts?.replay_batch_json ?? null;

  return {
    latestCalibration,
    codexBatch: await readJson(codexBatchPath),
    replayBatch: await readJson(replayBatchPath),
  };
}

async function writeCoreCalibrationArtifacts(
  outputDir,
  repoLabel,
  sessionTelemetry,
  scorecard,
  sessionCorpus,
) {
  await writeArtifactWithStableCompanion(
    outputDir,
    repoLabel,
    'session-telemetry-summary.json',
    `${JSON.stringify(sessionTelemetry, null, 2)}\n`,
  );
  await writeArtifactWithStableCompanion(
    outputDir,
    repoLabel,
    'session-telemetry-summary.md',
    formatSessionTelemetrySummaryMarkdown(sessionTelemetry),
  );
  await writeArtifactWithStableCompanion(
    outputDir,
    repoLabel,
    'signal-scorecard.json',
    `${JSON.stringify(scorecard, null, 2)}\n`,
  );
  await writeArtifactWithStableCompanion(
    outputDir,
    repoLabel,
    'signal-scorecard.md',
    formatSignalScorecardMarkdown(scorecard),
  );
  await writeArtifactWithStableCompanion(
    outputDir,
    repoLabel,
    'session-corpus.json',
    `${JSON.stringify(sessionCorpus, null, 2)}\n`,
  );
  await writeArtifactWithStableCompanion(
    outputDir,
    repoLabel,
    'session-corpus.md',
    formatSessionCorpusMarkdown(sessionCorpus),
  );
}

async function writeBacklogArtifacts(outputDir, repoLabel, backlog) {
  await writeArtifactWithStableCompanion(
    outputDir,
    repoLabel,
    'signal-backlog.json',
    `${JSON.stringify(backlog, null, 2)}\n`,
  );
  await writeArtifactWithStableCompanion(
    outputDir,
    repoLabel,
    'signal-backlog.md',
    formatSignalBacklogMarkdown(backlog),
  );
}

async function writeEvidenceReviewArtifacts(
  outputDir,
  repoLabel,
  scorecard,
  backlog,
  sessionCorpus,
) {
  const evidenceReview = buildEvidenceReview({
    scorecard,
    backlog,
    sessionCorpus,
  });
  await writeArtifactWithStableCompanion(
    outputDir,
    repoLabel,
    'evidence-review.json',
    `${JSON.stringify(evidenceReview, null, 2)}\n`,
  );
  await writeArtifactWithStableCompanion(
    outputDir,
    repoLabel,
    'evidence-review.md',
    formatEvidenceReviewMarkdown(evidenceReview),
  );
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
});
