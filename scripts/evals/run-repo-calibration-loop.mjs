#!/usr/bin/env node

import { access } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { execFile as execFileCallback } from 'node:child_process';
import { promisify } from 'node:util';
import {
  defaultBatchOutputDir,
  readJson,
  writeJson,
  writeText,
} from '../lib/eval-batch.mjs';
import {
  formatSessionTelemetrySummaryMarkdown,
  mergeSessionTelemetrySummaries,
} from '../lib/session-telemetry.mjs';

const execFile = promisify(execFileCallback);

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, '../..');

function parseArgs(argv) {
  const result = {
    manifestPath: null,
    outputDir: null,
    skipLive: false,
    skipReplay: false,
    skipReview: false,
    skipScorecard: false,
    skipBacklog: false,
  };

  for (let index = 2; index < argv.length; index += 1) {
    const value = argv[index];
    if (value === '--manifest') {
      index += 1;
      result.manifestPath = argv[index];
      continue;
    }
    if (value === '--output-dir') {
      index += 1;
      result.outputDir = argv[index];
      continue;
    }
    if (value === '--skip-live') {
      result.skipLive = true;
      continue;
    }
    if (value === '--skip-replay') {
      result.skipReplay = true;
      continue;
    }
    if (value === '--skip-review') {
      result.skipReview = true;
      continue;
    }
    if (value === '--skip-scorecard') {
      result.skipScorecard = true;
      continue;
    }
    if (value === '--skip-backlog') {
      result.skipBacklog = true;
      continue;
    }
    throw new Error(`Unknown argument: ${value}`);
  }

  if (!result.manifestPath) {
    throw new Error('Missing required --manifest path');
  }

  return result;
}

function nowIso() {
  return new Date().toISOString();
}

async function pathExists(targetPath) {
  if (!targetPath) {
    return false;
  }

  try {
    await access(targetPath);
    return true;
  } catch {
    return false;
  }
}

async function loadRepoCalibrationManifest(manifestPath) {
  const manifest = await readJson(manifestPath);
  if (manifest?.schema_version !== 1) {
    throw new Error(`Unsupported repo calibration manifest: ${manifestPath}`);
  }

  return manifest;
}

function resolveManifestPath(manifestDir, relativePath) {
  if (!relativePath) {
    return null;
  }

  return path.resolve(manifestDir, relativePath);
}

function resolveRepoArtifactPath(repoRootPath, relativePath) {
  if (!relativePath) {
    return null;
  }

  return path.resolve(repoRootPath, relativePath);
}

function isVerdictTemplatePath(targetPath) {
  return Boolean(targetPath) && path.basename(targetPath).includes('.template.');
}

function deriveCompanionPath(targetPath, extension) {
  if (!targetPath) {
    return null;
  }

  const parsed = path.parse(targetPath);
  if (parsed.ext === extension) {
    return targetPath;
  }

  return path.join(parsed.dir, `${parsed.name}${extension}`);
}

function buildBatchRunArgs(manifestPath, outputDir, cohortManifestPath, cohortId) {
  const args = ['--manifest', manifestPath, '--output-dir', outputDir];

  if (cohortManifestPath) {
    args.push('--cohort-manifest', cohortManifestPath);
  }
  if (cohortId) {
    args.push('--cohort-id', cohortId);
  }

  return args;
}

function buildNodeArgs(scriptPath, args) {
  return [scriptPath, ...args];
}

async function runNodeScript(scriptPath, args) {
  const { stdout, stderr } = await execFile(process.execPath, buildNodeArgs(scriptPath, args), {
    cwd: repoRoot,
    maxBuffer: 1024 * 1024 * 20,
  });

  return {
    script: path.relative(repoRoot, scriptPath),
    args,
    stdout: stdout.trim(),
    stderr: stderr.trim(),
  };
}

function buildSummaryMarkdown(summary) {
  const lines = [];
  lines.push('# Repo Calibration Loop');
  lines.push('');
  lines.push(`- repo id: \`${summary.repo_id}\``);
  lines.push(`- repo root: \`${summary.repo_root}\``);
  lines.push(`- generated at: \`${summary.generated_at}\``);
  lines.push(`- output dir: \`${summary.output_dir}\``);
  lines.push('');
  lines.push('## Artifacts');
  lines.push('');
  lines.push(`- live batch: ${summary.artifacts.codex_batch_json ? `\`${summary.artifacts.codex_batch_json}\`` : 'skipped'}`);
  lines.push(`- replay batch: ${summary.artifacts.replay_batch_json ? `\`${summary.artifacts.replay_batch_json}\`` : 'skipped'}`);
  lines.push(`- merged telemetry: ${summary.artifacts.session_telemetry_json ? `\`${summary.artifacts.session_telemetry_json}\`` : 'none'}`);
  lines.push(`- review packet: ${summary.artifacts.review_packet_json ? `\`${summary.artifacts.review_packet_json}\`` : 'skipped'}`);
  lines.push(`- scorecard: ${summary.artifacts.scorecard_json ? `\`${summary.artifacts.scorecard_json}\`` : 'skipped'}`);
  lines.push(`- backlog: ${summary.artifacts.backlog_json ? `\`${summary.artifacts.backlog_json}\`` : 'skipped'}`);
  lines.push('');
  lines.push('## Summary');
  lines.push('');
  lines.push(`- total sessions: ${summary.summary.session_count}`);
  lines.push(`- total signals: ${summary.summary.total_signals ?? 0}`);
  lines.push(`- weak signals: ${summary.summary.weak_signal_count ?? 0}`);
  lines.push(`- next signal: ${summary.summary.recommended_next_signal ?? 'none'}`);
  lines.push('');
  return `${lines.join('\n')}\n`;
}

async function selectReviewVerdictsPath(outputPath, inputPath) {
  if (outputPath && (await pathExists(outputPath))) {
    return outputPath;
  }
  if (inputPath && !isVerdictTemplatePath(inputPath) && (await pathExists(inputPath))) {
    return inputPath;
  }

  return null;
}

function buildReviewArgs(manifest, reviewPacketJsonPath, reviewPacketMarkdownPath, codexBatchPath, replayBatchPath) {
  const args = [
    '--tool',
    manifest.review_tool ?? 'check',
    '--output-json',
    reviewPacketJsonPath,
    '--output-md',
    reviewPacketMarkdownPath,
    '--limit',
    String(manifest.review_limit ?? 12),
  ];

  if (codexBatchPath && manifest.review_source !== 'replay') {
    args.push('--codex-batch', codexBatchPath);
  } else if (replayBatchPath) {
    args.push('--replay-batch', replayBatchPath);
  }

  return args;
}

async function main() {
  const args = parseArgs(process.argv);
  const manifestPath = path.resolve(args.manifestPath);
  const manifestDir = path.dirname(manifestPath);
  const manifest = await loadRepoCalibrationManifest(manifestPath);
  const repoRootPath = path.resolve(manifest.repo_root);
  const outputDir = path.resolve(
    args.outputDir ??
      defaultBatchOutputDir(repoRootPath, 'repo-calibration-loop', manifest.repo_id ?? path.basename(repoRootPath)),
  );

  const cohortManifestPath = resolveManifestPath(manifestDir, manifest.cohort_manifest);
  const codexBatchManifestPath = resolveManifestPath(
    manifestDir,
    manifest.live_batch_manifest ?? manifest.codex_batch_manifest,
  );
  const replayBatchManifestPath = resolveManifestPath(
    manifestDir,
    manifest.replay_batch_manifest,
  );
  const artifactConfig = manifest.artifacts ?? {};
  const reviewVerdictsOutputPath = resolveRepoArtifactPath(
    repoRootPath,
    artifactConfig.review_verdicts_output,
  );
  const reviewVerdictsPath = resolveManifestPath(
    manifestDir,
    artifactConfig.review_verdicts_input ?? manifest.review_verdicts,
  );
  const defectReportPath = resolveManifestPath(
    manifestDir,
    artifactConfig.seeded_defect_report ?? manifest.defect_report,
  );
  const remediationReportPath = resolveManifestPath(
    manifestDir,
    artifactConfig.remediation_report ?? manifest.remediation_report,
  );
  const benchmarkPath = resolveManifestPath(
    manifestDir,
    artifactConfig.benchmark_artifact ?? manifest.benchmark_artifact,
  );

  const codexBatchOutputDir = path.join(outputDir, 'codex-batch');
  const replayBatchOutputDir = path.join(outputDir, 'replay-batch');
  const configuredReviewPacketPath = resolveRepoArtifactPath(
    repoRootPath,
    artifactConfig.review_packet_output,
  );
  const reviewPacketJsonPath =
    deriveCompanionPath(configuredReviewPacketPath, '.json') ??
    path.join(outputDir, 'check-review-packet.json');
  const reviewPacketMarkdownPath =
    deriveCompanionPath(configuredReviewPacketPath, '.md') ??
    path.join(outputDir, 'check-review-packet.md');
  const mergedTelemetryJsonPath = path.join(outputDir, 'session-telemetry-summary.json');
  const mergedTelemetryMarkdownPath = path.join(outputDir, 'session-telemetry-summary.md');
  const configuredScorecardPath = resolveRepoArtifactPath(
    repoRootPath,
    artifactConfig.scorecard_output,
  );
  const configuredBacklogPath = resolveRepoArtifactPath(
    repoRootPath,
    artifactConfig.backlog_output,
  );
  const scorecardJsonPath =
    deriveCompanionPath(configuredScorecardPath, '.json') ??
    path.join(outputDir, 'signal-scorecard.json');
  const scorecardMarkdownPath =
    deriveCompanionPath(configuredScorecardPath, '.md') ??
    path.join(outputDir, 'signal-scorecard.md');
  const backlogJsonPath =
    deriveCompanionPath(configuredBacklogPath, '.json') ??
    path.join(outputDir, 'signal-backlog.json');
  const backlogMarkdownPath =
    deriveCompanionPath(configuredBacklogPath, '.md') ??
    path.join(outputDir, 'signal-backlog.md');

  const runs = [];
  let codexBatchResult = null;
  let replayBatchResult = null;
  let selectedReviewVerdictsPath = null;

  if (!args.skipLive && codexBatchManifestPath) {
    const liveArgs = buildBatchRunArgs(
      codexBatchManifestPath,
      codexBatchOutputDir,
      cohortManifestPath,
      manifest.cohort_id,
    );
    runs.push(await runNodeScript(path.join(repoRoot, 'scripts/evals/run-codex-session-batch.mjs'), liveArgs));
    codexBatchResult = await readJson(path.join(codexBatchOutputDir, 'codex-session-batch.json'));
  }

  if (!args.skipReplay && replayBatchManifestPath) {
    const replayArgs = buildBatchRunArgs(
      replayBatchManifestPath,
      replayBatchOutputDir,
      cohortManifestPath,
      manifest.cohort_id,
    );
    runs.push(await runNodeScript(path.join(repoRoot, 'scripts/evals/run-diff-replay-batch.mjs'), replayArgs));
    replayBatchResult = await readJson(path.join(replayBatchOutputDir, 'diff-replay-batch.json'));
  }

  const telemetrySummaries = [
    codexBatchResult?.telemetry_summary ?? null,
    replayBatchResult?.telemetry_summary ?? null,
  ].filter(Boolean);
  const mergedTelemetry = mergeSessionTelemetrySummaries(telemetrySummaries, {
    repoRoot: repoRootPath,
    sourcePaths: telemetrySummaries.flatMap((summary) => summary.source_paths ?? []),
  });

  await writeJson(mergedTelemetryJsonPath, mergedTelemetry);
  await writeText(
    mergedTelemetryMarkdownPath,
    formatSessionTelemetrySummaryMarkdown(mergedTelemetry),
  );

  if (!args.skipReview && (codexBatchResult || replayBatchResult)) {
    const reviewArgs = buildReviewArgs(
      manifest,
      reviewPacketJsonPath,
      reviewPacketMarkdownPath,
      codexBatchResult ? path.join(codexBatchOutputDir, 'codex-session-batch.json') : null,
      replayBatchResult ? path.join(replayBatchOutputDir, 'diff-replay-batch.json') : null,
    );

    runs.push(await runNodeScript(path.join(repoRoot, 'scripts/evals/build-check-review-packet.mjs'), reviewArgs));
  }

  if (!args.skipScorecard) {
    selectedReviewVerdictsPath = await selectReviewVerdictsPath(
      reviewVerdictsOutputPath,
      reviewVerdictsPath,
    );

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

    if (defectReportPath && (await pathExists(defectReportPath))) {
      scorecardArgs.push('--defect-report', defectReportPath);
    }
    if (selectedReviewVerdictsPath) {
      scorecardArgs.push('--review-verdicts', selectedReviewVerdictsPath);
    }
    if (remediationReportPath && (await pathExists(remediationReportPath))) {
      scorecardArgs.push('--remediation-report', remediationReportPath);
    }
    if (benchmarkPath && (await pathExists(benchmarkPath))) {
      scorecardArgs.push('--benchmark', benchmarkPath);
    }

    runs.push(await runNodeScript(path.join(repoRoot, 'scripts/evals/build-signal-scorecard.mjs'), scorecardArgs));
  }

  if (!args.skipBacklog) {
    if (!(await pathExists(scorecardJsonPath))) {
      throw new Error(
        `Cannot build backlog without a scorecard artifact: ${scorecardJsonPath}`,
      );
    }
    const backlogArgs = [
      '--scorecard',
      scorecardJsonPath,
      '--output-json',
      backlogJsonPath,
      '--output-md',
      backlogMarkdownPath,
    ];

    if (cohortManifestPath) {
      backlogArgs.push('--cohort-manifest', cohortManifestPath);
    }
    if (manifest.cohort_id) {
      backlogArgs.push('--cohort-id', manifest.cohort_id);
    }
    if (codexBatchResult) {
      backlogArgs.push('--codex-batch', path.join(codexBatchOutputDir, 'codex-session-batch.json'));
    }
    if (replayBatchResult) {
      backlogArgs.push('--replay-batch', path.join(replayBatchOutputDir, 'diff-replay-batch.json'));
    }

    runs.push(await runNodeScript(path.join(repoRoot, 'scripts/evals/build-signal-backlog.mjs'), backlogArgs));
  }

  const scorecard = (await pathExists(scorecardJsonPath)) ? await readJson(scorecardJsonPath) : null;
  const backlog = (await pathExists(backlogJsonPath)) ? await readJson(backlogJsonPath) : null;
  const summary = {
    schema_version: 1,
    generated_at: nowIso(),
    repo_id: manifest.repo_id ?? path.basename(repoRootPath),
    repo_label: manifest.repo_label ?? manifest.repo_id ?? path.basename(repoRootPath),
    repo_root: repoRootPath,
    output_dir: outputDir,
    cohort_id: manifest.cohort_id ?? codexBatchResult?.cohort_id ?? replayBatchResult?.cohort_id ?? null,
    artifacts: {
      codex_batch_json: codexBatchResult ? path.join(codexBatchOutputDir, 'codex-session-batch.json') : null,
      replay_batch_json: replayBatchResult ? path.join(replayBatchOutputDir, 'diff-replay-batch.json') : null,
      session_telemetry_json: mergedTelemetryJsonPath,
      review_packet_json: (await pathExists(reviewPacketJsonPath)) ? reviewPacketJsonPath : null,
      review_verdicts_input: selectedReviewVerdictsPath,
      review_verdicts_output: reviewVerdictsOutputPath,
      scorecard_json: scorecard ? scorecardJsonPath : null,
      backlog_json: backlog ? backlogJsonPath : null,
    },
    summary: {
      session_count: mergedTelemetry.summary.session_count ?? 0,
      total_signals: scorecard?.summary?.total_signals ?? 0,
      weak_signal_count: backlog?.summary?.weak_signal_count ?? 0,
      recommended_next_signal: backlog?.summary?.recommended_next_signal ?? null,
    },
    runs,
  };

  await writeJson(path.join(outputDir, 'repo-calibration-loop.json'), summary);
  await writeText(path.join(outputDir, 'repo-calibration-loop.md'), buildSummaryMarkdown(summary));

  console.log(
    `Completed repo calibration loop for ${summary.repo_label}. Artifacts written to ${outputDir}`,
  );
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
});
