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

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, '../..');

function parseArgs(argv) {
  const result = {
    repoRoot: null,
    repoLabel: 'repo',
    defectReportPath: null,
    reviewVerdictsPath: null,
    remediationReportPath: null,
    benchmarkPath: null,
    sessionEventsPath: null,
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

async function main() {
  const args = parseArgs(process.argv);
  const sessionEventsPath = args.sessionEventsPath ?? defaultSessionEventsPath(args.repoRoot);
  const sessionTelemetry = await loadSessionTelemetrySummary(sessionEventsPath, {
    repoRoot: args.repoRoot,
  });
  const defectReport = await readJson(args.defectReportPath);
  const reviewVerdicts = await readJson(args.reviewVerdictsPath);
  const remediationReport = await readJson(args.remediationReportPath);
  const benchmark = await readJson(args.benchmarkPath);

  const scorecard = buildSignalScorecard({
    repoLabel: args.repoLabel,
    defectReport,
    reviewVerdicts,
    remediationReport,
    benchmark,
    sessionTelemetry,
  });
  const summaryJsonPath = path.join(
    args.outputDir,
    `${args.repoLabel}-session-telemetry-summary.json`,
  );
  const summaryMarkdownPath = path.join(
    args.outputDir,
    `${args.repoLabel}-session-telemetry-summary.md`,
  );
  const scorecardJsonPath = path.join(
    args.outputDir,
    `${args.repoLabel}-signal-scorecard.json`,
  );
  const scorecardMarkdownPath = path.join(
    args.outputDir,
    `${args.repoLabel}-signal-scorecard.md`,
  );

  await writeArtifact(summaryJsonPath, `${JSON.stringify(sessionTelemetry, null, 2)}\n`);
  await writeArtifact(
    summaryMarkdownPath,
    formatSessionTelemetrySummaryMarkdown(sessionTelemetry),
  );
  await writeArtifact(scorecardJsonPath, `${JSON.stringify(scorecard, null, 2)}\n`);
  await writeArtifact(scorecardMarkdownPath, formatSignalScorecardMarkdown(scorecard));

  console.log(
    `Wrote calibration artifacts for ${args.repoLabel}: ${sessionTelemetry.summary.session_count} session(s), ${scorecard.summary.total_signals} signal(s).`,
  );
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
});
