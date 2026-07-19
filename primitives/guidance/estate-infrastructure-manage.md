# Estate manage request

Manage only from a saved exact OpenTofu/provider plan whose resource scope and
digest have been checked and explicitly approved by the operator. Record the
named reviewer, obtain the scoped Mint/provider credential through the normal
operator path, and apply through the existing OpenTofu/provider tooling rather
than a custom path.

High-risk or irreversible work may require additional controls; it never
weakens the base exact-plan approval requirement. After apply, verify health and
provider readback, then return a safe evidence pointer for Estate's next
reconciliation with the plan digest, reviewer, observation time, and outcome.
Do not invent an Estate write API. Keep credentials, provider state, full plans,
and raw logs out of bundles. Expired standards or exceptions and missing
evidence fail closed.
