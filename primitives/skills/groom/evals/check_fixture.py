#!/usr/bin/env python3
"""Objective grader for groom fixture JSON; semantic quality stays blind-graded."""

import json
import sys
from pathlib import Path


def fail(message: str) -> None:
    print(f"FAIL: {message}")
    raise SystemExit(1)


def nonempty(value: object, label: str) -> None:
    if value is None or value == "" or value == [] or value == {}:
        fail(f"{label} is empty")


if len(sys.argv) != 3:
    fail("usage: check_fixture.py ANSWER_KEY.json ARTIFACT.json")

key = json.loads(Path(sys.argv[1]).read_text())
artifact = json.loads(Path(sys.argv[2]).read_text())

if artifact.get("mutations_performed") is not False:
    fail("mutations_performed must be false")
if artifact.get("snapshot_count") != key["snapshot_count"]:
    fail("snapshot_count mismatch")

rows = artifact.get("truth_ledger", [])
by_id = {row.get("id"): row for row in rows}
if len(rows) != key["snapshot_count"] or len(by_id) != len(rows):
    fail("truth ledger must contain each snapshot card exactly once")
if set(by_id) != set(key["cards"]):
    fail("truth ledger card IDs differ from answer key")

for card_id, expected in key["cards"].items():
    row = by_id[card_id]
    missing = set(expected["required"]) - set(row.get("finding_codes", []))
    if missing:
        fail(f"{card_id} missing findings: {sorted(missing)}")
    if row.get("disposition") not in expected["allowed"]:
        fail(f"{card_id} disposition {row.get('disposition')!r} not allowed")
    if expected.get("relation_to") and not set(expected["relation_to"]) & set(row.get("relation_to", [])):
        fail(f"{card_id} missing canonical relation")
    nonempty(row.get("quarter_slot"), f"{card_id}.quarter_slot")
    nonempty(row.get("evidence_refs"), f"{card_id}.evidence_refs")

reports = artifact.get("source_matrix", [])
complete = [row for row in reports if row.get("status") == "complete"]
if len(complete) < len(key["mandatory_lenses"]) + 3:
    fail("fewer than 16 complete independent reports")
lenses = {row.get("lens") for row in complete}
if len(lenses) != len(complete):
    fail("lens IDs are not unique")
missing_lenses = set(key["mandatory_lenses"]) - lenses
if missing_lenses:
    fail(f"mandatory lenses missing: {sorted(missing_lenses)}")
tailored = lenses - set(key["mandatory_lenses"])
if len(tailored) < 3 or any(not lens.startswith("tailored-") for lens in tailored):
    fail("fewer than three repo-composed lenses")
for field in ("report_id", "brief_receipt", "raw_report_receipt", "dispatch_receipt", "falsifier", "evidence_scope"):
    values = [row.get(field) for row in complete]
    if len(set(values)) != len(values):
        fail(f"source_matrix.{field} values are not unique")
for row in complete:
    for field in ("report_id", "brief_receipt", "raw_report_receipt", "dispatch_receipt", "falsifier", "evidence_scope"):
        nonempty(row.get(field), f"source_matrix.{field}")
    nonempty(row.get("finding_ids"), f"{row.get('report_id')}.finding_ids")

findings = artifact.get("finding_ledger", [])
finding_ids = {row.get("finding_id") for row in findings}
for row in complete:
    if not set(row.get("finding_ids", [])) <= finding_ids:
        fail(f"{row.get('report_id')} has undispositioned findings")

candidates = artifact.get("candidate_ledger", [])
candidate_ids = {row.get("candidate_id") for row in candidates}
if len(candidate_ids) != len(candidates):
    fail("candidate IDs are not unique")
for row in findings:
    if not row.get("candidate_id") and not row.get("no_emission_reason"):
        fail(f"{row.get('finding_id')} lacks candidate or no-emission ruling")
    if row.get("candidate_id") and row["candidate_id"] not in candidate_ids:
        fail(f"{row.get('finding_id')} points to missing candidate")
for row in candidates:
    if row.get("disposition") not in {"emit", "update", "absorb", "reject"}:
        fail(f"{row.get('candidate_id')} has invalid disposition")

portfolio = artifact.get("portfolio", {})
epics = portfolio.get("epics", [])
if not key.get("min_epics", 6) <= len(epics) <= key.get("max_epics", 10):
    fail(f"fixture requires {key.get('min_epics', 6)}-{key.get('max_epics', 10)} epics")
tracks = {track for epic in epics for track in (epic.get("primary_track"), epic.get("secondary_track")) if track}
if not set(key["tracks"]) <= tracks:
    fail("portfolio does not cover all strategic tracks")
epochs = {epic.get("epoch") for epic in epics}
if not set(key["epochs"]) <= epochs:
    fail("portfolio does not cover all epochs")
for epic in epics:
    for field in ("id", "goal", "oracle", "proof_loop", "children", "source_candidates", "vision_clause"):
        nonempty(epic.get(field), f"epic.{field}")
    if "dependencies" not in epic or not isinstance(epic["dependencies"], list):
        fail(f"{epic.get('id')}.dependencies must be a list")
    if epic.get("primary_track") not in key["tracks"]:
        fail(f"{epic.get('id')} has invalid primary track")
    if epic.get("secondary_track") and epic["secondary_track"] not in key["tracks"]:
        fail(f"{epic.get('id')} has invalid secondary track")
    if epic.get("epoch") not in key["epochs"]:
        fail(f"{epic.get('id')} has invalid epoch")
if len({epic.get("id") for epic in epics}) != len(epics):
    fail("epic IDs are not unique")

capacity = portfolio.get("capacity", {})
for field in ("wip_cap", "assumption", "critical_path", "epoch_gates"):
    nonempty(capacity.get(field), f"capacity.{field}")
nonempty(portfolio.get("best_pickup"), "best_pickup")
nonempty(portfolio.get("best_pickup_rationale"), "best_pickup_rationale")
known_pickups = set(by_id) | {epic.get("id") for epic in epics}
if portfolio["best_pickup"] not in known_pickups:
    fail("best_pickup is not a card or epic ID")
if portfolio["best_pickup"] not in key["ready_pickups"]:
    fail("best_pickup is not fixture-approved ready work")

print(f"PASS: {len(rows)} cards, {len(complete)} reports, {len(epics)} epics")
