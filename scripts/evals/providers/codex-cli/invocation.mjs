import { mkdtemp, readFile, writeFile } from 'node:fs/promises';
import { existsSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';

export async function createInvocationFiles(jsonSchema) {
  const tempRoot = await mkdtemp(path.join(tmpdir(), 'sentrux-codex-provider-'));
  const lastMessagePath = path.join(tempRoot, 'last-message.txt');
  let schemaPath = null;

  if (jsonSchema) {
    schemaPath = path.join(tempRoot, 'output-schema.json');
    await writeFile(
      schemaPath,
      `${typeof jsonSchema === 'string' ? jsonSchema : JSON.stringify(jsonSchema, null, 2)}\n`,
      'utf8',
    );
  }

  return {
    tempRoot,
    lastMessagePath,
    schemaPath,
  };
}

export function buildCodexExecArgs(options, invocationFiles) {
  const args = [
    'exec',
    '--json',
    '--skip-git-repo-check',
    '--dangerously-bypass-approvals-and-sandbox',
    '--cd',
    options.cwd,
    '--output-last-message',
    invocationFiles.lastMessagePath,
  ];

  if (options.model) {
    args.push('--model', options.model);
  }
  if (options.sandbox) {
    args.push('--sandbox', options.sandbox);
  }
  for (const dir of options.addDirs ?? []) {
    if (typeof dir === 'string' && dir) {
      args.push('--add-dir', dir);
    }
  }
  if (invocationFiles.schemaPath) {
    args.push('--output-schema', invocationFiles.schemaPath);
  }
  for (const [key, value] of options.config ?? []) {
    args.push('--config', `${key}=${value}`);
  }

  args.push(options.prompt);
  return args;
}

export async function readLastMessage(lastMessagePath) {
  if (!existsSync(lastMessagePath)) {
    return null;
  }

  return readFile(lastMessagePath, 'utf8');
}
