import { asArray } from './signal-summary-utils.mjs';

const REVIEW_VERDICT_IDENTITY_FIELDS = [
  'source_kind',
  'source_label',
  'snapshot_label',
  'task_id',
  'replay_id',
  'commit',
];

function isBooleanValue(value) {
  return value === true || value === false;
}

function isPositiveInteger(value) {
  return Number.isInteger(value) && value > 0;
}

function hasText(value) {
  return typeof value === 'string' && value.trim().length > 0;
}

function normalizeText(value) {
  return hasText(value) ? value.trim() : null;
}

function normalizeStringArray(value) {
  if (!Array.isArray(value)) {
    return [];
  }

  return value.filter(hasText).map((item) => item.trim());
}

function buildIdentityFields(record) {
  const identity = {};

  for (const fieldName of REVIEW_VERDICT_IDENTITY_FIELDS) {
    identity[fieldName] = normalizeText(record?.[fieldName]);
  }

  return identity;
}

function buildReviewPacketIndex(reviewPacket) {
  return asArray(reviewPacket?.samples).map(function buildPacketEntry(sample, index) {
    return {
      sample,
      index,
      kind: normalizeText(sample?.kind),
      scope: normalizeText(sample?.scope),
      report_bucket: normalizeText(sample?.report_bucket),
      ...buildIdentityFields(sample),
      used: false,
    };
  });
}

function findPacketEntry(reviewPacketIndex, predicate) {
  if (!Array.isArray(reviewPacketIndex)) {
    return null;
  }

  return reviewPacketIndex.find((entry) => !entry.used && predicate(entry)) ?? null;
}

function findUniquePacketEntry(reviewPacketIndex, predicate) {
  const matches = reviewPacketIndex.filter((entry) => !entry.used && predicate(entry));
  if (matches.length !== 1) {
    return null;
  }

  return matches[0];
}

function buildVerdictIdentity(verdict) {
  return buildIdentityFields(verdict);
}

function hasIdentityFields(identity) {
  return REVIEW_VERDICT_IDENTITY_FIELDS.some((fieldName) => Boolean(identity[fieldName]));
}

function matchesIdentity(entry, identity) {
  return (
    (!identity.source_kind || entry.source_kind === identity.source_kind) &&
    (!identity.source_label || entry.source_label === identity.source_label) &&
    (!identity.snapshot_label || entry.snapshot_label === identity.snapshot_label) &&
    (!identity.task_id || entry.task_id === identity.task_id) &&
    (!identity.replay_id || entry.replay_id === identity.replay_id) &&
    (!identity.commit || entry.commit === identity.commit)
  );
}

function selectPacketSample(verdict, reviewPacketIndex) {
  const kind = normalizeText(verdict?.kind);
  const scope = normalizeText(verdict?.scope);
  const reportBucket = normalizeText(verdict?.report_bucket);
  const identity = buildVerdictIdentity(verdict);
  const hasIdentity = hasIdentityFields(identity);

  if (!kind) {
    return null;
  }

  if (hasIdentity && scope && reportBucket) {
    const identityExactMatch = findPacketEntry(
      reviewPacketIndex,
      (entry) =>
        entry.kind === kind &&
        entry.scope === scope &&
        entry.report_bucket === reportBucket &&
        matchesIdentity(entry, identity),
    );
    if (identityExactMatch) {
      return identityExactMatch;
    }
  }

  if (hasIdentity && scope) {
    const identityScopeMatch = findPacketEntry(
      reviewPacketIndex,
      (entry) => entry.kind === kind && entry.scope === scope && matchesIdentity(entry, identity),
    );
    if (identityScopeMatch) {
      return identityScopeMatch;
    }
  }

  if (scope && reportBucket) {
    const exactMatch = findPacketEntry(
      reviewPacketIndex,
      (entry) =>
        entry.kind === kind &&
        entry.scope === scope &&
        entry.report_bucket === reportBucket,
    );
    if (exactMatch) {
      return exactMatch;
    }
  }

  if (scope) {
    const kindScopeMatch = findPacketEntry(
      reviewPacketIndex,
      (entry) => entry.kind === kind && entry.scope === scope,
    );
    if (kindScopeMatch) {
      return kindScopeMatch;
    }
  }

  if (hasIdentity && reportBucket) {
    const identityBucketMatch = findUniquePacketEntry(
      reviewPacketIndex,
      (entry) =>
        entry.kind === kind &&
        entry.report_bucket === reportBucket &&
        matchesIdentity(entry, identity),
    );
    if (identityBucketMatch) {
      return identityBucketMatch;
    }
  }

  if (reportBucket) {
    const kindBucketMatch = findUniquePacketEntry(
      reviewPacketIndex,
      (entry) =>
        entry.kind === kind &&
        entry.report_bucket === reportBucket,
    );
    if (kindBucketMatch) {
      return kindBucketMatch;
    }
  }

  const kindMatch = findUniquePacketEntry(
    reviewPacketIndex,
    (entry) => entry.kind === kind,
  );
  if (kindMatch) {
    return kindMatch;
  }

  return null;
}

function selectStructuredVerdictFields(packetSample, verdictIndex) {
  const packetRank = isPositiveInteger(packetSample?.rank) ? packetSample.rank : null;

  return {
    rank_observed: packetRank ?? verdictIndex + 1,
    rank_preserved: packetRank === null ? null : packetRank === verdictIndex + 1,
    repair_packet_complete: isBooleanValue(packetSample?.repair_packet?.complete)
      ? packetSample.repair_packet.complete
      : null,
    repair_packet_missing_fields: normalizeStringArray(
      packetSample?.repair_packet?.missing_fields,
    ),
    repair_packet_fix_surface_clear: isBooleanValue(packetSample?.repair_packet?.fix_surface_clear)
      ? packetSample.repair_packet.fix_surface_clear
      : null,
    repair_packet_verification_clear: isBooleanValue(
      packetSample?.repair_packet?.verification_clear,
    )
      ? packetSample.repair_packet.verification_clear
      : null,
  };
}

export function buildStructuredReviewVerdictFieldsFromPacketSample(
  packetSample,
  verdictIndex,
) {
  return selectStructuredVerdictFields(packetSample, verdictIndex);
}

export {
  buildStructuredReviewVerdictFieldsFromPacketSample as buildStructuredVerdictFieldsFromPacketSample,
};

export function enrichReviewVerdictReport(reviewVerdicts, reviewPacket) {
  if (!reviewVerdicts || !reviewPacket) {
    return reviewVerdicts;
  }

  const reviewPacketIndex = buildReviewPacketIndex(reviewPacket);
  let changed = false;
  const verdicts = asArray(reviewVerdicts.verdicts).map(function enrichVerdict(verdict, index) {
    const packetEntry = selectPacketSample(verdict, reviewPacketIndex);
    if (!packetEntry) {
      return verdict;
    }

    packetEntry.used = true;
    const structuredFields = selectStructuredVerdictFields(packetEntry.sample, index);
    const enrichedVerdict = {
      ...verdict,
    };
    let verdictChanged = false;

    if (!isPositiveInteger(enrichedVerdict.rank_observed)) {
      enrichedVerdict.rank_observed = structuredFields.rank_observed;
      verdictChanged = true;
    }
    const effectiveObservedRank = isPositiveInteger(enrichedVerdict.rank_observed)
      ? enrichedVerdict.rank_observed
      : structuredFields.rank_observed;
    if (!isBooleanValue(enrichedVerdict.rank_preserved) && isPositiveInteger(effectiveObservedRank)) {
      enrichedVerdict.rank_preserved = effectiveObservedRank === index + 1;
      verdictChanged = true;
    }
    if (!isBooleanValue(enrichedVerdict.repair_packet_complete)) {
      enrichedVerdict.repair_packet_complete = structuredFields.repair_packet_complete;
      verdictChanged = true;
    }
    if (!Array.isArray(enrichedVerdict.repair_packet_missing_fields)) {
      enrichedVerdict.repair_packet_missing_fields =
        structuredFields.repair_packet_missing_fields;
      verdictChanged = true;
    }
    if (!isBooleanValue(enrichedVerdict.repair_packet_fix_surface_clear)) {
      enrichedVerdict.repair_packet_fix_surface_clear =
        structuredFields.repair_packet_fix_surface_clear;
      verdictChanged = true;
    }
    if (!isBooleanValue(enrichedVerdict.repair_packet_verification_clear)) {
      enrichedVerdict.repair_packet_verification_clear =
        structuredFields.repair_packet_verification_clear;
      verdictChanged = true;
    }

    if (!verdictChanged) {
      return verdict;
    }

    changed = true;
    return enrichedVerdict;
  });

  if (!changed) {
    return reviewVerdicts;
  }

  return {
    ...reviewVerdicts,
    verdicts,
  };
}

export {
  enrichReviewVerdictReport as enrichReviewVerdictsFromPacket,
};
