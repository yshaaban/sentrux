# Defect Injection Report

- repo: `parallel-code`
- root: `<parallel-code-root>`
- generated at: `2026-04-11T20:29:52.405Z`
- total defects: 6
- detected: 6
- partial: 0
- failed: 0
- check supported: 6
- check_rules detected: 0

## Results

### forbidden_raw_read

- title: Read task status directly from SidebarTaskRow.tsx
- status: `pass`
- check supported: true
- check matched: true
- check_rules matched: false
- gate matched: false
- findings matched: false
- session_end matched: true
- check evidence: `$.actions[0]:forbidden_raw_read, $.issues[0]:forbidden_raw_read, decision=fail`
- gate evidence: `$.introduced_findings[2]:forbidden_raw_read`
- session_end evidence: `$.actions[0]:forbidden_raw_read, $.finding_details[2]:forbidden_raw_read, $.introduced_findings[2]:forbidden_raw_read`

### large_file_growth

- title: Append 120 lines to SidebarTaskRow.tsx
- status: `pass`
- check supported: true
- check matched: true
- check_rules matched: false
- gate matched: true
- findings matched: true
- session_end matched: true
- check evidence: `$.actions[2]:large_file, $.issues[2]:large_file`
- gate evidence: `$.introduced_findings[0]:large_file`
- findings evidence: `$.debt_signals[0]:large_file, $.debt_signals[1]:large_file, $.debt_signals[2]:large_file, $.debt_signals[3]:large_file, $.debt_signals[4]:large_file, $.finding_details[3]:large_file, $.finding_details[4]:large_file, $.finding_details[5]:large_file, $.finding_details[6]:large_file, $.finding_details[7]:large_file, $.finding_details[8]:large_file, $.finding_details[9]:large_file, $.findings[3]:large_file, $.findings[4]:large_file, $.findings[5]:large_file, $.findings[6]:large_file, $.findings[7]:large_file, $.findings[8]:large_file, $.findings[9]:large_file`
- session_end evidence: `$.actions[2]:large_file, $.debt_signals[1]:large_file, $.finding_details[0]:large_file, $.introduced_findings[0]:large_file, $.resolved_findings[0]:large_file`

### missing_exhaustiveness

- title: Add a TaskDotStatus variant without updating consumers
- status: `pass`
- check supported: true
- check matched: true
- check_rules matched: false
- gate matched: false
- findings matched: true
- session_end matched: true
- check evidence: `$.actions[0]:closed_domain_exhaustiveness, $.issues[0]:closed_domain_exhaustiveness, decision=fail`
- gate evidence: `$.blocking_findings[1]:closed_domain_exhaustiveness, $.introduced_findings[2]:closed_domain_exhaustiveness, $.missing_obligations[0]:closed_domain_exhaustiveness`
- findings evidence: `$.finding_details[0]:closed_domain_exhaustiveness, $.findings[0]:closed_domain_exhaustiveness`
- session_end evidence: `$.actions[0]:closed_domain_exhaustiveness, $.actions[1]:closed_domain_exhaustiveness, $.finding_details[2]:closed_domain_exhaustiveness, $.introduced_findings[2]:closed_domain_exhaustiveness, $.missing_obligations[0]:closed_domain_exhaustiveness, $.resolved_findings[2]:closed_domain_exhaustiveness, $.touched_concept_gate.blocking_findings[1]:closed_domain_exhaustiveness`

### missing_test

- title: Add a new production helper without a sibling test
- status: `pass`
- check supported: true
- check matched: true
- check_rules matched: false
- gate matched: true
- findings matched: false
- session_end matched: false
- check evidence: `$.actions[0]:missing_test_coverage, $.issues[0]:missing_test_coverage, decision=pass`
- gate evidence: `decision=pass`

### multi_writer_concept

- title: Write task git status from task-presentation-status.ts
- status: `pass`
- check supported: true
- check matched: true
- check_rules matched: false
- gate matched: true
- findings matched: true
- session_end matched: true
- check evidence: `$.actions[0]:multi_writer_concept, $.issues[0]:multi_writer_concept, decision=fail`
- gate evidence: `$.blocking_findings[1]:multi_writer_concept, $.introduced_findings[1]:multi_writer_concept, decision=fail`
- findings evidence: `$.finding_details[0]:multi_writer_concept, $.findings[0]:multi_writer_concept`
- session_end evidence: `$.actions[0]:multi_writer_concept, $.finding_details[1]:multi_writer_concept, $.introduced_findings[1]:multi_writer_concept, $.touched_concept_gate.blocking_findings[1]:multi_writer_concept`

### writer_outside_allowlist

- title: Write task git status from a non-owner presentation module
- status: `pass`
- check supported: true
- check matched: true
- check_rules matched: false
- gate matched: true
- findings matched: true
- session_end matched: true
- check evidence: `$.actions[1]:writer_outside_allowlist, $.issues[1]:writer_outside_allowlist, decision=fail`
- gate evidence: `$.blocking_findings[2]:writer_outside_allowlist, $.introduced_findings[2]:writer_outside_allowlist, decision=fail`
- findings evidence: `$.finding_details[1]:writer_outside_allowlist, $.findings[1]:writer_outside_allowlist`
- session_end evidence: `$.actions[1]:writer_outside_allowlist, $.finding_details[2]:writer_outside_allowlist, $.introduced_findings[2]:writer_outside_allowlist, $.touched_concept_gate.blocking_findings[2]:writer_outside_allowlist`

