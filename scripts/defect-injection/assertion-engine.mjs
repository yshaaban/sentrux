function collectKindMatches(value, expectedKinds, path = '$', matches = []) {
  if (!value || typeof value !== 'object') {
    return matches;
  }

  if (Array.isArray(value)) {
    value.forEach((entry, index) => {
      collectKindMatches(entry, expectedKinds, `${path}[${index}]`, matches);
    });
    return matches;
  }

  if (typeof value.kind === 'string' && expectedKinds.has(value.kind)) {
    matches.push({
      kind: value.kind,
      path,
    });
  }

  for (const [key, entry] of Object.entries(value)) {
    collectKindMatches(entry, expectedKinds, `${path}.${key}`, matches);
  }

  return matches;
}

function normalizeDecisionPayload(payload) {
  return {
    ...payload,
    decision: payload?.gate ?? payload?.decision ?? null,
  };
}

function evaluateCheckExpectation(expectation, checkResult) {
  if (!expectation?.supported) {
    return {
      supported: false,
      matched: false,
      evidence: [expectation?.reason ?? 'check stage disabled for this defect'],
    };
  }

  const result = evaluatePayloadExpectation(
    {
      decision: expectation.gate,
      kinds: expectation.kinds,
    },
    normalizeDecisionPayload(checkResult),
  );

  return {
    supported: true,
    matched: result.matched,
    evidence: result.evidence,
  };
}

function evaluatePayloadExpectation(expectation, payload) {
  const expectedKinds = new Set(expectation?.kinds ?? []);
  const expectsKinds = expectedKinds.size > 0;
  const expectsDecision = typeof expectation?.decision === 'string';
  if (!expectsKinds && !expectsDecision) {
    return {
      supported: false,
      matched: false,
      evidence: [],
      matched_kinds: [],
    };
  }
  const matches = expectedKinds.size > 0 ? collectKindMatches(payload, expectedKinds) : [];
  const evidence = matches.map((match) => `${match.path}:${match.kind}`);
  const matchedKinds = [...new Set(matches.map((match) => match.kind))];
  let decisionMatched = !expectsDecision;

  if (expectsDecision) {
    const decision = payload?.decision ?? payload?.summary?.decision ?? null;
    if (decision === expectation.decision) {
      evidence.push(`decision=${decision}`);
      decisionMatched = true;
    }
  }

  return {
    supported: true,
    matched: (!expectsKinds || matches.length > 0) && decisionMatched,
    evidence,
    matched_kinds: matchedKinds,
  };
}

export function evaluateDefectAssertion(defect, artifacts) {
  const check = evaluateCheckExpectation(defect.check_support, artifacts.check);
  const checkRules = evaluatePayloadExpectation(
    { kinds: defect.expected_check_rules_kinds },
    artifacts.check_rules,
  );
  const gate = evaluatePayloadExpectation(
    {
      decision: defect.expected_gate_decision,
      kinds: defect.expected_gate_kinds,
    },
    artifacts.gate,
  );
  const findings = evaluatePayloadExpectation(
    { kinds: defect.expected_finding_kinds },
    artifacts.findings,
  );
  const sessionEnd = evaluatePayloadExpectation(
    { kinds: defect.expected_session_end_kinds },
    artifacts.session_end,
  );

  const detected =
    check.matched || checkRules.matched || gate.matched || findings.matched || sessionEnd.matched;
  const supportedPrimary = check.supported;

  return {
    defect_id: defect.id,
    title: defect.title,
    supported_primary: supportedPrimary,
    check,
    check_rules: checkRules,
    gate,
    findings,
    session_end: sessionEnd,
    detected,
    status: detected ? (supportedPrimary && check.matched ? 'pass' : 'partial') : 'fail',
  };
}

export function assertDefectAssertion(defect, artifacts) {
  const result = evaluateDefectAssertion(defect, artifacts);

  if (!result.detected) {
    throw new Error(
      [
        `Defect '${defect.id}' was not detected by the check or follow-up assertions.`,
        `Check supported: ${result.check.supported}`,
        `Check matched: ${result.check.matched}`,
        `Check rules matched: ${result.check_rules.matched}`,
        `Gate matched: ${result.gate.matched}`,
        `Findings matched: ${result.findings.matched}`,
        `Session end matched: ${result.session_end.matched}`,
      ].join(' '),
    );
  }

  return result;
}
