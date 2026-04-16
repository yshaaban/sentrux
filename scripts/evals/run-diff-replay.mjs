#!/usr/bin/env node

import { writeFile } from 'node:fs/promises';
import { existsSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { createDisposableRepoClone } from '../lib/disposable-repo.mjs';
import { runCommand, runTool } from '../lib/benchmark-harness.mjs';
import { prepareTypeScriptBenchmarkHome } from '../lib/benchmark-plugin-home.mjs';
import { resolveWorkspaceRepoRoot } from '../lib/path-roots.mjs';
import {
  createEvalMcpSession,
  defaultOutputDir as buildDefaultOutputDir,
  defaultRulesSource,
  maybeCopyFile,
  parseCliArgs,
} from '../lib/eval-support.mjs';
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
  formatSessionTelemetrySummaryMarkdown,
  loadSessionTelemetrySummary,
  summarizeOutcome,
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
      '--keep-clone': function enableKeepClone(target) {
        target.keepClone = true;
      },
    },
    values: {
      '--source-root': function setSourceRoot(target, value) {
        target.sourceRoot = value;
      },
      '--repo-label': function setRepoLabel(target, value) {
        target.repoLabel = value;
      },
      '--commit': function setCommit(target, value) {
        target.commit = value;
      },
      '--replay-id': function setReplayId(target, value) {
        target.replayId = value;
      },
      '--base': function setBaseCommit(target, value) {
        target.baseCommit = value;
      },
      '--defect-id': function setDefectId(target, value) {
        target.defectId = value;
      },
      '--fixture-repo': function setFixtureRepo(target, value) {
        target.fixtureRepo = value;
      },
      '--tag': function appendTag(target, value) {
        target.tags.push(value);
      },
      '--expected-signal-kind': function appendExpectedSignalKind(target, value) {
        target.expectedSignalKinds.push(value);
      },
      '--expected-fix-surface': function setExpectedFixSurface(target, value) {
        target.expectedFixSurface = value;
      },
      '--rules-source': function setRulesSource(target, value) {
        target.rulesSource = value;
      },
      '--output-dir': function setOutputDir(target, value) {
        target.outputDir = value;
      },
    },
  });

  if (!result.commit && !result.defectId) {
    throw new Error('Missing required --commit or --defect-id');
  }

  return result;
}

function nowIso() {
  return new Date().toISOString();
}

function defaultOutputDir(sourceRoot, replayTarget) {
  return buildDefaultOutputDir(sourceRoot, 'replay', replayTarget);
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

async function recordSnapshot(session, workRoot, label) {
  const scanResult = await runTool(session, 'scan', { path: workRoot });
  const checkResult = await runTool(session, 'check', {});

  return {
    label,
    recorded_at: nowIso(),
    scan_elapsed_ms: scanResult.elapsed_ms,
    check_elapsed_ms: checkResult.elapsed_ms,
    gate: checkResult.payload.gate ?? null,
    changed_files: checkResult.payload.changed_files ?? [],
    top_action_kind: checkResult.payload.actions?.[0]?.kind ?? null,
    action_kinds: (checkResult.payload.actions ?? []).map((action) => action.kind).filter(Boolean),
    check: checkResult.payload,
  };
}

export async function runDiffReplay(options) {
  const args = {
    ...options,
    tags: [...(options.tags ?? [])],
    expectedSignalKinds: [...(options.expectedSignalKinds ?? [])],
  };
  const replayTarget = args.commit ?? args.defectId;
  const sourceRoot = resolveReplaySourceRoot(args);
  const repoLabel = args.repoLabel ?? path.basename(sourceRoot);
  const outputDir = path.resolve(
    args.outputDir ?? defaultOutputDir(sourceRoot, replayTarget),
  );
  const bundlePath = path.join(outputDir, 'diff-replay.json');
  const telemetryJsonPath = path.join(outputDir, 'session-telemetry-summary.json');
  const telemetryMarkdownPath = path.join(outputDir, 'session-telemetry-summary.md');
  const copiedTelemetryLogPath = path.join(outputDir, 'agent-session-events.jsonl');
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

  await mkdir(outputDir, { recursive: true });

  try {
    let replayMetadata = null;
    let initialSnapshot = null;
    let replaySnapshot = null;
    let changedFileCount = 0;

    if (args.defectId) {
      const defect = resolveReplayDefect(args.fixtureRepo, args.defectId);
      await prepareDefectFixture(defect, clone.workRoot, repoRoot);
      await runTool(session, 'scan', { path: clone.workRoot });
      await runTool(session, 'session_start', {});
      initialSnapshot = await recordSnapshot(session, clone.workRoot, 'initial');
      const injectedPaths = normalizeDefectPaths(
        await defect.inject(clone.workRoot),
        'injected_paths',
      );
      replaySnapshot = await recordSnapshot(session, clone.workRoot, 'replay');
      replayMetadata = {
        replay_type: 'defect_fixture',
        defect_id: defect.id,
        defect_title: defect.title,
        fixture_repo: args.fixtureRepo ?? 'self',
        changed_files: relativeReplayPaths(clone.workRoot, injectedPaths),
      };
      changedFileCount = replayMetadata.changed_files.length;
    } else {
      const baseCommit = await resolveBaseCommit(sourceRoot, args.commit, args.baseCommit);
      const replayInputs = await buildReplayInputs(sourceRoot, baseCommit, args.commit);
      const checkoutResult = await runCommand(
        'git',
        ['checkout', '--quiet', baseCommit],
        { cwd: clone.workRoot },
      );
      if (checkoutResult.exit_code !== 0) {
        throw new Error(checkoutResult.stderr.trim() || `git checkout ${baseCommit} failed`);
      }

      await runTool(session, 'scan', { path: clone.workRoot });
      await runTool(session, 'session_start', {});
      initialSnapshot = await recordSnapshot(session, clone.workRoot, 'initial');

      const applyResult = await runCommand(
        'git',
        ['apply', '--whitespace=nowarn', '-'],
        { cwd: clone.workRoot, input: replayInputs.patch },
      );
      if (applyResult.exit_code !== 0) {
        throw new Error(applyResult.stderr.trim() || `git apply for ${args.commit} failed`);
      }

      replaySnapshot = await recordSnapshot(session, clone.workRoot, 'replay');
      replayMetadata = {
        replay_type: 'commit',
        commit: args.commit,
        base_commit: baseCommit,
        commit_subject: replayInputs.commit_subject,
        changed_files: replayInputs.changed_files,
      };
      changedFileCount = replayInputs.changed_files.length;
    }

    const finalGate = await runTool(session, 'gate', {});
    const sessionEnd = await runTool(session, 'session_end', {});
    const telemetryLogPath = path.join(clone.workRoot, '.sentrux', 'agent-session-events.jsonl');

    await maybeCopyFile(telemetryLogPath, copiedTelemetryLogPath);
    const sessionTelemetry = existsSync(telemetryLogPath)
      ? await loadSessionTelemetrySummary(telemetryLogPath, {
          repoRoot: sourceRoot,
        })
      : {
          schema_version: 1,
          generated_at: nowIso(),
          repo_root: sourceRoot,
          source_path: null,
          summary: {
            event_count: 0,
            session_count: 0,
            explicit_session_count: 0,
            implicit_session_count: 0,
            check_run_count: 0,
          },
          sessions: [],
          signals: [],
        };
    const bundle = {
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

    await writeFile(bundlePath, `${JSON.stringify(bundle, null, 2)}\n`, 'utf8');
    await writeFile(telemetryJsonPath, `${JSON.stringify(sessionTelemetry, null, 2)}\n`, 'utf8');
    await writeFile(
      telemetryMarkdownPath,
      formatSessionTelemetrySummaryMarkdown(sessionTelemetry),
      'utf8',
    );

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
