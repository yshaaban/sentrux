import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';

function summarizeResults(results) {
  const summary = {
    total: results.length,
    detected: 0,
    partial: 0,
    failed: 0,
    check_supported: 0,
    check_rules_detected: 0,
  };

  for (const result of results) {
    if (result.detected) {
      summary.detected += 1;
    }
    if (result.status === 'partial') {
      summary.partial += 1;
    } else if (result.status === 'fail') {
      summary.failed += 1;
    }
    if (result.check.supported) {
      summary.check_supported += 1;
    }
    if (result.check_rules?.matched) {
      summary.check_rules_detected += 1;
    }
  }

  return summary;
}

export function buildInjectionReport({
  repoLabel,
  repoRoot,
  generatedAt,
  defects,
  results,
}) {
  return {
    repo_label: repoLabel,
    repo_root: repoRoot,
    generated_at: generatedAt,
    summary: summarizeResults(results),
    defects,
    results,
  };
}

export function formatInjectionReportMarkdown(report) {
  const lines = [];
  lines.push('# Defect Injection Report');
  lines.push('');
  lines.push(`- repo: \`${report.repo_label}\``);
  lines.push(`- root: \`${report.repo_root}\``);
  lines.push(`- generated at: \`${report.generated_at}\``);
  lines.push(`- total defects: ${report.summary.total}`);
  lines.push(`- detected: ${report.summary.detected}`);
  lines.push(`- partial: ${report.summary.partial}`);
  lines.push(`- failed: ${report.summary.failed}`);
    lines.push(`- check supported: ${report.summary.check_supported}`);
  lines.push(`- check_rules detected: ${report.summary.check_rules_detected}`);
  lines.push('');
  lines.push('## Results');
  lines.push('');

  for (const result of report.results) {
    lines.push(`### ${result.defect_id}`);
    lines.push('');
    lines.push(`- title: ${result.title}`);
    lines.push(`- status: \`${result.status}\``);
    lines.push(`- check supported: ${result.check.supported}`);
    lines.push(`- check matched: ${result.check.matched}`);
    lines.push(`- check_rules matched: ${result.check_rules?.matched ?? false}`);
    lines.push(`- gate matched: ${result.gate.matched}`);
    lines.push(`- findings matched: ${result.findings.matched}`);
    lines.push(`- session_end matched: ${result.session_end.matched}`);
    if (result.check.evidence.length > 0) {
      lines.push(`- check evidence: \`${result.check.evidence.join(', ')}\``);
    }
    if (result.gate.evidence.length > 0) {
      lines.push(`- gate evidence: \`${result.gate.evidence.join(', ')}\``);
    }
    if ((result.check_rules?.evidence ?? []).length > 0) {
      lines.push(`- check_rules evidence: \`${result.check_rules.evidence.join(', ')}\``);
    }
    if (result.findings.evidence.length > 0) {
      lines.push(`- findings evidence: \`${result.findings.evidence.join(', ')}\``);
    }
    if (result.session_end.evidence.length > 0) {
      lines.push(`- session_end evidence: \`${result.session_end.evidence.join(', ')}\``);
    }
    lines.push('');
  }

  return `${lines.join('\n')}\n`;
}

export async function writeInjectionReportFiles({
  report,
  jsonPath,
  markdownPath,
}) {
  if (jsonPath) {
    await mkdir(path.dirname(jsonPath), { recursive: true });
    await writeFile(jsonPath, `${JSON.stringify(report, null, 2)}\n`, 'utf8');
  }

  if (markdownPath) {
    const markdown = formatInjectionReportMarkdown(report);
    await mkdir(path.dirname(markdownPath), { recursive: true });
    await writeFile(markdownPath, markdown, 'utf8');
  }
}
