# /groom eval

The one claim `/groom` must earn: **given a frozen repo and messy board, the
skill makes the same model reconcile every active card and produce a coherent,
evidence-backed 12-week epic portfolio where a sharp raw prompt stops at
triage, misses contradictions, or pads the queue.**

This is a `mode-eval` A/B run. Both arms receive the same worker packet and
neutral output contract; Arm A alone sees `/groom`. Outputs are read-only JSON
groom artifacts. Grade shuffled artifacts blind, objective checks first.

## Fixtures

| # | Frozen task | Forbidden changes | Failure stressed |
|---|---|---|---|
| 1 | **Conviction minefield.** Materialized at `fixtures/conviction-minefield/worker-packet.md`; hidden key at `answer-key.json`; common schema at `fixtures/output-contract.md`. | board/repo edits, closures, external contact | total card reconciliation, state/priority/overlap correction, strategic starvation |
| 2 | **Maintenance monoculture.** Materialized at `fixtures/maintenance-monoculture/worker-packet.md`; hidden key alongside it. | board/repo edits | vision backchaining, product/ops/security/agent breadth without confetti |
| 3 | **Healthy small repo.** Materialized at `fixtures/healthy-small/worker-packet.md`; hidden key alongside it. Rotate its temptations after contract changes. | board/repo edits | restraint; broad accounting without arbitrary emissions |

Freeze each serious run as exact prompt, repo SHA or self-contained repo packet,
board export, answer key authored before either arm, and forbidden-edit list.
Never show answer keys to workers. Fixture 2's key requires each named repo gap
to be emitted, absorbed, or rejected. Fixture 3 rotates after contract changes.

## Raw baseline prompt

> Perform a read-only, exhaustive strategic grooming of this frozen repository
> and backlog snapshot. Reconcile every backlog claim against repository
> evidence; identify shipped, stale, duplicate, blocked, contradictory, or
> wrongly framed cards without silently deleting or merging anything. Produce
> an evidence-backed world-class roadmap with strategic themes, coherent epics,
> sequencing, proposed deletions/consolidations, and one best next pickup. Cite
> exact evidence, account for skipped or failed areas, and return only the final
> groom artifact.

## Objective checks

Run fixture 1's mechanical grader with:

```sh
python3 primitives/skills/groom/evals/check_fixture.py \
  primitives/skills/groom/evals/fixtures/conviction-minefield/answer-key.json \
  /path/to/arm-artifact.json
```

Negative control (the observed thin-groom shape) must stay red:

```sh
python3 primitives/skills/groom/evals/check_fixture.py \
  primitives/skills/groom/evals/fixtures/conviction-minefield/answer-key.json \
  primitives/skills/groom/evals/fixtures/conviction-minefield/known-bad-thin-groom.json
# FAIL: truth ledger must contain each snapshot card exactly once
```

Fatal mechanical checks fail the arm regardless of prose quality:

- every snapshot card ID appears exactly once in a truth ledger;
- every answer-key contradiction is named with an accepted disposition;
- `mutations_performed` is false; the eval harness separately verifies the
  forbidden-edit boundary from the repo diff;
- no ready card retains a blocker or lacks an executable proof loop;
- every overlap names survivor, containment, or keep-both rationale;
- every mandatory lens and three `tailored-*` lens names have one complete row;
- report/lens IDs, dispatch/brief/raw-report receipts, falsifiers, and evidence
  scopes are non-empty and unique;
- at least three lenses are repo-composed rather than stock-role renames;
- every lane candidate maps to emit, update, absorb-into ID, or evidenced reject;
- every emitted epic has Goal, whole-arc Oracle, proof loop, ordered children,
  source candidate, vision clause, dependencies, track, and epoch;
- the portfolio has 6–10 epics for fixtures 1–2, covers all five strategic
  tracks, spans three evidence-producing epochs, and names exactly one ready
  best pickup;
- fixture 3 rejects unsupported scope and adds no more than its keyed outcomes.

The blind grader, not the script, judges whether evidence was fabricated,
receipts represent real independent lanes, proof loops are executable, lenses
fail differently, overlaps preserve distinct goals, required strategic gaps
are substantively covered, and epics are coherent.

## Blind rubric

Score each artifact 1–5 with one-line evidence:

| Dimension | 5 | 1 |
|---|---|---|
| Strategic causality | vision and evidence determine emissions, sequencing, and rejections | generic roadmap detached from sources |
| Board truth | every card and contradiction resolves coherently | selective tidy or conflicting dispositions |
| Portfolio architecture | distinct epic outcomes form a credible quarter | three-ticket triage, omnibus, or confetti |
| Project specificity | tailored lenses reveal non-obvious domain moves | stock software checklist |
| Prioritization | one pickup clearly dominates by value, risk, learning, or enablement | arbitrary priority list |

## Pass condition

Arm A passes every fatal check on all fixtures, never loses an objective check
to B, wins the paired claim verdict on at least 2 of 3 fixtures, and loses none.
Each win includes either Strategic causality or Portfolio architecture. A blind
human anchor must agree on at least fixture 2 before `keep`; until then the
automated verdict is provisional. Three fixtures detect large regressions, not
a statistically precise population effect.

## Cadence

- Every groom edit: fixture 1 native-subagent smoke, shared-family waiver.
- Contract change: all three fixtures, enforced skill visibility, decorrelated
  workers/grader, and refreshed human anchor.
- Major model release or proxy/human disagreement: full rerun.
- Every real groom failure mutates a permanent fixture or answer key.

## Run log

Append-only. Store sanitized arms, objective receipts, blind verdict, and report
under `.evidence/harness-evals/groom/<date>/` or a durable linked artifact. A
run without both arms and a grader is not evidence.

**2026-07-09 — fixture 1 calibration A/B.** Native subagents; same-family
limitation; human anchor pending. Both arms passed the calibrated mechanical grader: 18
cards, 16 reports, all seeded contradictions, all tracks/epochs; skill Arm A
produced 7 epics and raw Arm B produced 6. The shuffled blind grader chose the
skill arm **25/25 vs 19/25**. Decisive delta: A carried 16 distinct dispatch
paths and project-composed finance-authority/Rust-math lenses; B reused two
contexts for 16 relabeled reports, then produced an omnibus trust epic and a
contradictory CV-008 dependency. The run exposed two answer-key constraints
that rejected defensible alternate dispositions; those were widened after the
arms, so this is rubric calibration, **not held-out pass evidence**. Frozen
post-calibration hashes: checker `530eabee027392a72f67d51c4bca1a441a4a54b2c554d10ffbdaa56a658db71e`;
key `d9fccd5e81cb3d8ce090f96f6def6cc26c59fa5cb8b6d3846828c0f8ee9c099e`.
Decision: **needs-more-tasks** — the large on-claim calibration win justifies
the rewrite, while held-out fixtures 2–3, decorrelated grading, and the human
anchor remain. Native Codex subagents supplied both arms and grader; the surface
did not expose a more specific model ID, so the same-family waiver applies.
Durable receipts: [skill arm](https://bastion.tail5f5eb4.ts.net/artifacts/a/roster-962-arm-y/)
`sha256:d3c7e9c3ba85799f6de5d13a3b5cbd92d4ea8847fa2c2d78b81dc45127b0057d`;
[raw arm](https://bastion.tail5f5eb4.ts.net/artifacts/a/roster-962-arm-x/)
`sha256:e54fe8ace0a8a7368b0e2f71534c5e37d38f27c122921270537b038b127da5ac`;
[blind grade](https://bastion.tail5f5eb4.ts.net/artifacts/a/roster-962-grade/)
`sha256:183b9fa9db955725155798077688ff18198be2565d43d17e5f2a5098dd41c80f`.
