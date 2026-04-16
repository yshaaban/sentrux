#!/usr/bin/env node

import { spawnSync } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, '..');
const nodeBin = process.execPath;
const goldensOnly = process.argv.includes('--goldens-only');

const validators = [path.join(repoRoot, 'scripts/validate_parallel_code_v2.mjs')];

if (!goldensOnly) {
  validators.push(path.join(repoRoot, 'scripts/validate_one_tool_v2.mjs'));
}

function runValidator(validatorPath) {
  const label = path.basename(validatorPath);
  console.log(`Running ${label}...`);

  const result = spawnSync(nodeBin, [validatorPath, ...process.argv.slice(2)], {
    cwd: repoRoot,
    env: process.env,
    stdio: 'inherit',
    shell: false,
  });

  if (result.error) {
    throw result.error;
  }
  if (result.status !== 0) {
    process.exit(result.status ?? 1);
  }

  console.log(`Finished ${label}.`);
}

for (const validator of validators) {
  runValidator(validator);
}

console.log('All checked-in public v2 benchmark repo validation loops completed successfully.');
