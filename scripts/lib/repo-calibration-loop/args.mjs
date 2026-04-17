import { parseCliArgs } from '../eval-support.mjs';
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
      '--skip-live': function setSkipLive(target) {
        target.skipLive = true;
      },
      '--skip-replay': function setSkipReplay(target) {
        target.skipReplay = true;
      },
      '--skip-review': function setSkipReview(target) {
        target.skipReview = true;
      },
      '--skip-scorecard': function setSkipScorecard(target) {
        target.skipScorecard = true;
      },
      '--skip-backlog': function setSkipBacklog(target) {
        target.skipBacklog = true;
      },
    },
    values: {
      '--manifest': function setManifestPath(target, value) {
        target.manifestPath = value;
      },
      '--output-dir': function setOutputDir(target, value) {
        target.outputDir = value;
      },
    },
  });

  if (!result.manifestPath) {
    throw new Error('Missing required --manifest path');
  }

  return result;
}

export { nowIso };
