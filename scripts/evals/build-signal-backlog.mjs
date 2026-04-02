#!/usr/bin/env node

import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { buildSignalBacklog, formatSignalBacklogMarkdown } from '../lib/signal-backlog.mjs';
import { getSignalCohort, loadSignalCohortManifest } from '../lib/signal-cohorts.mjs';
import { readJson, writeJson, writeText } from '../lib/eval-batch.mjs';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, '../..');

function parseArgs(argv) {
  const result = {
    cohortManifestPath: path.join(repoRoot, 'docs/v2/evals', 'signal-cohorts.json'),
    cohortId: null,
    scorecardPath: null,
    codexBatchPath: null,
    replayBatchPath: null,
    outputJsonPath: null,
    outputMarkdownPath: null,
  };

  for (let index = 2; index < argv.length; index += 1) {
    const value = argv[index];
    if (value === '--cohort-manifest') {
      index += 1;
      result.cohortManifestPath = argv[index];
      continue;
    }
    if (value === '--cohort-id') {
      index += 1;
      result.cohortId = argv[index];
      continue;
    }
    if (value === '--scorecard') {
      index += 1;
      result.scorecardPath = argv[index];
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

  if (!result.scorecardPath) {
    throw new Error('Missing required --scorecard path');
  }

  return result;
}

async function main() {
  const args = parseArgs(process.argv);
  const cohortManifest = await loadSignalCohortManifest(args.cohortManifestPath);
  const cohort = getSignalCohort(cohortManifest, args.cohortId ?? null);
  const scorecard = await readJson(args.scorecardPath);
  const codexBatch = args.codexBatchPath ? await readJson(args.codexBatchPath) : null;
  const replayBatch = args.replayBatchPath ? await readJson(args.replayBatchPath) : null;
  const backlog = buildSignalBacklog({
    cohort,
    scorecard,
    codexBatch,
    replayBatch,
  });
  const outputJsonPath =
    args.outputJsonPath ??
    path.join(repoRoot, 'docs/v2/examples', `${cohort.cohort_id}-signal-backlog.json`);
  const outputMarkdownPath =
    args.outputMarkdownPath ??
    path.join(repoRoot, 'docs/v2/examples', `${cohort.cohort_id}-signal-backlog.md`);

  await writeJson(outputJsonPath, backlog);
  await writeText(outputMarkdownPath, formatSignalBacklogMarkdown(backlog));

  console.log(
    `Built signal backlog for cohort ${cohort.cohort_id} with ${backlog.summary.weak_signal_count} weak signal(s) and ${backlog.summary.next_candidate_count} candidate next signal(s).`,
  );
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
});
