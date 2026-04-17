#!/usr/bin/env node

import { mkdir, mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

import { createMcpSession, runTool } from '../lib/benchmark-harness.mjs';
import { prepareTypeScriptBenchmarkHome } from '../lib/benchmark-plugin-home.mjs';
import { defaultBatchOutputDir } from '../lib/eval-batch.mjs';
import { collectRepoMetadata, readJson, runNodeScript } from '../lib/eval-runtime/common.mjs';
import {
  buildExternalValidationPaths,
  buildReviewPackets,
  maybeBuildSessionTelemetrySummary,
  writeExternalValidationArtifacts,
} from '../lib/external-validation/artifacts.mjs';
import {
  buildEngineeringReport,
  buildPacketValidation,
  buildValidationReport,
} from '../lib/external-validation/report.mjs';
import {
  buildRawToolSummary,
  buildScanCoverageBreakdown,
  formatScanCoverageBreakdownMarkdown,
} from '../lib/external-validation/scan-coverage.mjs';
import { repoRootFromImportMeta } from '../lib/script-artifacts.mjs';

const repoRoot = repoRootFromImportMeta(import.meta.url, 2);
const sentruxBin = process.env.SENTRUX_BIN ?? path.join(repoRoot, 'target/debug/sentrux');

export {
  buildEngineeringReport,
  buildPacketValidation,
  buildRawToolSummary,
  buildScanCoverageBreakdown,
  buildValidationReport,
  formatScanCoverageBreakdownMarkdown,
};

export function parseArgs(argv) {
  const result = {
    repoRoot: null,
    repoLabel: null,
    outputDir: null,
    findingsLimit: 25,
    deadPrivateLimit: 10,
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
    if (value === '--output-dir') {
      index += 1;
      result.outputDir = argv[index];
      continue;
    }
    if (value === '--findings-limit') {
      index += 1;
      result.findingsLimit = Number(argv[index]);
      continue;
    }
    if (value === '--dead-private-limit') {
      index += 1;
      result.deadPrivateLimit = Number(argv[index]);
      continue;
    }
    throw new Error(`Unknown argument: ${value}`);
  }

  if (!result.repoRoot) {
    throw new Error('Missing required --repo-root');
  }

  if (!result.repoLabel) {
    result.repoLabel = path.basename(result.repoRoot);
  }

  return result;
}

async function captureRawToolAnalysis(repoRootPath, findingsLimit) {
  const tempRoot = await mkdtemp(path.join(tmpdir(), 'sentrux-external-repo-validation-'));
  const pluginHome = await prepareTypeScriptBenchmarkHome({ tempRoot });
  const session = createMcpSession({
    binPath: sentruxBin,
    repoRoot: repoRootPath,
    homeOverride: pluginHome,
    skipGrammarDownload: process.env.SENTRUX_SKIP_GRAMMAR_DOWNLOAD ?? '1',
    requestTimeoutMs: Number(process.env.REQUEST_TIMEOUT_MS ?? '180000'),
  });

  try {
    const analysis = {};
    analysis.scan = (await runTool(session, 'scan', { path: repoRootPath })).payload;
    analysis.check = (await runTool(session, 'check', {})).payload;
    analysis.gate = (await runTool(session, 'gate', {})).payload;
    analysis.findings = (await runTool(session, 'findings', { limit: findingsLimit })).payload;
    await runTool(session, 'session_start', {});
    analysis.session_end = (await runTool(session, 'session_end', {})).payload;

    return analysis;
  } finally {
    await session.close();
    await rm(tempRoot, { recursive: true, force: true });
  }
}

async function main() {
  const args = parseArgs(process.argv);
  const repoRootPath = path.resolve(args.repoRoot);
  const outputDir = path.resolve(
    args.outputDir ??
      defaultBatchOutputDir(repoRootPath, 'external-repo-validation', args.repoLabel),
  );
  const metadata = await collectRepoMetadata(repoRootPath);
  const artifactPaths = buildExternalValidationPaths(outputDir, args.repoLabel);

  await mkdir(outputDir, { recursive: true });

  const reviewPackets = await buildReviewPackets(args, repoRootPath, outputDir);
  await runNodeScript(path.join(repoRoot, 'scripts/evals/review_dead_private.mjs'), [
    '--repo-root',
    repoRootPath,
    '--repo-name',
    args.repoLabel,
    '--limit',
    String(args.deadPrivateLimit),
    '--findings-limit',
    String(Math.max(args.findingsLimit, args.deadPrivateLimit)),
    '--dry-run',
    '--output',
    artifactPaths.deadPrivatePath,
  ], { cwd: repoRoot });

  const findingsReviewPacket = await readJson(reviewPackets.findingsJsonPath);
  const packetValidation = buildPacketValidation(findingsReviewPacket);
  const rawToolAnalysis = await captureRawToolAnalysis(repoRootPath, Math.max(args.findingsLimit, 50));
  const rawToolSummary = buildRawToolSummary(rawToolAnalysis);
  const scanCoverageBreakdown = buildScanCoverageBreakdown(rawToolAnalysis);
  const engineeringReport = buildEngineeringReport({
    repoRootPath,
    repoLabel: args.repoLabel,
    branch: metadata.branch,
    commit: metadata.commit,
    rawToolAnalysis,
  });
  const validationReport = buildValidationReport({
    repoRootPath,
    repoLabel: args.repoLabel,
    branch: metadata.branch,
    commit: metadata.commit,
    workingTreeClean: metadata.workingTreeClean,
    rawToolAnalysis,
    rawToolSummary,
    packetValidation,
    scanCoverageBreakdown,
  });

  await maybeBuildSessionTelemetrySummary(repoRootPath, outputDir);
  await writeExternalValidationArtifacts({
    rawToolAnalysis,
    rawToolSummary,
    scanCoverageBreakdown,
    validationReport,
    engineeringReport,
    ...artifactPaths,
  });

  console.log(JSON.stringify({
    output_dir: outputDir,
    report_path: artifactPaths.reportPath,
    engineering_report_path: artifactPaths.engineeringReportPath,
    repo_engineering_report_path: artifactPaths.repoEngineeringReportPath,
    scan_coverage_breakdown_json_path: artifactPaths.scanCoverageBreakdownJsonPath,
    scan_coverage_breakdown_markdown_path: artifactPaths.scanCoverageBreakdownMarkdownPath,
  }, null, 2));
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main().catch(function handleError(error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  });
}
