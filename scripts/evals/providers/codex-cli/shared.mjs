import { spawnSync } from 'node:child_process';

export const defaultCodexBin = process.env.CODEX_BIN ?? 'codex';

let cachedVersion = undefined;

export function nowMs() {
  return Number(process.hrtime.bigint()) / 1_000_000;
}

export function resolveCodexVersion(codexBin) {
  if (cachedVersion !== undefined) {
    return cachedVersion;
  }

  const result = spawnSync(codexBin, ['--version'], {
    encoding: 'utf8',
    shell: false,
    stdio: ['ignore', 'pipe', 'pipe'],
  });

  if (result.status === 0) {
    cachedVersion = result.stdout.trim() || null;
    return cachedVersion;
  }

  cachedVersion = null;
  return cachedVersion;
}
