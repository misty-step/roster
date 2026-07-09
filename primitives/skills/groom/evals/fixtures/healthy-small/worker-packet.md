# Fixture 3 — Healthy small repo

Frozen synthetic snapshot of a focused Rust library. Read-only. Return one JSON
object matching `../output-contract.md`; do not edit any system.

## Product contract

Ledgerlite is a 2,000-line embeddable Rust library for signed, append-only local
journals. It prizes a tiny API, deterministic recovery, and offline operation.
It is not an application, hosted service, multi-tenant platform, or autonomous
agent. Excellent means difficult data loss, calm errors, and boring upgrades.

## Frozen repo evidence

| Ref | Evidence |
|---|---|
| S1 | Public API has six functions; consumer compile tests, docs, CI, release signing, and benchmarks are strong. |
| S2 | Crash recovery is unit-tested but never fault-injected across a real filesystem/process boundary. |
| S3 | Key rotation is documented as export/reimport; no atomic migration or rollback receipt exists. |
| S4 | Three parse failures collapse into one opaque `InvalidJournal` error. |
| S5 | A legacy v1 serializer remains behind a flag used by no pinned consumer fixture. |
| S6 | Board snapshot: 2026-07-09T15:00:00-05:00; 7 active cards. |

## Active cards

| ID | Pri | Status | Goal / Oracle / proof / relations / evidence |
|---|---|---|---|
| HS-001 | P1 | ready | Real crash/recovery fault-injection proof. Executable process-kill Oracle; S2. |
| HS-002 | P1 | backlog | Atomic key rotation with rollback. Goal present; no Oracle; S3. |
| HS-003 | P2 | backlog | Typed recovery/parse errors. Goal present; no Oracle; S4. |
| HS-004 | P2 | backlog | Remove unused v1 serializer if consumer proof permits. Decision-seeking Oracle; S5. |
| HS-005 | P1 | ready | Build a web administration UI. Executable UI Oracle; violates product contract. |
| HS-006 | P2 | backlog | Add multi-tenant cloud synchronization. Violates local/offline contract. |
| HS-007 | P2 | backlog | Add an autonomous journal-maintenance agent. No evidenced user need; violates library focus. |

## Runtime contract

Both arms receive this packet, the common output contract, identical model,
tools, time, read-only repo access, and up to 16 read-only lanes. Arm A alone
may read `/groom`. Actual dispatch/brief/report receipts are required. The
correct result may be broad investigation with a small evidenced portfolio.
