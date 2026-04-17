import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const signalPolicy = loadSignalPolicy();

function loadSignalPolicy() {
  const signalPolicyPath = path.resolve(__dirname, '../../.sentrux/signal-policy.json');
  return JSON.parse(readFileSync(signalPolicyPath, 'utf8'));
}

function actionWeight(weights, key, fallbackValue) {
  return weights[key] ?? fallbackValue;
}

function orderPriority(order, value) {
  const index = order.indexOf(value);
  return index === -1 ? order.length : index;
}

export function actionKindWeight(kind) {
  return actionWeight(signalPolicy.action_ranking.kind_weights, kind, 4);
}

export function actionLeverageWeight(leverageClass) {
  return actionWeight(signalPolicy.action_ranking.leverage_weights, leverageClass, 0);
}

export function actionPresentationWeight(presentationClass) {
  return actionWeight(signalPolicy.action_ranking.presentation_weights, presentationClass, 0);
}

export function reportLeveragePriority(leverageClass) {
  return orderPriority(signalPolicy.report_selection.leverage_order, leverageClass);
}

export function reportPresentationPriority(presentationClass) {
  return orderPriority(signalPolicy.report_selection.presentation_order, presentationClass);
}

export function scoreBandLabel(score) {
  for (const band of signalPolicy.score_bands) {
    if (score >= band.minimum_score) {
      return band.label;
    }
  }

  return 'supporting_signal';
}
