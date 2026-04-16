#!/usr/bin/env node

import { mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { createMcpSession, runTool } from '../lib/benchmark-harness.mjs';
import { prepareTypeScriptBenchmarkHome } from '../lib/benchmark-plugin-home.mjs';
import {
  buildPacketFromArtifactInput,
  buildPacketFromRepoHeadPayload,
  buildVerdictTemplate,
  formatPacketMarkdown,
  loadArtifactInput,
} from '../lib/check-review-packet-support.mjs';
import { resolveWorkspaceRepoRoot } from '../lib/path-roots.mjs';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, '../..');
const sentruxBin = process.env.SENTRUX_BIN ?? path.join(repoRoot, 'target/debug/sentrux');

export {
  buildPacketFromArtifactInput,
  buildPacketFromRepoHeadPayload,
  formatPacketMarkdown,
  loadArtifactInput,
} from '../lib/check-review-packet-support.mjs';

export function parseArgs(argv) {
  const result = {
    repoRoot: resolveWorkspaceRepoRoot(process.env.PARALLEL_CODE_ROOT, 'parallel-code', repoRoot),
    tool: 'check',
    limit: 10,
    bundlePath: null,
    codexBatchPath: null,
    replayBatchPath: null,
    kinds: [],
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
    if (value === '--bundle') {
      index += 1;
      result.bundlePath = argv[index];
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
    if (value === '--kind') {
      index += 1;
      result.kinds.push(argv[index]);
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

function buildToolArgs(tool, limit) {
  if (tool === 'findings') {
    return { limit };
  }
  return {};
}

async function buildRepoHeadPacket(args) {
  const tempRoot = await mkdtemp(path.join(tmpdir(), 'sentrux-check-review-'));
  const pluginHome = await prepareTypeScriptBenchmarkHome({ tempRoot });
  const session = createMcpSession({
    binPath: sentruxBin,
    repoRoot: args.repoRoot,
    homeOverride: pluginHome,
    skipGrammarDownload: process.env.SENTRUX_SKIP_GRAMMAR_DOWNLOAD ?? '1',
    requestTimeoutMs: Number(process.env.REQUEST_TIMEOUT_MS ?? '120000'),
  });

  try {
    const scanPayload = (await runTool(session, 'scan', { path: args.repoRoot })).payload;
    if (args.tool === 'session_end') {
      await runTool(session, 'session_start', {});
    }
    const payload = (
      await runTool(session, args.tool, buildToolArgs(args.tool, args.limit))
    ).payload;
    return buildPacketFromRepoHeadPayload(args, payload, scanPayload);
  } finally {
    await session.close();
    await rm(tempRoot, { recursive: true, force: true });
  }
}

async function main() {
  const args = parseArgs(process.argv);
  const artifactInput = await loadArtifactInput(args);
  const packet = artifactInput
    ? buildPacketFromArtifactInput(args, artifactInput)
    : await buildRepoHeadPacket(args);
  const outputName = artifactInput
    ? `${artifactInput.label}-${args.tool}-review-packet`
    : `${args.tool}-review-packet`;
  const jsonPath =
    args.outputJsonPath ??
    path.join(repoRoot, 'docs/v2/examples', `${outputName}.json`);
  const markdownPath =
    args.outputMarkdownPath ??
    path.join(repoRoot, 'docs/v2/examples', `${outputName}.md`);
  const verdictTemplatePath = path.join(
    path.dirname(jsonPath),
    `${path.parse(jsonPath).name}-verdicts.template.json`,
  );
  await mkdir(path.dirname(jsonPath), { recursive: true });
  await writeFile(jsonPath, `${JSON.stringify(packet, null, 2)}\n`, 'utf8');
  await mkdir(path.dirname(markdownPath), { recursive: true });
  await writeFile(markdownPath, formatPacketMarkdown(packet), 'utf8');
  await writeFile(
    verdictTemplatePath,
    `${JSON.stringify(buildVerdictTemplate(packet, markdownPath), null, 2)}\n`,
    'utf8',
  );
  console.log(`Wrote ${packet.samples.length} review sample(s) for ${args.tool}.`);
}

const invokedPath = process.argv[1] ? pathToFileURL(path.resolve(process.argv[1])).href : null;

if (invokedPath === import.meta.url) {
  main().catch((error) => {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  });
}
