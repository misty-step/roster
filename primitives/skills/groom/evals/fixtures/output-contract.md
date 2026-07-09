# Common groom eval output contract

Both arms return JSON in this neutral shape. The contract exposes the required
artifact, not the hidden answer key.

```json
{
  "snapshot_count": 0,
  "mutations_performed": false,
  "truth_ledger": [{
    "id": "...", "finding_codes": ["..."], "disposition": "...",
    "relation_to": ["..."], "quarter_slot": "weeks-1-4|weeks-5-8|weeks-9-12|outside-quarter",
    "evidence_refs": ["R1"]
  }],
  "source_matrix": [{
    "report_id": "...", "lens": "...", "status": "complete",
    "brief_receipt": "...", "raw_report_receipt": "...", "dispatch_receipt": "...",
    "falsifier": "...", "evidence_scope": "...",
    "finding_ids": ["..."], "candidate_ids": ["..."]
  }],
  "finding_ledger": [{"finding_id": "...", "candidate_id": "...", "no_emission_reason": null}],
  "candidate_ledger": [{
    "candidate_id": "...", "disposition": "emit|update|absorb|reject",
    "target_id": "...", "evidence_refs": ["R2"]
  }],
  "portfolio": {
    "epics": [{
      "id": "...", "goal": "...", "oracle": "...", "proof_loop": "...",
      "children": ["..."], "dependencies": ["..."],
      "primary_track": "user-value|trust|system-quality|comprehension|operability",
      "secondary_track": null, "source_candidates": ["..."],
      "gap_refs": ["..."], "vision_clause": "...",
      "epoch": "weeks-1-4|weeks-5-8|weeks-9-12"
    }],
    "outside_quarter": [{"card_id": "...", "reason": "..."}],
    "capacity": {
      "wip_cap": 2, "assumption": "...", "critical_path": ["..."],
      "epoch_gates": {"weeks-1-4": "...", "weeks-5-8": "...", "weeks-9-12": "..."}
    },
    "best_pickup": "...",
    "best_pickup_rationale": "..."
  }
}
```

Every string represented by `...` is non-empty. Workers discover which finding
codes apply; the vocabulary below only normalizes output. Receipts identify
actual lane dispatch/brief/report artifacts, not invented role labels.

Normalize universal lens IDs as: `product-value`, `vision-premise`,
`user-operator-journey`, `domain-specialist`, `architecture`,
`simplification-deletion`, `runtime-reliability`, `security-privacy`,
`verification`, `operations-infrastructure`, `docs-onboarding`,
`agent-readiness`, `external-exemplars`. Prefix additional lens IDs with
`tailored-`.

Finding-code vocabulary: `ready-with-blocker`, `ready-no-oracle`,
`active-no-oracle`, `priority-evidence-drift`, `vision-conflict`,
`duplicate-overlap`, `epic-containment`, `stale-claim`, `merged-work-active`,
`deleted-branch`, `weak-proof-loop`, `symptom-not-outcome`,
`implementation-without-outcome`, `unsupported-scope`. Truth-ledger
dispositions: `keep`, `promote`, `block`, `reframe`, `demote`,
`outside-quarter`, `close-proposal`, `reject`, `merge-proposal`, `absorb`,
`promote-done`, `release-claim`.
