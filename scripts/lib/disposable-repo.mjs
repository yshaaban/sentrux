import { spawnSync } from 'node:child_process';
import { existsSync } from 'node:fs';
import { cp, mkdir, mkdtemp, rm } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';

export function assertPathExists(targetPath, label) {
  if (!existsSync(targetPath)) {
    throw new Error(`Missing ${label}: ${targetPath}`);
  }
}

function runChecked(command, args) {
  const result = spawnSync(command, args, {
    stdio: 'inherit',
    shell: false,
  });

  if (result.error) {
    throw result.error;
  }

  if (result.status !== 0) {
    throw new Error(`${command} ${args.join(' ')} exited with code ${result.status}`);
  }
}

export async function createDisposableRepoClone({ sourceRoot, label, rulesSource }) {
  assertPathExists(sourceRoot, `${label} repo`);
  assertPathExists(rulesSource, `${label} rules source`);

  const tempRoot = await mkdtemp(path.join(os.tmpdir(), `sentrux-${label}-`));
  const workRoot = path.join(tempRoot, label);
  const sentruxDir = path.join(workRoot, '.sentrux');
  const rulesPath = path.join(sentruxDir, 'rules.toml');

  runChecked('git', ['clone', '--quiet', '--local', '--no-hardlinks', sourceRoot, workRoot]);
  await mkdir(sentruxDir, { recursive: true });
  await cp(rulesSource, rulesPath);

  return {
    tempRoot,
    workRoot,
    async cleanup() {
      await rm(tempRoot, { recursive: true, force: true });
    },
  };
}
