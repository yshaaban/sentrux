#!/usr/bin/env node

import path from 'node:path';
import { pathToFileURL } from 'node:url';

import { parseArgs } from './run-codex-session/args.mjs';
import { runCodexSession } from './run-codex-session/session.mjs';

export { runCodexSession };

async function main() {
  const args = parseArgs(process.argv);
  await runCodexSession(args);
}

const invokedPath = process.argv[1] ? pathToFileURL(path.resolve(process.argv[1])).href : null;

if (invokedPath === import.meta.url) {
  main().catch((error) => {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  });
}
