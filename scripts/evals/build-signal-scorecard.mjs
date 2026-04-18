#!/usr/bin/env node

import path from 'node:path';
import {
  buildSignalScorecard,
  formatSignalScorecardMarkdown,
} from '../lib/signal-scorecard.mjs';
import { resolveLatestRepoCalibrationArtifacts } from '../lib/repo-calibration-artifacts.mjs';
import {
  readJsonFile,
  repoRootFromImportMeta,
  writeMaybe,
} from './build-artifact-support.mjs';

const repoRoot = repoRootFromImportMeta(import.meta.url);

function parseArgs(argv) {
  const result = {
    repoRootPath: null,
    repoLabel: null,
    defectReportPath: null,
    reviewVerdictsPath: null,
    sessionVerdictsPath: null,
    remediationReportPath: null,
    benchmarkPath: null,
    sessionTelemetryPath: null,
    codexBatchPath: null,
    replayBatchPath: null,
    latestCalibrationPath: null,
    outputJsonPath: null,
    outputMarkdownPath: null,
  };

  for (let index = 2; index < argv.length; index += 1) {
    const value = argv[index];
    if (value === '--repo-root') {
      index += 1;
      result.repoRootPath = argv[index];
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
    if (value === '--session-telemetry') {
      index += 1;
      result.sessionTelemetryPath = argv[index];
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
    if (value === '--output-json') {
      index += 1;
      result.outputJsonPath = argv[index];
      continue;
    }
    if (value === '--output-md') {
      index += 1;
      result.outputMarkdownPath = argv[index];
      continue;
    }
    throw new Error(`Unknown argument: ${value}`);
  }

  if (!hasAnyScorecardInput(result)) {
    throw new Error(
      'Provide at least one scorecard input: --defect-report, --review-verdicts, --session-verdicts, --remediation-report, --benchmark, --session-telemetry, --codex-batch, or --replay-batch',
    );
  }

  return result;
}

function hasAnyScorecardInput(args) {
  return Boolean(
    args.defectReportPath ||
      args.reviewVerdictsPath ||
      args.sessionVerdictsPath ||
      args.remediationReportPath ||
      args.benchmarkPath ||
      args.sessionTelemetryPath ||
      args.codexBatchPath ||
      args.replayBatchPath,
  );
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
    (args.repoRootPath ? path.basename(args.repoRootPath) : null) ??
    null
  );
}

async function main() {
  const args = parseArgs(process.argv);
  const defectReport = args.defectReportPath ? await readJsonFile(args.defectReportPath) : null;
  const reviewVerdicts = args.reviewVerdictsPath
    ? await readJsonFile(args.reviewVerdictsPath)
    : null;
  const sessionVerdicts = args.sessionVerdictsPath
    ? await readJsonFile(args.sessionVerdictsPath)
    : null;
  const remediationReport = args.remediationReportPath
    ? await readJsonFile(args.remediationReportPath)
    : null;
  const benchmark = args.benchmarkPath ? await readJsonFile(args.benchmarkPath) : null;
  const sessionTelemetry = args.sessionTelemetryPath
    ? await readJsonFile(args.sessionTelemetryPath)
    : null;
  const resolvedRepoLabel = resolveRepoLabel(args, {
    defectReport,
    reviewVerdicts,
    sessionVerdicts,
    remediationReport,
    benchmark,
    sessionTelemetry,
  });
  const latestCalibration =
    (!args.codexBatchPath || !args.replayBatchPath) && resolvedRepoLabel
      ? await resolveLatestRepoCalibrationArtifacts({
          repoRootPath: args.repoRootPath,
          repoLabel: resolvedRepoLabel,
          latestCalibrationPath: args.latestCalibrationPath,
        })
      : null;
  const codexBatchPath = args.codexBatchPath ?? latestCalibration?.artifacts?.codex_batch_json ?? null;
  const replayBatchPath =
    args.replayBatchPath ?? latestCalibration?.artifacts?.replay_batch_json ?? null;
  const codexBatch = codexBatchPath ? await readJsonFile(codexBatchPath) : null;
  const replayBatch = replayBatchPath ? await readJsonFile(replayBatchPath) : null;

  const scorecard = buildSignalScorecard({
    repoLabel: resolvedRepoLabel,
    defectReport,
    reviewVerdicts,
    sessionVerdicts,
    remediationReport,
    benchmark,
    sessionTelemetry,
    codexBatch,
    replayBatch,
  });
  const markdown = formatSignalScorecardMarkdown(scorecard);

  await writeMaybe(
    args.outputJsonPath ??
      path.join(repoRoot, 'docs/v2/examples', 'signal-scorecard.json'),
    `${JSON.stringify(scorecard, null, 2)}\n`,
  );
  await writeMaybe(
    args.outputMarkdownPath ??
      path.join(repoRoot, 'docs/v2/examples', 'signal-scorecard.md'),
    markdown,
  );

  console.log(
    `Built signal scorecard for ${scorecard.repo_label ?? 'unknown'} with ${scorecard.summary.total_signals} signal(s).`,
  );
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
});
