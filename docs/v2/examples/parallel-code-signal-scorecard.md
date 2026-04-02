# Signal Quality Scorecard

- repo: `parallel-code`
- generated at: `2026-04-02T12:13:38.359Z`
- signals: 8
- trusted: 3
- watchpoint: 1
- needs review: 0
- degrade or quarantine: 1

| Signal | Family | Status | Primary Lane | Seeded Recall | Primary Recall | Reviewed Precision | Useful Precision | Remediation Success | Latency | Recommendation |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `closed_domain_exhaustiveness` | `obligation` | `trusted` | `check` | 1 | 1 | 1 | 1 | 1 | 66.7 | `keep_trusted` |
| `cycle_cluster` | `unknown` | `unspecified` | `n/a` | n/a | n/a | 1 | 0 | n/a | n/a | `keep_unspecified` |
| `dead_private_code_cluster` | `unknown` | `unspecified` | `n/a` | n/a | n/a | 0 | 0 | n/a | n/a | `degrade_or_quarantine` |
| `dependency_sprawl` | `unknown` | `unspecified` | `n/a` | n/a | n/a | 1 | 0.75 | n/a | n/a | `keep_unspecified` |
| `forbidden_raw_read` | `rules` | `trusted` | `check` | 1 | 1 | n/a | n/a | 0 | 66.7 | `improve_fix_guidance` |
| `large_file` | `structural` | `trusted` | `check` | 1 | 1 | 1 | 1 | n/a | 66.7 | `keep_trusted` |
| `missing_test_coverage` | `structural` | `watchpoint` | `check` | 1 | 1 | n/a | n/a | 1 | 66.7 | `keep_watchpoint` |
| `unstable_hotspot` | `unknown` | `unspecified` | `n/a` | n/a | n/a | 1 | 1 | n/a | n/a | `keep_unspecified` |

