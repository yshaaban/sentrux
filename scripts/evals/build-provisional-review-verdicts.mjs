#!/usr/bin/env node

import { mkdir, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, '../..');

const DEFAULT_KIND_POLICY = {
  category: 'useful_watchpoint',
  expected_trust_tier: 'watchpoint',
  expected_presentation_class: 'watchpoint',
  expected_leverage_class: 'local_refactor_target',
  expected_summary_presence: 'section_present',
};

const KIND_POLICY = {
  closed_domain_exhaustiveness: {
    category: 'useful',
    expected_trust_tier: 'trusted',
    expected_presentation_class: 'hardening_note',
    expected_leverage_class: 'hardening_note',
    expected_summary_presence: 'section_present',
  },
  forbidden_raw_read: {
    category: 'useful',
    expected_trust_tier: 'trusted',
    expected_presentation_class: 'boundary_discipline',
    expected_leverage_class: 'boundary_discipline',
    expected_summary_presence: 'headline',
  },
  incomplete_propagation: {
    category: 'useful',
    expected_trust_tier: 'watchpoint',
    expected_presentation_class: 'hardening_note',
    expected_leverage_class: 'hardening_note',
    expected_summary_presence: 'section_present',
  },
  session_introduced_clone: {
    category: 'useful_watchpoint',
    expected_trust_tier: 'watchpoint',
    expected_presentation_class: 'watchpoint',
    expected_leverage_class: 'local_refactor_target',
    expected_summary_presence: 'section_present',
  },
  clone_propagation_drift: {
    category: 'useful_watchpoint',
    expected_trust_tier: 'watchpoint',
    expected_presentation_class: 'watchpoint',
    expected_leverage_class: 'local_refactor_target',
    expected_summary_presence: 'section_present',
  },
  multi_writer_concept: {
    category: 'useful',
    expected_trust_tier: 'watchpoint',
    expected_presentation_class: 'boundary_discipline',
    expected_leverage_class: 'boundary_discipline',
    expected_summary_presence: 'section_present',
  },
  writer_outside_allowlist: {
    category: 'useful',
    expected_trust_tier: 'watchpoint',
    expected_presentation_class: 'boundary_discipline',
    expected_leverage_class: 'boundary_discipline',
    expected_summary_presence: 'section_present',
  },
  missing_test_coverage: {
    category: 'useful_watchpoint',
    expected_trust_tier: 'watchpoint',
    expected_presentation_class: 'watchpoint',
    expected_leverage_class: 'regrowth_watchpoint',
    expected_summary_presence: 'section_present',
  },
  large_file: {
    category: 'useful_watchpoint',
    expected_trust_tier: 'watchpoint',
    expected_presentation_class: 'structural_debt',
    expected_leverage_class: 'regrowth_watchpoint',
    expected_summary_presence: 'section_present',
  },
  touched_clone_family: {
    category: 'low_value',
    expected_trust_tier: 'experimental',
    expected_presentation_class: 'experimental',
    expected_leverage_class: 'experimental',
    expected_summary_presence: 'side_channel',
  },
};

export function parseArgs(argv) {
  const result = {
    packetPath: null,
    outputJsonPath: null,
    sourceReport: null,
    sourceFeedback: null,
    repo: null,
    kinds: [],
  };

  for (let index = 2; index < argv.length; index += 1) {
    const value = argv[index];
    if (value === '--packet') {
      index += 1;
      result.packetPath = argv[index];
      continue;
    }
    if (value === '--output-json') {
      index += 1;
      result.outputJsonPath = argv[index];
      continue;
    }
    if (value === '--source-report') {
      index += 1;
      result.sourceReport = argv[index];
      continue;
    }
    if (value === '--source-feedback') {
      index += 1;
      result.sourceFeedback = argv[index];
      continue;
    }
    if (value === '--repo') {
      index += 1;
      result.repo = argv[index];
      continue;
    }
    if (value === '--kind') {
      index += 1;
      result.kinds.push(argv[index]);
      continue;
    }
    throw new Error(`Unknown argument: ${value}`);
  }

  if (!result.packetPath) {
    throw new Error('Missing required --packet path');
  }

  return result;
}

async function readJson(targetPath) {
  const source = await readFile(targetPath, 'utf8');
  return JSON.parse(source);
}

function getKindPolicy(kind) {
  return KIND_POLICY[kind] ?? DEFAULT_KIND_POLICY;
}

function buildEngineerNote(sample) {
  const summary = sample.summary ?? 'Review this finding against the changed scope.';
  const repairPacket = sample.repair_packet ?? null;
  if (repairPacket?.complete === false) {
    return `${summary} Provisional verdict derived from packet metadata; confirm usefulness, relative rank, and whether missing repair guidance (${repairPacket.missing_fields.join(', ')}) keeps this out of the primary surface.`;
  }

  return `${summary} Provisional verdict derived from packet metadata; confirm usefulness and relative rank manually.`;
}

function buildExpectedBehavior(sample) {
  const leadSurface = sample.rank <= 3 ? 'lead surface' : 'supporting surface';
  const repairPacket = sample.repair_packet ?? null;
  if (repairPacket?.complete === false) {
    return `Keep ${sample.kind ?? 'this finding'} on the ${leadSurface} only if manual review confirms the rank and the missing repair guidance is filled in before promotion-grade use.`;
  }

  return `Keep ${sample.kind ?? 'this finding'} visible on the ${leadSurface} with the recorded trust/presentation defaults unless manual review shows it is too noisy or misranked.`;
}

function shouldIncludeSample(sample, allowedKinds) {
  if (allowedKinds.size === 0) {
    return true;
  }

  return allowedKinds.has(sample.kind);
}

export function buildVerdicts(packet, args) {
  const allowedKinds = new Set(args.kinds);

  return (packet.samples ?? [])
    .filter((sample) => shouldIncludeSample(sample, allowedKinds))
    .map((sample) => {
      const policy = getKindPolicy(sample.kind);
      return {
        scope: sample.scope ?? sample.source_label ?? 'unknown-scope',
        kind: sample.kind ?? 'unknown-kind',
        report_bucket: sample.report_bucket ?? packet.tool ?? 'packet',
        category: policy.category,
        expected_trust_tier: policy.expected_trust_tier,
        expected_presentation_class: policy.expected_presentation_class,
        expected_leverage_class: policy.expected_leverage_class,
        expected_summary_presence: policy.expected_summary_presence,
        preferred_over: [],
        engineer_note: buildEngineerNote(sample),
        expected_v2_behavior: buildExpectedBehavior(sample),
      };
    });
}

export function buildProvisionalReviewVerdictReport(packet, args) {
  return {
    repo: args.repo ?? path.basename(packet.repo_root ?? 'unknown'),
    captured_at: packet.generated_at,
    source_report: args.sourceReport ?? args.packetPath,
    source_feedback:
      args.sourceFeedback ??
      'Provisional AI-curated review verdicts generated from packet metadata. Keep verdict order rank-preserving because top-1/top-3/top-10 actionable precision uses this order. Replace or confirm manually before treating this as promotion-grade evidence.',
    provisional: true,
    verdicts: buildVerdicts(packet, args),
  };
}

async function main() {
  const args = parseArgs(process.argv);
  const packet = await readJson(args.packetPath);
  const report = buildProvisionalReviewVerdictReport(packet, args);
  const outputJsonPath =
    args.outputJsonPath ??
    path.join(
      repoRoot,
      'docs/v2/examples',
      `${path.parse(args.packetPath).name}-provisional-verdicts.json`,
    );

  await mkdir(path.dirname(outputJsonPath), { recursive: true });
  await writeFile(outputJsonPath, `${JSON.stringify(report, null, 2)}\n`, 'utf8');
  console.log(`Wrote ${report.verdicts.length} provisional verdict(s) to ${outputJsonPath}`);
}

const invokedPath = process.argv[1] ? pathToFileURL(path.resolve(process.argv[1])).href : null;

if (invokedPath === import.meta.url) {
  main().catch((error) => {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  });
}
