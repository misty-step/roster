# Crucible: epochs — run each task k times; pass@k / pass^k / variance-honest intervals

Status: ready · Priority: p2

Observed live 2026-07-08: minimax-m2 on commit-message-quality-v1 scored 28/30 in two consecutive runs with DIFFERENT failing task sets (docs-interval-widen fail->pass, fix-sheet-overflow pass->fail) — temp-0 is not deterministic across OpenRouter invocations. Every single-run score Crucible reports carries invisible run-to-run variance the interval does not include.

## What to build
Inspect-style epochs: `EvalSpec` (or run flag) declares `epochs: k`; the runner executes each task k times and a declared reducer aggregates: pass@k (any success — coding-style), pass^k (all succeed — consistency-style), or mean rate with between-epoch variance folded into the reported interval (Miller Rec 3: resample rather than tune temperature; Eve/Anthropic: the pass@k-vs-pass^k choice is a product decision that diverges fast). Persist per-epoch results (composes with run_records + matrix cells); config identity includes k + reducer. Single-run reports gain an explicit "1 epoch" caveat in runs list/show/UI.

## Oracle
- [ ] A spec/run can declare epochs k>1 + reducer; per-epoch task results persisted.
- [ ] pass@k and pass^k both computable from the same run; headline states which.
- [ ] Interval for mean-rate reducer widens with observed between-epoch variance (test with synthetic flip data).
- [ ] 1-epoch runs display the caveat.

Provenance: eval-design skill already teaches the pass@k/pass^k choice; the runner cannot yet execute it. Related: crucible-truncation-aware-grading, crucible-run-evidence-overwrite (per-epoch evidence paths must not collide — same root cause).
