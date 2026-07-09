# Strategic Quarter Sweep

This is the single source for strategic `/groom` breadth, synthesis, and
portfolio completeness.

## Truth ledger

One row per active Powder card at the frozen snapshot:

| Field | Required judgment |
|---|---|
| ID / title | exact board identity |
| Current state | priority, status, age, claim, branch/merge evidence |
| Outcome | Goal and user/operator value |
| Evidence | live source plus freshness |
| Strategy | vision clause, theme, primary track |
| Premise | valid, reframed, disproven, uncertain |
| Relations | canonical outcome, duplicates, dependencies, unlocks |
| Readiness | Oracle and verification-system completeness |
| Disposition | keep, reframe, merge-proposal, demote, close-proposal, promote, block |
| Quarter slot | weeks 1–4, 5–8, 9–12, or outside-quarter with reason |

Automatic findings: ready without an executable proof loop; ready with an
unresolved blocker; P0/P1 without current urgency evidence; stale or missing
claim evidence; duplicate outcomes without a relation; epic without whole-arc
done criteria; research without a decision target.

## Independent lens map

Commission every universal lens separately. One report may inform several
surfaces, but it counts only for the perspective it was commissioned to hold.

| Lens | Question |
|---|---|
| Product/value | Who is this for, why does it matter, what makes it indispensable? |
| Vision/premise | Is this the right category, audience, and problem? |
| User/operator journey | Can the core job be completed clearly and delightfully? |
| Domain specialist | What does this specific domain demand that generic software advice misses? |
| Architecture | Are modules deep, boundaries honest, and interfaces small? |
| Simplification/deletion | What can disappear, collapse, or become declarative? |
| Runtime reliability | What fails under restart, pressure, retries, or partial outage? |
| Security/privacy | Where can authority, secrets, data, metadata, or logs leak? |
| Verification | Which likely failures escape the live proof loops and gates? |
| Operations/infrastructure | Are deploy, backup, recovery, cost, and observability boring? |
| Docs/onboarding | Can a cold human understand, run, and debug it? |
| Agent readiness | Can a cold agent discover, act, verify, and leave receipts? |
| External exemplars | What adjacent systems prove, contradict, or warn against? |

Add **at least three repo-composed lenses** derived from a named project
invariant, persona, incident, competitor, or product tension. Each names a
distinct falsifier and evidence scope that generic review would miss. A normal
strategic sweep therefore produces at least 16 independent reports. If a lens
cannot run, mark the whole strategic run degraded; do not replace its judgment
with a combined report or claim comprehensive PASS.

Each report returns:

```markdown
**<Report ID> — <Lens>**
Evidence: <files, commands, routes, URLs, artifacts>
Top findings: <stable finding ID; ranked; impact + confidence>
World-class delta: <what excellent requires>
Backlog move: <candidate ID(s), or no-emission; group finding IDs explicitly>
Provenance: <lane brief path/hash, raw report path, dispatch/run receipt>
```

## Source and candidate matrices

The source matrix has one row per commissioned report: status (`complete`,
`partial`, `failed`, `skipped`), brief/report/dispatch provenance, falsifier,
evidence scope, finding IDs, contribution, and candidate IDs. Reject duplicate
falsifiers or substantially identical evidence scopes as one lens in costumes.
The finding ledger maps every finding to a candidate or explicit no-emission;
the candidate ledger maps every candidate to exactly one disposition:
`emit`, `update`, `absorb into <card>`, or `reject because <evidence>`.

These ledgers prevent swarm theater: provenance plus distinct falsifiers and
evidence scopes demonstrate independence; finding and candidate disposition
demonstrate synthesis. Many reports with no mapped consequences fail.

## Quarter portfolio

Normally produce 6–10 coherent epics under themes, sequenced into weeks 1–4,
5–8, and 9–12. Each epoch ends in inspectable evidence: working route, replay,
benchmark, recovery drill, rendered artifact, or cold-agent run.

Every epic has one observable outcome, Goal, whole-arc Oracle, proof loop,
ordered inline child outcomes, dependencies, primary track, source
candidate(s), and vision clause. Children remain inline until selected.

The portfolio must cover five tracks:

1. **User value** — UX and/or a major product capability.
2. **Trust** — correctness, reliability, security, or verification.
3. **System quality** — architecture, simplification, or deletion.
4. **Comprehension** — docs, onboarding, or agent readiness.
5. **Operability** — delivery, backups, observability, or routine operations.

One epic may serve at most two tracks and names one primary. A track with no
epic needs live evidence that it already meets the world-class target. Each
quarter also contains two user-visible bets (or an evidenced ruling that the
project has no user-facing product surface), one explicit deletion/consolidation,
one externally informed premise challenge, and one operational or
agent-leverage move. `Now` is ready; `Next` is shaped with dependencies; `Later` is an outcome,
not an idea. Exactly one card is the best next pickup, with a ranked explanation
of what it unlocks or de-risks.

Add a feasibility table: declared parallel WIP cap, capacity assumption,
dependency critical path, epic-to-epoch assignment, and the evidence/decision
gate ending each epoch. Work beyond the WIP cap is sequenced. A structurally
complete fantasy quarter fails.

## Anti-padding

- Coverage earns investigation, not a ticket.
- Generic tests/docs/polish become children of an evidenced outcome.
- One finding does not become sibling cards without distinct observables.
- Report new-card count and net active-card change.
- More than 10 epics requires a consolidation audit.
- Fewer than 6 epics requires a candidate-kill matrix showing every other
  evidenced opportunity was absorbed, rejected, already satisfied, or outside
  vision; all tracks and epochs still apply.
- Active work outside the quarter remains explicit with its reason; it is not
  forced into the portfolio or mislabeled rejected.
- No top-level outcome survives without live evidence, a vision link, and a
  place in the sequence.

## Final artifact

First viewport: verdict, world-class target, five-track portfolio, epochs, and
best pickup. Below it: before/after board truth, source matrix, candidate
ledger, themes, epics with children, deletions/consolidations, board diff,
critic verdict, and residual risk.
