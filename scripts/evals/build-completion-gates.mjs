#!/usr/bin/env node

import { readdir } from 'node:fs/promises';
import path from 'node:path';
import {
  buildCompletionGates,
  formatCompletionGatesMarkdown,
} from '../lib/completion-gates.mjs';
import { readJsonFile, repoRootFromImportMeta, writeMaybe } from './build-artifact-support.mjs';

const repoRoot = repoRootFromImportMeta(import.meta.url);

function parseArgs(argv) {
  const result = {
    rubricPath: path.join(repoRoot, 'docs/v2/evals/master-plan-completion-rubric.json'),
    scorecardPath: null,
    sessionCorpusPath: null,
    evidenceReviewPath: null,
    backlogPath: null,
    reviewPacketPath: null,
    decisionRecordsDir: path.join(repoRoot, 'docs/v2/experiments/decisions'),
    generatedAt: null,
    outputJsonPath: null,
    outputMarkdownPath: null,
  };

  for (let index = 2; index < argv.length; index += 1) {
    const value = argv[index];
    if (value === '--rubric') {
      index += 1;
      result.rubricPath = argv[index];
      continue;
    }
    if (value === '--scorecard') {
      index += 1;
      result.scorecardPath = argv[index];
      continue;
    }
    if (value === '--session-corpus' || value === '--session-summary') {
      index += 1;
      result.sessionCorpusPath = argv[index];
      continue;
    }
    if (value === '--evidence-review' || value === '--evidence-summary') {
      index += 1;
      result.evidenceReviewPath = argv[index];
      continue;
    }
    if (value === '--backlog') {
      index += 1;
      result.backlogPath = argv[index];
      continue;
    }
    if (value === '--review-packet') {
      index += 1;
      result.reviewPacketPath = argv[index];
      continue;
    }
    if (value === '--decision-records-dir') {
      index += 1;
      result.decisionRecordsDir = argv[index];
      continue;
    }
    if (value === '--generated-at') {
      index += 1;
      result.generatedAt = argv[index];
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

  if (!result.rubricPath) {
    throw new Error('Missing required input: --rubric');
  }

  if (
    !result.scorecardPath &&
    !result.sessionCorpusPath &&
    !result.evidenceReviewPath &&
    !result.backlogPath &&
    !result.reviewPacketPath
  ) {
    throw new Error(
      'Provide at least one evidence input: --scorecard, --session-corpus, --evidence-review, --backlog, or --review-packet',
    );
  }

  return result;
}

async function readOptionalJson(targetPath) {
  return targetPath ? await readJsonFile(targetPath) : null;
}

function phaseKeyFromDecisionFilename(filename) {
  const match = filename.match(/(?:^|-)phase-(\d+)(?:-|$)/);
  if (!match) {
    return null;
  }

  return `phase_${match[1]}`;
}

function buildDecisionRecordSummary(files) {
  const phaseCounts = {};
  for (const file of files) {
    const phaseKey = phaseKeyFromDecisionFilename(file);
    if (!phaseKey) {
      continue;
    }

    phaseCounts[phaseKey] = (phaseCounts[phaseKey] ?? 0) + 1;
  }

  return {
    count: files.length,
    files,
    phase_counts: phaseCounts,
  };
}

async function readDecisionRecords(directoryPath) {
  if (!directoryPath) {
    return buildDecisionRecordSummary([]);
  }

  let entries = [];
  try {
    entries = await readdir(directoryPath, { withFileTypes: true });
  } catch (error) {
    if (error && error.code === 'ENOENT') {
      return buildDecisionRecordSummary([]);
    }

    throw error;
  }

  const files = entries
    .filter(function isDecisionRecord(entry) {
      return entry.isFile() && entry.name.endsWith('.md') && entry.name !== 'README.md';
    })
    .map(function decisionRecordName(entry) {
      return entry.name;
    })
    .sort();

  return buildDecisionRecordSummary(files);
}

async function main() {
  const args = parseArgs(process.argv);
  const rubric = await readJsonFile(args.rubricPath);
  const scorecard = await readOptionalJson(args.scorecardPath);
  const sessionCorpus = await readOptionalJson(args.sessionCorpusPath);
  const evidenceReview = await readOptionalJson(args.evidenceReviewPath);
  const backlog = await readOptionalJson(args.backlogPath);
  const reviewPacket = await readOptionalJson(args.reviewPacketPath);
  const decisionRecords = await readDecisionRecords(args.decisionRecordsDir);
  const result = buildCompletionGates({
    rubric,
    scorecard,
    sessionCorpus,
    evidenceReview,
    backlog,
    reviewPacket,
    decisionRecords,
    generatedAt: args.generatedAt ?? new Date().toISOString(),
  });
  const markdown = formatCompletionGatesMarkdown(result);

  await writeMaybe(
    args.outputJsonPath ?? path.join(repoRoot, 'docs/v2/examples', 'completion-gates.json'),
    `${JSON.stringify(result, null, 2)}\n`,
  );
  await writeMaybe(
    args.outputMarkdownPath ?? path.join(repoRoot, 'docs/v2/examples', 'completion-gates.md'),
    markdown,
  );

  console.log(
    `Built completion gates for ${result.repo_label ?? 'unknown'}: ${result.summary.status} (${result.summary.required_failure_count} required failure(s)).`,
  );
}

main().catch(function handleError(error) {
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
});
