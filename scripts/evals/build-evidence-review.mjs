#!/usr/bin/env node

import path from 'node:path';
import { buildEvidenceReview, formatEvidenceReviewMarkdown } from '../lib/evidence-review.mjs';
import { readJsonFile, repoRootFromImportMeta, writeMaybe } from './build-artifact-support.mjs';

const repoRoot = repoRootFromImportMeta(import.meta.url);

function parseArgs(argv) {
  const result = {
    scorecardPath: null,
    backlogPath: null,
    sessionCorpusPath: null,
    reviewPacketPath: null,
    outputJsonPath: null,
    outputMarkdownPath: null,
  };

  for (let index = 2; index < argv.length; index += 1) {
    const value = argv[index];
    if (value === '--scorecard') {
      index += 1;
      result.scorecardPath = argv[index];
      continue;
    }
    if (value === '--backlog') {
      index += 1;
      result.backlogPath = argv[index];
      continue;
    }
    if (value === '--session-corpus') {
      index += 1;
      result.sessionCorpusPath = argv[index];
      continue;
    }
    if (value === '--review-packet') {
      index += 1;
      result.reviewPacketPath = argv[index];
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

  if (!result.scorecardPath || !result.backlogPath || !result.sessionCorpusPath) {
    throw new Error(
      'Missing required inputs: --scorecard, --backlog, and --session-corpus are all required',
    );
  }

  return result;
}

async function main() {
  const args = parseArgs(process.argv);
  const scorecard = await readJsonFile(args.scorecardPath);
  const backlog = await readJsonFile(args.backlogPath);
  const sessionCorpus = await readJsonFile(args.sessionCorpusPath);
  const reviewPacket = args.reviewPacketPath ? await readJsonFile(args.reviewPacketPath) : null;
  const review = buildEvidenceReview({
    scorecard,
    backlog,
    sessionCorpus,
    reviewPacket,
  });
  const markdown = formatEvidenceReviewMarkdown(review);

  await writeMaybe(
    args.outputJsonPath ?? path.join(repoRoot, 'docs/v2/examples', 'evidence-review.json'),
    `${JSON.stringify(review, null, 2)}\n`,
  );
  await writeMaybe(
    args.outputMarkdownPath ?? path.join(repoRoot, 'docs/v2/examples', 'evidence-review.md'),
    markdown,
  );

  console.log(
    `Built weekly evidence review for ${review.repo_label ?? 'unknown'} with ${review.summary.ranking_miss_count} ranking miss(es).`,
  );
}

main().catch(function handleError(error) {
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
});
