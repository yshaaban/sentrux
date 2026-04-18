import path from 'node:path';

import { pathExists, readJson } from '../repo-calibration-artifacts.mjs';
import { runNodeScript } from './runtime.mjs';

function isVerdictTemplatePath(targetPath) {
  return Boolean(targetPath) && path.basename(targetPath).includes('.template.');
}

export async function selectReviewVerdictsPath(outputPath, inputPath) {
  if (inputPath && !isVerdictTemplatePath(inputPath) && (await pathExists(inputPath))) {
    return inputPath;
  }
  if (outputPath && (await pathExists(outputPath))) {
    return outputPath;
  }

  return null;
}

export async function selectSessionVerdictsPath(outputPath, inputPath) {
  if (inputPath && (await pathExists(inputPath))) {
    return inputPath;
  }
  if (outputPath && (await pathExists(outputPath))) {
    return outputPath;
  }

  return null;
}

export function countReviewSamples(reviewPacket) {
  return reviewPacket?.summary?.sample_count ?? reviewPacket?.samples?.length ?? 0;
}

export async function maybeBuildProvisionalReviewVerdicts({
  repoRoot,
  manifest,
  reviewPacketJsonPath,
  reviewPacketMarkdownPath,
  reviewVerdictsOutputPath,
  reviewVerdictsPath,
}) {
  const hasCuratedReviewVerdicts =
    reviewVerdictsPath &&
    !isVerdictTemplatePath(reviewVerdictsPath) &&
    (await pathExists(reviewVerdictsPath));
  if (hasCuratedReviewVerdicts || !reviewVerdictsOutputPath) {
    return false;
  }

  const reviewPacket = await readExistingJson(reviewPacketJsonPath);
  if (countReviewSamples(reviewPacket) === 0) {
    return null;
  }

  return runNodeScript(
    repoRoot,
    path.join(repoRoot, 'scripts/evals/build-provisional-review-verdicts.mjs'),
    [
      '--packet',
      reviewPacketJsonPath,
      '--output-json',
      reviewVerdictsOutputPath,
      '--source-report',
      reviewPacketMarkdownPath,
      '--repo',
      manifest.repo_label ?? manifest.repo_id ?? path.basename(manifest.repo_root),
    ],
  );
}

export function buildReviewArgs(
  manifest,
  reviewPacketJsonPath,
  reviewPacketMarkdownPath,
  codexBatchPath,
  replayBatchPath,
) {
  const args = [
    '--repo-root',
    manifest.repo_root,
    '--tool',
    manifest.review_tool ?? 'check',
    '--output-json',
    reviewPacketJsonPath,
    '--output-md',
    reviewPacketMarkdownPath,
    '--limit',
    String(manifest.review_limit ?? 12),
  ];

  if (manifest.review_source === 'codex') {
    if (!codexBatchPath) {
      throw new Error('review_source "codex" requires a live batch artifact');
    }
    args.push('--codex-batch', codexBatchPath);
    return args;
  }

  if (manifest.review_source === 'replay') {
    if (!replayBatchPath) {
      throw new Error('review_source "replay" requires a replay batch artifact');
    }
    args.push('--replay-batch', replayBatchPath);
    return args;
  }

  if (codexBatchPath) {
    args.push('--codex-batch', codexBatchPath);
  }
  if (replayBatchPath) {
    args.push('--replay-batch', replayBatchPath);
  }

  return args;
}

async function pushExistingPathArg(args, flag, targetPath) {
  if (targetPath && (await pathExists(targetPath))) {
    args.push(flag, targetPath);
  }
}

export async function buildScorecardArgs({
  manifest,
  repoRootPath,
  mergedTelemetryJsonPath,
  scorecardJsonPath,
  scorecardMarkdownPath,
  codexBatchPath = null,
  replayBatchPath = null,
  defectReportPath = null,
  selectedReviewVerdictsPath = null,
  selectedSessionVerdictsPath = null,
  remediationReportPath = null,
  benchmarkPath = null,
}) {
  const scorecardArgs = [
    '--repo-label',
    manifest.repo_label ?? manifest.repo_id ?? path.basename(repoRootPath),
    '--session-telemetry',
    mergedTelemetryJsonPath,
    '--output-json',
    scorecardJsonPath,
    '--output-md',
    scorecardMarkdownPath,
  ];

  const optionalArtifactArgs = [
    ['--codex-batch', codexBatchPath],
    ['--replay-batch', replayBatchPath],
    ['--defect-report', defectReportPath],
    ['--remediation-report', remediationReportPath],
    ['--benchmark', benchmarkPath],
  ];

  for (const [flag, targetPath] of optionalArtifactArgs) {
    await pushExistingPathArg(scorecardArgs, flag, targetPath);
  }
  if (selectedReviewVerdictsPath) {
    scorecardArgs.push('--review-verdicts', selectedReviewVerdictsPath);
  }
  if (selectedSessionVerdictsPath) {
    scorecardArgs.push('--session-verdicts', selectedSessionVerdictsPath);
  }

  return scorecardArgs;
}

export async function readExistingJson(targetPath) {
  if (!(await pathExists(targetPath))) {
    return null;
  }

  return readJson(targetPath);
}

export async function existingPathOrNull(targetPath) {
  if (targetPath && (await pathExists(targetPath))) {
    return targetPath;
  }

  return null;
}
