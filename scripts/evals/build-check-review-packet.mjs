#!/usr/bin/env node

import { mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createMcpSession, runTool } from '../lib/benchmark-harness.mjs';
import { resolveWorkspaceRepoRoot } from '../lib/path-roots.mjs';
import { prepareTypeScriptBenchmarkHome } from '../lib/benchmark-plugin-home.mjs';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, '../..');
const sentruxBin = process.env.SENTRUX_BIN ?? path.join(repoRoot, 'target/debug/sentrux');

function parseArgs(argv) {
  const result = {
    repoRoot: resolveWorkspaceRepoRoot(process.env.PARALLEL_CODE_ROOT, 'parallel-code', repoRoot),
    tool: 'check',
    limit: 10,
    outputJsonPath: null,
    outputMarkdownPath: null,
  };

  for (let index = 2; index < argv.length; index += 1) {
    const value = argv[index];
    if (value === '--repo-root') {
      index += 1;
      result.repoRoot = argv[index];
      continue;
    }
    if (value === '--tool') {
      index += 1;
      result.tool = argv[index];
      continue;
    }
    if (value === '--limit') {
      index += 1;
      result.limit = Number(argv[index]);
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

  return result;
}

function selectRawSamples(tool, payload) {
  if (tool === 'findings') {
    return payload.findings ?? [];
  }
  if (tool === 'session_end') {
    return payload.introduced_findings ?? [];
  }
  return payload.actions ?? payload.issues ?? [];
}

function buildToolArgs(tool, limit) {
  if (tool === 'findings') {
    return { limit };
  }
  return {};
}

function extractSamples(tool, payload, limit) {
  const rawSamples = selectRawSamples(tool, payload);

  return rawSamples.slice(0, Math.max(limit, 1)).map((sample, index) => ({
    review_id: `${tool}-${index + 1}`,
    kind: sample.kind ?? null,
    scope: sample.scope ?? sample.file ?? null,
    severity: sample.severity ?? null,
    summary: sample.summary ?? sample.message ?? null,
    evidence: sample.evidence ?? [],
    classification: null,
    notes: '',
    action: '',
  }));
}

function escapeMarkdownCell(value) {
  if (value === null || value === undefined) {
    return '';
  }

  return String(value)
    .replace(/\|/g, '\\|')
    .replace(/\r?\n/g, '<br>');
}

function formatPacketMarkdown(packet) {
  const lines = [];
  lines.push('# Check Review Packet');
  lines.push('');
  lines.push(`- repo root: \`${packet.repo_root}\``);
  lines.push(`- tool: \`${packet.tool}\``);
  lines.push(`- generated at: \`${packet.generated_at}\``);
  lines.push(`- sample count: ${packet.samples.length}`);
  lines.push('');
  lines.push('| Review ID | Kind | Scope | Severity | Summary | Classification | Action |');
  lines.push('| --- | --- | --- | --- | --- | --- | --- |');
  for (const sample of packet.samples) {
    lines.push(
      `| \`${escapeMarkdownCell(sample.review_id)}\` | \`${escapeMarkdownCell(sample.kind ?? 'unknown')}\` | \`${escapeMarkdownCell(sample.scope ?? 'unknown')}\` | \`${escapeMarkdownCell(sample.severity ?? 'unknown')}\` | ${escapeMarkdownCell(sample.summary ?? '')} |  |  |`,
    );
  }
  lines.push('');
  return `${lines.join('\n')}\n`;
}

async function main() {
  const args = parseArgs(process.argv);
  const tempRoot = await mkdtemp(path.join(tmpdir(), 'sentrux-check-review-'));
  const pluginHome = await prepareTypeScriptBenchmarkHome({ tempRoot });
  const session = createMcpSession({
    binPath: sentruxBin,
    repoRoot,
    homeOverride: pluginHome,
    skipGrammarDownload: process.env.SENTRUX_SKIP_GRAMMAR_DOWNLOAD ?? '1',
    requestTimeoutMs: Number(process.env.REQUEST_TIMEOUT_MS ?? '120000'),
  });

  try {
    await runTool(session, 'scan', { path: args.repoRoot });
    if (args.tool === 'session_end') {
      await runTool(session, 'session_start', {});
    }
    const payload = (
      await runTool(session, args.tool, buildToolArgs(args.tool, args.limit))
    ).payload;
    const packet = {
      schema_version: 1,
      generated_at: new Date().toISOString(),
      repo_root: args.repoRoot,
      tool: args.tool,
      samples: extractSamples(args.tool, payload, args.limit),
    };
    const jsonPath =
      args.outputJsonPath ??
      path.join(repoRoot, 'docs/v2/examples', `${args.tool}-review-packet.json`);
    const markdownPath =
      args.outputMarkdownPath ??
      path.join(repoRoot, 'docs/v2/examples', `${args.tool}-review-packet.md`);
    await mkdir(path.dirname(jsonPath), { recursive: true });
    await writeFile(jsonPath, `${JSON.stringify(packet, null, 2)}\n`, 'utf8');
    await mkdir(path.dirname(markdownPath), { recursive: true });
    await writeFile(markdownPath, formatPacketMarkdown(packet), 'utf8');
    console.log(`Wrote ${packet.samples.length} review sample(s) for ${args.tool}.`);
  } finally {
    await session.close();
    await rm(tempRoot, { recursive: true, force: true });
  }
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
});
