import { mkdir, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';

function ensureTrailingNewline(text) {
  return text.endsWith('\n') ? text : `${text}\n`;
}

function buildCommentBlock(label, lineCount) {
  const lines = [`// defect-injection: ${label}`];
  for (let index = 0; index < lineCount; index += 1) {
    lines.push(`// defect-injection filler ${index + 1}`);
  }
  return `${lines.join('\n')}\n`;
}

async function appendToFile(workRoot, relativePath, text) {
  const targetPath = path.join(workRoot, relativePath);
  const currentText = await readFile(targetPath, 'utf8');
  await mkdir(path.dirname(targetPath), { recursive: true });
  await writeFile(targetPath, `${ensureTrailingNewline(currentText)}${text}`, 'utf8');
  return targetPath;
}

async function replaceInFile(workRoot, relativePath, matcher, replacement) {
  const targetPath = path.join(workRoot, relativePath);
  const currentText = await readFile(targetPath, 'utf8');
  const nextText = currentText.replace(matcher, replacement);
  if (nextText === currentText) {
    throw new Error(`Failed to apply defect patch for ${relativePath}`);
  }
  await mkdir(path.dirname(targetPath), { recursive: true });
  await writeFile(targetPath, nextText, 'utf8');
  return targetPath;
}

async function appendToFiles(workRoot, patches) {
  const injectedPaths = [];
  for (const patch of patches) {
    injectedPaths.push(await appendToFile(workRoot, patch.path, patch.text));
  }
  return injectedPaths;
}

function buildDuplicateNetworkHelpers() {
  return [
    '',
    'function buildInjectionAccessUrl(host: string, port: number, token: string): string {',
    '  return `http://${host}:${port}?token=${token}`;',
    '}',
    '',
    'function buildInjectionOptionalAccessUrl(',
    '  host: string | null,',
    '  port: number,',
    '  token: string,',
    '): string | null {',
    '  if (!host) return null;',
    '  return buildInjectionAccessUrl(host, port, token);',
    '}',
    '',
  ].join('\n');
}

function buildDuplicateTextLineHelpers() {
  return [
    '',
    'function roundMsCopy(value: number): number {',
    '  return Number(value.toFixed(1));',
    '}',
    '',
    'function countTextLinesCopy(value: string): number {',
    '  if (value.length === 0) {',
    '    return 0;',
    '  }',
    '',
    '  return value.split(/\\r?\\n/).filter((line) => line.length > 0).length;',
    '}',
    '',
  ].join('\n');
}

function buildForbiddenRawReadSnippet() {
  return [
    '',
    'function defectInjectionReadTaskGitStatus() {',
    '  return store.taskGitStatus;',
    '}',
    '',
  ].join('\n');
}

function buildMissingTestSource() {
  return [
    'export function summarizeTaskHealth(samples) {',
    '  return samples.filter(Boolean).length;',
    '}',
    '',
  ].join('\n');
}

function buildCycleImportSnippet(importPath, importedName) {
  return [
    '',
    `import { ${importedName} } from '${importPath}';`,
    '',
  ].join('\n');
}

function buildRendererBoundaryViolationSnippet() {
  return [
    '',
    'use crate::analysis::scanner;',
    '',
    'const _: fn(&str) -> bool = scanner::common::is_probably_generated_path;',
    '',
  ].join('\n');
}

function createDefect({
  id,
  title,
  repoLabel,
  targetPath,
  inject,
  signalKind,
  signalFamily,
  promotionStatus = 'trusted',
  blockingIntent = 'blocking',
  checkSupport = {
    supported: false,
    reason: 'The current fast check path does not guarantee this signal.',
  },
  checkRulesKinds = [],
  gateKinds = [],
  findingKinds = [],
  sessionEndKinds = [],
  expectedGateDecision = 'warn',
}) {
  return {
    id,
    title,
    repo_label: repoLabel,
    target_path: targetPath,
    signal_kind: signalKind,
    signal_family: signalFamily,
    promotion_status: promotionStatus,
    blocking_intent: blockingIntent,
    check_support: checkSupport,
    expected_check_rules_kinds: checkRulesKinds,
    expected_gate_decision: expectedGateDecision,
    expected_gate_kinds: gateKinds,
    expected_finding_kinds: findingKinds,
    expected_session_end_kinds: sessionEndKinds,
    async inject(workRoot) {
      const injectedPaths = await inject(workRoot);
      return {
        injected_paths: Array.isArray(injectedPaths) ? injectedPaths : [injectedPaths],
      };
    },
  };
}

function buildParallelCodeCatalog() {
  return [
    createDefect({
      id: 'large_file_growth',
      title: 'Append 120 lines to SidebarTaskRow.tsx',
      repoLabel: 'parallel-code',
      targetPath: 'src/components/SidebarTaskRow.tsx',
      signalKind: 'large_file',
      signalFamily: 'structural',
      blockingIntent: 'watchpoint',
      checkSupport: {
        supported: true,
        kinds: ['large_file'],
      },
      gateKinds: ['large_file'],
      findingKinds: ['large_file'],
      sessionEndKinds: ['large_file'],
      expectedGateDecision: null,
      async inject(workRoot) {
        return appendToFile(
          workRoot,
          'src/components/SidebarTaskRow.tsx',
          buildCommentBlock('parallel-code large-file growth', 120),
        );
      },
    }),
    createDefect({
      id: 'forbidden_raw_read',
      title: 'Read task status directly from SidebarTaskRow.tsx',
      repoLabel: 'parallel-code',
      targetPath: 'src/components/SidebarTaskRow.tsx',
      signalKind: 'forbidden_raw_read',
      signalFamily: 'rules',
      checkSupport: {
        supported: true,
        gate: 'fail',
        kinds: ['forbidden_raw_read'],
      },
      gateKinds: ['forbidden_raw_read'],
      findingKinds: ['forbidden_raw_read'],
      sessionEndKinds: ['forbidden_raw_read'],
      async inject(workRoot) {
        return appendToFile(
          workRoot,
          'src/components/SidebarTaskRow.tsx',
          buildForbiddenRawReadSnippet(),
        );
      },
    }),
    createDefect({
      id: 'clone_injection',
      title: 'Duplicate the network access helper into the remote HTTP handler',
      repoLabel: 'parallel-code',
      targetPath: 'electron/remote/http-handler.ts',
      signalKind: 'exact_clone_group',
      signalFamily: 'clone',
      blockingIntent: 'full_path_only',
      promotionStatus: 'watchpoint',
      checkSupport: {
        supported: false,
        reason: 'Clone findings are not guaranteed on the check fast path.',
      },
      gateKinds: ['clone_family', 'exact_clone_group'],
      findingKinds: ['clone_family', 'exact_clone_group'],
      sessionEndKinds: ['clone_family', 'exact_clone_group'],
      async inject(workRoot) {
        return appendToFile(
          workRoot,
          'electron/remote/http-handler.ts',
          buildDuplicateNetworkHelpers(),
        );
      },
    }),
    createDefect({
      id: 'session_introduced_clone',
      title: 'Introduce a fresh duplicate helper into the remote HTTP handler',
      repoLabel: 'parallel-code',
      targetPath: 'electron/remote/http-handler.ts',
      signalKind: 'session_introduced_clone',
      signalFamily: 'clone',
      blockingIntent: 'watchpoint',
      checkSupport: {
        supported: true,
        gate: 'pass',
        kinds: ['session_introduced_clone'],
      },
      gateKinds: ['session_introduced_clone'],
      findingKinds: ['exact_clone_group'],
      sessionEndKinds: ['session_introduced_clone'],
      expectedGateDecision: 'pass',
      async inject(workRoot) {
        return appendToFile(
          workRoot,
          'electron/remote/http-handler.ts',
          buildDuplicateNetworkHelpers(),
        );
      },
    }),
    createDefect({
      id: 'missing_exhaustiveness',
      title: 'Add a TaskDotStatus variant without updating consumers',
      repoLabel: 'parallel-code',
      targetPath: 'src/app/task-presentation-status.ts',
      signalKind: 'closed_domain_exhaustiveness',
      signalFamily: 'obligation',
      checkSupport: {
        supported: true,
        gate: 'fail',
        kinds: ['closed_domain_exhaustiveness'],
      },
      gateKinds: ['closed_domain_exhaustiveness'],
      findingKinds: ['closed_domain_exhaustiveness'],
      sessionEndKinds: ['closed_domain_exhaustiveness'],
      async inject(workRoot) {
        return replaceInFile(
          workRoot,
          'src/app/task-presentation-status.ts',
          "| 'failed';",
          "| 'failed'\n  | 'attention-needed';",
        );
      },
    }),
    createDefect({
      id: 'missing_test',
      title: 'Add a new production helper without a sibling test',
      repoLabel: 'parallel-code',
      targetPath: 'src/app/task-health-monitor.ts',
      signalKind: 'missing_test_coverage',
      signalFamily: 'structural',
      promotionStatus: 'watchpoint',
      blockingIntent: 'watchpoint',
      checkSupport: {
        supported: true,
        gate: 'pass',
        kinds: ['missing_test_coverage'],
      },
      gateKinds: [],
      findingKinds: ['missing_test_coverage'],
      sessionEndKinds: [],
      expectedGateDecision: 'pass',
      async inject(workRoot) {
        const targetPath = path.join(workRoot, 'src/app/task-health-monitor.ts');
        await mkdir(path.dirname(targetPath), { recursive: true });
        await writeFile(targetPath, buildMissingTestSource(), 'utf8');
        return targetPath;
      },
    }),
  ];
}

function buildDogfoodCatalog() {
  return [
    createDefect({
      id: 'self_large_file',
      title: 'Append 120 lines to the benchmark harness',
      repoLabel: 'sentrux',
      targetPath: 'scripts/lib/benchmark-harness.mjs',
      signalKind: 'large_file',
      signalFamily: 'structural',
      blockingIntent: 'watchpoint',
      checkSupport: {
        supported: true,
        gate: 'warn',
        kinds: ['large_file'],
      },
      gateKinds: ['large_file'],
      findingKinds: ['large_file'],
      sessionEndKinds: ['large_file'],
      async inject(workRoot) {
        return appendToFile(
          workRoot,
          'scripts/lib/benchmark-harness.mjs',
          buildCommentBlock('self large-file growth', 120),
        );
      },
    }),
    createDefect({
      id: 'self_cycle_introduction',
      title: 'Introduce a dependency cycle between benchmark support modules',
      repoLabel: 'sentrux',
      targetPath: 'scripts/lib/benchmark-harness.mjs',
      signalKind: 'cycle_cluster',
      signalFamily: 'structural',
      promotionStatus: 'watchpoint',
      blockingIntent: 'watchpoint',
      gateKinds: ['cycle_cluster'],
      findingKinds: ['cycle_cluster'],
      sessionEndKinds: ['cycle_cluster'],
      async inject(workRoot) {
        return appendToFiles(workRoot, [
          {
            path: 'scripts/lib/benchmark-harness.mjs',
            text: buildCycleImportSnippet('./benchmark-summaries.mjs', 'summarizeCheck'),
          },
          {
            path: 'scripts/lib/benchmark-summaries.mjs',
            text: buildCycleImportSnippet('./benchmark-harness.mjs', 'buildBenchmarkPolicy'),
          },
        ]);
      },
    }),
    createDefect({
      id: 'self_boundary_violation',
      title: 'Introduce a renderer-to-analysis boundary shortcut',
      repoLabel: 'sentrux',
      targetPath: 'sentrux-core/src/renderer/mod.rs',
      signalKind: 'check_rules',
      signalFamily: 'rules',
      checkSupport: {
        supported: false,
        reason: 'Explicit boundary rules currently surface through check_rules, not fast-path check.',
      },
      checkRulesKinds: ['boundary'],
      async inject(workRoot) {
        return appendToFile(
          workRoot,
          'sentrux-core/src/renderer/mod.rs',
          buildRendererBoundaryViolationSnippet(),
        );
      },
    }),
  ];
}

export function createParallelCodeCatalog() {
  return buildParallelCodeCatalog();
}

export function createDogfoodCatalog() {
  return buildDogfoodCatalog();
}

export function selectDefects(catalog, defectIds) {
  if (!Array.isArray(defectIds) || defectIds.length === 0) {
    return catalog;
  }

  const selected = new Set(defectIds);
  return catalog.filter((defect) => selected.has(defect.id));
}
