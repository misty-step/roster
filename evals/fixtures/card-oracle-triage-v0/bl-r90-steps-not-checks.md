# 002 — Grok + groom the four R90 projects, then dispatch lanes

**Status:** in progress
**Goal:** for each of Habitat, Olympus, Time Tracker, Allie — build a live
understanding, groom the backlog, and dispatch agent work in a dedicated herdr space.

## Per project
- [ ] **Grok**: read VISION/DESIGN/SPEC/AGENTS + backlog + recent git; write a
  current-state summary into that repo (and refresh the "Current focus" section of
  `context/<name>.md` here — pointer only, no context duplication).
- [ ] **Groom**: tidy + prioritize the backlog (repo `backlog.d` and/or Habitat MCP).
- [ ] **Dispatch**: open a herdr space rooted at the project, hand the top lane a
  card (end state, oracle, boundaries), and manage it.

## Order (suggested)
1. Habitat (it's the work-management bus — grok it first so dispatch has a system of record).
2. Olympus, Allie (coordinate — Allie is dogfooding the Olympus dashboard).
3. Time Tracker.

## Guardrails
- Size fan-out to review capacity. One unit of work per worktree.
- Keep domain context in each repo; only update registry pointers + `log.md` here.
