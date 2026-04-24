#!/usr/bin/env node

import { existsSync } from 'node:fs';
import { mkdir } from 'node:fs/promises';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

import {
  buildPacketValidation,
  captureRawToolAnalysis,
} from './evals/run-external-repo-validation.mjs';
import { buildReviewPackets } from './lib/external-validation/artifacts.mjs';
import { collectRepoMetadata, readJson, runNodeScript } from './lib/eval-runtime/common.mjs';
import {
  buildAdvisorEvidence,
  buildAdvisorPaths,
  buildAdvisorSummaryMarkdown,
  buildBeforeAfterComparison,
  buildReportsForAdvisor,
  buildRulesBootstrap,
  canApplyGeneratedRulesToWorkspace,
  createRepoAdvisorWorkspace,
  loadPreviousAnalysis,
  markRulesAppliedInEvidence,
  maybeWriteGeneratedRules,
  parseRepoAdvisorArgs,
  writeAdvisorArtifacts,
} from './lib/repo-advisor.mjs';
import {
  buildRawToolSummary,
  buildScanCoverageBreakdown,
} from './lib/external-validation/scan-coverage.mjs';
import { repoRootFromImportMeta } from './lib/script-artifacts.mjs';

const repoRoot = repoRootFromImportMeta(import.meta.url, 1);

async function maybeRunDeadPrivateReview(args, workspace, paths) {
  await runNodeScript(path.join(repoRoot, 'scripts/evals/review_dead_private.mjs'), [
    '--repo-root',
    workspace.workRoot,
    '--repo-name',
    args.repoLabel,
    '--limit',
    String(args.deadPrivateLimit),
    '--findings-limit',
    String(Math.max(args.findingsLimit, args.deadPrivateLimit)),
    '--dry-run',
    '--output',
    paths.deadPrivatePath,
  ], { cwd: repoRoot });
}

async function collectPacketValidation(args, workspace, outputDir) {
  const reviewPackets = await buildReviewPackets(args, workspace.workRoot, outputDir);
  const findingsReviewPacket = await readJson(reviewPackets.findingsJsonPath);
  return buildPacketValidation(findingsReviewPacket);
}

async function captureAnalysisWithOptionalGeneratedRules(args, workspace) {
  const workspaceRulesPath = path.join(workspace.workRoot, '.sentrux', 'rules.toml');
  const configuredRulesPath =
    args.rulesSource ?? (existsSync(workspaceRulesPath) ? workspaceRulesPath : null);
  const canApplySuggestedRules = canApplyGeneratedRulesToWorkspace(
    args.applySuggestedRules,
    workspace,
  );
  const initialAnalysis = await captureRawToolAnalysis(
    workspace.workRoot,
    Math.max(args.findingsLimit, 50),
  );
  const rulesBootstrap = buildRulesBootstrap(initialAnalysis, { configuredRulesPath });
  const rulesApplication = await maybeWriteGeneratedRules(
    workspace.workRoot,
    rulesBootstrap,
    canApplySuggestedRules,
  );

  if (!rulesApplication.applied) {
    return {
      rawToolAnalysis: initialAnalysis,
      rulesBootstrap,
      rulesApplication,
    };
  }

  return {
    rawToolAnalysis: await captureRawToolAnalysis(
      workspace.workRoot,
      Math.max(args.findingsLimit, 50),
    ),
    rulesBootstrap,
    rulesApplication,
  };
}

async function runRepoAdvisor(args) {
  const workspace = await createRepoAdvisorWorkspace({
    repoRoot: args.repoRoot,
    repoLabel: args.repoLabel,
    analysisMode: args.analysisMode,
    rulesSource: args.rulesSource,
  });
  const paths = buildAdvisorPaths(args.outputDir, args.repoLabel);

  try {
    await mkdir(args.outputDir, { recursive: true });
    const metadata = await collectRepoMetadata(args.repoRoot);
    const { rawToolAnalysis, rulesBootstrap, rulesApplication } =
      await captureAnalysisWithOptionalGeneratedRules(args, workspace);
    const rawToolSummary = buildRawToolSummary(rawToolAnalysis);
    const scanCoverageBreakdown = buildScanCoverageBreakdown(rawToolAnalysis);
    const previousAnalysis = await loadPreviousAnalysis(args.previousAnalysisPath);
    const previousComparison = previousAnalysis
      ? buildBeforeAfterComparison(previousAnalysis, rawToolAnalysis)
      : null;

    await maybeRunDeadPrivateReview(args, workspace, paths);
    const packetValidation = await collectPacketValidation(args, workspace, args.outputDir);
    const { engineeringReport, validationReport } = buildReportsForAdvisor({
      repoRootPath: args.repoRoot,
      repoLabel: args.repoLabel,
      metadata,
      rawToolAnalysis,
      rawToolSummary,
      packetValidation,
      scanCoverageBreakdown,
    });
    const advisorEvidence = markRulesAppliedInEvidence(
      buildAdvisorEvidence({
        repoLabel: args.repoLabel,
        sourceRepoRoot: args.repoRoot,
        analyzedRepoRoot: workspace.workRoot,
        workspace,
        rawToolAnalysis,
        rulesBootstrap,
        previousComparison,
      }),
      rulesApplication,
    );
    const advisorSummary = buildAdvisorSummaryMarkdown({
      repoLabel: args.repoLabel,
      rawToolAnalysis,
      evidence: advisorEvidence,
      artifactPaths: paths,
    });

    await writeAdvisorArtifacts({
      paths,
      rawToolAnalysis,
      rawToolSummary,
      scanCoverageBreakdown,
      validationReport,
      engineeringReport,
      rulesBootstrap,
      advisorEvidence,
      advisorSummary,
      previousComparison,
    });

    return {
      output_dir: args.outputDir,
      analysis_mode: workspace.analysisMode,
      analyzed_workspace: args.keepWorkspace ? workspace.workRoot : null,
      advisor_summary_path: paths.advisorSummaryPath,
      engineering_report_path: paths.engineeringReportPath,
      validation_report_path: paths.reportPath,
      raw_tool_analysis_path: paths.rawToolAnalysisPath,
      advisor_evidence_path: paths.advisorEvidencePath,
      rules_bootstrap_path: paths.rulesBootstrapMarkdownPath,
      before_after_path: previousComparison ? paths.comparisonMarkdownPath : null,
    };
  } finally {
    if (args.keepWorkspace) {
      console.error(`Kept analysis workspace: ${workspace.workRoot}`);
    } else if (workspace.tempRoot) {
      await workspace.cleanup();
    }
  }
}

async function main() {
  const args = parseRepoAdvisorArgs(process.argv);
  const result = await runRepoAdvisor(args);
  console.log(JSON.stringify(result, null, 2));
}

export { runRepoAdvisor };

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main().catch(function handleError(error) {
    console.error(error instanceof Error ? error.stack ?? error.message : String(error));
    process.exitCode = 1;
  });
}
