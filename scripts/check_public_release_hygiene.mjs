#!/usr/bin/env node

import process from 'node:process';

import { listRepoFiles, scanFiles } from './lib/public-release-hygiene.mjs';

async function main() {
  const rootDir = process.cwd();
  const filePaths = listRepoFiles(rootDir);
  const matches = await scanFiles(rootDir, filePaths);

  if (matches.length === 0) {
    console.log('public release hygiene: clean');
    return;
  }

  console.error('public release hygiene: found banned content');
  for (const match of matches) {
    console.error(
      `${match.filePath}:${match.line} [${match.ruleId}] ${match.message} (${match.snippet})`,
    );
  }
  process.exitCode = 1;
}

main().catch(function handleError(error) {
  console.error(error instanceof Error ? error.stack ?? error.message : String(error));
  process.exitCode = 1;
});
