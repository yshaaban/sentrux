import {
  appendToFile,
  buildCommentBlock,
  createDefect,
  replaceInFile,
  writeTextFile,
} from './catalog-core.mjs';

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

function buildStructuralDefects() {
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
        return writeTextFile(workRoot, 'src/app/task-health-monitor.ts', buildMissingTestSource());
      },
    }),
  ];
}

function buildCloneDefects() {
  return [
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
  ];
}

function buildObligationDefects() {
  return [
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
  ];
}

function buildRuleDefects() {
  return [
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
  ];
}

export function createParallelCodeCatalog() {
  const structuralDefects = buildStructuralDefects();
  const [largeFileGrowth, forbiddenRawRead, missingTest] = structuralDefects;

  return [
    largeFileGrowth,
    forbiddenRawRead,
    ...buildCloneDefects(),
    ...buildObligationDefects(),
    ...buildRuleDefects(),
    missingTest,
  ];
}
