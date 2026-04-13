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

async function writeTextFile(workRoot, relativePath, text) {
  const targetPath = path.join(workRoot, relativePath);
  await mkdir(path.dirname(targetPath), { recursive: true });
  await writeFile(targetPath, text, 'utf8');
  return targetPath;
}

async function writeFiles(workRoot, patches) {
  const injectedPaths = [];
  for (const patch of patches) {
    injectedPaths.push(await writeTextFile(workRoot, patch.path, patch.text));
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

function buildRemoteKillHandler(commandName, commandKind = null) {
  const commandKindSegment = commandKind ? `'${commandKind}', ` : '';
  return [
    '      kill: (currentMessage) => {',
    `        ${commandName}(client, currentMessage.agentId, ${commandKindSegment}() => {`,
    '          killAgent(currentMessage.agentId);',
    '        });',
    '      },',
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

async function injectTaskGitStatusOwnershipRegression(workRoot) {
  const relativePath = 'src/app/task-presentation-status.ts';
  await replaceInFile(
    workRoot,
    relativePath,
    "import { store } from '../store/state';\n",
    "import { setStore, store } from '../store/state';\n",
  );

  return replaceInFile(
    workRoot,
    relativePath,
    '  const gitStatus = store.taskGitStatus[taskId];\n',
    "  const gitStatus = store.taskGitStatus[taskId];\n  setStore('taskGitStatus', taskId, gitStatus);\n",
  );
}

function buildServerStateBootstrapPayloadMapLines() {
  return [
    "  'git-status': GitStatusSyncSnapshotEvent[];",
    "  'remote-status': RemoteAccessStatus;",
    "  'peer-presence': PeerPresenceSnapshot[];",
    "  'task-command-controller': TaskCommandControllerSnapshot[];",
    "  'agent-supervision': AgentSupervisionSnapshot[];",
    "  'task-convergence': TaskConvergenceSnapshot[];",
    "  'task-review': TaskReviewSnapshot[];",
    "  'task-ports': TaskPortSnapshot[];",
    "  'session-diagnostics': RemoteAccessStatus;",
  ];
}

function buildServerStateEventPayloadMapLines() {
  return [
    "  'git-status': GitStatusSyncEvent;",
    "  'remote-status': RemoteAccessStatus;",
    "  'peer-presence': PeerPresenceSnapshot[];",
    "  'task-command-controller': TaskCommandControllerSnapshot;",
    "  'agent-supervision': AgentSupervisionEvent;",
    "  'task-convergence': TaskConvergenceEvent;",
    "  'task-review': TaskReviewEvent;",
    "  'task-ports': TaskPortsEvent;",
    "  'session-diagnostics': RemoteAccessStatus;",
  ];
}

function buildIncompletePropagationCategoryPatch() {
  return {
    matcher:
      /  'task-ports',\n\] as const;\n[\s\S]*?export interface ServerStateBootstrapPayloadMap \{\n([\s\S]*?)\n\}\n\nexport interface ServerStateEventPayloadMap \{\n([\s\S]*?)\n\}/m,
    replacement: [
      "  'task-ports',",
      "  'session-diagnostics',",
      '] as const;',
      '',
      'export type ServerStateBootstrapCategory = (typeof SERVER_STATE_BOOTSTRAP_CATEGORIES)[number];',
      '',
      'export interface ServerStateBootstrapPayloadMap {',
      ...buildServerStateBootstrapPayloadMapLines(),
      '}',
      '',
      'export interface ServerStateEventPayloadMap {',
      ...buildServerStateEventPayloadMapLines(),
      '}',
    ].join('\n'),
  };
}

function buildSelfTypeScriptFixturePackageJson(name) {
  return [
    '{',
    `  "name": "${name}",`,
    '  "type": "module"',
    '}',
  ].join('\n');
}

function buildSelfTypeScriptFixtureTsconfig() {
  return [
    '{',
    '  "compilerOptions": {',
    '    "module": "esnext",',
    '    "target": "es2020",',
    '    "strict": true',
    '  },',
    '  "include": ["src/**/*.ts", "src/**/*.tsx"]',
    '}',
  ].join('\n');
}

function buildSelfForbiddenRawReadRules() {
  return [
    '',
    '[[concept]]',
    'id = "task_presentation_status"',
    'kind = "projection"',
    'anchors = ["src/app/task-presentation-status.ts::getTaskDotStatus"]',
    'authoritative_inputs = ["src/store/core.ts::store.taskGitStatus"]',
    'canonical_accessors = [',
    '  "src/app/task-presentation-status.ts::getTaskDotStatus",',
    '  "src/app/task-presentation-status.ts::getTaskDotStatusLabel",',
    ']',
    'forbid_raw_reads = ["src/components/**::store.taskGitStatus"]',
    '',
  ].join('\n');
}

function buildSelfTaskStatusStoreSource() {
  return ['export const store = { taskGitStatus: "idle" };', ''].join('\n');
}

function buildSelfTaskPresentationStatusSource() {
  return [
    "import { store } from '../store/core';",
    '',
    'export function getTaskDotStatus(): string {',
    '  return store.taskGitStatus;',
    '}',
    '',
    'export function getTaskDotStatusLabel(): string {',
    '  return getTaskDotStatus();',
    '}',
    '',
  ].join('\n');
}

function buildSelfSidebarTaskRowSource() {
  return [
    "import { store } from '../store/core';",
    '',
    'export function SidebarTaskRow(): string {',
    '  return store.taskGitStatus;',
    '}',
    '',
  ].join('\n');
}

function buildSelfIncompletePropagationRules() {
  return [
    '',
    '[[contract]]',
    'id = "defect_injection_toolchain"',
    'categories_symbol = "scripts/defect-injection/catalog.mjs::createDogfoodCatalog"',
    'registry_symbol = "scripts/defect-injection/run-injection.mjs::runDefectInjection"',
    'required_symbols = ["scripts/defect-injection/report.mjs::buildInjectionReport"]',
    'required_files = ["scripts/tests/defect-injection.test.mjs"]',
    '',
  ].join('\n');
}

function buildSelfSessionCloneSource() {
  return [
    'export function buildAccessUrl(host: string, port: number, token: string): string {',
    '  return `http://${host}:${port}?token=${token}`;',
    '}',
    '',
    'export function buildOptionalAccessUrl(',
    '  host: string | null,',
    '  port: number,',
    '  token: string,',
    '): string | null {',
    '  if (!host) return null;',
    '  return buildAccessUrl(host, port, token);',
    '}',
    '',
  ].join('\n');
}

function buildSelfClonePropagationDriftSource() {
  return [
    'export function buildStatusBadge(status: string, isStale: boolean): string {',
    "  const label = status === 'done' ? 'done' : 'todo';",
    "  const staleSuffix = isStale ? ' stale' : '';",
    '  return `${label.toUpperCase()}${staleSuffix}`;',
    '}',
    '',
  ].join('\n');
}

function buildSelfClonePropagationBaselineSource() {
  return [
    'export function buildStatusBadge(status: string, isStale: boolean): string {',
    "  const label = status === 'done' ? 'done' : 'todo';",
    "  const staleSuffix = isStale ? ' stale' : '';",
    '  return `${label}${staleSuffix}`;',
    '}',
    '',
  ].join('\n');
}

function buildSelfSessionCloneDistractor() {
  return [
    'export function buildTaskLabel(status: string): string {',
    "  return status === 'done' ? 'done' : 'todo';",
    '}',
    '',
  ].join('\n');
}

function buildSelfTaskStatusModuleIndexSource() {
  return [
    "export { formatTaskHealth } from './internal';",
    '',
  ].join('\n');
}

function buildSelfTaskStatusModuleInternalSource() {
  return [
    'export function formatTaskHealth(status: string): string {',
    "  return status === 'ready' ? 'ready' : 'waiting';",
    '}',
    '',
  ].join('\n');
}

function buildSelfTaskDashboardSource() {
  return [
    "import { formatTaskHealth } from '../modules/task-status/internal';",
    '',
    'export function renderTaskDashboard(): string {',
    "  return formatTaskHealth('ready');",
    '}',
    '',
  ].join('\n');
}

function createDefect({
  id,
  title,
  repoLabel,
  targetPath,
  setup = null,
  setupCommitMessage = null,
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
    setup_commit_message: setupCommitMessage,
    ...(setup
      ? {
          async setup(workRoot) {
            const preparedPaths = await setup(workRoot);
            return {
              prepared_paths: Array.isArray(preparedPaths) ? preparedPaths : [preparedPaths],
            };
          },
        }
      : {}),
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
      id: 'clone_propagation_drift',
      title: 'Change one member of a committed websocket clone pair without syncing its sibling',
      repoLabel: 'parallel-code',
      targetPath: 'electron/remote/ws-server.ts',
      signalKind: 'clone_propagation_drift',
      signalFamily: 'clone',
      blockingIntent: 'watchpoint',
      promotionStatus: 'watchpoint',
      checkSupport: {
        supported: true,
        gate: 'warn',
        kinds: ['clone_propagation_drift'],
      },
      gateKinds: ['clone_propagation_drift'],
      findingKinds: [],
      sessionEndKinds: ['clone_propagation_drift'],
      expectedGateDecision: 'pass',
      async inject(workRoot) {
        return replaceInFile(
          workRoot,
          'electron/remote/ws-server.ts',
          buildRemoteKillHandler('runAgentCommand', 'kill'),
          buildRemoteKillHandler('runRemoteKillCommand'),
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
      id: 'incomplete_propagation',
      title: 'Add a server bootstrap category without updating the contract siblings',
      repoLabel: 'parallel-code',
      targetPath: 'src/domain/server-state-bootstrap.ts',
      signalKind: 'incomplete_propagation',
      signalFamily: 'obligation',
      checkSupport: {
        supported: true,
        kinds: ['incomplete_propagation'],
      },
      gateKinds: ['contract_surface_completeness'],
      findingKinds: ['contract_surface_completeness'],
      sessionEndKinds: ['contract_surface_completeness'],
      async inject(workRoot) {
        const { matcher, replacement } = buildIncompletePropagationCategoryPatch();
        return replaceInFile(
          workRoot,
          'src/domain/server-state-bootstrap.ts',
          matcher,
          replacement,
        );
      },
    }),
    createDefect({
      id: 'multi_writer_concept',
      title: 'Write task git status from task-presentation-status.ts',
      repoLabel: 'parallel-code',
      targetPath: 'src/app/task-presentation-status.ts',
      signalKind: 'multi_writer_concept',
      signalFamily: 'rules',
      promotionStatus: 'watchpoint',
      checkSupport: {
        supported: true,
        gate: 'fail',
        kinds: ['multi_writer_concept'],
      },
      gateKinds: ['multi_writer_concept'],
      findingKinds: ['multi_writer_concept'],
      sessionEndKinds: ['multi_writer_concept'],
      expectedGateDecision: 'fail',
      async inject(workRoot) {
        return injectTaskGitStatusOwnershipRegression(workRoot);
      },
    }),
    createDefect({
      id: 'writer_outside_allowlist',
      title: 'Write task git status from a non-owner presentation module',
      repoLabel: 'parallel-code',
      targetPath: 'src/app/task-presentation-status.ts',
      signalKind: 'writer_outside_allowlist',
      signalFamily: 'rules',
      promotionStatus: 'watchpoint',
      checkSupport: {
        supported: true,
        gate: 'fail',
        kinds: ['writer_outside_allowlist'],
      },
      gateKinds: ['writer_outside_allowlist'],
      findingKinds: ['writer_outside_allowlist'],
      sessionEndKinds: ['writer_outside_allowlist'],
      expectedGateDecision: 'fail',
      async inject(workRoot) {
        return injectTaskGitStatusOwnershipRegression(workRoot);
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

function createDogfoodLargeFileDefect({
  id,
  title,
  targetPath,
  label,
}) {
  return createDefect({
    id,
    title,
    repoLabel: 'sentrux',
    targetPath,
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
      return appendToFile(workRoot, targetPath, buildCommentBlock(label, 120));
    },
  });
}

function buildDogfoodCatalog() {
  return [
    createDogfoodLargeFileDefect({
      id: 'self_large_file',
      title: 'Append 120 lines to the benchmark harness',
      targetPath: 'scripts/lib/benchmark-harness.mjs',
      label: 'self benchmark-harness large-file growth',
    }),
    createDefect({
      id: 'self_forbidden_raw_read',
      title: 'Read task presentation status through a forbidden raw access path',
      repoLabel: 'sentrux',
      targetPath: 'src/components/SidebarTaskRow.tsx',
      signalKind: 'forbidden_raw_read',
      signalFamily: 'rules',
      checkSupport: {
        supported: true,
        kinds: ['forbidden_raw_read'],
      },
      findingKinds: ['forbidden_raw_read'],
      expectedGateDecision: null,
      async inject(workRoot) {
        const rulesPath = await appendToFile(
          workRoot,
          '.sentrux/rules.toml',
          buildSelfForbiddenRawReadRules(),
        );
        const injectedPaths = await writeFiles(workRoot, [
          {
            path: 'package.json',
            text: `${buildSelfTypeScriptFixturePackageJson('sentrux-self-forbidden-raw-read')}
`,
          },
          {
            path: 'tsconfig.json',
            text: `${buildSelfTypeScriptFixtureTsconfig()}
`,
          },
          {
            path: 'src/store/core.ts',
            text: buildSelfTaskStatusStoreSource(),
          },
          {
            path: 'src/app/task-presentation-status.ts',
            text: buildSelfTaskPresentationStatusSource(),
          },
          {
            path: 'src/components/SidebarTaskRow.tsx',
            text: buildSelfSidebarTaskRowSource(),
          },
        ]);
        return [rulesPath, ...injectedPaths];
      },
    }),
    createDefect({
      id: 'self_incomplete_propagation',
      title: 'Change one defect-injection surface without updating its sibling contract sites',
      repoLabel: 'sentrux',
      targetPath: 'scripts/defect-injection/catalog.mjs',
      signalKind: 'incomplete_propagation',
      signalFamily: 'obligation',
      checkSupport: {
        supported: true,
        kinds: ['incomplete_propagation'],
      },
      findingKinds: ['contract_surface_completeness'],
      expectedGateDecision: null,
      async inject(workRoot) {
        const rulesPath = await appendToFile(
          workRoot,
          '.sentrux/rules.toml',
          buildSelfIncompletePropagationRules(),
        );
        const injectedPath = await appendToFile(
          workRoot,
          'scripts/defect-injection/catalog.mjs',
          buildCommentBlock('self incomplete-propagation trigger', 2),
        );
        return [rulesPath, injectedPath];
      },
    }),
    createDefect({
      id: 'self_session_introduced_clone',
      title: 'Introduce a fresh duplicate helper after the session baseline',
      repoLabel: 'sentrux',
      targetPath: 'src/copy.ts',
      signalKind: 'session_introduced_clone',
      signalFamily: 'clone',
      blockingIntent: 'watchpoint',
      checkSupport: {
        supported: true,
        kinds: ['session_introduced_clone'],
      },
      sessionEndKinds: ['session_introduced_clone'],
      expectedGateDecision: null,
      async inject(workRoot) {
        return writeFiles(workRoot, [
          {
            path: 'package.json',
            text: `${buildSelfTypeScriptFixturePackageJson('sentrux-self-session-introduced-clone')}
`,
          },
          {
            path: 'tsconfig.json',
            text: `${buildSelfTypeScriptFixtureTsconfig()}
`,
          },
          {
            path: 'src/source.ts',
            text: buildSelfClonePropagationBaselineSource(),
          },
          {
            path: 'src/copy.ts',
            text: buildSelfClonePropagationBaselineSource(),
          },
          {
            path: 'src/notes.ts',
            text: buildSelfSessionCloneDistractor(),
          },
        ]);
      },
    }),
    createDefect({
      id: 'self_clone_propagation_drift',
      title: 'Edit one side of a committed duplicate helper pair without syncing its sibling',
      repoLabel: 'sentrux',
      targetPath: 'src/source.ts',
      signalKind: 'clone_propagation_drift',
      signalFamily: 'clone',
      promotionStatus: 'watchpoint',
      blockingIntent: 'watchpoint',
      checkSupport: {
        supported: true,
        gate: 'warn',
        kinds: ['clone_propagation_drift'],
      },
      gateKinds: ['clone_propagation_drift'],
      sessionEndKinds: ['clone_propagation_drift'],
      expectedGateDecision: null,
      setupCommitMessage: 'Seed clone propagation baseline fixture',
      async setup(workRoot) {
        return writeFiles(workRoot, [
          {
            path: 'package.json',
            text: `${buildSelfTypeScriptFixturePackageJson('sentrux-self-clone-propagation-drift')}
`,
          },
          {
            path: 'tsconfig.json',
            text: `${buildSelfTypeScriptFixtureTsconfig()}
`,
          },
          {
            path: 'src/source.ts',
            text: buildSelfSessionCloneSource(),
          },
          {
            path: 'src/copy.ts',
            text: buildSelfSessionCloneSource(),
          },
          {
            path: 'src/notes.ts',
            text: buildSelfSessionCloneDistractor(),
          },
        ]);
      },
      async inject(workRoot) {
        return writeTextFile(workRoot, 'src/source.ts', buildSelfClonePropagationDriftSource());
      },
    }),
    createDefect({
      id: 'self_zero_config_boundary_violation',
      title: 'Deep import a task-status helper without a module-contract rule',
      repoLabel: 'sentrux',
      targetPath: 'src/app/task-dashboard.ts',
      signalKind: 'zero_config_boundary_violation',
      signalFamily: 'rules',
      promotionStatus: 'watchpoint',
      blockingIntent: 'watchpoint',
      checkSupport: {
        supported: true,
        gate: 'pass',
        kinds: ['zero_config_boundary_violation'],
      },
      gateKinds: ['zero_config_boundary_violation'],
      findingKinds: ['zero_config_boundary_violation'],
      sessionEndKinds: ['zero_config_boundary_violation'],
      expectedGateDecision: 'pass',
      async inject(workRoot) {
        return writeFiles(workRoot, [
          {
            path: 'package.json',
            text: `${buildSelfTypeScriptFixturePackageJson('sentrux-self-zero-config-boundary')}\n`,
          },
          {
            path: 'tsconfig.json',
            text: `${buildSelfTypeScriptFixtureTsconfig()}\n`,
          },
          {
            path: 'src/modules/task-status/index.ts',
            text: buildSelfTaskStatusModuleIndexSource(),
          },
          {
            path: 'src/modules/task-status/internal.ts',
            text: buildSelfTaskStatusModuleInternalSource(),
          },
          {
            path: 'src/app/task-dashboard.ts',
            text: buildSelfTaskDashboardSource(),
          },
        ]);
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
