---
name: eval-design
description: |
  Design an LLM/agentic eval that can change a decision — a task + a
  model/agent under test producing fresh output + a grader — and separate real
  capability evals from instrumentation (linters, CI gates, KPIs). Use when
  designing or critiquing an eval, building an eval corpus, designing or
  calibrating an LLM-as-judge, or comparing models/harnesses on a measured
  capability. Trigger: /eval-design.
argument-hint: "[capability|decision] [--corpus|--judge|--compare]"
---

# /eval-design

Build an eval that can change a decision, or don't build it. An eval is a **task
+ a model or agent under test that produces fresh output + a grader that scores
that output**, run over a corpus, with an interval on every rate, calibration on
every model-judge, and a noise-floor check on every comparison. Anything that
scores a *fixed artifact* (a lint pass, a snapshot diff) or a *past metric* (a
dashboard KPI, a CI trend) is **instrumentation, not an eval** — useful, but it
cannot tell you whether a model or agent is *capable*, because nothing fresh was
produced against a task. The most common failure this skill exists to prevent is
shipping instrumentation and calling it an eval.

This is design judgment. Crucible is the enforcement engine underneath it: this
skill decides *what* to measure and *how* to grade and align the judge; Crucible
(`crucible author/validate/run/runs compare`, the CalibrationRecord, the
judge-gaming canary, the paired resolution/MDE) enforces the statistics and
persists the evidence. Never re-implement the statistics in prose; point into
Crucible.

## The demarcation test (run this first)

Before designing anything, answer three questions. A "no" means you have
instrumentation, a KPI, or a unit test — valuable, but not an eval. Say so and
stop, or reshape it into a real eval.

1. **Is there a decision?** Name the decision the number will change — ship this
   prompt, pick model A over B, accept this harness change. No downstream
   decision → you're collecting a metric, not running an eval.
2. **Is fresh output produced against a task?** The model/agent must *generate*
   something in response to a task. Grading a checked-in file, a git-history
   trend, or a static rule is instrumentation.
3. **Does a grader score that output against a defensible standard?** "The build
   is green" is a gate. "The agent's patch resolves the issue, judged against a
   reference, at 62% ± 6%" is an eval.

## The design sequence

Size the effort to the stakes — a prompt tweak needs a light pass, a model-
selection decision under budget needs the full treatment. But the order is fixed.

### 1. Decision → capability
State the decision (step above), then the **one capability under test** as a
falsifiable claim: "agent resolves real GitHub issues," not "agent is good."
Multidimensional targets are fine and usual (correctness AND no regression AND
under budget) — make each dimension its own measurable criterion (Anthropic:
"less than 0.1% of 10,000 trials flagged," not "safe"). One capability per eval;
if you're measuring three, that's three graders, not one fuzzy score.

### 2. Corpus
- **Draw from real failures.** 20–50 tasks from the bug tracker, support queue,
  and the failures you hit during development is a strong start (Anthropic
  2026-01). Synthetic tasks fill gaps but real failures anchor the distribution.
- **Every task must be expert-agreement-solvable.** "A good task is one where two
  domain experts would independently reach the same pass/fail verdict." Everything
  the grader checks must be stated in the task — an agent must never fail on an
  ambiguous spec.
- **Attach a reference solution.** A known output that passes all graders proves
  the task is solvable *and* verifies the grader is configured right. A 0% pass
  rate across many trials is almost always a broken task or grader, not an
  incapable model — read the transcripts before believing it.
- **Class-balance it.** Include cases where the behavior should occur AND where
  it shouldn't. A corpus of only-positive cases measures recall with no precision;
  the model that always says "yes" scores 100%.
- **Cover edge cases:** empty/irrelevant input, over-long input, adversarial
  input, and genuinely ambiguous cases where humans would disagree (label these).
- **Size to the effect.** State the minimum effect you need to detect. Detecting a
  3% difference at 80% power needs ~1,000 tasks (Miller 2024); most bespoke evals
  can only resolve larger effects — that's fine, but *declare the resolvable
  effect* rather than over-reading a small delta. Let `crucible validate` warn you.

### 3. Grader — climb the ladder, stop at the first rung that holds
Prefer the fastest, most reproducible grader that captures the truth:
- **Code / deterministic first.** Exact match, string/regex, "does the test
  suite pass," "is the JSON valid," did the file reach the target state. Fast,
  cheap, reproducible; brittle to valid variation. Push every check you can here.
- **Model-judge** for open-ended quality that code can't capture (correctness of
  prose, adherence to a rubric, tone). Needs the alignment step below before you
  trust it.
- **Human** as the calibration anchor and for the irreducibly subjective call.
  Slow and expensive — reserve it for anchoring the model judge, not for volume.

**For agents specifically:** grade the **outcome and the transcript, not the
trajectory.** Verify the final environment state (did the reservation actually
land in the DB, does the patch actually fix the bug) plus the transcript (tool
calls, reasoning). Do NOT assert a specific sequence of steps — agents find valid
paths you didn't anticipate, and step-checking makes brittle tests. Cap behavior
(`max_turns`) but never prescribe *how*. Use partial credit for multi-part tasks.
For stochastic agents, the rate is a design choice: **`pass@k`** (at least one of
k attempts succeeds — coding, where one success suffices) vs **`pass^k`** (all k
succeed — customer-facing, where consistency is the product). They diverge fast:
at k=10 `pass@k`→100% while `pass^k`→0% tell opposite stories. Isolate each trial
in a clean environment so infra flakiness doesn't correlate failures.

### 4. Judge alignment (mandatory before trusting any model-judge)
A model-judge is a sub-eval and it is biased until proven otherwise. Do not quote
a judge's rate until it's aligned. The loop (Hamel's critique-align):
1. Hand-label a calibration set (positive and negative cases).
2. Run the judge blind and compare to the human labels.
3. Measure **precision/recall on the fail class**, not raw agreement and not a
   bare Cohen's κ — the minority (failing) class is what a judge must catch, and
   aggregate agreement hides it on imbalanced data.
4. Iterate the judge prompt until alignment stabilizes; re-anchor whenever the
   judge model or the worker model changes.

**Judge best-practices bundle** (consensus across MT-Bench, G-Eval, Anthropic,
Braintrust, promptfoo):
- **Decorrelated family.** Grade with a different model family than the one under
  test — a judge favors its own family (self-preference/self-enhancement bias).
- **Reason before the verdict.** Instruct chain-of-thought, then read only the
  final tagged verdict. Improves agreement on hard judgments.
- **Observable rubric.** "Must mention X in the first sentence, else incorrect" —
  clear, checkable standards, not "is it good."
- **Low-precision scale.** Binary or ≤5-point; 1-10 scales are noisier and less
  consistent (Databricks via promptfoo).
- **Give it an exit.** Let the judge return "Unknown" when it lacks information,
  so it doesn't hallucinate a grade.
- **Probe position/format sensitivity.** Swap answer order (or cosmetically
  reorder the prompt) and require agreement; a high flip rate means the judge is
  reading form, not substance.
- **Average multiple runs** to tame non-determinism; watch **verbosity bias**
  (longer ≠ better) and **G-Eval's bias toward fluent LLM-generated text**.

### 5. Comparison — refuse a delta you can't defend
- **Interval on every rate.** Report `62% (±6%)`, never a bare `62%`. Binary
  scores: `SE = sqrt(p(1-p)/n)`.
- **Pair the comparison.** Run both arms on the *same* tasks and compute
  per-task differences — correlation is a free reduction in variance (Miller
  2024). Crucible's `runs compare` does this.
- **Check the noise floor.** Before believing a delta, confirm the eval can
  *resolve* it (Crucible's resolution ratio + MDE). A delta inside the noise
  floor is not a result — say "underpowered" or "no effect," don't report a
  number.
- **Cluster your SEs when tasks are grouped** (multiple prompts per repo/PR, one
  task run across many models). Clustered standard errors can be >3× naive ones;
  ignoring grouping gives falsely tight intervals. (Crucible does not yet compute
  clustered SEs — flag it when your corpus is grouped.)
- **Attribute to one axis.** A delta that spans model *and* harness at once is
  unattributable — change one axis per comparison, or label it `config_delta`.

## Anti-patterns (the ways a "good eval" is fake)

- **Instrumentation in an eval costume.** A linter, CI gate, or KPI dashboard
  presented as a capability eval. No fresh output, no task, no defensible grader
  → not an eval. This is the operator's canonical failure; run the demarcation
  test.
- **Recall-only corpus.** Only-positive cases; the model that always fires scores
  perfectly. Class-balance or your rate is meaningless.
- **Uncalibrated judge.** Quoting a model-judge's rate before aligning it to
  human labels on the fail class. The judge is a biased sub-eval until proven.
- **Self-graded family.** A judge grading its own model family inflates the
  margin. Decorrelate, or it's a smoke test, not a verdict.
- **Trajectory policing.** Asserting a specific tool sequence for an agent —
  brittle, and it fails competent agents who found a better path. Grade the
  end state.
- **Bare delta.** A rate or a two-model gap with no interval and no noise-floor
  check. Inside the noise floor, `62% vs 60%` is nothing.
- **Goodhart / saturation.** An eval the target trains against, or one that has
  hit 100% and yields no signal. Keep a held-out slice; retire saturated evals.
- **Eval theater.** Building the corpus and grader but never *looking at the
  data*. "You are doing it wrong if you aren't looking at lots of data" (Hamel).
  Read transcripts; the grader bugs and broken tasks only surface there.

## How this plugs into Crucible

The skill is the design front-end; Crucible is the enforcement + evidence engine.

| Design step | Crucible surface |
|---|---|
| author the spec from the design | `crucible author` (flags or `--interactive`) |
| import an externally-authored eval | `crucible import <adapter> <source>` (promptfoo today) |
| gate the spec before it runs | `crucible validate` / MCP `crucible_validate` — refuses unsupported aggregation/uncertainty/missing grader; warns when task count can't resolve `min_effect_of_interest` |
| run + compare across model/harness/env | `crucible run <spec> --env A --env B` (first `--env` = baseline) |
| model-judge + calibration | `agentic_judge` runner: live judge, `CalibrationRecord` (fail-class precision/recall, per-family scope, κ), reasoning-first tail-anchored verdict, `format_sensitivity_flip_rate` |
| judge-gaming defense | the canary that hard-refuses a run if the judge rubber-stamps a known-bad candidate |
| trust gate | `run_records.trusted` — a locked/unmeasured judge run can't back a comparison or a Signal finding |
| interval + paired noise-floor + attribution | `crucible runs compare` — `paired`, `resolution` (q + MDE), `diagnosis`, attribution label (`model_delta`/`harness_delta`/`config_delta`), `--strict` |
| durable ledger + trace | `crucible runs list/show/compare/history/pivot`; `trace_path` for judge runs |

Every deterministic check lives in Crucible; the skill never re-derives it in
prose. Read Crucible's `SKILL.md` for the exact command contract.

## Gotchas

- **Designing the grader before the corpus.** You can't write a defensible grader
  until you've read real failures. Corpus and grader co-evolve; a reference
  solution is how you know the grader works.
- **Skipping the demarcation test on a familiar-looking request.** "Add an eval
  for X" is often a request for a lint or a gate. Check for a decision and fresh
  output first — reshaping saves the whole build.
- **Trusting a judge's margin without a recent human anchor.** Judges drift with
  model upgrades. Re-anchor on any judge/worker model change or don't quote it.
- **Over-reading a sub-1,000-task delta.** Most bespoke evals resolve only large
  effects. State the MDE; let `crucible validate` warn you; don't ship a story
  built on a delta inside the noise floor.
- **Grouped tasks scored as independent.** Multiple prompts per repo, or one task
  across languages/models, violate the independence the naive interval assumes.

## Verification

An eval design is done when: the demarcation test passes (decision + fresh output
+ defensible grader); the corpus is class-balanced, real-failure-sourced, and
carries a reference solution per task; the grader climbs no higher than it must;
any model-judge is calibrated against human labels with fail-class
precision/recall recorded; and the comparison plan names the interval, the paired
noise-floor check, and the single axis under test. The runnable proof is the
Crucible loop: `crucible validate` clean → `crucible run` produces persisted
records → `crucible runs compare` returns a paired `resolution` that can actually
detect the effect you care about, or honestly reports it can't.

## Sources

The annotated primary-source canon lives in
[`references/sources.md`](references/sources.md) — load it when you need to
justify a methodology choice, go deeper than this distillation, or hand a lane
the literature (Anthropic agentic-evals 2026-01, Hamel Husain, Miller's
error-bars paper, the MT-Bench/G-Eval judge-bias catalog, Inspect as reference
architecture).
