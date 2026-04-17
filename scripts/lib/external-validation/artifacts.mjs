import { writeFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import {
  formatSessionTelemetrySummaryMarkdown,
  loadSessionTelemetrySummary,
} from '../session-telemetry.mjs';
import { pathExists, runNodeScript } from '../eval-runtime/common.mjs';
import {
  formatScanCoverageBreakdownMarkdown,
  sanitizeRepoArtifactLabel,
} from './scan-coverage.mjs';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, '../../..');

export function buildExternalValidationPaths(outputDir, repoLabel) {
  return {
    deadPrivatePath: path.join(outputDir, 'dead-private-dry-run.json'),
    rawToolAnalysisPath: path.join(outputDir, 'raw-tool-analysis.json'),
    rawToolSummaryPath: path.join(outputDir, 'raw-tool-summary.json'),
    scanCoverageBreakdownJsonPath: path.join(outputDir, 'scan-coverage-breakdown.json'),
    scanCoverageBreakdownMarkdownPath: path.join(outputDir, 'scan-coverage-breakdown.md'),
    reportPath: path.join(outputDir, 'REPORT.md'),
    engineeringReportPath: path.join(outputDir, 'ENGINEERING_REPORT.md'),
    repoEngineeringReportPath: path.join(
      outputDir,
      `${sanitizeRepoArtifactLabel(repoLabel)}_ENGINEERING_REPORT.md`,
    ),
  };
}

export async function maybeBuildSessionTelemetrySummary(repoRootPath, outputDir) {
  const sessionEventsPath = path.join(repoRootPath, '.sentrux', 'agent-session-events.jsonl');
  if (!(await pathExists(sessionEventsPath))) {
    return null;
  }

  const summary = await loadSessionTelemetrySummary(sessionEventsPath, {
    repoRoot: repoRootPath,
  });
  const markdown = formatSessionTelemetrySummaryMarkdown(summary);
  const jsonPath = path.join(outputDir, 'session-telemetry-summary.json');
  const markdownPath = path.join(outputDir, 'session-telemetry-summary.md');
  await writeFile(jsonPath, `${JSON.stringify(summary, null, 2)}\n`, 'utf8');
  await writeFile(markdownPath, markdown, 'utf8');

  return {
    jsonPath,
    markdownPath,
  };
}

export async function buildReviewPackets(args, repoRootPath, outputDir) {
  const basePacketArgs = [
    '--repo-root',
    repoRootPath,
    '--limit',
    String(args.findingsLimit),
  ];
  const packetSpecs = [
    {
      tool: 'check',
      jsonPath: path.join(outputDir, 'check-review-packet.json'),
      markdownPath: path.join(outputDir, 'check-review-packet.md'),
    },
    {
      tool: 'findings',
      jsonPath: path.join(outputDir, 'findings-review-packet.json'),
      markdownPath: path.join(outputDir, 'findings-review-packet.md'),
    },
    {
      tool: 'session_end',
      jsonPath: path.join(outputDir, 'session-end-review-packet.json'),
      markdownPath: path.join(outputDir, 'session-end-review-packet.md'),
    },
  ];

  for (const packet of packetSpecs) {
    await runNodeScript(path.join(repoRoot, 'scripts/evals/build-check-review-packet.mjs'), [
      ...basePacketArgs,
      '--tool',
      packet.tool,
      '--output-json',
      packet.jsonPath,
      '--output-md',
      packet.markdownPath,
    ], { cwd: repoRoot });
  }

  return {
    checkJsonPath: packetSpecs[0].jsonPath,
    findingsJsonPath: packetSpecs[1].jsonPath,
    sessionEndJsonPath: packetSpecs[2].jsonPath,
  };
}

export async function writeExternalValidationArtifacts({
  rawToolAnalysis,
  rawToolSummary,
  scanCoverageBreakdown,
  reportPath,
  validationReport,
  engineeringReport,
  engineeringReportPath,
  repoEngineeringReportPath,
  rawToolAnalysisPath,
  rawToolSummaryPath,
  scanCoverageBreakdownJsonPath,
  scanCoverageBreakdownMarkdownPath,
}) {
  await writeFile(rawToolAnalysisPath, `${JSON.stringify(rawToolAnalysis, null, 2)}\n`, 'utf8');
  await writeFile(rawToolSummaryPath, `${JSON.stringify(rawToolSummary, null, 2)}\n`, 'utf8');
  await writeFile(
    scanCoverageBreakdownJsonPath,
    `${JSON.stringify(scanCoverageBreakdown, null, 2)}\n`,
    'utf8',
  );
  await writeFile(
    scanCoverageBreakdownMarkdownPath,
    formatScanCoverageBreakdownMarkdown(scanCoverageBreakdown),
    'utf8',
  );
  await writeFile(reportPath, validationReport, 'utf8');
  await writeFile(engineeringReportPath, engineeringReport, 'utf8');
  await writeFile(repoEngineeringReportPath, engineeringReport, 'utf8');
}
