#!/usr/bin/env node

import { writeFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { createDisposableRepoClone } from '../lib/disposable-repo.mjs';
import { runCommand, runTool } from '../lib/benchmark-harness.mjs';
import { prepareTypeScriptBenchmarkHome } from '../lib/benchmark-plugin-home.mjs';
import { resolveWorkspaceRepoRoot } from '../lib/path-roots.mjs';
import {
  createEvalMcpSession,
  defaultRulesSource,
  maybeCopyFile,
  parseCliArgs,
} from '../lib/eval-support.mjs';
import {
  appendStringOption,
  defaultEvalOutputDir,
  resolveRepoLabel,
  setFlag,
  setStringOption,
} from '../lib/eval-cli-shared.mjs';
import { recordSessionSnapshot } from '../lib/eval-runtime/session-snapshot.mjs';
import { nowIso } from '../lib/eval-runtime/common.mjs';
import {
  createDogfoodCatalog,
  createParallelCodeCatalog,
  selectDefects,
} from '../defect-injection/catalog.mjs';
import {
  normalizeDefectPaths,
  prepareDefectFixture,
} from '../defect-injection/fixture-setup.mjs';
import {
  loadSessionTelemetrySummaryOrEmpty,
  summarizeOutcome,
  writeSessionTelemetryArtifacts,
} from '../lib/session-telemetry.mjs';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, '../..');
const sentruxBin = process.env.SENTRUX_BIN ?? path.join(repoRoot, 'target/debug/sentrux');
const parallelCodeRoot = resolveWorkspaceRepoRoot(
  process.env.PARALLEL_CODE_ROOT,
  'parallel-code',
  repoRoot,
);

function parseArgs(argv) {
  const result = {
    sourceRoot: process.cwd(),
    repoLabel: null,
    replayId: null,
    commit: null,
    baseCommit: null,
    defectId: null,
    fixtureRepo: 'self',
    tags: [],
    expectedSignalKinds: [],
    expectedFixSurface: null,
    rulesSource: null,
    outputDir: null,
    keepClone: false,
  };

  parseCliArgs(argv, result, {
    flags: {
      '--keep-clone': setFlag('keepClone'),
    },
    values: {
      '--source-root': setStringOption('sourceRoot'),
      '--repo-label': setStringOption('repoLabel'),
      '--commit': setStringOption('commit'),
      '--replay-id': setStringOption('replayId'),
      '--base': setStringOption('baseCommit'),
      '--defect-id': setStringOption('defectId'),
      '--fixture-repo': setStringOption('fixtureRepo'),
      '--tag': appendStringOption('tags'),
      '--expected-signal-kind': appendStringOption('expectedSignalKinds'),
      '--expected-fix-surface': setStringOption('expectedFixSurface'),
      '--rules-source': setStringOption('rulesSource'),
      '--output-dir': setStringOption('outputDir'),
    },
  });

  if (!result.commit && !result.defectId) {
    throw new Error('Missing required --commit or --defect-id');
  }

  return result;
}

function defaultOutputDir(sourceRoot, replayTarget) {
  return defaultEvalOutputDir(sourceRoot, 'replay', replayTarget);
}

async function gitRead(sourceRoot, args) {
  const result = await runCommand('git', args, { cwd: sourceRoot });
  if (result.exit_code !== 0) {
    throw new Error(result.stderr.trim() || `git ${args.join(' ')} failed`);
  }

  return result.stdout.trimEnd();
}

async function gitReadRaw(sourceRoot, args) {
  const result = await runCommand('git', args, { cwd: sourceRoot });
  if (result.exit_code !== 0) {
    throw new Error(result.stderr.trim() || `git ${args.join(' ')} failed`);
  }

  return result.stdout;
}

async function resolveBaseCommit(sourceRoot, commit, explicitBaseCommit) {
  if (explicitBaseCommit) {
    return explicitBaseCommit;
  }

  return gitRead(sourceRoot, ['rev-parse', `${commit}^1`]);
}

async function buildReplayInputs(sourceRoot, baseCommit, commit) {
  const patch = await gitReadRaw(sourceRoot, ['diff', '--binary', baseCommit, commit]);
  const changedFiles = await gitRead(sourceRoot, ['diff', '--name-only', baseCommit, commit]);
  const commitSubject = await gitRead(sourceRoot, ['show', '--quiet', '--format=%s', commit]);

  return {
    patch,
    changed_files: changedFiles ? changedFiles.split(/\r?\n/).filter(Boolean) : [],
    commit_subject: commitSubject || null,
  };
}

function resolveReplayCatalog(fixtureRepo) {
  if (fixtureRepo === 'parallel-code') {
    return createParallelCodeCatalog();
  }

  return createDogfoodCatalog();
}

function resolveReplayDefect(fixtureRepo, defectId) {
  const catalog = resolveReplayCatalog(fixtureRepo);
  const defects = selectDefects(catalog, [defectId]);
  if (defects.length !== 1) {
    throw new Error(`Unknown replay defect id: ${defectId}`);
  }

  return defects[0];
}

function resolveReplayTargetPath(workRoot, targetPath) {
  if (path.isAbsolute(targetPath)) {
    return targetPath;
  }

  return path.join(workRoot, targetPath);
}

function relativeReplayPaths(workRoot, targetPaths) {
  const relativePaths = targetPaths
    .map((targetPath) => path.relative(workRoot, resolveReplayTargetPath(workRoot, targetPath)))
    .filter(Boolean);
  return [...new Set(relativePaths)].sort();
}

function resolveReplaySourceRoot(args) {
  if (args.defectId && args.fixtureRepo === 'parallel-code') {
    return path.resolve(parallelCodeRoot);
  }

  return path.resolve(args.sourceRoot);
}

function buildReplayOutputPaths(outputDir) {
  return {
    bundlePath: path.join(outputDir, 'diff-replay.json'),
    telemetryJsonPath: path.join(outputDir, 'session-telemetry-summary.json'),
    telemetryMarkdownPath: path.join(outputDir, 'session-telemetry-summary.md'),
    copiedTelemetryLogPath: path.join(outputDir, 'agent-session-events.jsonl'),
  };
}

async function createReplayResources({ sourceRoot, repoLabel, replayTarget, args }) {
  const rulesSource =
    args.rulesSource === null ? defaultRulesSource(sourceRoot) : path.resolve(args.rulesSource);
  const clone = await createDisposableRepoClone({
    sourceRoot,
    label: `diff-replay-${slugify(repoLabel)}-${slugify(replayTarget)}`,
    rulesSource,
    analysisMode: 'head_clone',
  });
  const pluginHome = await prepareTypeScriptBenchmarkHome({ tempRoot: clone.tempRoot });
  const session = createEvalMcpSession({
    repoRoot,
    binPath: sentruxBin,
    homeOverride: pluginHome,
  });

  return { clone, session };
}

async function startReplaySession(session, workRoot) {
  await runTool(session, 'scan', { path: workRoot });
  await runTool(session, 'session_start', {});
  return recordSessionSnapshot(session, workRoot, 'initial');
}

async function prepareFixtureReplay(args, clone, session) {
  const defect = resolveReplayDefect(args.fixtureRepo, args.defectId);
  await prepareDefectFixture(defect, clone.workRoot, repoRoot);
  const initialSnapshot = await startReplaySession(session, clone.workRoot);
  const injectedPaths = normalizeDefectPaths(
    await defect.inject(clone.workRoot),
    'injected_paths',
  );
  const replaySnapshot = await recordSessionSnapshot(session, clone.workRoot, 'replay');
  const replayMetadata = {
    replay_type: 'defect_fixture',
    defect_id: defect.id,
    defect_title: defect.title,
    fixture_repo: args.fixtureRepo ?? 'self',
    changed_files: relativeReplayPaths(clone.workRoot, injectedPaths),
  };

  return {
    replayMetadata,
    initialSnapshot,
    replaySnapshot,
    changedFileCount: replayMetadata.changed_files.length,
  };
}

async function checkoutReplayBase(clone, baseCommit) {
  const checkoutResult = await runCommand('git', ['checkout', '--quiet', baseCommit], {
    cwd: clone.workRoot,
  });
  if (checkoutResult.exit_code !== 0) {
    throw new Error(checkoutResult.stderr.trim() || `git checkout ${baseCommit} failed`);
  }
}

async function applyReplayPatch(clone, replayInputs, commit) {
  const applyResult = await runCommand('git', ['apply', '--whitespace=nowarn', '-'], {
    cwd: clone.workRoot,
    input: replayInputs.patch,
  });
  if (applyResult.exit_code !== 0) {
    throw new Error(applyResult.stderr.trim() || `git apply for ${commit} failed`);
  }
}

async function prepareCommitReplay(args, sourceRoot, clone, session) {
  const baseCommit = await resolveBaseCommit(sourceRoot, args.commit, args.baseCommit);
  const replayInputs = await buildReplayInputs(sourceRoot, baseCommit, args.commit);
  await checkoutReplayBase(clone, baseCommit);
  const initialSnapshot = await startReplaySession(session, clone.workRoot);
  await applyReplayPatch(clone, replayInputs, args.commit);
  const replaySnapshot = await recordSessionSnapshot(session, clone.workRoot, 'replay');

  return {
    replayMetadata: {
      replay_type: 'commit',
      commit: args.commit,
      base_commit: baseCommit,
      commit_subject: replayInputs.commit_subject,
      changed_files: replayInputs.changed_files,
    },
    initialSnapshot,
    replaySnapshot,
    changedFileCount: replayInputs.changed_files.length,
  };
}

async function prepareReplayRun(args, sourceRoot, clone, session) {
  if (args.defectId) {
    return prepareFixtureReplay(args, clone, session);
  }

  return prepareCommitReplay(args, sourceRoot, clone, session);
}

async function loadReplaySessionTelemetry(sourceRoot, clone, copiedTelemetryLogPath) {
  const telemetryLogPath = path.join(clone.workRoot, '.sentrux', 'agent-session-events.jsonl');
  await maybeCopyFile(telemetryLogPath, copiedTelemetryLogPath);
  return loadSessionTelemetrySummaryOrEmpty(telemetryLogPath, {
    repoRoot: sourceRoot,
  });
}

function buildReplayBundle({
  args,
  replayTarget,
  sourceRoot,
  repoLabel,
  clone,
  replayMetadata,
  initialSnapshot,
  replaySnapshot,
  finalGate,
  sessionEnd,
  sessionTelemetry,
}) {
  return {
    schema_version: 1,
    generated_at: nowIso(),
    repo_label: repoLabel,
    replay_id: args.replayId ?? slugify(replayTarget),
    source_root: sourceRoot,
    analyzed_root: clone.workRoot,
    tags: args.tags,
    expected_signal_kinds: args.expectedSignalKinds,
    expected_fix_surface: args.expectedFixSurface ?? null,
    replay: replayMetadata,
    initial_check: initialSnapshot.check,
    snapshots: [initialSnapshot, replaySnapshot],
    final_check: replaySnapshot.check,
    final_gate: finalGate.payload,
    session_end: sessionEnd.payload,
    telemetry_summary: sessionTelemetry,
    outcome: summarizeOutcome(sessionTelemetry),
  };
}

async function writeReplayArtifacts(paths, bundle, sessionTelemetry) {
  await writeFile(paths.bundlePath, `${JSON.stringify(bundle, null, 2)}\n`, 'utf8');
  await writeSessionTelemetryArtifacts({
    telemetryJsonPath: paths.telemetryJsonPath,
    telemetryMarkdownPath: paths.telemetryMarkdownPath,
    summary: sessionTelemetry,
  });
}

export async function runDiffReplay(options) {
  const args = {
    ...options,
    tags: [...(options.tags ?? [])],
    expectedSignalKinds: [...(options.expectedSignalKinds ?? [])],
  };
  const replayTarget = args.commit ?? args.defectId;
  const sourceRoot = resolveReplaySourceRoot(args);
  const repoLabel = resolveRepoLabel(sourceRoot, args.repoLabel);
  const outputDir = path.resolve(
    args.outputDir ?? defaultOutputDir(sourceRoot, replayTarget),
  );
  const paths = buildReplayOutputPaths(outputDir);
  const { clone, session } = await createReplayResources({
    sourceRoot,
    repoLabel,
    replayTarget,
    args,
  });

  await mkdir(outputDir, { recursive: true });

  try {
    const { replayMetadata, initialSnapshot, replaySnapshot, changedFileCount } =
      await prepareReplayRun(args, sourceRoot, clone, session);
    const finalGate = await runTool(session, 'gate', {});
    const sessionEnd = await runTool(session, 'session_end', {});
    const sessionTelemetry = await loadReplaySessionTelemetry(
      sourceRoot,
      clone,
      paths.copiedTelemetryLogPath,
    );
    const bundle = buildReplayBundle({
      args,
      replayTarget,
      sourceRoot,
      repoLabel,
      clone,
      replayMetadata,
      initialSnapshot,
      replaySnapshot,
      finalGate,
      sessionEnd,
      sessionTelemetry,
    });
    await writeReplayArtifacts(paths, bundle, sessionTelemetry);

    console.log(
      `Replayed ${replayTarget} on ${repoLabel}; final gate=${bundle.outcome.final_gate ?? 'unknown'} with ${changedFileCount} changed file(s).`,
    );
    console.log(`Artifacts written to ${outputDir}`);
    return bundle;
  } finally {
    await session.close();
    if (!args.keepClone) {
      await clone.cleanup();
    }
  }
}

async function main() {
  const args = parseArgs(process.argv);
  await runDiffReplay(args);
}

const invokedPath = process.argv[1] ? pathToFileURL(path.resolve(process.argv[1])).href : null;

if (invokedPath === import.meta.url) {
  main().catch((error) => {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  });
}
