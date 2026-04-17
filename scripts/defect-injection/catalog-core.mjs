import { mkdir, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';

function ensureTrailingNewline(text) {
  return text.endsWith('\n') ? text : `${text}\n`;
}

export function buildCommentBlock(label, lineCount) {
  const lines = [`// defect-injection: ${label}`];
  for (let index = 0; index < lineCount; index += 1) {
    lines.push(`// defect-injection filler ${index + 1}`);
  }
  return `${lines.join('\n')}\n`;
}

export async function appendToFile(workRoot, relativePath, text) {
  const targetPath = path.join(workRoot, relativePath);
  const currentText = await readFile(targetPath, 'utf8');
  await mkdir(path.dirname(targetPath), { recursive: true });
  await writeFile(targetPath, `${ensureTrailingNewline(currentText)}${text}`, 'utf8');
  return targetPath;
}

export async function replaceInFile(workRoot, relativePath, matcher, replacement) {
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

export async function writeTextFile(workRoot, relativePath, text) {
  const targetPath = path.join(workRoot, relativePath);
  await mkdir(path.dirname(targetPath), { recursive: true });
  await writeFile(targetPath, text, 'utf8');
  return targetPath;
}

export async function writeFiles(workRoot, patches) {
  const injectedPaths = [];
  for (const patch of patches) {
    injectedPaths.push(await writeTextFile(workRoot, patch.path, patch.text));
  }
  return injectedPaths;
}

export function createDefect({
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
