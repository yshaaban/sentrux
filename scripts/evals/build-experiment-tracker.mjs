#!/usr/bin/env node

import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

import { writeJson, writeText } from '../lib/eval-batch.mjs';
import { parseCliArgs } from '../lib/eval-support.mjs';
import {
  buildExperimentTracker,
  formatExperimentTrackerMarkdown,
} from '../lib/experiment-program.mjs';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, '../..');

function setResolvedPathOption(property) {
  return function setResolvedPathValue(target, value) {
    target[property] = path.resolve(value);
  };
}

export function parseArgs(argv) {
  return parseCliArgs(
    argv,
    {
      indexPath: path.join(repoRoot, 'docs', 'v2', 'evals', 'experiments', 'index.json'),
      outputJson: null,
      outputMarkdown: null,
    },
    {
      values: {
        '--index': setResolvedPathOption('indexPath'),
        '--output-json': setResolvedPathOption('outputJson'),
        '--output-md': setResolvedPathOption('outputMarkdown'),
      },
    },
  );
}

async function main(argv = process.argv) {
  const args = parseArgs(argv);
  const tracker = await buildExperimentTracker({
    indexPath: args.indexPath,
    repoRootPath: repoRoot,
  });
  const markdown = formatExperimentTrackerMarkdown(tracker);

  if (args.outputJson) {
    await writeJson(args.outputJson, tracker);
  }
  if (args.outputMarkdown) {
    await writeText(args.outputMarkdown, markdown);
  }

  process.stdout.write(markdown);
}

const invokedPath = process.argv[1] ? pathToFileURL(path.resolve(process.argv[1])).href : null;

if (invokedPath === import.meta.url) {
  main().catch(function handleMainError(error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  });
}
