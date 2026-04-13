import { runCommand } from '../lib/benchmark-harness.mjs';

function normalizePathList(value) {
  if (Array.isArray(value)) {
    return value.filter(Boolean);
  }

  return value ? [value] : [];
}

export function normalizeDefectPaths(result, key) {
  if (!result || typeof result !== 'object') {
    return normalizePathList(result);
  }

  return normalizePathList(result[key]);
}

export async function gitConfigValue(root, key) {
  const result = await runCommand('git', ['config', '--get', key], { cwd: root });
  if (result.exit_code !== 0) {
    return null;
  }

  const value = result.stdout.trim();
  return value || null;
}

export async function commitPreparedFixture(workRoot, repoRoot, message) {
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

export async function prepareDefectFixture(defect, workRoot, repoRoot) {
  if (typeof defect.setup !== 'function') {
    return;
  }

  await defect.setup(workRoot);
  if (typeof defect.setup_commit_message === 'string' && defect.setup_commit_message) {
    await commitPreparedFixture(workRoot, repoRoot, defect.setup_commit_message);
  }
}
