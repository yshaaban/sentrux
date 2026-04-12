# Defect Injection Report

- repo: `sentrux`
- root: `<sentrux-root>`
- generated at: `2026-04-12T09:34:52.135Z`
- total defects: 4
- detected: 4
- partial: 0
- failed: 0
- check supported: 4
- check_rules detected: 0

## Results

### self_large_file

- title: Append 120 lines to the benchmark harness
- status: `pass`
- check supported: true
- check matched: true
- check_rules matched: false
- gate matched: false
- findings matched: true
- session_end matched: true
- check evidence: `$.actions[0]:large_file, $.issues[0]:large_file, decision=warn`
- gate evidence: `$.introduced_findings[0]:large_file`
- findings evidence: `$.debt_signals[0]:large_file, $.debt_signals[1]:large_file, $.debt_signals[2]:large_file, $.debt_signals[3]:large_file, $.debt_signals[4]:large_file, $.finding_details[0]:large_file, $.finding_details[1]:large_file, $.finding_details[5]:large_file, $.finding_details[6]:large_file, $.finding_details[7]:large_file, $.finding_details[8]:large_file, $.finding_details[9]:large_file, $.finding_details[10]:large_file, $.finding_details[11]:large_file, $.findings[0]:large_file, $.findings[1]:large_file, $.findings[5]:large_file, $.findings[6]:large_file, $.findings[7]:large_file, $.findings[8]:large_file, $.findings[9]:large_file, $.findings[10]:large_file, $.findings[11]:large_file`
- session_end evidence: `$.actions[0]:large_file, $.debt_signals[0]:large_file, $.finding_details[0]:large_file, $.introduced_findings[0]:large_file`

### self_forbidden_raw_read

- title: Read task presentation status through a forbidden raw access path
- status: `pass`
- check supported: true
- check matched: true
- check_rules matched: false
- gate matched: false
- findings matched: true
- session_end matched: false
- check evidence: `$.actions[1]:forbidden_raw_read, $.issues[1]:forbidden_raw_read`
- findings evidence: `$.finding_details[5]:forbidden_raw_read, $.findings[5]:forbidden_raw_read`

### self_incomplete_propagation

- title: Change one defect-injection surface without updating its sibling contract sites
- status: `pass`
- check supported: true
- check matched: true
- check_rules matched: false
- gate matched: false
- findings matched: true
- session_end matched: false
- check evidence: `$.actions[0]:incomplete_propagation, $.actions[1]:incomplete_propagation, $.issues[0]:incomplete_propagation, $.issues[1]:incomplete_propagation`
- findings evidence: `$.finding_details[5]:contract_surface_completeness, $.findings[5]:contract_surface_completeness`

### self_session_introduced_clone

- title: Introduce a fresh duplicate helper after the session baseline
- status: `pass`
- check supported: true
- check matched: true
- check_rules matched: false
- gate matched: false
- findings matched: false
- session_end matched: true
- check evidence: `$.actions[0]:session_introduced_clone, $.actions[1]:session_introduced_clone, $.issues[0]:session_introduced_clone, $.issues[1]:session_introduced_clone`
- session_end evidence: `$.actions[0]:session_introduced_clone, $.actions[1]:session_introduced_clone, $.finding_details[0]:session_introduced_clone, $.finding_details[1]:session_introduced_clone, $.introduced_clone_findings[0]:session_introduced_clone, $.introduced_clone_findings[1]:session_introduced_clone, $.introduced_findings[0]:session_introduced_clone, $.introduced_findings[1]:session_introduced_clone`

