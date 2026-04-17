import { parseCliArgs } from '../eval-support.mjs';
import { setFlag, setStringOption } from '../eval-cli-shared.mjs';
import { nowIso } from '../eval-runtime/common.mjs';

export function parseArgs(argv) {
  const result = {
    manifestPath: null,
    outputDir: null,
    skipLive: false,
    skipReplay: false,
    skipReview: false,
    skipScorecard: false,
    skipBacklog: false,
  };

  parseCliArgs(argv, result, {
    flags: {
      '--skip-live': setFlag('skipLive'),
      '--skip-replay': setFlag('skipReplay'),
      '--skip-review': setFlag('skipReview'),
      '--skip-scorecard': setFlag('skipScorecard'),
      '--skip-backlog': setFlag('skipBacklog'),
    },
    values: {
      '--manifest': setStringOption('manifestPath'),
      '--output-dir': setStringOption('outputDir'),
    },
  });

  if (!result.manifestPath) {
    throw new Error('Missing required --manifest path');
  }

  return result;
}

export { nowIso };
