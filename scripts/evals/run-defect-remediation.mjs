#!/usr/bin/env node

import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createDisposableRepoClone } from '../lib/disposable-repo.mjs';
import { createMcpSession, runCommand, runTool } from '../lib/benchmark-harness.mjs';
import { resolveWorkspaceRepoRoot } from '../lib/path-roots.mjs';
import { prepareTypeScriptBenchmarkHome } from '../lib/benchmark-plugin-home.mjs';
import { runClaudeCode } from './providers/claude-code.mjs';
import { runCodexExec } from './providers/codex-cli.mjs';
import { createDogfoodCatalog, createParallelCodeCatalog, selectDefects } from '../defect-injection/catalog.mjs';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, '../..');
const sentruxBin = process.env.SENTRUX_BIN ?? path.join(repoRoot, 'target/debug/sentrux');

function parseArgs(argv) {
  const result = {
    repo: 'parallel-code',
    defects: [],
    analysisMode: 'head_clone',
    dryRun: false,
    provider: process.env.EVAL_PROVIDER ?? 'claude-code',
    model: process.env.EVAL_MODEL ?? null,
    timeoutMs: Number(process.env.EVAL_TIMEOUT_MS ?? '1800000'),
    outputJsonPath: null,
  };

  for (let index = 2; index < argv.length; index += 1) {
    const value = argv[index];
    if (value === '--repo') {
      index += 1;
      result.repo = argv[index];
      continue;
    }
    if (value === '--defect') {
      index += 1;
      result.defects.push(argv[index]);
      continue;
    }
    if (value === '--analysis-mode') {
      index += 1;
      result.analysisMode = argv[index];
      continue;
    }
    if (value === '--dry-run') {
      result.dryRun = true;
      continue;
    }
    if (value === '--provider') {
      index += 1;
      result.provider = argv[index];
      continue;
    }
    if (value === '--output-json') {
      index += 1;
      result.outputJsonPath = argv[index];
      continue;
    }
    throw new Error(`Unknown argument: ${value}`);
  }

  return result;
}

function buildRepoConfig(repo) {
  const parallelCodeRoot = resolveWorkspaceRepoRoot(
    process.env.PARALLEL_CODE_ROOT,
    'parallel-code',
    repoRoot,
  );
  if (repo === 'self') {
    return {
      repoLabel: 'sentrux',
      sourceRoot: repoRoot,
      rulesSource: path.join(repoRoot, '.sentrux', 'rules.toml'),
      catalog: createDogfoodCatalog(),
    };
  }

  return {
    repoLabel: 'parallel-code',
    sourceRoot: parallelCodeRoot,
    rulesSource: path.join(repoRoot, 'docs/v2/examples/parallel-code.rules.toml'),
    catalog: createParallelCodeCatalog(),
  };
}

function summarizeRelevantIssues(defect, initialCheck) {
  const issues = Array.isArray(initialCheck?.issues) ? initialCheck.issues : [];
  const targetKinds = new Set(defect.check_support.kinds ?? []);
  const matchingIssues = issues.filter((issue) => targetKinds.has(issue.kind));
  const blockingIssues =
    initialCheck.gate === 'fail'
      ? issues.filter((issue) => issue.kind !== defect.signal_kind)
      : [];

  return {
    gate: initialCheck.gate ?? null,
    changed_files: initialCheck.changed_files ?? [],
    target_issues: matchingIssues,
    other_blockers: blockingIssues.slice(0, 3),
    summary: initialCheck.summary ?? null,
  };
}

function buildRemediationPrompt(defect, initialCheck) {
  const relevantIssues = summarizeRelevantIssues(defect, initialCheck);
  return [
    'You are fixing a seeded defect in a disposable repo clone.',
    `Defect id: ${defect.id}`,
    `Defect title: ${defect.title}`,
    `Primary signal kind: ${defect.signal_kind}`,
    `Target path: ${defect.target_path}`,
    '',
    'Use the current repository checkout to make the minimal fix.',
    'The goal is to remove the seeded target issue kind without introducing new regressions.',
    'Prefer fixing the issue directly instead of suppressing it.',
    'Prioritize the target issue kind over unrelated watchpoints or existing debt.',
    'Do not output prose. Just make the edits and exit.',
    '',
    'Targeted check context:',
    JSON.stringify(relevantIssues, null, 2),
  ].join('\n');
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

async function gitConfigValue(root, key) {
  const result = await runCommand('git', ['config', '--get', key], { cwd: root });
  if (result.exit_code !== 0) {
    return null;
  }

  const value = result.stdout.trim();
  return value || null;
}

async function commitPreparedFixture(workRoot, message) {
  const userName = (await gitConfigValue(repoRoot, 'user.name')) ?? 'Sentrux Eval';
  const userEmail =
    (await gitConfigValue(repoRoot, 'user.email')) ?? 'sentrux-eval@example.com';
  const commands = [
    ['config', 'user.name', userName],
    ['config', 'user.email', userEmail],
    ['add', '.'],
    ['commit', '--quiet', '-m', message],
  ];

  for (const args of commands) {
    const result = await runCommand('git', args, { cwd: workRoot });
    if (result.exit_code !== 0) {
      throw new Error(result.stderr.trim() || `git ${args.join(' ')} failed`);
    }
  }
}

function summarizeResultKinds(payload) {
  return (payload.issues ?? payload.findings ?? payload.introduced_findings ?? [])
    .map((entry) => entry.kind)
    .filter(Boolean);
}

function uniqueKinds(kinds) {
  return [...new Set(kinds)];
}

function diffKinds(nextKinds, previousKinds) {
  const previous = new Set(previousKinds);
  return uniqueKinds(nextKinds).filter((kind) => !previous.has(kind));
}

function providerRunSucceeded(providerRun, dryRun) {
  if (dryRun) {
    return true;
  }

  return providerRun?.exit_code === 0 && !providerRun?.timed_out;
}

async function runProvider(options) {
  if (options.provider === 'claude-code') {
    return runClaudeCode({
      cwd: options.cwd,
      prompt: options.prompt,
      model: options.model,
      timeoutMs: options.timeoutMs,
      tools: 'default',
    });
  }

  if (options.provider === 'codex-cli') {
    return runCodexExec({
      cwd: options.cwd,
      prompt: options.prompt,
      model: options.model,
      timeoutMs: options.timeoutMs,
    });
  }

  throw new Error(`Unsupported provider: ${options.provider}`);
}

function buildRemediationStatus({ dryRun, fixed, targetRemoved, regressionFree }) {
  if (dryRun) {
    return 'dry_run';
  }
  if (fixed) {
    return 'fixed';
  }
  if (targetRemoved && !regressionFree) {
    return 'fixed_with_regressions';
  }
  return 'not_fixed';
}

async function runRemediation(defect, repoConfig, options) {
  const clone = await createDisposableRepoClone({
    sourceRoot: repoConfig.sourceRoot,
    label: `defect-remediation-${repoConfig.repoLabel}-${defect.id}`,
    rulesSource: repoConfig.rulesSource,
    analysisMode: options.analysisMode,
  });
  const pluginHome = await prepareTypeScriptBenchmarkHome({ tempRoot: clone.tempRoot });
  const baselineSession = createSession(pluginHome);
  const repairSession = createSession(pluginHome);

  try {
    if (typeof defect.setup === 'function') {
      await defect.setup(clone.workRoot);
      if (typeof defect.setup_commit_message === 'string' && defect.setup_commit_message) {
        await commitPreparedFixture(clone.workRoot, defect.setup_commit_message);
      }
    }

    await runTool(baselineSession, 'scan', { path: clone.workRoot });
    await runTool(baselineSession, 'session_start', {});
    await defect.inject(clone.workRoot);
    await runTool(repairSession, 'scan', { path: clone.workRoot });
    const initialCheck = (await runTool(repairSession, 'check', {})).payload;

    let providerRun = null;
    if (!options.dryRun) {
      providerRun = await runProvider({
        provider: options.provider,
        cwd: clone.workRoot,
        prompt: buildRemediationPrompt(defect, initialCheck),
        model: options.model,
        timeoutMs: options.timeoutMs,
      });
    }

    await runTool(repairSession, 'scan', { path: clone.workRoot });
    const finalCheck = (await runTool(repairSession, 'check', {})).payload;
    const finalGate = (await runTool(repairSession, 'gate', {})).payload;
    const initialKinds = uniqueKinds(summarizeResultKinds(initialCheck));
    const finalKinds = uniqueKinds(summarizeResultKinds(finalCheck));
    const targetKinds = defect.check_support.kinds ?? [];
    const targetRemoved = defect.check_support.supported
      ? !targetKinds.some((kind) => finalKinds.includes(kind))
      : false;
    const introducedRegressions = diffKinds(finalKinds, initialKinds);
    const regressionFree = introducedRegressions.length === 0;
    const providerSucceeded = providerRunSucceeded(providerRun, options.dryRun);
    const fixed = targetRemoved && regressionFree && providerSucceeded;
    const status = buildRemediationStatus({
      dryRun: options.dryRun,
      fixed,
      targetRemoved,
      regressionFree,
    });

    return {
      defect_id: defect.id,
      signal_kind: defect.signal_kind,
      status,
      initial_gate: initialCheck.gate ?? null,
      final_gate: finalCheck.gate ?? finalGate.decision ?? null,
      initial_kinds: initialKinds,
      final_kinds: finalKinds,
      target_removed: targetRemoved,
      regression_free: regressionFree,
      introduced_regressions: introducedRegressions,
      fixed,
      provider_run: providerRun
        ? {
            provider: providerRun.provider,
            provider_version: providerRun.provider_version,
            duration_ms: providerRun.duration_ms,
            exit_code: providerRun.exit_code,
            timed_out: providerRun.timed_out,
            stdout: providerRun.stdout,
            stderr: providerRun.stderr,
            last_message: providerRun.last_message ?? null,
          }
        : null,
    };
  } finally {
    await baselineSession.close();
    await repairSession.close();
    await clone.cleanup();
  }
}

async function main() {
  const options = parseArgs(process.argv);
  const repoConfig = buildRepoConfig(options.repo);
  const supportedDefects = repoConfig.catalog.filter((defect) => defect.check_support.supported);
  const selectedDefects = selectDefects(
    supportedDefects,
    options.defects.length > 0 ? options.defects : null,
  );
  const results = [];
  for (const defect of selectedDefects) {
    results.push(await runRemediation(defect, repoConfig, options));
  }

  const summary = {
    total: results.length,
    fixed: results.filter((result) => result.fixed).length,
    dry_run: options.dryRun,
  };
  const report = buildReport(repoConfig.repoLabel, summary, results);
  await writeReportMaybe(options.outputJsonPath, report);

  console.log(JSON.stringify(report, null, 2));
}

function buildReport(repoLabel, summary, results) {
  return {
    schema_version: 1,
    generated_at: new Date().toISOString(),
    repo_label: repoLabel,
    summary,
    results,
  };
}

async function writeReportMaybe(outputJsonPath, report) {
  if (!outputJsonPath) {
    return;
  }
  await mkdir(path.dirname(outputJsonPath), { recursive: true });
  await writeFile(outputJsonPath, `${JSON.stringify(report, null, 2)}\n`, 'utf8');
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
});
