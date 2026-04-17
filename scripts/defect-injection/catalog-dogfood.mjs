import {
  appendToFile,
  buildCommentBlock,
  createDefect,
  writeFiles,
  writeTextFile,
} from './catalog-core.mjs';

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

function buildFixtureRulesDefects() {
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
            text: `${buildSelfTypeScriptFixturePackageJson('sentrux-self-forbidden-raw-read')}\n`,
          },
          {
            path: 'tsconfig.json',
            text: `${buildSelfTypeScriptFixtureTsconfig()}\n`,
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
  ];
}

function buildCloneDefects() {
  return [
    createDefect({
      id: 'self_session_introduced_clone',
      title: 'Introduce a fresh duplicate helper after the session baseline',
      repoLabel: 'sentrux',
      targetPath: 'src/copy.ts',
      signalKind: 'session_introduced_clone',
      signalFamily: 'clone',
      promotionStatus: 'watchpoint',
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
            text: `${buildSelfTypeScriptFixturePackageJson('sentrux-self-session-introduced-clone')}\n`,
          },
          {
            path: 'tsconfig.json',
            text: `${buildSelfTypeScriptFixtureTsconfig()}\n`,
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
            text: `${buildSelfTypeScriptFixturePackageJson('sentrux-self-clone-propagation-drift')}\n`,
          },
          {
            path: 'tsconfig.json',
            text: `${buildSelfTypeScriptFixtureTsconfig()}\n`,
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
  ];
}

function buildBoundaryDefects() {
  return [
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

export function createDogfoodCatalog() {
  return [
    ...buildFixtureRulesDefects(),
    ...buildCloneDefects(),
    ...buildBoundaryDefects(),
  ];
}
