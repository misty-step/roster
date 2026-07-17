---
review_due: 2026-08-17
last_reviewed: 2026-07-17
---

# Model Capability Matrix

One row per **(model, reasoning-level) pair** — reasoning effort measurably
shifts coding/architecture capability, so a model dispatched at its lowest
level and its highest level are different dispatch choices, not the same row.
Provider (native subscription vs. OpenRouter) is a separate routing layer on
top of this matrix, not a column — the same model scores the same regardless
of which door you dispatch it through; prefer the subscribed native provider,
fall back to OpenRouter when capped. Full pricing/context/reasoning-mechanism
facts live in `peer-harnesses/references/model-provider-harness-index.md`
(the "Reasoning-effort taxonomies" table there is the source for the Level
column here) — this file does not duplicate those facts, only the judgment
built on top of them.

## Method

We do not have first-party evals on every model at every reasoning level.
Per operator ruling (2026-07-17): **aggregate broadly — third-party
benchmarks, vendor docs, community sentiment, own operating experience —
rather than waiting for complete first-party coverage.** Do not over-weight
Crucible: it is one input among several, and its own benchmark roster skews
cheap/open-model by design (see "Historical Crucible spot-checks" at the
bottom — supplementary, not authoritative).

Score derivation, in priority order where available: (1) third-party
benchmark citations named in `model-provider-harness-index.md` (Artificial
Analysis Coding/Intelligence/Agentic indices, SWE-bench Pro/Verified,
Terminal-Bench, Design Arena, Harvey LAB, GDPval-AA, and similar); (2)
vendor-published positioning and comparative claims, read skeptically; (3)
community sentiment (Reddit, HN, Simon Willison, practitioner writeups) where
it converges across independent sources; (4) session operating experience,
lowest-priority. Every score is curated, not computed — a human or agent
judgment call informed by that evidence, not a formula. Scale 1-10.

**Reasoning-level deltas are a simplifying model, not measured per-level
data.** Only Coding/Impl and Architecture/Reasoning are modeled as
level-sensitive (the two dimensions evidence actually supports moving with
thinking depth); Writing, Design, Research, Adversarial, and Autonomy are
held constant per model across levels — a simplification pending finer-grained
evidence, not a claim that they never shift. Where real per-level benchmark
deltas exist (e.g. DeepSeek V4's Artificial Analysis Index: high=37/41,
xhigh=40/44 for Flash/Pro respectively — roughly +3 points out of 100), the
true gap is smaller than a full matrix-point; treat the +1/+2 deltas below as
directionally correct, not precisely calibrated.

## The matrix

| Model | Level | Coding/Impl | Architecture/Reasoning | Writing/Comm | Design/Visual | Research/Synth | Adversarial/Critique | Agentic Autonomy |
|---|---|---:|---:|---:|---:|---:|---:|---:|
| Gemini 3.5 Flash | minimal | 4 | 4 | 6 | 6 | 7 | 5 | 5 |
| Gemini 3.5 Flash | low | 5 | 5 | 6 | 6 | 7 | 5 | 5 |
| Gemini 3.5 Flash | medium (default) | 6 | 6 | 6 | 6 | 7 | 5 | 5 |
| Gemini 3.5 Flash | high | 7 | 7 | 6 | 6 | 7 | 5 | 5 |
| Gemini 3.1 Flash-Lite | minimal (default) | 4 | 4 | 4 | 4 | 5 | 3 | 4 |
| Gemini 3.1 Flash-Lite | low | 5 | 5 | 4 | 4 | 5 | 3 | 4 |
| Gemini 3.1 Flash-Lite | medium | 6 | 6 | 4 | 4 | 5 | 3 | 4 |
| Gemini 3.1 Flash-Lite | high | 6 | 6 | 4 | 4 | 5 | 3 | 4 |
| Gemini 3.1 Pro (Preview) | low | 5 | 6 | 6 | 6 | 8 | 6 | 6 |
| Gemini 3.1 Pro (Preview) | medium | 6 | 7 | 6 | 6 | 8 | 6 | 6 |
| Gemini 3.1 Pro (Preview) | high (default) | 7 | 8 | 6 | 6 | 8 | 6 | 6 |
| Sonnet 5 | disabled | 6 | 4 | 7 | 5 | 6 | 6 | 9 |
| Sonnet 5 | low | 7 | 5 | 7 | 5 | 6 | 6 | 9 |
| Sonnet 5 | medium | 8 | 6 | 7 | 5 | 6 | 6 | 9 |
| Sonnet 5 | high (default) | 9 | 7 | 7 | 5 | 6 | 6 | 9 |
| Sonnet 5 | xhigh | 9 | 8 | 7 | 5 | 6 | 6 | 9 |
| Sonnet 5 | max | 10 | 9 | 7 | 5 | 6 | 6 | 9 |
| Fable 5 | low | 7 | 8 | 8 | 6 | 8 | 7 | 8 |
| Fable 5 | medium | 8 | 9 | 8 | 6 | 8 | 7 | 8 |
| Fable 5 | high (default) | 9 | 10 | 8 | 6 | 8 | 7 | 8 |
| Fable 5 | xhigh | 10 | 10 | 8 | 6 | 8 | 7 | 8 |
| Fable 5 | max | 10 | 10 | 8 | 6 | 8 | 7 | 8 |
| Grok 4.5 | low | 6 | 6 | 7 | 5 | 7 | 10 | 7 |
| Grok 4.5 | medium | 7 | 7 | 7 | 5 | 7 | 10 | 7 |
| Grok 4.5 | high (default) | 8 | 8 | 7 | 5 | 7 | 10 | 7 |
| GPT-5.6 Sol | low | 8 | 7 | 6 | 5 | 7 | 7 | 9 |
| GPT-5.6 Sol | medium | 9 | 8 | 6 | 5 | 7 | 7 | 9 |
| GPT-5.6 Sol | high (default) | 10 | 9 | 6 | 5 | 7 | 7 | 9 |
| GPT-5.6 Sol | xhigh | 10 | 10 | 6 | 5 | 7 | 7 | 9 |
| GPT-5.6 Sol | max | 10 | 10 | 6 | 5 | 7 | 7 | 9 |
| GPT-5.6 Terra | low | 7 | 5 | 6 | 5 | 6 | 6 | 8 |
| GPT-5.6 Terra | medium | 8 | 6 | 6 | 5 | 6 | 6 | 8 |
| GPT-5.6 Terra | high (default) | 9 | 7 | 6 | 5 | 6 | 6 | 8 |
| GPT-5.6 Terra | xhigh | 9 | 8 | 6 | 5 | 6 | 6 | 8 |
| GPT-5.6 Terra | max | 10 | 8 | 6 | 5 | 6 | 6 | 8 |
| GPT-5.6 Luna | low | 6 | 4 | 5 | 4 | 5 | 5 | 8 |
| GPT-5.6 Luna | medium | 7 | 5 | 5 | 4 | 5 | 5 | 8 |
| GPT-5.6 Luna | high (default) | 8 | 6 | 5 | 4 | 5 | 5 | 8 |
| GPT-5.6 Luna | xhigh | 8 | 7 | 5 | 4 | 5 | 5 | 8 |
| GPT-5.6 Luna | max | 9 | 7 | 5 | 4 | 5 | 5 | 8 |
| Thinking Machines Inkling | none | 4 | 4 | 5 | 5 | 6 | 5 | 5 |
| Thinking Machines Inkling | minimal | 4 | 4 | 5 | 5 | 6 | 5 | 5 |
| Thinking Machines Inkling | low | 4 | 4 | 5 | 5 | 6 | 5 | 5 |
| Thinking Machines Inkling | medium | 5 | 5 | 5 | 5 | 6 | 5 | 5 |
| Thinking Machines Inkling | high (default) | 6 | 6 | 5 | 5 | 6 | 5 | 5 |
| Thinking Machines Inkling | xhigh | 7 | 7 | 5 | 5 | 6 | 5 | 5 |
| Thinking Machines Inkling | max | 8 | 8 | 5 | 5 | 6 | 5 | 5 |
| Meta Muse Spark 1.1 | minimal | 5 | 4 | 6 | 6 | 6 | 5 | 6 |
| Meta Muse Spark 1.1 | low | 6 | 5 | 6 | 6 | 6 | 5 | 6 |
| Meta Muse Spark 1.1 | medium (default, unconfirmed) | 7 | 6 | 6 | 6 | 6 | 5 | 6 |
| Meta Muse Spark 1.1 | high | 8 | 7 | 6 | 6 | 6 | 5 | 6 |
| Kimi K3 | max (only level shipped) | 9 | 8 | 7 | 6 | 9 | 5 | 7 |
| MiniMax M3 | reasoning off | 5 | 3 | 5 | 4 | 5 | 4 | 6 |
| MiniMax M3 | reasoning on (default) | 7 | 5 | 5 | 4 | 5 | 4 | 6 |
| GLM 5.2 | high (default) | 7 | 6 | 6 | 10 | 6 | 5 | 6 |
| GLM 5.2 | xhigh | 8 | 7 | 6 | 10 | 6 | 5 | 6 |
| DeepSeek V4 Flash | high (default) | 7 | 4 | 5 | 4 | 5 | 5 | 5 |
| DeepSeek V4 Flash | xhigh | 8 | 5 | 5 | 4 | 5 | 5 | 5 |
| DeepSeek V4 Pro | high (default) | 8 | 7 | 6 | 4 | 6 | 6 | 6 |
| DeepSeek V4 Pro | xhigh | 9 | 8 | 6 | 4 | 6 | 6 | 6 |
| Qwen 3.7 Max | high (default) | 8 | 7 | 4 | 4 | 6 | 5 | 8 |
| Qwen 3.7 Max | xhigh (unconfirmed) | 9 | 8 | 4 | 4 | 6 | 5 | 8 |
| Nemotron 3 Ultra | reasoning off | 6 | 5 | 5 | 4 | 7 | 5 | 6 |
| Nemotron 3 Ultra | reasoning on (default) | 8 | 7 | 5 | 4 | 7 | 5 | 6 |

## Reading this table for a dispatch decision

- **The reasoning-level axis matters most for Coding/Architecture-heavy
  work.** For writing, design, or research-synthesis tasks, the level a
  model runs at barely moves the needle in this model — pick the model, then
  default to its vendor-recommended default level rather than paying for
  `max` on a task that won't use the extra depth.
- **GLM 5.2 leads Design/Visual by a wide margin (10/10) at every level** —
  OpenRouter's Design Arena ranks it #1 of all tracked models in both the
  "website" (1343 Elo, 60.9% win rate) and "code" categories (1348 Elo,
  61.2% win rate). Community sentiment is polarized but net-positive for
  frontend generation specifically; it over-edits on small surgical changes.
- **GPT-5.6 Sol at high/xhigh is the strongest measured coding tier in this
  table** — Artificial Analysis Coding Agent Index 80, a new SOTA at launch,
  beating Claude Fable 5 by 2.8 points using under half the tokens/time/cost.
  This is a genuine, dated, sourced finding, not a vibe check.
- **DeepSeek V4 Pro at xhigh rivals Sonnet 5's mid-tier coding score at a
  fraction of the price** ($0.435/$0.87 per M vs. $2/$10) — the cheap ladder
  keeps being underrated for spec-shaped work.
- **Kimi K3 is one day old at time of writing** (released 2026-07-16) and
  only ships one reasoning level (`max`) so far. Its jump over K2.7 Code on
  Artificial Analysis (Coding Index 58.6 -> 76.2) is real per the live
  OpenRouter benchmark metadata, but treat it as unverified-independently —
  community sentiment itself flags "FUD" skepticism about vendor-marketed
  scores this fresh.
- **Qwen 3.7 Max is explicitly weak at creative writing** per converging
  community sentiment, despite strong coding/agentic scores (35-hour
  autonomous run claims) — do not route writing-heavy work here regardless
  of its otherwise-strong profile.
- **Nemotron 3 Ultra's cited SWE-bench Verified score (71.9%) is not
  comparable to other rows' SWE-bench Pro scores** — different benchmark
  variants. Do not rank it against Pro-scored rows on that number alone.
- No Crucible score does not mean "unproven" for any row here — it means
  Crucible hasn't run that exact benchmark shape against that model yet.
  Treat blank Crucible coverage as a benchmark-coverage gap, not a capability
  signal, exactly as the third-party citations above are meant to fill in.

## Historical Crucible spot-checks (supplementary, not authoritative)

Per operator instruction, do not lean on these — they are one thin, dated
data point among the many above, and Crucible's own benchmark roster skews
cheap/open-model by design. Constraint-gauntlet-v1 (n=34, temp=0, default
effort, Wilson-scored): Qwen 3.7 Max 100%, DeepSeek V4 Pro 91.2%, Claude
Sonnet 5 91.2%, Grok 4.5 88.2%, GLM 5.2 / Gemini 3.5 Flash / Kimi K2.6 85.3%,
DeepSeek V4 Flash 79.4%, MiniMax M3 82.4%. Card-oracle-triage-v0 (n=32): GLM
5.2 / Kimi K2 / DeepSeek V4 Flash all 100%. Kimi K2 (predecessor to K3) is
the sharpest reminder in this whole file that one score does not generalize:
worst constraint-gauntlet performer (60-68% across three runs) but
tied-best on card-oracle-triage (100%) — task-shape match beats reputation.
None of the models added 2026-07-17 (Gemini 3.1 Flash-Lite/Pro, GPT-5.6
Sol/Terra, Thinking Machines Inkling, Meta Muse Spark 1.1, Kimi K3, Nemotron
3 Ultra) have any Crucible runs yet.
