#!/usr/bin/env node

import { mkdir, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  buildSignalScorecard,
  formatSignalScorecardMarkdown,
} from '../lib/signal-scorecard.mjs';
import { resolveLatestRepoCalibrationArtifacts } from '../lib/repo-calibration-artifacts.mjs';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, '../..');

function parseArgs(argv) {
  const result = {
    repoRootPath: null,
    repoLabel: null,
    defectReportPath: null,
    reviewVerdictsPath: null,
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
      'Provide at least one scorecard input: --defect-report, --review-verdicts, --remediation-report, --benchmark, --session-telemetry, --codex-batch, or --replay-batch',
    );
  }

  return result;
}

function hasAnyScorecardInput(args) {
  return Boolean(
    args.defectReportPath ||
      args.reviewVerdictsPath ||
      args.remediationReportPath ||
      args.benchmarkPath ||
      args.sessionTelemetryPath ||
      args.codexBatchPath ||
      args.replayBatchPath,
  );
}

async function readJson(targetPath) {
  const source = await readFile(targetPath, 'utf8');
  return JSON.parse(source);
}

async function writeMaybe(targetPath, text) {
  if (!targetPath) {
    return;
  }
  await mkdir(path.dirname(targetPath), { recursive: true });
  await writeFile(targetPath, text, 'utf8');
}

function resolveRepoLabel(args, inputs = {}) {
  return (
    args.repoLabel ??
    inputs.defectReport?.repo_label ??
    inputs.reviewVerdicts?.repo ??
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
  const defectReport = args.defectReportPath ? await readJson(args.defectReportPath) : null;
  const reviewVerdicts = args.reviewVerdictsPath
    ? await readJson(args.reviewVerdictsPath)
    : null;
  const remediationReport = args.remediationReportPath
    ? await readJson(args.remediationReportPath)
    : null;
  const benchmark = args.benchmarkPath ? await readJson(args.benchmarkPath) : null;
  const sessionTelemetry = args.sessionTelemetryPath
    ? await readJson(args.sessionTelemetryPath)
    : null;
  const resolvedRepoLabel = resolveRepoLabel(args, {
    defectReport,
    reviewVerdicts,
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
  const codexBatch = codexBatchPath ? await readJson(codexBatchPath) : null;
  const replayBatch = replayBatchPath ? await readJson(replayBatchPath) : null;

  const scorecard = buildSignalScorecard({
    repoLabel: resolvedRepoLabel,
    defectReport,
    reviewVerdicts,
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
