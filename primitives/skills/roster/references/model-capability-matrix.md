---
review_due: 2026-08-17
last_reviewed: 2026-07-17
---

# Model Capability Matrix

One row per model — not per provider/model pair. Provider choice (native
subscription vs. OpenRouter) is a *routing* decision layered on top of this
matrix, not a capability dimension: the same model scores the same regardless
of which door you dispatch it through. Prefer the subscribed native provider;
fall back to OpenRouter when a subscription is capped or unavailable. See
`peer-harnesses/references/model-provider-harness-index.md` for exact ids,
pricing, and context windows per provider — this file does not duplicate that
data.

## Method (read before trusting a number)

We do not have enough first-party evals to score every model on every
dimension from measured data alone. Each cell below is a synthesis of:

1. **Measured** — Crucible Wilson-scored pass rates, where a run exists
   (`crucible_runs_list`/`runs_pivot`). Reported in the dedicated column,
   with `n` and the benchmark id. This is the only column with a real
   confidence interval.
2. **Ledger** — incident-level judgment from `model-capability-ledger.md`
   (this repo) — real lane failures/standouts, not vibes.
3. **Vendor/community positioning** — provider docs, launch coverage, and
   third-party benchmark claims recorded in `model-provider-harness-index.md`
   (e.g. Grok 4.5's Harvey Legal Agent Benchmark #1 claim, Opus-class
   positioning).
4. **Operator/session vibe check** — where none of the above exists. Marked
   implicitly by the absence of a Measured score; do not treat these cells as
   more precise than "roughly right, low confidence."

Scale: 1-10 per capability column, curated not computed. **Do not** recompute
this file mechanically from Crucible — Crucible's own benchmark roster skews
cheap/open-model (that's its design purpose; see its own doctrine), so most
frontier proprietary models (Fable 5, Sol, Luna, Opus 4.8) have **no**
Crucible coverage at all. That gap is itself evidence for the operator's
2026-07-17 ruling: aggregate whatever exists (measured + ledger + community +
vibe) rather than waiting for full first-party coverage before this file is
useful. Re-score a row when a new Crucible run lands for it, a ledger entry
names a standout/failure, or a new model version ships.

## The matrix

| Model | Coding/Impl | Architecture/Reasoning | Writing/Communication | Design/Visual | Research/Synthesis | Adversarial/Critique | Agentic Autonomy | Crucible measured |
|---|---:|---:|---:|---:|---:|---:|---:|---|
| Fable 5 (`claude-fable-5`) | 9 | 10 | 8 | 6 | 8 | 7 | 8 | — untested (frontier tier has no Crucible coverage) |
| Sonnet 5 (`claude-sonnet-5`) | 9 | 7 | 7 | 5 | 6 | 6 | 9 | 91.2% (n=34, constraint-gauntlet-v1) |
| Opus 4.8 (`claude-opus-4-8`) | 8 | 8 | 7 | 5 | 7 | 6 | 7 | — untested; **operator-deprioritized 2026-07-16, prefer Fable 5 / Sonnet 5** |
| Sol (`gpt-5.6-sol`) | 8 | 10 | 6 | 4 | 7 | 7 | 7 | — untested (predecessor `gpt-5.4` scored 85.3%, n=34 — indirect signal only, not the same model) |
| Luna (`gpt-5.6-luna`) | 10 | 7 | 5 | 4 | 5 | 5 | 9 | — untested (same `gpt-5.4` caveat as Sol) |
| Grok 4.5 (`x-ai/grok-4.5`) | 8 | 8 | 7 | 5 | 7 | 10 | 7 | 88.2% (n=34, constraint-gauntlet-v1) |
| GLM 5.2 (`z-ai/glm-5.2`) | 7 | 6 | 6 | 9 | 6 | 5 | 6 | 85.3% (n=34, v1); 100% (n=32, card-oracle-triage) |
| Gemini 3.5 Flash (`gemini-3.5-flash`) | 6 | 6 | 6 | 6 | 7 | 5 | 5 | 85.3% (n=34, constraint-gauntlet-v1) |
| Kimi K2.7 Code (`moonshotai/kimi-k2.7-code`) | 7 | 6 | 7 | 7 | 9 | 5 | 6 | not tested at 2.7; predecessor `kimi-k2` is the **worst** constraint-gauntlet performer (68% v1 / 60-65% v0, n=20-34) but **tied-best** on card-oracle-triage (100%, n=32) — task-dependent, do not treat as uniformly strong |
| DeepSeek V4 Flash (`deepseek/deepseek-v4-flash`) | 7 | 5 | 5 | 4 | 5 | 5 | 5 | 79.4% (n=34, v1); 95% (n=20, v0); 100% (n=32, card-oracle-triage) — best $/performance in the roster ($0.09/$0.18 per M) |
| DeepSeek V4 Pro (`deepseek/deepseek-v4-pro`) | 8 | 7 | 6 | 5 | 6 | 6 | 6 | 91.2% (n=34, v1) — ties Sonnet 5's measured score at ~5% of the cost |
| Qwen3.7 Max (`qwen/qwen3.7-max`) | 7 | 6 | 5 | 4 | 6 | 5 | 5 | **100%** (n=34, constraint-gauntlet-v1) — best measured score in the whole roster |
| MiniMax M3 (`minimax/minimax-m3`) | 6 | 5 | 5 | 4 | 5 | 4 | 5 | 82.4% (n=34, v1); predecessor M2 ranged 73.5-90% across three v0 runs — noisier than most |
| Composer 2.5 (Cursor, `composer-2.5`) | 8 | 5 | 4 | 5 | 4 | 4 | 6 | — untested; public model card/pricing not independently verified (`model-provider-harness-index.md`) — treat all columns here as operator-anecdotal only |

## Reading this table for a dispatch decision

- **A high Crucible score at low cost beats a high-vibe score at high cost**
  when the task looks like the benchmark shape (bounded, spec-checkable,
  rubric-gradeable). Qwen3.7 Max and DeepSeek V4 Pro both measured at or near
  the top of the roster for a fraction of Fable/Sonnet pricing — real
  evidence, not folklore, that the cheap ladder is underrated for
  spec-shaped work.
- **No Crucible score does not mean "unproven," it means "untested on this
  benchmark shape."** Fable 5, Sol, and Luna's blank cells reflect Crucible's
  own cheap-ladder design bias, not a capability gap. Deep-ambiguity,
  cross-system, root-cause work still routes to Fable/Sol per the ledger's
  incident evidence (`model-capability-ledger.md`), independent of the blank
  Crucible cell.
- **kimi-k2's split result is the sharpest reminder in this table that one
  score does not generalize across task shapes.** Do not read "Kimi is
  good/bad" from a single benchmark; read the task-shape match.
- Community sentiment and public leaderboards were **not** used to source
  this file — public benchmarks under these exact model names were not
  independently found/verified at authoring time (2026-07-17). Treat the
  "vendor/community positioning" method layer above as provider-claimed
  facts already in `model-provider-harness-index.md` (e.g. Grok 4.5's Harvey
  Legal Agent Benchmark claim), not third-party leaderboard aggregation.
  Revisit this file once such sources exist and are verifiable.
