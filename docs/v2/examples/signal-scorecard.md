# Signal Quality Scorecard

- repo: `sentrux`
- generated at: `2026-04-18T12:41:33.501Z`
- signals: 8
- trusted: 3
- watchpoint: 3
- needs review: 0
- degrade or quarantine: 0
- complete promotion evidence: 4
- seeded samples: 6
- reviewed samples: 12
- provisional reviewed samples: 0
- remediation samples: 6
- sessions: 15
- session verdicts: 6
- actionable reviewed samples: 12
- top-1 actionable precision: 1 (1/1)
- top-3 actionable precision: 1 (3/3)
- top-10 actionable precision: 1 (10/10)
- ranking preference satisfaction: 1
- rank preserved rate: 1
- repair packet complete rate: 1
- repair packet fix-surface clarity: 1
- repair packet verification clarity: 1
- sample helpfulness mean: 1.833
- sample distraction cost mean: 0.583
- primary-target policy: pass
- thrashing sessions: 0
- top-action sessions: 2
- agent clear rate: 0 (0/2)
- follow-up regression session rate: 0 (0/2)
- regression-after-fix rate: 0 (0/2)
- session clean rate: 0 (0/2)
- session thrash rate: 0
- average checks to clear: n/a
- average entropy delta: 0
- top-action follow rate: 0.333
- top-action help rate: 0
- task success rate: 0.667
- patch expansion rate: 0
- intervention cost checks mean: 0.667
- intervention net value score: 0.333

| Signal | Family | Status | Primary Lane | Seeded Recall | Primary Recall | Reviewed Precision | Noise Rate | Remediation Success | Trials | Top Action Sessions | Trial Miss Rate | Top Action Clear | Regression Rate | Session Clean Rate | Thrash Rate | Avg Entropy Delta | Avg Checks To Clear | Latency | Recommendation |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `authoritative_import_bypass` | `unknown` | `unspecified` | `n/a` | n/a | n/a | 1 | 0 | n/a | 0 | 0 | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | `keep_unspecified` |
| `clone_propagation_drift` | `clone` | `watchpoint` | `check` | 1 | 1 | 1 | 0 | 1 | 4 | 0 | 0.75 | n/a | n/a | n/a | n/a | n/a | n/a | 39.1 | `keep_watchpoint` |
| `forbidden_raw_read` | `rules` | `trusted` | `check` | 1 | 1 | 1 | 0 | 1 | 1 | 0 | 0 | n/a | n/a | n/a | n/a | n/a | n/a | 39.1 | `keep_trusted` |
| `incomplete_propagation` | `obligation` | `trusted` | `check` | 1 | 1 | 1 | 0 | 1 | 1 | 0 | 0 | n/a | n/a | n/a | n/a | n/a | n/a | 39.1 | `keep_trusted` |
| `large_file` | `structural` | `trusted` | `check` | 1 | 1 | n/a | n/a | 1 | 0 | 2 | n/a | 0 | 0 | 0 | 0 | 0 | n/a | 39.1 | `improve_fix_guidance` |
| `missing_test_coverage` | `unknown` | `unspecified` | `n/a` | n/a | n/a | 1 | 0 | n/a | 0 | 0 | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | `keep_unspecified` |
| `session_introduced_clone` | `clone` | `watchpoint` | `check` | 1 | 1 | 1 | 0 | 1 | 1 | 0 | 0 | n/a | n/a | n/a | n/a | n/a | n/a | 39.1 | `keep_watchpoint` |
| `zero_config_boundary_violation` | `rules` | `watchpoint` | `check` | 1 | 1 | n/a | n/a | 1 | 1 | 0 | 0 | n/a | n/a | n/a | n/a | n/a | n/a | 39.1 | `keep_watchpoint` |
