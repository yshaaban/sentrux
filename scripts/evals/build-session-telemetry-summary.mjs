#!/usr/bin/env node

import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  formatSessionTelemetrySummaryMarkdown,
  loadSessionTelemetrySummary,
} from '../lib/session-telemetry.mjs';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, '../..');

function parseArgs(argv) {
  const result = {
    sessionEventsPath: null,
    repoRoot: null,
    outputJsonPath: null,
    outputMarkdownPath: null,
  };

  for (let index = 2; index < argv.length; index += 1) {
    const value = argv[index];
    if (value === '--session-events') {
      index += 1;
      result.sessionEventsPath = argv[index];
      continue;
    }
    if (value === '--repo-root') {
      index += 1;
      result.repoRoot = argv[index];
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

  if (!result.sessionEventsPath && !result.repoRoot) {
    throw new Error('Provide either --session-events or --repo-root');
  }

  return result;
}

function defaultSessionEventsPath(repoRootPath) {
  return path.join(repoRootPath, '.sentrux', 'agent-session-events.jsonl');
}

async function writeMaybe(targetPath, text) {
  if (!targetPath) {
    return;
  }

  await mkdir(path.dirname(targetPath), { recursive: true });
  await writeFile(targetPath, text, 'utf8');
}

async function main() {
  const args = parseArgs(process.argv);
  const sessionEventsPath = args.sessionEventsPath ?? defaultSessionEventsPath(args.repoRoot);
  const summary = await loadSessionTelemetrySummary(sessionEventsPath, {
    repoRoot: args.repoRoot,
  });
  const markdown = formatSessionTelemetrySummaryMarkdown(summary);
  const jsonPath =
    args.outputJsonPath ??
    path.join(repoRoot, 'docs/v2/examples', 'session-telemetry-summary.json');
  const markdownPath =
    args.outputMarkdownPath ??
    path.join(repoRoot, 'docs/v2/examples', 'session-telemetry-summary.md');

  await writeMaybe(jsonPath, `${JSON.stringify(summary, null, 2)}\n`);
  await writeMaybe(markdownPath, markdown);

  console.log(
    `Built session telemetry summary from ${summary.summary.event_count} event(s) across ${summary.summary.session_count} session(s).`,
  );
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
});
