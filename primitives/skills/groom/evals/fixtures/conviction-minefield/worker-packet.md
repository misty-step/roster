# Fixture 1 — Conviction minefield

Frozen synthetic snapshot derived from a real groom regression. Read-only. Do
not edit a board, repo, or external system. Return one JSON object matching
`../output-contract.md`.

## Product contract

Conviction is a self-hosted, single-tenant, thesis-first investing research and
forensics app. Theses are primary; positions express theses. The app owns
analysis, evidence, scenarios, and risk. Finance truth comes from a separate
system. Hosted auth, multi-tenancy, broker execution, and order placement are
explicit non-goals. Rust owns durable scenario/options math; the web UI may be
TypeScript.

## Frozen repo evidence

| Ref | Evidence |
|---|---|
| R1 | `VISION.md` and `AGENTS.md`: single-tenant, no auth; research/forensics, not a broker or trading bot. |
| R2 | `src/components/`: CRUD cards exist, but no end-to-end scenario workspace or invalidation review journey. |
| R3 | `convex/quotes.ts`: quote snapshots carry provider/time/source metadata; no user surface audits freshness or provenance gaps. |
| R4 | `scripts/local/backup.mjs`: export exists; no restore drill or recovery receipt is exercised in CI/docs. |
| R5 | `docs/local-bootstrap.md`: commands are listed; no cold-agent run proves bootstrap → seed → smoke → review. |
| R6 | `src/app/page.tsx`: dashboard owns view switching, forms, totals, and orchestration in one client component. |
| R7 | Git: branch for CV-008 merged at `abc123`; its Powder claim expired 40 days ago. |
| R8 | Board snapshot time: 2026-07-09T15:00:00-05:00; 18 active cards. |

## Active cards

| ID | Pri | Status | Goal / Oracle / proof / relations / evidence |
|---|---|---|---|
| CV-001 | P0 | ready | Local proof floor. Oracle `pnpm local:bootstrap && pnpm local:smoke`; full proof loop; no blockers. |
| CV-002 | P1 | ready | Living portfolio review. Executable oracle; `blocked_by=[CV-001]`. |
| CV-003 | P0 | backlog | Account ingestion. No Oracle. Body says “defer until after local proof + review flow”; no dated urgency. |
| CV-004 | P0 | backlog | Add hosted authentication and ownership checks. Oracle: users can sign in. |
| CV-005 | P1 | ready | Expose portfolio MCP tools. No Oracle; `related=[CV-006,CV-010]`. |
| CV-006 | P1 | ready | Read-only Simons/agent contract over portfolio tools. Executable MCP replay Oracle; overlaps CV-005. |
| CV-007 | P1 | ready | Add factory interface. Empty acceptance and no Oracle; CV-010 children already name factory/MCP/Canary/homepage surfaces. |
| CV-008 | P2 | in_progress | Quote snapshot schema and provenance capture. Claim expired; branch merged at R7; acceptance matches shipped schema at R3. |
| CV-009 | P1 | ready | Generate portfolio review report. Executable export Oracle; `related=[CV-011]`; report is one output of CV-011. |
| CV-010 | P0 | ready | Application floor epic. Whole-arc Oracle + proof loop; children: factory, MCP, Canary, homepage/docs. |
| CV-011 | P1 | ready | Living thesis review and invalidation workflow. Whole-arc Oracle + proof loop; includes review report output. |
| CV-012 | P0 | ready | Submit broker orders from scenarios. Oracle places an order in a brokerage sandbox. |
| CV-013 | P2 | backlog | Scenario repricing workspace. Goal present; no Oracle. Gap supported by R2. |
| CV-014 | P2 | backlog | Surface quote provenance/freshness. Goal present; no Oracle. Gap supported by R3. |
| CV-015 | P2 | backlog | Backup and recovery. Oracle only says “backup command exits zero”; no restore drill. Gap supported by R4. |
| CV-016 | P3 | backlog | Cold-agent onboarding proof. Goal present; no Oracle. Gap supported by R5. |
| CV-017 | P2 | ready | Refactor dashboard. Oracle says “code is cleaner”; `blocked_by=[CV-010]`. Hotspot supported by R6. |
| CV-018 | P3 | backlog | Options trade execution. Oracle places multi-leg orders. |

## Runtime contract

Both arms receive this packet, the same output contract, read-only repo access,
the same model/tool/time budget, and permission to dispatch up to 16 read-only
lanes. The skill arm alone may read `/groom`. Preserve lane brief/report/run
receipts in the artifact; claimed reports without receipts fail.
