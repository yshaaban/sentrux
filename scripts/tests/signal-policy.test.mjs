import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

import {
  actionKindWeight,
  actionLeverageWeight,
  actionPresentationWeight,
  reportLeveragePriority,
  reportPresentationPriority,
  scoreBandLabel,
} from '../lib/signal-policy.mjs';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const fixturePath = path.join(__dirname, 'fixtures', 'policy-parity', 'shared-policy.json');

async function readSharedPolicyFixture() {
  return JSON.parse(await readFile(fixturePath, 'utf8'));
}

test('shared policy score-band labels match the parity fixture', async function () {
  const fixture = await readSharedPolicyFixture();

  for (const testCase of fixture.score_bands) {
    assert.equal(scoreBandLabel(testCase.score), testCase.label);
  }
});

test('shared action-ranking weights match the parity fixture', async function () {
  const fixture = await readSharedPolicyFixture();

  for (const testCase of fixture.action_kind_weights) {
    assert.equal(actionKindWeight(testCase.name), testCase.weight);
  }
  for (const testCase of fixture.action_leverage_weights) {
    assert.equal(actionLeverageWeight(testCase.name), testCase.weight);
  }
  for (const testCase of fixture.action_presentation_weights) {
    assert.equal(actionPresentationWeight(testCase.name), testCase.weight);
  }
});

test('shared report-selection priorities match the parity fixture', async function () {
  const fixture = await readSharedPolicyFixture();

  for (const testCase of fixture.report_leverage_priority) {
    assert.equal(reportLeveragePriority(testCase.name), testCase.priority);
  }
  for (const testCase of fixture.report_presentation_priority) {
    assert.equal(reportPresentationPriority(testCase.name), testCase.priority);
  }
});
