#!/usr/bin/env node

import { existsSync } from 'node:fs';
import { mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { createMcpSession, runTool } from '../lib/benchmark-harness.mjs';
import { resolveWorkspaceRepoRoot } from '../lib/path-roots.mjs';
import { prepareTypeScriptBenchmarkHome } from '../lib/benchmark-plugin-home.mjs';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, '../..');
const sentruxBin = process.env.SENTRUX_BIN ?? path.join(repoRoot, 'target/debug/sentrux');

export function parseArgs(argv) {
  const result = {
    repoRoot: resolveWorkspaceRepoRoot(process.env.PARALLEL_CODE_ROOT, 'parallel-code', repoRoot),
    tool: 'check',
    limit: 10,
    bundlePath: null,
    codexBatchPath: null,
    replayBatchPath: null,
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

  const artifactSources = [result.bundlePath, result.codexBatchPath, result.replayBatchPath].filter(
    Boolean,
  );
  if (artifactSources.length > 1) {
    throw new Error('Provide only one of --bundle, --codex-batch, or --replay-batch');
  }

  return result;
}

function loadJson(targetPath) {
  return readFile(targetPath, 'utf8').then((source) => JSON.parse(source));
}

function sourceLabelFromPath(targetPath) {
  const baseName = path.basename(targetPath);
  return baseName.endsWith('.json') ? baseName.slice(0, -5) : baseName;
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

function selectArtifactPayload(tool, bundle) {
  if (tool === 'findings') {
    return bundle.findings ?? null;
  }
  if (tool === 'session_end') {
    return bundle.session_end ?? null;
  }
  return bundle.initial_check ?? bundle.final_check ?? null;
}

async function loadBundleArtifact(bundlePath) {
  const bundle = await loadJson(bundlePath);
  return {
    source_mode: 'bundle',
    source_paths: [path.resolve(bundlePath)],
    repo_root: bundle.repo_root ?? bundle.source_root ?? null,
    label: sourceLabelFromPath(bundlePath),
    bundles: [bundle],
  };
}

async function loadBatchArtifact(batchPath, kind) {
  const batch = await loadJson(batchPath);
  const batchDir = path.dirname(path.resolve(batchPath));
  const bundleFileName = kind === 'codex-batch' ? 'codex-session.json' : 'diff-replay.json';
  const bundles = [];
  const bundlePaths = [];

  for (const result of batch.results ?? []) {
    const outputDir = result.output_dir ? path.resolve(batchDir, result.output_dir) : null;
    if (!outputDir) {
      throw new Error(`Missing output_dir for batch result in ${batchPath}`);
    }

    const bundlePath = path.join(outputDir, bundleFileName);
    if (!existsSync(bundlePath)) {
      throw new Error(`Missing bundle artifact: ${bundlePath}`);
    }

    bundlePaths.push(bundlePath);
    bundles.push(await loadJson(bundlePath));
  }

  return {
    source_mode: kind,
    source_paths: [path.resolve(batchPath), ...bundlePaths],
    repo_root: batch.repo_root ?? bundles[0]?.repo_root ?? null,
    label: sourceLabelFromPath(batchPath),
    bundles,
  };
}

export async function loadArtifactInput(args) {
  if (args.bundlePath) {
    return loadBundleArtifact(args.bundlePath);
  }
  if (args.codexBatchPath) {
    return loadBatchArtifact(args.codexBatchPath, 'codex-batch');
  }
  if (args.replayBatchPath) {
    return loadBatchArtifact(args.replayBatchPath, 'replay-batch');
  }

  return null;
}

function buildPacketSamplesFromBundles(tool, bundles, limit) {
  const samples = [];
  for (const bundle of bundles) {
    const payload = selectArtifactPayload(tool, bundle);
    if (!payload) {
      continue;
    }

    const bundleSamples = extractSamples(tool, payload, limit);
    for (const sample of bundleSamples) {
      samples.push(sample);
      if (samples.length >= Math.max(limit, 1)) {
        return samples;
      }
    }
  }

  return samples;
}

function renumberSamples(tool, samples) {
  return samples.map((sample, index) => ({
    ...sample,
    review_id: `${tool}-${index + 1}`,
  }));
}

export function buildPacketFromArtifactInput(args, source) {
  const repoRootValue = source.repo_root ?? args.repoRoot;
  const samples = renumberSamples(
    args.tool,
    buildPacketSamplesFromBundles(args.tool, source.bundles, args.limit),
  );

  return {
    schema_version: 1,
    generated_at: new Date().toISOString(),
    repo_root: repoRootValue,
    tool: args.tool,
    source_mode: source.source_mode,
    source_paths: source.source_paths,
    samples,
  };
}

function escapeMarkdownCell(value) {
  if (value === null || value === undefined) {
    return '';
  }

  return String(value)
    .replace(/\|/g, '\\|')
    .replace(/\r?\n/g, '<br>');
}

export function formatPacketMarkdown(packet) {
  const lines = [];
  lines.push('# Check Review Packet');
  lines.push('');
  lines.push(`- repo root: \`${packet.repo_root}\``);
  lines.push(`- tool: \`${packet.tool}\``);
  lines.push(`- source mode: \`${packet.source_mode ?? 'repo-head'}\``);
  if (Array.isArray(packet.source_paths) && packet.source_paths.length > 0) {
    lines.push(`- source path(s):`);
    for (const sourcePath of packet.source_paths) {
      lines.push(`  - \`${sourcePath}\``);
    }
  }
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

async function buildRepoHeadPacket(args) {
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
    return {
      schema_version: 1,
      generated_at: new Date().toISOString(),
      repo_root: args.repoRoot,
      tool: args.tool,
      source_mode: 'repo-head',
      source_paths: [],
      samples: extractSamples(args.tool, payload, args.limit),
    };
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
  const jsonPath =
    args.outputJsonPath ??
    path.join(
      repoRoot,
      'docs/v2/examples',
      `${artifactInput ? `${artifactInput.label}-${args.tool}-review-packet` : `${args.tool}-review-packet`}.json`,
    );
  const markdownPath =
    args.outputMarkdownPath ??
    path.join(
      repoRoot,
      'docs/v2/examples',
      `${artifactInput ? `${artifactInput.label}-${args.tool}-review-packet` : `${args.tool}-review-packet`}.md`,
    );
  await mkdir(path.dirname(jsonPath), { recursive: true });
  await writeFile(jsonPath, `${JSON.stringify(packet, null, 2)}\n`, 'utf8');
  await mkdir(path.dirname(markdownPath), { recursive: true });
  await writeFile(markdownPath, formatPacketMarkdown(packet), 'utf8');
  console.log(`Wrote ${packet.samples.length} review sample(s) for ${args.tool}.`);
}

const invokedPath = process.argv[1] ? pathToFileURL(path.resolve(process.argv[1])).href : null;

if (invokedPath === import.meta.url) {
  main().catch((error) => {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  });
}
