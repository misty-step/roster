# Fixture 2 — Maintenance monoculture

Frozen synthetic snapshot of a mature deployment CLI. Read-only. Return one
JSON object matching `../output-contract.md`; do not edit any system.

## Product contract

Harborline is a local-first, zero-trust deployment CLI for small operators. Its
promise is boring, reversible deployments from a terminal. It may expose
machine-readable agent interfaces, but a hosted dashboard and autonomous
production mutation are non-goals.

## Frozen repo evidence

| Ref | Evidence |
|---|---|
| M1 | A fresh install reaches `harborline deploy` but fails with an opaque missing-config error; no guided init exists. |
| M2 | `--debug` logs the complete Authorization header on provider failures. |
| M3 | Rollback code has unit coverage; no real deploy→failure→rollback drill or receipt exists. |
| M4 | Four plugin adapters are shallow pass-throughs around one unstable registry interface. |
| M5 | Human-formatted stdout is the only surface; no stable JSON, MCP, or agent receipt contract exists. |
| M6 | CI, release signing, docs reference, and the core deploy happy path are otherwise strong. |
| M7 | Board snapshot: 2026-07-09T15:00:00-05:00; 14 active cards. |

## Active cards

| ID | Pri | Status | Goal / Oracle / proof / relations / evidence |
|---|---|---|---|
| HM-001 | P1 | ready | Rename plugin modules. No Oracle; `related=[HM-002,HM-014]`. |
| HM-002 | P1 | ready | Extract plugin wrapper helpers. Oracle says “less duplication”; overlaps HM-001/HM-014. |
| HM-003 | P2 | ready | Add plugin unit tests. Oracle runs unit tests; no integration proof; belongs to plugin outcome. |
| HM-004 | P2 | backlog | Update README config flags. No Oracle; first-run symptom at M1. |
| HM-005 | P1 | in_progress | Improve missing-config error. Claim expired 45 days ago; branch deleted; gap remains at M1. |
| HM-006 | P2 | ready | Refactor plugin registry. Oracle says “cleaner API”; gap at M4. |
| HM-007 | P2 | backlog | Raise line coverage. No user/system outcome; core coverage already strong at M6. |
| HM-008 | P1 | ready | Redact secrets from debug logs. Executable failure replay Oracle; full proof loop; M2. |
| HM-009 | P2 | backlog | Document rollback. Oracle checks page exists; no live drill; M3. |
| HM-010 | P1 | ready | Build hosted deployment dashboard. Executable UI Oracle; conflicts with product contract. |
| HM-011 | P2 | backlog | Let an agent deploy production autonomously. Conflicts with zero-trust/reversible human operation. |
| HM-012 | P1 | backlog | Guided first-run init. Goal present; no Oracle; M1. |
| HM-013 | P2 | backlog | Machine-readable agent surface and receipts. Goal present; no Oracle; M5. |
| HM-014 | P2 | backlog | Deep plugin contract with stable adapter boundary. Goal present; no Oracle; M4. |

## Runtime contract

Both arms receive this packet, the common output contract, identical model,
tools, time, read-only repo access, and up to 16 read-only lanes. Arm A alone
may read `/groom`. Actual dispatch/brief/report receipts are required.
