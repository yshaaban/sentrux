import test from 'node:test';
import assert from 'node:assert/strict';

import {
  createDogfoodCatalog,
  createParallelCodeCatalog,
  selectDefects,
} from '../defect-injection/catalog.mjs';
import { evaluateDefectAssertion } from '../defect-injection/assertion-engine.mjs';
import {
  buildInjectionReport,
  formatInjectionReportMarkdown,
} from '../defect-injection/report.mjs';

test('catalogs expose the expected defect ids', function () {
  const parallelCatalog = createParallelCodeCatalog();
  const dogfoodCatalog = createDogfoodCatalog();

  function findDefect(catalog, defectId) {
    return catalog.find((defect) => defect.id === defectId);
  }

  const cloneInjection = findDefect(parallelCatalog, 'clone_injection');
  const sessionClone = findDefect(parallelCatalog, 'session_introduced_clone');
  const clonePropagationDrift = findDefect(parallelCatalog, 'clone_propagation_drift');
  const incompletePropagation = findDefect(parallelCatalog, 'incomplete_propagation');
  const multiWriterConcept = findDefect(parallelCatalog, 'multi_writer_concept');
  const writerOutsideAllowlist = findDefect(parallelCatalog, 'writer_outside_allowlist');
  const selfForbiddenRawRead = findDefect(dogfoodCatalog, 'self_forbidden_raw_read');
  const selfIncompletePropagation = findDefect(dogfoodCatalog, 'self_incomplete_propagation');
  const selfSessionClone = findDefect(dogfoodCatalog, 'self_session_introduced_clone');
  const zeroConfigBoundaryViolation = findDefect(
    dogfoodCatalog,
    'self_zero_config_boundary_violation',
  );

  assert.deepEqual(
    parallelCatalog.map((defect) => defect.id),
    [
      'large_file_growth',
      'forbidden_raw_read',
      'clone_injection',
      'session_introduced_clone',
      'clone_propagation_drift',
      'missing_exhaustiveness',
      'incomplete_propagation',
      'multi_writer_concept',
      'writer_outside_allowlist',
      'missing_test',
    ],
  );
  assert.deepEqual(
    dogfoodCatalog.map((defect) => defect.id),
    [
      'self_large_file',
      'self_forbidden_raw_read',
      'self_incomplete_propagation',
      'self_session_introduced_clone',
      'self_zero_config_boundary_violation',
    ],
  );
  assert.equal(cloneInjection.check_support.supported, false);
  assert.equal(sessionClone.check_support.supported, true);
  assert.equal(clonePropagationDrift.check_support.supported, true);
  assert.equal(clonePropagationDrift.expected_gate_decision, 'pass');
  assert.deepEqual(clonePropagationDrift.expected_gate_kinds, ['clone_propagation_drift']);
  assert.equal(incompletePropagation.check_support.supported, true);
  assert.equal(multiWriterConcept.expected_gate_decision, 'fail');
  assert.equal(writerOutsideAllowlist.check_support.supported, true);
  assert.equal(selfForbiddenRawRead.check_support.supported, true);
  assert.equal(selfIncompletePropagation.check_support.supported, true);
  assert.equal(selfSessionClone.check_support.supported, true);
  assert.equal(zeroConfigBoundaryViolation.check_support.supported, true);
  assert.equal(zeroConfigBoundaryViolation.check_support.gate, 'pass');
  assert.equal(zeroConfigBoundaryViolation.expected_gate_decision, 'pass');
});

test('selectDefects filters by requested ids', function () {
  const catalog = createParallelCodeCatalog();
  const selected = selectDefects(catalog, ['forbidden_raw_read']);

  assert.equal(selected.length, 1);
  assert.equal(selected[0].id, 'forbidden_raw_read');
});

test('evaluateDefectAssertion uses structural payloads as secondary evidence', function () {
  const defect = createParallelCodeCatalog()[0];
  const result = evaluateDefectAssertion(defect, {
    check: {
      gate: 'warn',
      issues: [{ kind: 'large_file', message: 'grown file' }],
    },
    gate: {
      decision: 'warn',
      introduced_findings: [{ kind: 'large_file', summary: 'grown file' }],
    },
    findings: {
      findings: [{ kind: 'large_file', summary: 'grown file' }],
    },
    session_end: {
      introduced_findings: [{ kind: 'large_file', summary: 'grown file' }],
    },
  });

  assert.equal(result.detected, true);
  assert.equal(result.status, 'pass');
  assert.equal(result.check.matched, true);
  assert.equal(result.gate.matched, true);
  assert.equal(result.findings.matched, true);
});

test('evaluateDefectAssertion requires both decision and kind when both are expected', function () {
  const defect = createParallelCodeCatalog()[1];
  const result = evaluateDefectAssertion(defect, {
    check: {
      gate: 'fail',
      issues: [{ kind: 'large_file', message: 'wrong issue kind' }],
    },
    gate: {
      decision: 'fail',
      introduced_findings: [{ kind: 'large_file', summary: 'wrong issue kind' }],
    },
    findings: {
      findings: [{ kind: 'large_file', summary: 'wrong issue kind' }],
    },
    session_end: {
      introduced_findings: [{ kind: 'large_file', summary: 'wrong issue kind' }],
    },
  });

  assert.equal(result.check.matched, false);
  assert.equal(result.gate.matched, false);
  assert.equal(result.detected, false);
  assert.equal(result.status, 'fail');
});

test('formatInjectionReportMarkdown summarizes the run', function () {
  const report = buildInjectionReport({
    repoLabel: 'parallel-code',
    repoRoot: '/tmp/parallel-code',
    generatedAt: '2026-04-02T00:00:00.000Z',
    defects: [{ id: 'large_file_growth', title: 'Append 120 lines', target_path: 'foo' }],
    results: [
      {
        defect_id: 'large_file_growth',
        title: 'Append 120 lines',
        status: 'partial',
        check: { supported: false, matched: false, evidence: [] },
        check_rules: { matched: true, evidence: ['$.violations[0].rule:boundary'] },
        gate: { matched: true, evidence: ['$.gate:introduced_findings[0].kind:large_file'] },
        findings: { matched: true, evidence: ['$.findings[0].kind:large_file'] },
        session_end: { matched: false, evidence: [] },
      },
    ],
  });

  const markdown = formatInjectionReportMarkdown(report);

  assert.match(markdown, /Defect Injection Report/);
  assert.match(markdown, /large_file_growth/);
  assert.match(markdown, /partial/);
  assert.match(markdown, /check_rules matched/);
});
