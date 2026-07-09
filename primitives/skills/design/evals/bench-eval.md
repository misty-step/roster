# /design bench eval

The one claim `/design` must earn: **given any design request, the skill
produces a composed multi-philosophy catalog — ≥4 blind lanes, a Law-clearing
count of structurally distinct options, per-option provenance — where raw
prompting produces a single answer or correlated variants of one layout.**

This is a `mode-eval` A/B run. Arms: A = skill installed + invoked; B = raw
same-model given the identical request and artifact; C = n/a. Grade blind,
objective first, judge a different model family than the workers.

Seeded 2026-07-09 with the always-bench rewrite (Powder `design-always-bench`);
supersedes `routing-eval.md`, which tested the retired routing claim (preserved
with its run log at `primitives/skills/skill-eval/examples/routing-eval.md`).

## Fixtures

| # | Prompt | Repo @ SHA | Forbidden edits | What it stresses |
|---|---|---|---|---|
| 1 | "redesign this dashboard page" + a real dense operational surface | frozen at run time | product source outside the lab dir | density-honest lane selection; fence adherence on a within-system task |
| 2 | "build a landing page for <small product brief>" (greenfield) | frozen at run time | none (greenfield) | widest spread; genre poles (hallmark vs soft vs brutalist) |
| 3 | "this screen feels generic — critique it" | frozen at run time | product source outside the lab dir | inverts the assumption that critique returns prose: the answer must still be a rendered catalog of fixed options |

## Objective checks (scriptable, pass/fail, ~free)

- [ ] A catalog exists in lab-registry layout (`index.html`, `frame.html`,
      composer `frame.js`, `lanes/`) and loads with zero console errors.
- [ ] ≥4 lane modules under `lanes/`, each mapping to a distinct vendored
      philosophy alias; every option's manifest entry carries a `lane` that
      names a real module.
- [ ] Option count within the Design Labs Law range; round-1 baseline
      (current shipped state) present for fixtures 1 and 3.
- [ ] Namespaced stable option IDs; no ID collisions across lanes.
- [ ] B-arm comparison artifact captured (whatever raw prompting produced).

## Rubric (1–5, blind, justify each)

| Dimension | What 5 looks like | What 1 looks like |
|---|---|---|
| Structural spread | a blind counter (different model family) finds ≥6 structurally distinct layouts; directions differ in macrostructure, not costume | palette/font swaps of one layout; the counter finds ≤2 |
| Lane fidelity | each lane's options show its philosophy's signature moves (verifiable against the source skill) | options are indistinguishable house style regardless of lane |
| Fence adherence | every option respects the stated FIXED set; divergence lives on the declared axes | brilliant but off-system, or timid reskins inside the fence |

## Pass condition

A beats B paired on the structural-spread count across ≥2 of 3 fixtures, AND
ties-or-wins every objective check. The bar a no-op skill fails: the raw arm
reliably returns one answer (or prose, for fixture 3) with no provenance and
no navigable catalog.

## Human anchor

The operator blind-grades ≥1 fixture's catalog (browse + verdicts). Record the
verdict here and whether the agent grader matched it. On disagreement, fix the
rubric — don't overrule the human.

## Cadence

Seeded, unrun. First run before the next substantive `/design` prose edit or
after a major model release (railroading re-audit), whichever comes first.

## Run log

(none yet — seeded 2026-07-09)
