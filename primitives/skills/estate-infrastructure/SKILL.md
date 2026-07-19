---
name: estate-infrastructure
description: |
  Use when infrastructure work needs the current Estate map, resource state,
  declarations, provider readback, an exact plan, or the provider-native
  management loop. Trigger: /estate-infrastructure, /estate.
argument-hint: "[observe-plan|manage]"
---

# /estate-infrastructure

Estate is the map and evidence surface for infrastructure context. This primitive
teaches an agent to inspect that context and, when explicitly asked, operate the
provider through the ordinary OpenTofu/provider tooling. Estate does not apply
changes, mint credentials, or execute provider commands.

## Navigate the live Estate surface

Read the live Estate revision selected by the workspace or control plane before
making a recommendation or taking action:

1. Run `estate map` to find declared resources and their current Estate ids.
2. Run `estate resource <id>` for the selected resource's desired, observed,
   cost, health, recovery, and evidence projections.
3. Read the linked declarations and the exact plan context for the requested
   scope. Do not infer resource identity, provider, or consequence from a role,
   pack, or remembered preference.
4. Read the provider readback and its observation time. Treat missing, stale, or
   contradictory projections as a stop condition, not an invitation to guess.
5. Read the current Estate standards, vendor defaults, applicable exceptions, and
   decisions that govern the resource. A standard with a passed `review_date` or
   an expired exception fails closed: report it and remain read-only until the
   evidence is refreshed.

## Intent classes

- `observe-plan`: use Estate map/resource/declaration/provider-readback data for
  inventory, reconciliation, drift, cost, health, recovery, or forecasting. This
  intent may produce an exact OpenTofu/provider plan for review, but it never
  applies a plan, changes a provider, or requests a mutation credential.
- `manage`: use only after a saved exact OpenTofu/provider plan exists for the
  requested scope. Confirm the plan digest and resource bounds, obtain explicit
  operator approval of that exact plan and digest, record the named reviewer,
  obtain a scoped Mint/provider credential through the normal operator path,
  and apply with the existing OpenTofu/provider tooling. After apply, verify
  health and provider readback, then return a
  secret-free evidence pointer for Estate's next reconciliation. Do not
  fabricate an Estate write command or API.

High-risk or irreversible changes may require additional provider or
organizational controls; they never weaken the base requirement for explicit
operator approval of the exact plan. A role, pack, chat message, or Estate map
entry cannot stand in for that approval. Do not add an Estate execution or
permission layer or an alternate mutation path; use the provider's ordinary
tooling and its scoped credential instead.

## Evidence and bundle hygiene

Keep literal credentials, provider snapshots, state, full plans, and raw logs out
of Git and resolved bundles. Keep only safe references: the Estate revision and
resource id, standards and exceptions read, exact-plan digest or external plan
location, named operator review, provider-readback clock, health/cost/recovery
summary, and the evidence pointer returned for Estate reconciliation. Never
claim an apply, operator review, rollback, recovery, or healthy readback without
live evidence.
