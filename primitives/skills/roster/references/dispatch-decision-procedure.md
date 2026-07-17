# Dispatch Decision Procedure

Judgment for two questions the roster CLI itself doesn't answer: **which
model** to bind to a role, and **native subagent vs. an autonomous herdr
space**. Read `model-capability-matrix.md` first for the capability data this
procedure consumes.

## 1. Counter the same-family default

Frontier models were RLHF'd to reach for their own family when asked to
delegate — Sol dispatches Sol sub-agents, Fable dispatches Fable sub-agents,
by trained default, not by evidence. This is the specific bias this procedure
exists to counteract. Before dispatching, name the model you'd pick if your
own family were unavailable; if the answer changes only because of family
loyalty, it was the wrong default.

## 2. Check capacity before a wide wave

Query `overmind_fleet` (or `overmind_my_session` for just this session)
before dispatching more than one or two lanes. A subscription near its
5-hour or weekly usage fraction should route the wave to a cheaper/open
alternative from the matrix, not silently degrade or queue behind a cap
death. This is a live signal, not a static budget line — quote it at
dispatch time.

## 3. Two decisions, not one: model, then reasoning level

The matrix is keyed by (model, level) pairs because they are two separate
dispatch decisions with different sensitivity:

- **Model choice** moves every capability dimension — pick by task
  domain (coding vs. writing vs. design vs. research vs. adversarial
  critique vs. long-autonomy).
- **Reasoning level** mainly moves Coding/Impl and Architecture/Reasoning,
  barely anything else (see the matrix's "Reading this table" notes) — so
  paying for a model's top level on a task that doesn't need deep
  architecture reasoning (writing, design, routine research) buys latency
  and cost without buying capability. Match the level to how much of the
  task is actually reasoning-bound, not to how much budget is available.

Task-shape routing:

- Spec-checkable, rubric-gradeable, bounded work → prefer a high measured
  third-party benchmark (Artificial Analysis Coding Agent Index, SWE-bench
  Pro, Design Arena) at the lowest cost/reasoning-level tier that clears
  your risk bar. GPT-5.6 Sol at high, DeepSeek V4 Pro at xhigh, and Qwen 3.7
  Max both measured competitively with or above Sonnet 5/Fable 5 at a
  fraction of the price — verify this hasn't gone stale before trusting it
  blindly.
- Deep ambiguity, cross-system root-cause, or an unwritten invariant set →
  route up-tier (Fable 5 or GPT-5.6 Sol, both at high/xhigh) per
  `model-capability-ledger.md`'s standing rule, even where a model carries
  no Crucible cell — a blank Crucible cell is a benchmark-coverage gap, not
  a capability signal.
- Design/visual, product/frontend taste → Kimi K3 (9/10), Fable 5 (8/10), or GPT-5.6 Sol (8/10) first (matrix's Design column). GLM 5.2 (7/10) remains a highly cost-effective alternative but is no longer ranked #1 overall. It over-edits on small surgical changes, so pair it with a narrower diff-only critic for cleanup passes.
- Long-context research/synthesis → Kimi K3 (fresh, unverified
  independently — sanity-check its first few outputs) or Kimi K2.7 Code
  (the still-configured local default, more proven).
- Adversarial critique / assumption-breaking → Grok 4.5.
- Long unsupervised autonomous coding → GPT-5.6 Luna, GPT-5.6 Sol, or
  Sonnet 5 (all score highest on Agentic Autonomy; Sonnet 5 and Sol both
  carry measured or vendor-cited benchmark support).
- Creative/prose writing → avoid Qwen 3.7 Max specifically (community
  sentiment converges on it being weak here despite strong coding scores);
  Fable 5 and GPT-5.6 Sol score highest on Writing/Communication.

## 4. herdr space vs. native OMP subagent

Neither repo (`collie`/herdr or roster) has ever written this boundary down
before 2026-07-17; herdr has zero built-in cost/capacity awareness (verified
by exhaustive grep of the collie/herdr source — it only mirrors pane text,
never computes usage). Use:

| Trigger | Route |
|---|---|
| Autonomous, end-to-end, hours-long delivery; survives session death; needs its own terminal/tab lifecycle | herdr space (`workspace.create`/`tab.create` via Collie's `POST /api/workspace`/`POST /api/tab`) |
| Bounded, single-outcome, minutes-to-an-hour; lead stays in the loop | Native OMP/Roster subagent |
| Uncertain which | Default to native subagent — herdr's structural write actions are gated behind device auth and carry more setup cost; earn the heavier lane with evidence the task actually runs unattended for hours |

## 5. Refresh discipline

This procedure and the matrix are curated, not computed. Re-score a matrix
row after a new Crucible run, a ledger incident, or a model version bump.
Do not let this file's absence of automation become an excuse to skip
updating it — stale routing judgment is worse than no judgment (it hides
that the evidence moved).
