export const SIGNAL_PROMOTION_POLICY = Object.freeze({
  seededRecallMin: 0.95,
  reviewedPrecisionMin: 0.8,
  reviewNoiseRateMax: 0.2,
  remediationSuccessMin: 0.6,
  topActionClearRateMin: 0.6,
  topActionFollowRateMin: 0.6,
  topActionHelpRateMin: 0.6,
  taskSuccessRateMin: 0.5,
  patchExpansionRateMax: 0.4,
  interventionNetValueScoreMin: 0,
  sessionVerdictMinSamples: 1,
  sessionCleanRateMin: 0.6,
  followupRegressionRateMax: 0.4,
  sessionThrashRateMax: 0.3,
  entropyIncreaseRateMax: 0.3,
});

export const SIGNAL_PRIMARY_TARGET_POLICY = Object.freeze({
  top1ActionablePrecisionMin: 1,
  top3ActionablePrecisionMin: 0.67,
  top10ActionablePrecisionMin: 0.6,
  rankingPreferenceSatisfactionMin: 0.8,
  top1MinReviewedSamples: 1,
  top3MinReviewedSamples: 3,
  top10MinReviewedSamples: 10,
  rankingPreferenceMinComparisons: 1,
});

export const REVIEW_PACKET_COMPLETENESS_POLICY = Object.freeze({
  requiredFields: Object.freeze(['scope', 'summary', 'evidence', 'repair_surface']),
  preferredFields: Object.freeze(['fix_hint', 'likely_fix_sites']),
  top3CompleteRateMin: 0.8,
  top10CompleteRateMin: 0.7,
});

export const SIGNAL_BACKLOG_PRIORITY_WEIGHTS = Object.freeze({
  liveMiss: 3,
  replayMiss: 2,
  regressionFollowup: 2,
  outOfCohortBonus: 1,
});
