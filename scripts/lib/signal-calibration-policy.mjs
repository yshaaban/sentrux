export const SIGNAL_PROMOTION_POLICY = Object.freeze({
  seededRecallMin: 0.95,
  reviewedPrecisionMin: 0.8,
  reviewNoiseRateMax: 0.2,
  remediationSuccessMin: 0.6,
  topActionClearRateMin: 0.6,
  sessionCleanRateMin: 0.6,
  followupRegressionRateMax: 0.4,
});

export const SIGNAL_BACKLOG_PRIORITY_WEIGHTS = Object.freeze({
  liveMiss: 3,
  replayMiss: 2,
  regressionFollowup: 2,
  outOfCohortBonus: 1,
});
