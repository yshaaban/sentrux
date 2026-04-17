import { buildPacketFromRepoHeadPayload } from '../../evals/build-check-review-packet.mjs';
import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';

async function writeJson(targetPath, value) {
  await mkdir(path.dirname(targetPath), { recursive: true });
  await writeFile(targetPath, `${JSON.stringify(value, null, 2)}\n`, 'utf8');
}

function buildDefaultRepoHeadCheckPacket(actions) {
  return buildPacketFromRepoHeadPayload(
    {
      tool: 'check',
      limit: 1,
      repoRoot: '/tmp/sentrux',
      kinds: [],
    },
    { actions },
  );
}

export { buildDefaultRepoHeadCheckPacket, writeJson };
