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

function selectCheckPayloads(bundle) {
  const payloads = [];
  if (bundle.initial_check) {
    payloads.push({ snapshot_label: 'initial_check', payload: bundle.initial_check });
  }
  for (const snapshot of bundle.snapshots ?? []) {
    if (snapshot.check) {
      payloads.push({
        snapshot_label: snapshot.label ?? 'snapshot',
        payload: snapshot.check,
      });
    }
  }
  if (bundle.final_check) {
    payloads.push({ snapshot_label: 'final_check', payload: bundle.final_check });
  }

  return payloads;
}

function selectArtifactPayload(tool, bundle) {
  if (tool === 'check') {
    const payloads = selectCheckPayloads(bundle);
    for (const payloadEntry of payloads) {
      if (selectRawSamples(tool, payloadEntry.payload).length > 0) {
        return payloadEntry;
      }
    }

    return payloads.at(-1) ?? null;
  }
  if (tool === 'findings') {
    return bundle.findings ? { snapshot_label: 'findings', payload: bundle.findings } : null;
  }
  if (tool === 'session_end') {
    return bundle.session_end
      ? { snapshot_label: 'session_end', payload: bundle.session_end }
      : null;
  }

  return null;
}

async function loadBundleArtifact(bundlePath) {
  const bundle = await loadJson(bundlePath);
  return {
    source_mode: 'bundle',
    source_paths: [path.resolve(bundlePath)],
    repo_root: bundle.repo_root ?? bundle.source_root ?? null,
    label: sourceLabelFromPath(bundlePath),
    entries: [
      {
        bundle,
        bundle_path: path.resolve(bundlePath),
        output_dir: path.dirname(path.resolve(bundlePath)),
        source_kind: 'bundle',
        source_label:
          bundle.task_label ??
          bundle.task_id ??
          bundle.replay_id ??
          bundle.replay?.commit ??
          sourceLabelFromPath(bundlePath),
        task_id: bundle.task_id ?? null,
        task_label: bundle.task_label ?? null,
        replay_id: bundle.replay_id ?? null,
        commit: bundle.replay?.commit ?? null,
        expected_signal_kinds: bundle.expected_signal_kinds ?? [],
        expected_fix_surface: bundle.expected_fix_surface ?? null,
      },
    ],
  };
}

async function loadBatchArtifact(batchPath, kind) {
  const batch = await loadJson(batchPath);
  const batchDir = path.dirname(path.resolve(batchPath));
  const bundleFileName = kind === 'codex-batch' ? 'codex-session.json' : 'diff-replay.json';
  const bundlePaths = [];
  const entries = [];

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
    const bundle = await loadJson(bundlePath);
    entries.push({
      bundle,
      bundle_path: bundlePath,
      output_dir: outputDir,
      source_kind: kind,
      source_label:
        result.task_label ??
        result.task_id ??
        result.replay_id ??
        result.commit ??
        bundle.task_label ??
        bundle.task_id ??
        bundle.replay_id ??
        bundle.replay?.commit ??
        sourceLabelFromPath(bundlePath),
      task_id: result.task_id ?? bundle.task_id ?? null,
      task_label: result.task_label ?? bundle.task_label ?? null,
      replay_id: result.replay_id ?? bundle.replay_id ?? null,
      commit: result.commit ?? bundle.replay?.commit ?? null,
      expected_signal_kinds: result.expected_signal_kinds ?? bundle.expected_signal_kinds ?? [],
      expected_fix_surface: result.expected_fix_surface ?? bundle.expected_fix_surface ?? null,
    });
  }

  return {
    source_mode: kind,
    source_paths: [path.resolve(batchPath), ...bundlePaths],
    repo_root: batch.repo_root ?? entries[0]?.bundle?.repo_root ?? null,
    label: sourceLabelFromPath(batchPath),
    entries,
  };
}

export async function loadArtifactInput(args) {
  const sources = [];
  if (args.bundlePath) {
    sources.push(await loadBundleArtifact(args.bundlePath));
  }
  if (args.codexBatchPath) {
    sources.push(await loadBatchArtifact(args.codexBatchPath, 'codex-batch'));
  }
  if (args.replayBatchPath) {
    sources.push(await loadBatchArtifact(args.replayBatchPath, 'replay-batch'));
  }

  if (sources.length === 0) {
    return null;
  }
  if (sources.length === 1) {
    return sources[0];
  }

  return {
    source_mode: 'combined',
    source_paths: [...new Set(sources.flatMap((source) => source.source_paths))],
    repo_root: sources[0]?.repo_root ?? null,
    label: sources.map((source) => source.label).join('-'),
    entries: sources.flatMap((source) => source.entries),
  };
}

function extractSamples(tool, payloadEntry, sourceEntry) {
  const rawSamples = selectRawSamples(tool, payloadEntry.payload);

  return rawSamples.map((sample, index) => ({
    review_id: `${tool}-${index + 1}`,
    rank: index + 1,
    kind: sample.kind ?? null,
    report_bucket: tool === 'check' ? 'actions' : tool,
    scope: sample.scope ?? sample.file ?? null,
    severity: sample.severity ?? null,
    summary: sample.summary ?? sample.message ?? null,
    evidence: sample.evidence ?? [],
    source_kind: sourceEntry.source_kind,
    source_label: sourceEntry.source_label,
    snapshot_label: payloadEntry.snapshot_label,
    task_id: sourceEntry.task_id,
    task_label: sourceEntry.task_label,
    replay_id: sourceEntry.replay_id,
    commit: sourceEntry.commit,
    output_dir: sourceEntry.output_dir,
    expected_signal_kinds: sourceEntry.expected_signal_kinds,
    expected_fix_surface: sourceEntry.expected_fix_surface,
    classification: null,
    notes: '',
    action: '',
  }));
}

function filterSamplesByKinds(samples, kinds) {
  const normalizedKinds = new Set(kinds ?? []);
  if (normalizedKinds.size === 0) {
    return samples;
  }

  return samples.filter((sample) => normalizedKinds.has(sample.kind));
}

function limitSamples(samples, limit) {
  return samples.slice(0, Math.max(limit, 1));
}

function buildPacketSamplesFromEntries(tool, entries, limit, kinds) {
  const samples = [];

  for (const entry of entries) {
    const payloadEntry = selectArtifactPayload(tool, entry.bundle);
    if (!payloadEntry) {
      continue;
    }

    const entrySamples = filterSamplesByKinds(extractSamples(tool, payloadEntry, entry), kinds);
    for (const sample of entrySamples) {
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
    rank: index + 1,
    review_id: `${tool}-${index + 1}`,
  }));
}

function buildPacketSummary(samples) {
  const kindCounts = new Map();

  for (const sample of samples) {
    const key = sample.kind ?? 'unknown';
    kindCounts.set(key, (kindCounts.get(key) ?? 0) + 1);
  }

  return {
    sample_count: samples.length,
    kind_counts: [...kindCounts.entries()]
      .map(([kind, count]) => ({ kind, count }))
      .sort((left, right) => right.count - left.count || left.kind.localeCompare(right.kind)),
  };
}

function buildPacket(args, repoRootValue, sourceMode, sourcePaths, samples) {
  return {
    schema_version: 1,
    generated_at: new Date().toISOString(),
    repo_root: repoRootValue,
    tool: args.tool,
    source_mode: sourceMode,
    source_paths: sourcePaths,
    filters: {
      kinds: args.kinds,
    },
    summary: buildPacketSummary(samples),
    samples,
  };
}

function createRepoHeadEntry() {
  return {
    source_kind: 'repo-head',
    source_label: 'repo-head',
    task_id: null,
    task_label: null,
    replay_id: null,
    commit: null,
    output_dir: null,
    expected_signal_kinds: [],
    expected_fix_surface: null,
  };
}

export function buildPacketFromArtifactInput(args, source) {
  const repoRootValue = source.repo_root ?? args.repoRoot;
  const samples = renumberSamples(
    args.tool,
    buildPacketSamplesFromEntries(args.tool, source.entries, args.limit, args.kinds),
  );

  return buildPacket(
    args,
    repoRootValue,
    source.source_mode,
    source.source_paths,
    samples,
  );
}

export function buildPacketFromRepoHeadPayload(args, payload) {
  const repoHeadEntry = createRepoHeadEntry();
  const samples = renumberSamples(
    args.tool,
    limitSamples(
      filterSamplesByKinds(
        extractSamples(
          args.tool,
          { snapshot_label: 'repo_head', payload },
          repoHeadEntry,
        ),
        args.kinds,
      ),
      args.limit,
    ),
  );

  return buildPacket(args, args.repoRoot, 'repo-head', [], samples);
}

function buildVerdictTemplate(packet, sourceReport) {
  return {
    repo: packet.repo_root ? path.basename(packet.repo_root) : 'unknown',
    captured_at: packet.generated_at,
    source_report: sourceReport,
    source_feedback:
      'Replace the placeholder verdict values below after reviewing the packet. Do not use this template as scored evidence until it has been curated by a reviewer.',
    verdicts: packet.samples.map((sample) => ({
      scope: sample.scope ?? sample.source_label ?? 'unknown-scope',
      kind: sample.kind ?? 'unknown-kind',
      report_bucket: sample.report_bucket,
      category: 'useful',
      expected_trust_tier: sample.severity === 'high' ? 'trusted' : 'watchpoint',
      expected_presentation_class: 'review_required',
      expected_leverage_class: sample.expected_fix_surface ?? 'local_refactor_target',
      expected_summary_presence: 'section_present',
      preferred_over: [],
      engineer_note: sample.summary ?? 'Replace with reviewer rationale.',
      expected_v2_behavior: `Confirm the ranking and presentation for ${sample.kind ?? 'this finding'}.`,
    })),
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
  if (Array.isArray(packet.filters?.kinds) && packet.filters.kinds.length > 0) {
    lines.push(`- filtered kinds: \`${packet.filters.kinds.join('`, `')}\``);
  }
  if (Array.isArray(packet.source_paths) && packet.source_paths.length > 0) {
    lines.push(`- source path(s):`);
    for (const sourcePath of packet.source_paths) {
      lines.push(`  - \`${sourcePath}\``);
    }
  }
  lines.push(`- generated at: \`${packet.generated_at}\``);
  lines.push(`- sample count: ${packet.samples.length}`);
  if (Array.isArray(packet.summary?.kind_counts) && packet.summary.kind_counts.length > 0) {
    lines.push(`- kind counts: ${packet.summary.kind_counts.map((entry) => `${entry.kind}=${entry.count}`).join(', ')}`);
  }
  lines.push('');
  lines.push('| Review ID | Kind | Source | Snapshot | Rank | Scope | Severity | Summary | Classification | Action |');
  lines.push('| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |');
  for (const sample of packet.samples) {
    lines.push(
      `| \`${escapeMarkdownCell(sample.review_id)}\` | \`${escapeMarkdownCell(sample.kind ?? 'unknown')}\` | \`${escapeMarkdownCell(sample.source_label ?? sample.source_kind ?? 'unknown')}\` | \`${escapeMarkdownCell(sample.snapshot_label ?? 'n/a')}\` | ${escapeMarkdownCell(sample.rank ?? 'n/a')} | \`${escapeMarkdownCell(sample.scope ?? 'unknown')}\` | \`${escapeMarkdownCell(sample.severity ?? 'unknown')}\` | ${escapeMarkdownCell(sample.summary ?? '')} |  |  |`,
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
    repoRoot: args.repoRoot,
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
    return buildPacketFromRepoHeadPayload(args, payload);
  } finally {
    await session.close();
    await rm(tempRoot, { recursive: true, force: true });
  }
}

function defaultPacketOutputName(artifactInput, tool) {
  if (artifactInput) {
    return `${artifactInput.label}-${tool}-review-packet`;
  }

  return `${tool}-review-packet`;
}

async function main() {
  const args = parseArgs(process.argv);
  const artifactInput = await loadArtifactInput(args);
  const packet = artifactInput
    ? buildPacketFromArtifactInput(args, artifactInput)
    : await buildRepoHeadPacket(args);
  const outputName = defaultPacketOutputName(artifactInput, args.tool);
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
