#!/usr/bin/env node

import path from 'node:path';
import { existsSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { runDefectInjection } from './run-injection.mjs';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, '../..');

async function main() {
  const selfRulesSource = path.join(repoRoot, '.sentrux', 'rules.toml');
  const report = await runDefectInjection({
    repo: 'self',
    defects: [],
    analysisMode: 'head_clone',
    rulesSource: existsSync(selfRulesSource) ? selfRulesSource : undefined,
    outputJsonPath: process.env.OUTPUT_JSON_PATH ?? null,
    outputMarkdownPath: process.env.OUTPUT_MD_PATH ?? null,
  });

  console.log(
    `Dogfood run complete for ${report.repo_label}: ${report.summary.detected}/${report.summary.total} detected.`,
  );
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
});
