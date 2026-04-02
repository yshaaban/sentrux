#!/usr/bin/env node

import { cp, mkdir, readFile, writeFile } from 'node:fs/promises';
import { existsSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { createDisposableRepoClone } from '../lib/disposable-repo.mjs';
import { createMcpSession, runTool } from '../lib/benchmark-harness.mjs';
import { prepareTypeScriptBenchmarkHome } from '../lib/benchmark-plugin-home.mjs';
import { collectRepoIdentity } from '../lib/repo-identity.mjs';
import {
  formatSessionTelemetrySummaryMarkdown,
  loadSessionTelemetrySummary,
} from '../lib/session-telemetry.mjs';
import { startCodexExec } from './providers/codex-cli.mjs';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, '../..');
const sentruxBin = process.env.SENTRUX_BIN ?? path.join(repoRoot, 'target/debug/sentrux');

function parseArgs(argv) {
  const result = {
    sourceRoot: process.cwd(),
    repoLabel: null,
    taskId: null,
    task: null,
    taskFile: null,
    taskLabel: null,
    tags: [],
    expectedSignalKinds: [],
    expectedFixSurface: null,
    rulesSource: null,
    analysisMode: 'working_tree',
    model: process.env.EVAL_MODEL ?? null,
    timeoutMs: Number(process.env.EVAL_TIMEOUT_MS ?? '1800000'),
    pollMs: Number(process.env.EVAL_POLL_MS ?? '4000'),
    outputDir: null,
    keepClone: false,
    codexBin: process.env.CODEX_BIN ?? 'codex',
  };

  for (let index = 2; index < argv.length; index += 1) {
    const value = argv[index];
    if (value === '--source-root') {
      index += 1;
      result.sourceRoot = argv[index];
      continue;
    }
    if (value === '--repo-label') {
      index += 1;
      result.repoLabel = argv[index];
      continue;
    }
    if (value === '--task') {
      index += 1;
      result.task = argv[index];
      continue;
    }
    if (value === '--task-id') {
      index += 1;
      result.taskId = argv[index];
      continue;
    }
    if (value === '--task-file') {
      index += 1;
      result.taskFile = argv[index];
      continue;
    }
    if (value === '--task-label') {
      index += 1;
      result.taskLabel = argv[index];
      continue;
    }
    if (value === '--tag') {
      index += 1;
      result.tags.push(argv[index]);
      continue;
    }
    if (value === '--expected-signal-kind') {
      index += 1;
      result.expectedSignalKinds.push(argv[index]);
      continue;
    }
    if (value === '--expected-fix-surface') {
      index += 1;
      result.expectedFixSurface = argv[index];
      continue;
    }
    if (value === '--rules-source') {
      index += 1;
      result.rulesSource = argv[index];
      continue;
    }
    if (value === '--analysis-mode') {
      index += 1;
      result.analysisMode = argv[index];
      continue;
    }
    if (value === '--model') {
      index += 1;
      result.model = argv[index];
      continue;
    }
    if (value === '--timeout-ms') {
      index += 1;
      result.timeoutMs = Number(argv[index]);
      continue;
    }
    if (value === '--poll-ms') {
      index += 1;
      result.pollMs = Number(argv[index]);
      continue;
    }
    if (value === '--output-dir') {
      index += 1;
      result.outputDir = argv[index];
      continue;
    }
    if (value === '--keep-clone') {
      result.keepClone = true;
      continue;
    }
    if (value === '--codex-bin') {
      index += 1;
      result.codexBin = argv[index];
      continue;
    }
    throw new Error(`Unknown argument: ${value}`);
  }

  if (!result.task && !result.taskFile) {
    throw new Error('Provide either --task or --task-file');
  }
  if (!Number.isFinite(result.timeoutMs) || result.timeoutMs <= 0) {
    throw new Error(`Invalid --timeout-ms value: ${result.timeoutMs}`);
  }
  if (!Number.isFinite(result.pollMs) || result.pollMs <= 0) {
    throw new Error(`Invalid --poll-ms value: ${result.pollMs}`);
  }

  return result;
}

function nowIso() {
  return new Date().toISOString();
}

function slugify(value) {
  return String(value ?? '')
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '')
    .slice(0, 48) || 'task';
}

function defaultRulesSource(sourceRoot) {
  const candidate = path.join(sourceRoot, '.sentrux', 'rules.toml');
  return existsSync(candidate) ? candidate : null;
}

function defaultOutputDir(sourceRoot, taskLabel) {
  const timestamp = new Date().toISOString().replace(/[:.]/g, '-');
  return path.join(sourceRoot, '.sentrux', 'evals', `${timestamp}-${slugify(taskLabel)}`);
}

function sleep(ms) {
  return new Promise((resolve) => {
    setTimeout(resolve, ms);
  });
}

async function loadPrompt(args) {
  if (args.task) {
    return args.task;
  }

  return readFile(args.taskFile, 'utf8');
}

function createSession(homeOverride) {
  return createMcpSession({
    binPath: sentruxBin,
    repoRoot,
    homeOverride,
    skipGrammarDownload: process.env.SENTRUX_SKIP_GRAMMAR_DOWNLOAD ?? '1',
    requestTimeoutMs: Number(process.env.REQUEST_TIMEOUT_MS ?? '120000'),
  });
}

async function maybeCopyFile(sourcePath, targetPath) {
  if (!sourcePath || !existsSync(sourcePath)) {
    return false;
  }

  await mkdir(path.dirname(targetPath), { recursive: true });
  await cp(sourcePath, targetPath);
  return true;
}

function buildSnapshot(label, scanResult, checkResult, repoIdentity) {
  return {
    label,
    recorded_at: nowIso(),
    repo_identity: repoIdentity,
    scan_elapsed_ms: scanResult.elapsed_ms,
    check_elapsed_ms: checkResult.elapsed_ms,
    gate: checkResult.payload.gate ?? null,
    changed_files: checkResult.payload.changed_files ?? [],
    action_kinds: (checkResult.payload.actions ?? []).map((action) => action.kind).filter(Boolean),
    top_action_kind: checkResult.payload.actions?.[0]?.kind ?? null,
    check: checkResult.payload,
  };
}

async function recordSnapshot(session, workRoot, label) {
  const scanResult = await runTool(session, 'scan', { path: workRoot });
  const checkResult = await runTool(session, 'check', {});
  const repoIdentity = collectRepoIdentity(workRoot);

  return buildSnapshot(label, scanResult, checkResult, repoIdentity);
}

function shouldCaptureSnapshot(previousSnapshot, nextIdentity) {
  if (!previousSnapshot) {
    return true;
  }

  const previousIdentity = previousSnapshot.repo_identity ?? null;
  if (!previousIdentity) {
    return true;
  }

  return (
    previousIdentity.dirty_paths_fingerprint !== nextIdentity.dirty_paths_fingerprint ||
    previousIdentity.dirty_paths_count !== nextIdentity.dirty_paths_count ||
    previousIdentity.tree_fingerprint !== nextIdentity.tree_fingerprint
  );
}

function summarizeOutcome(sessionTelemetry) {
  const lastSession = sessionTelemetry.sessions.at(-1) ?? null;

  return {
    session_count: sessionTelemetry.summary.session_count,
    initial_top_action_kind: lastSession?.initial_top_action_kind ?? null,
    top_action_cleared: lastSession?.top_action_cleared ?? false,
    checks_to_clear_top_action: lastSession?.checks_to_clear_top_action ?? null,
    followup_regression_introduced: lastSession?.followup_regression_introduced ?? false,
    final_gate: lastSession?.final_gate ?? null,
    final_session_clean: lastSession?.final_session_clean ?? false,
  };
}

export async function runCodexSession(options) {
  const args = {
    ...options,
    tags: [...(options.tags ?? [])],
    expectedSignalKinds: [...(options.expectedSignalKinds ?? [])],
  };
  const sourceRoot = path.resolve(args.sourceRoot);
  const repoLabel = args.repoLabel ?? path.basename(sourceRoot);
  const prompt = await loadPrompt(args);
  const taskLabel = args.taskLabel ?? prompt.split(/\r?\n/, 1)[0] ?? 'codex-session';
  const outputDir = path.resolve(args.outputDir ?? defaultOutputDir(sourceRoot, taskLabel));
  const promptPath = path.join(outputDir, 'task-prompt.txt');
  const bundlePath = path.join(outputDir, 'codex-session.json');
  const telemetryJsonPath = path.join(outputDir, 'session-telemetry-summary.json');
  const telemetryMarkdownPath = path.join(outputDir, 'session-telemetry-summary.md');
  const copiedTelemetryLogPath = path.join(outputDir, 'agent-session-events.jsonl');
  const rulesSource =
    args.rulesSource === null ? defaultRulesSource(sourceRoot) : path.resolve(args.rulesSource);
  const clone = await createDisposableRepoClone({
    sourceRoot,
    label: `codex-session-${slugify(repoLabel)}-${slugify(taskLabel)}`,
    rulesSource,
    analysisMode: args.analysisMode,
  });
  const pluginHome = await prepareTypeScriptBenchmarkHome({ tempRoot: clone.tempRoot });
  const session = createSession(pluginHome);
  const startedAt = nowIso();

  await mkdir(outputDir, { recursive: true });
  await writeFile(promptPath, prompt, 'utf8');

  try {
    await runTool(session, 'scan', { path: clone.workRoot });
    await runTool(session, 'session_start', {});

    const snapshots = [await recordSnapshot(session, clone.workRoot, 'initial')];
    let latestSnapshot = snapshots[0];
    const running = await startCodexExec({
      cwd: clone.workRoot,
      prompt,
      model: args.model,
      timeoutMs: args.timeoutMs,
      codexBin: args.codexBin,
    });

    while (!running.finished) {
      await sleep(args.pollMs);
      const identity = collectRepoIdentity(clone.workRoot);
      if (!shouldCaptureSnapshot(latestSnapshot, identity)) {
        continue;
      }

      const snapshot = await recordSnapshot(
        session,
        clone.workRoot,
        `poll-${snapshots.length}`,
      );
      snapshots.push(snapshot);
      latestSnapshot = snapshot;
    }

    const providerRun = await running.wait();
    const finalSnapshot = await recordSnapshot(session, clone.workRoot, 'final');
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
      task_id: args.taskId ?? slugify(taskLabel),
      source_root: sourceRoot,
      analyzed_root: clone.workRoot,
      analysis_mode: args.analysisMode,
      task_label: taskLabel,
      tags: args.tags,
      expected_signal_kinds: args.expectedSignalKinds,
      expected_fix_surface: args.expectedFixSurface ?? null,
      prompt_path: promptPath,
      started_at: startedAt,
      finished_at: nowIso(),
      provider_run: providerRun,
      initial_check: snapshots[0].check,
      snapshots,
      final_check: finalSnapshot.check,
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
      `Captured Codex session for ${repoLabel} with ${snapshots.length} check snapshot(s); final gate=${bundle.outcome.final_gate ?? 'unknown'}.`,
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
  await runCodexSession(args);
}

const invokedPath = process.argv[1] ? pathToFileURL(path.resolve(process.argv[1])).href : null;

if (invokedPath === import.meta.url) {
  main().catch((error) => {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  });
}
