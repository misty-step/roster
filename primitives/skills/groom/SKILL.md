---
name: groom
description: |
  Overhaul a backlog from live repo and board evidence: reconcile every active
  card, fan out independent product and engineering perspectives, and leave a
  coherent quarter-scale epic portfolio. Use when: "groom", "backlog",
  "what should we build", "prioritize", "rethink this", "biggest opportunity",
  "moonshot", "audit skills", "skill quality audit".
  Trigger: /groom, /groom audit, /backlog, /moonshot.
argument-hint: "[audit|tidy|scoped|moonshot] [context]"
---

# /groom

Make the board true, then make the project more ambitious. Bare `/groom` means
a **strategic quarter sweep**. `tidy` and `scoped` are narrower only when the
operator says so. A strategic run cannot downgrade itself because the repo is
small, the queue looks tidy, or delegation is inconvenient.

Powder is the board of record. A repo-local `backlog.d/` is an import/fallback
surface only when Powder is unavailable for that repo. Read the live repo,
board, root vision, and recent history; never groom ticket prose in isolation.

## 1. Lock the sweep

Resolve the operator's ambition, vision deltas, and hidden priors before
ranking. Ask when they are not already explicit; do not ask for facts the repo
or board can answer. Record the board snapshot time and active-card IDs.

**Complete when:** the run has a stated ambition, canonical vision source, and
frozen active-card set. Any later card is labeled post-snapshot intake.

## 2. Reconcile every active card

Create one truth-ledger row per active card using the schema in
`references/mega-groom.md`. Row count must equal the active-card snapshot.
Resolve or propose a resolution for shipped state, stale claims, priority,
claimability, dependencies, oracle quality, overlap, vision alignment, and
quarter disposition.

State invariants:

- `ready` means no unresolved blocker and an executable verification system.
- every active card has Goal + Oracle; ready/M+ work has the full proof loop.
- every P0/P1 cites current urgency evidence and is compared with the best
  next pickup.
- every overlap names a canonical survivor, containment relation, or explicit
  keep-both rationale.

**Complete when:** every snapshot ID has exactly one disposition and no state,
priority, relation, or readiness contradiction is merely "surfaced."

## 3. Fan out the quarter sweep

Load `references/mega-groom.md`. Commission its universal lenses as distinct,
fresh-context reports, then add at least three perspectives composed for this
repo. Waves are fine; combined roles are not independent evidence. Use
`references/investigation-bench.md` only for the lane-card shape.

If delegation is unavailable, do the coverage locally and label the run
**degraded**. Degraded work may improve the board but cannot claim a
comprehensive strategic groom.

**Complete when:** every mandatory lens has its own dispatch receipt, raw report,
evidence, distinct falsifier, world-class delta, and backlog move or evidenced
no-emission ruling. Any failed mandatory lens makes the strategic run degraded
and ineligible for comprehensive PASS.

## 4. Synthesize, then mutate

Build a candidate ledger before editing the board. Every lane recommendation
maps to one outcome: emitted card, updated card, absorbed-into ID, or evidenced
rejection. Resolve disagreements; do not average them into vague themes.

Create the 12-week portfolio defined in `references/mega-groom.md`. Apply safe
card creates, updates, and status corrections. Deletions and merges remain
explicit proposals unless the operator authorized them.

**Complete when:** every finding, candidate, and active card traces into the
portfolio, outside-quarter ledger, or evidenced rejection; the capacity and
critical path support all three evidence epochs; the board has one ready best
pickup; the net change is coherent rather than issue confetti.

## 5. Adversarial completion gate

Give a fresh critic only the before/after truth ledger, dispatch receipts, raw
lane reports, source matrix, candidate ledger, quarter portfolio, board diff,
and this contract. A PASS requires every snapshot card and mandatory lens
completed, dispatch/report provenance and lens distinctness verified, no
contradictory board state, all five strategic tracks investigated and either
represented or closed with an evidenced no-emission ruling, three feasible
evidence-producing epochs, and exactly one best pickup. Fix blockers and re-run
the critic.

Publish the plan as the ticket-linked HTML artifact required by shared
doctrine. Close with exact board changes, evidence, skipped/failed lanes, and
residual risk. End with a clean tree and Powder receipt.

## Branches

- **`/groom tidy`** — reconcile every active card and apply safe truth fixes;
  no strategic swarm or quarter portfolio.
- **`/groom scoped <target>`** — reconcile every card touching the target,
  then use the smallest independent lens set that can refute the target plan.
- **`/groom audit`** — read-only skill/harness usage and staleness report; use
  live telemetry, no automatic fixes.
- **`/groom moonshot`** — the full strategic contract plus premise inversion
  and external exemplars; never a substitute for board reconciliation.

## Card shape

Every emitted card follows `references/ticket-format.md`. Strategic emissions
are epic-shaped by default. Read `references/backlog-doctrine.md` only when
grooming Harness Kit/roster or a repo using the file fallback.

## Completion evidence

Report: snapshot count; truth-ledger count; source-matrix count and report IDs;
candidate dispositions; applied/proposed board diff; 12-week portfolio; one
best pickup; HTML artifact; critic verdict; exact gate/live evidence; residual.
