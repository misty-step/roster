# Estate observe-plan request

Use Estate's live map and `estate resource <id>` projections to inspect declared
resources, desired state, observed provider state, cost, health, recovery, and
provider-readback evidence. Read linked declarations and exact plan context before
producing an exact OpenTofu/provider plan.

This intent is strictly read-only: it may save an exact plan for operator review,
but it never applies a plan, changes a provider, or requests a mutation credential.
If standards, exceptions, readback, or evidence are stale or missing, report the
condition and remain read-only.
