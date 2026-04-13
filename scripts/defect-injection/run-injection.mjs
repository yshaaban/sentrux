#!/usr/bin/env node
import { existsSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { createMcpSession, runCommand, runTool } from '../lib/benchmark-harness.mjs';
import { assertPathExists, createDisposableRepoClone } from '../lib/disposable-repo.mjs';
import { resolveWorkspaceRepoRoot } from '../lib/path-roots.mjs';
import { prepareTypeScriptBenchmarkHome } from '../lib/benchmark-plugin-home.mjs';
import {
  summarizeCheck,
  summarizeGate,
  summarizeFindings,
  summarizeScan,
  summarizeSessionEnd,
  summarizeSessionSave,
} from '../lib/benchmark-summaries.mjs';
import { createDogfoodCatalog, createParallelCodeCatalog, selectDefects } from './catalog.mjs';
import { assertDefectAssertion } from './assertion-engine.mjs';
import {
  buildInjectionReport,
  writeInjectionReportFiles,
} from './report.mjs';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, '../..');
const sentruxBin = process.env.SENTRUX_BIN ?? path.join(repoRoot, 'target/debug/sentrux');
const parallelCodeRoot = resolveWorkspaceRepoRoot(
  process.env.PARALLEL_CODE_ROOT,
  'parallel-code',
  repoRoot,
);
const outputJsonPath = process.env.OUTPUT_JSON_PATH ?? null;
const outputMarkdownPath = process.env.OUTPUT_MD_PATH ?? null;
const requestTimeoutMs = Number(process.env.REQUEST_TIMEOUT_MS ?? '120000');
const skipGrammarDownload = process.env.SENTRUX_SKIP_GRAMMAR_DOWNLOAD ?? '1';

function parseArgs(argv) {
  const result = {
    repo: 'parallel-code',
    defects: [],
    analysisMode: 'head_clone',
    rulesSource: null,
    outputJsonPath,
    outputMarkdownPath,
    help: false,
  };

  for (let index = 2; index < argv.length; index += 1) {
    const value = argv[index];
    if (value === '--help' || value === '-h') {
      result.help = true;
      continue;
    }
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
    if (value === '--rules-source') {
      index += 1;
      result.rulesSource = argv[index];
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

function printUsage() {
  console.log(
    [
      'Usage: node scripts/defect-injection/run-injection.mjs [--repo parallel-code|self] [--defect id]...',
      '       [--analysis-mode head_clone|working_tree] [--rules-source path] [--output-json path] [--output-md path]',
      '',
      'When no defects are selected, the full catalog for the chosen repo runs.',
    ].join('\n'),
  );
}

function buildRepoConfig(repo) {
  if (repo === 'self') {
    const selfRulesSource = path.join(repoRoot, '.sentrux', 'rules.toml');
    return {
      repoLabel: 'sentrux',
      sourceRoot: repoRoot,
      rulesSource: existsSync(selfRulesSource)
        ? selfRulesSource
        : path.join(repoRoot, 'docs/v2/examples/parallel-code.rules.toml'),
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

function createSession(homeOverride) {
  return createMcpSession({
    binPath: sentruxBin,
    repoRoot,
    homeOverride,
    skipGrammarDownload,
    requestTimeoutMs,
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

async function runDefect(workRoot, homeOverride, defect) {
  const baselineSession = createSession(homeOverride);
  const postSession = createSession(homeOverride);

  try {
    if (typeof defect.setup === 'function') {
      await defect.setup(workRoot);
      if (typeof defect.setup_commit_message === 'string' && defect.setup_commit_message) {
        await commitPreparedFixture(workRoot, defect.setup_commit_message);
      }
    }

    const baselineScan = await runTool(baselineSession, 'scan', { path: workRoot });
    const baselineSessionStart = await runTool(baselineSession, 'session_start', {});
    const injection = await defect.inject(workRoot);
    const postScan = await runTool(postSession, 'scan', { path: workRoot });
    const check = await runTool(postSession, 'check', {});
    const checkRules = await runTool(postSession, 'check_rules', {});
    const gate = await runTool(postSession, 'gate', {});
    const findings = await runTool(postSession, 'findings', { limit: 12 });
    const sessionEnd = await runTool(postSession, 'session_end', {});

    const artifacts = {
      check: check.payload,
      check_rules: checkRules.payload,
      gate: gate.payload,
      findings: findings.payload,
      session_end: sessionEnd.payload,
    };
    const assertion = assertDefectAssertion(defect, artifacts);

    return {
      defect_id: defect.id,
      title: defect.title,
      injected_paths: injection.injected_paths,
      baseline_scan: summarizeScan(baselineScan.payload),
      baseline_session_start: summarizeSessionSave(baselineSessionStart.payload),
      post_scan: summarizeScan(postScan.payload),
      check_summary: summarizeCheck(check.payload),
      check: assertion.check,
      check_rules: assertion.check_rules,
      gate: assertion.gate,
      gate_summary: summarizeGate(gate.payload),
      findings: assertion.findings,
      findings_summary: summarizeFindings(findings.payload),
      session_end: assertion.session_end,
      session_end_summary: summarizeSessionEnd(sessionEnd.payload),
      assertions: assertion,
      detected: assertion.detected,
      status: assertion.status,
    };
  } finally {
    await baselineSession.close();
    await postSession.close();
  }
}

export async function runDefectInjection(options) {
  const repoConfig = buildRepoConfig(options.repo);
  const selectedDefects = selectDefects(
    repoConfig.catalog,
    (options.defects ?? []).length > 0 ? options.defects : null,
  );
  if (selectedDefects.length === 0) {
    const requested = (options.defects ?? []).join(', ');
    throw new Error(
      requested
        ? `No defects matched the requested ids: ${requested}`
        : 'No defects available for the selected catalog',
    );
  }
  assertPathExists(sentruxBin, 'sentrux binary');
  assertPathExists(repoConfig.sourceRoot, `${repoConfig.repoLabel} repo`);
  assertPathExists(repoConfig.rulesSource, `${repoConfig.repoLabel} rules source`);

  const results = [];
  for (const defect of selectedDefects) {
    const clone = await createDisposableRepoClone({
      sourceRoot: repoConfig.sourceRoot,
      label: `defect-injection-${repoConfig.repoLabel}-${defect.id}`,
      rulesSource: options.rulesSource ?? repoConfig.rulesSource,
      analysisMode: options.analysisMode,
    });
    const pluginHome = await prepareTypeScriptBenchmarkHome({ tempRoot: clone.tempRoot });

    try {
      results.push(await runDefect(clone.workRoot, pluginHome, defect));
    } finally {
      await clone.cleanup();
    }
  }

  const report = buildInjectionReport({
    repoLabel: repoConfig.repoLabel,
    repoRoot: repoConfig.sourceRoot,
    generatedAt: new Date().toISOString(),
    defects: selectedDefects.map((defect) => ({
      id: defect.id,
      title: defect.title,
      target_path: defect.target_path,
      signal_kind: defect.signal_kind,
      signal_family: defect.signal_family,
      promotion_status: defect.promotion_status,
      blocking_intent: defect.blocking_intent,
      check_supported: defect.check_support.supported,
    })),
    results,
  });

  await writeInjectionReportFiles({
    report,
    jsonPath: options.outputJsonPath,
    markdownPath: options.outputMarkdownPath,
  });

  return report;
}

async function main() {
  const options = parseArgs(process.argv);
  if (options.help) {
    printUsage();
    return;
  }

  const report = await runDefectInjection(options);
  console.log(
    `Ran ${report.summary.total} defect(s) for ${report.repo_label}: ${report.summary.detected} detected, ${report.summary.partial} partial, ${report.summary.failed} failed.`,
  );
  if (!options.outputJsonPath && !options.outputMarkdownPath) {
    console.log(JSON.stringify(report, null, 2));
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main().catch((error) => {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  });
}
