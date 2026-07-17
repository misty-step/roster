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

## 3. Match task shape to the matrix, not the model's reputation

- Spec-checkable, rubric-gradeable, bounded work (the shape Crucible
  benchmarks) → prefer the highest **measured** Crucible score at the lowest
  cost tier that clears your risk bar. Qwen3.7 Max and DeepSeek V4 Pro both
  measured competitively with Sonnet 5 at a fraction of the price — verify
  this hasn't gone stale before trusting it blindly.
- Deep ambiguity, cross-system root-cause, or an unwritten invariant set →
  route up-tier (Fable 5 / Sol) per `model-capability-ledger.md`'s standing
  rule, even though these models carry no Crucible cell — the blank cell is
  a benchmark-coverage gap, not a capability signal.
- Design/visual, product taste → GLM 5.2 first (matrix column + ledger both
  name it); Fable 5 or Kimi K2.7 as fallback.
- Long-context research/synthesis → Kimi K2.7 Code.
- Adversarial critique / assumption-breaking → Grok 4.5.
- Long unsupervised autonomous coding → Luna or Sonnet 5 (both score highest
  on Agentic Autonomy; Sonnet 5 additionally has a measured Crucible score).

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
