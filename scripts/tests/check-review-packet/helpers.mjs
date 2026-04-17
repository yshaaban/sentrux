import { buildPacketFromRepoHeadPayload } from '../../evals/build-check-review-packet.mjs';
import { writeJson } from '../../lib/script-artifacts.mjs';

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
