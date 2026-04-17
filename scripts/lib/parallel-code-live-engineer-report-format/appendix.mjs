import path from 'node:path';

import { selectLeverageBuckets } from '../v2-report-selection.mjs';
import { isHeadCloneAnalysis } from './identity.mjs';
import {
  appendAnalysisHeader,
  appendAppendixMethodSection,
  appendAppendixScanCoverageSection,
  appendCodeList,
  appendRepoLinkList,
  buildAnalysisGeneratedLine,
  buildAppendixHeading,
  finalizeMarkdown,
  formatScopeBullet,
  formatScopeHeading,
} from './common.mjs';

function buildFindingDetailKey(scope, kind) {
  return `${scope}\u0000${kind}`;
}

function buildFindingDetailMap(findings) {
  const detailMap = new Map();
  for (const detail of findings.finding_details ?? []) {
    detailMap.set(buildFindingDetailKey(detail.scope, detail.kind), detail);
  }
  return detailMap;
}

function appendCandidateEvidence(lines, detail) {
  lines.push('- evidence:');
  if ((detail?.role_tags ?? []).length > 0) {
    lines.push(`  - role tags: \`${detail.role_tags.join(', ')}\``);
  }
  for (const [metric, value] of Object.entries(detail?.metrics ?? {})) {
    lines.push(`  - ${metric.replaceAll('_', ' ')}: \`${value}\``);
  }
  for (const evidence of detail?.evidence ?? []) {
    lines.push(`  - ${evidence}`);
  }
}

function appendLeadCandidateBlock(lines, candidate, detail, repoRoot) {
  lines.push(`### ${formatScopeHeading(repoRoot, candidate.scope)}`);
  lines.push('');
  lines.push(`- \`${candidate.trust_tier ?? 'trusted'}\``);
  lines.push(`- class: \`${candidate.presentation_class}\``);
  lines.push(`- leverage: \`${candidate.leverage_class}\``);
  lines.push(`- signal band: \`${candidate.score_band ?? 'supporting_signal'}\``);
  lines.push(`- \`${candidate.kind}\``);
  lines.push(`- summary: \`${candidate.summary}\``);
  lines.push(`- impact: ${candidate.impact}`);
  appendCodeList(lines, 'leverage reasons', candidate.leverage_reasons);
  appendCodeList(lines, 'ranking reasons', candidate.ranking_reasons);
  appendCandidateEvidence(lines, detail);
  appendCodeList(lines, 'candidate split axes', candidate.candidate_split_axes);
  appendRepoLinkList(lines, 'related surfaces', repoRoot, candidate.related_surfaces, 5);
  lines.push('');
}

function appendLeverageSummarySection(lines, leadCandidates, detailMap, repoRoot) {
  lines.push('## Leverage Summary');
  lines.push('');
  for (const candidate of leadCandidates) {
    const detail = detailMap.get(buildFindingDetailKey(candidate.scope, candidate.kind));
    appendLeadCandidateBlock(lines, candidate, detail, repoRoot);
  }
}

function appendCompactCandidateSection(lines, title, candidates) {
  lines.push(`## ${title}`);
  lines.push('');
  for (const candidate of candidates) {
    lines.push(
      `- \`${candidate.scope}\` \`${candidate.leverage_class}\` \`${candidate.score_band ?? 'supporting_signal'}\` ${candidate.summary}`,
    );
  }
  if (candidates.length === 0) {
    lines.push('- none');
  }
  lines.push('');
}

function appendHardeningNotesSection(lines, hardeningNotes) {
  lines.push('## Targeted Hardening Notes');
  lines.push('');
  for (const candidate of hardeningNotes) {
    lines.push(`- \`${candidate.scope}\` ${candidate.summary}`);
  }
  if (hardeningNotes.length === 0) {
    lines.push('- none');
  }
  lines.push('');
}

function appendToolingDebtSection(lines, toolingDebt, repoRoot) {
  lines.push('## Tooling Debt');
  lines.push('');
  for (const candidate of toolingDebt) {
    lines.push(`- ${formatScopeBullet(repoRoot, candidate.scope)} ${candidate.summary}`);
  }
  if (toolingDebt.length === 0) {
    lines.push('- none');
  }
  lines.push('');
}

function appendTopWatchpointsSection(lines, watchpoints) {
  lines.push('## Top Watchpoints');
  lines.push('');
  for (const watchpoint of watchpoints.slice(0, 6)) {
    lines.push(`### ${watchpoint.scope}`);
    lines.push('');
    lines.push(`- \`${watchpoint.trust_tier ?? 'watchpoint'}\``);
    lines.push(`- leverage: \`${watchpoint.leverage_class ?? 'secondary_cleanup'}\``);
    lines.push(`- signal band: \`${watchpoint.score_band ?? 'supporting_signal'}\``);
    lines.push(`- \`${watchpoint.kind}\``);
    lines.push(`- summary: \`${watchpoint.summary}\``);
    appendCodeList(lines, 'ranking reasons', watchpoint.ranking_reasons);
    if (watchpoint.metrics?.length > 0) {
      lines.push('- evidence:');
      for (const metric of watchpoint.metrics) {
        if (metric.value === undefined || metric.value === null) {
          continue;
        }
        lines.push(`  - ${metric.label}: \`${metric.value}\``);
      }
    }
    if (watchpoint.cut_candidates?.length > 0) {
      lines.push('- candidate cuts:');
      for (const candidate of watchpoint.cut_candidates.slice(0, 3)) {
        lines.push(`  - \`${candidate.from} -> ${candidate.to}\``);
        lines.push(`    - seam kind: \`${candidate.seam_kind}\``);
        lines.push(`    - reduction: \`${candidate.reduction}\``);
      }
    }
    lines.push('');
  }
}

function appendTrustedDebtClustersSection(lines, trustedClusters) {
  lines.push('## Trusted Debt Clusters');
  lines.push('');
  for (const cluster of trustedClusters.slice(0, 5)) {
    lines.push(`### ${cluster.scope}`);
    lines.push('');
    lines.push(`- summary: \`${cluster.summary}\``);
    lines.push(`- trust tier: \`${cluster.trust_tier}\``);
    if (cluster.signal_kinds?.length > 0) {
      lines.push('- signal kinds:');
      for (const signalKind of cluster.signal_kinds) {
        lines.push(`  - \`${signalKind}\``);
      }
    }
    if (cluster.role_tags?.length > 0) {
      lines.push(`- role tags: \`${cluster.role_tags.join(', ')}\``);
    }
    lines.push('');
  }
}

function appendExperimentalSideChannelSection(lines, snapshot, experimentalSignals, repoRoot) {
  lines.push('## Experimental Side Channel');
  lines.push('');
  lines.push('Current experimental counts:');
  lines.push('');
  lines.push(`- experimental findings: \`${snapshot.experimental_findings.length}\``);
  lines.push(`- experimental debt signals: \`${experimentalSignals.length}\``);
  lines.push('');
  if (experimentalSignals.length > 0) {
    lines.push('Representative examples:');
    lines.push('');
    for (const signal of experimentalSignals.slice(0, 5)) {
      lines.push(`- ${formatScopeBullet(repoRoot, signal.scope)}`);
    }
    lines.push('');
  }
  lines.push('Current rule:');
  lines.push('');
  lines.push('- these are visible for analyzer follow-up');
  lines.push('- they should not be used as maintainer-facing debt guidance until the detector is fixed');
  lines.push('');
}

function appendConceptSummariesSection(lines, conceptSummaries) {
  lines.push('## Configured Concepts And Current State');
  lines.push('');
  for (const concept of conceptSummaries.slice(0, 5)) {
    lines.push(`### \`${concept.concept_id}\``);
    lines.push('');
    lines.push(`- score: \`${concept.score_0_10000 ?? 'n/a'} / 10000\``);
    lines.push(`- missing update sites: \`${concept.missing_site_count ?? 0}\``);
    lines.push(`- boundary pressure count: \`${concept.boundary_pressure_count ?? 0}\``);
    if ((concept.dominant_kinds ?? []).length > 0) {
      lines.push(`- dominant finding kinds: \`${concept.dominant_kinds.join(', ')}\``);
    }
    if (concept.summary) {
      lines.push(`- summary: ${concept.summary}`);
    }
    lines.push('');
  }
}

export function buildLiveEngineerAppendix({
  snapshot,
  findings,
  scan,
  metadata,
  reportMarkdownPath,
  repoRoot,
}) {
  const lines = [];
  const headCloneAnalysis = isHeadCloneAnalysis(metadata);
  const leverageBuckets = selectLeverageBuckets(findings);
  const trustedClusters = snapshot.debt_clusters.filter((cluster) => cluster.trust_tier === 'trusted');
  const experimentalSignals = findings.experimental_debt_signals ?? snapshot.experimental_debt_signals ?? [];
  const detailMap = buildFindingDetailMap(findings);
  const appendixSections = [
    { title: 'Architecture Signals', candidates: leverageBuckets.architecture_signals },
    { title: 'Best Local Refactor Targets', candidates: leverageBuckets.local_refactor_targets },
    { title: 'Boundary Discipline', candidates: leverageBuckets.boundary_discipline },
    { title: 'Regrowth Watchpoints', candidates: leverageBuckets.regrowth_watchpoints },
    { title: 'Secondary Cleanup', candidates: leverageBuckets.secondary_cleanup },
  ];

  appendAnalysisHeader(
    lines,
    buildAppendixHeading(headCloneAnalysis),
    buildAnalysisGeneratedLine(snapshot, metadata, headCloneAnalysis),
    [
      'This appendix contains the evidence behind',
      `[${path.basename(reportMarkdownPath)}](${reportMarkdownPath}).`,
    ],
  );
  appendAppendixMethodSection(lines, metadata, repoRoot, headCloneAnalysis);
  appendAppendixScanCoverageSection(lines, scan);
  appendLeverageSummarySection(
    lines,
    leverageBuckets.summary_candidates,
    detailMap,
    metadata.parallel_code_root,
  );
  for (const section of appendixSections) {
    appendCompactCandidateSection(lines, section.title, section.candidates);
  }
  appendHardeningNotesSection(lines, leverageBuckets.hardening_notes);
  appendToolingDebtSection(lines, leverageBuckets.tooling_debt, metadata.parallel_code_root);
  appendTopWatchpointsSection(lines, leverageBuckets.trusted_watchpoints);
  appendTrustedDebtClustersSection(lines, trustedClusters);
  appendExperimentalSideChannelSection(
    lines,
    snapshot,
    experimentalSignals,
    metadata.parallel_code_root,
  );
  appendConceptSummariesSection(lines, snapshot.concept_summaries);

  return finalizeMarkdown(lines);
}
