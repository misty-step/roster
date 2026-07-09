# Context Packet: Evaluate OpenCode as the code-review runner substrate

Priority: P1
Status: shaped
Estimate: M

## Goal

Prove whether Harness Kit should add an OpenCode session/service runner path
for code-review and review-eval lanes, or explicitly stay with the current thin
CLI dispatch surfaces.

## Premise

A 2026-06-19 coding-agent substrate report recommends OpenCode as the strongest
open per-job kernel for an owned PR-review system because it is
server/session-shaped rather than terminal-first. That maps to Harness Kit's
review/eval needs, but adopting it without evidence would recreate the
historical semantic-wrapper failure mode.

The outcome is not "use OpenCode because the report says so." The outcome is a
small bake-off that tells us whether OpenCode's session/event surface improves
review lane observability, context hygiene, retries, and structured evidence
over the existing `dispatch-agent` CLI path.

## Non-Goals

- Do not build a production review control plane in Harness Kit.
- Do not add semantic provider ranking, automatic fallback trees, or a workflow
  engine around provider CLIs.
- Do not move Mode B event orchestration into Harness Kit; Bitterblossom and
  product repos own event-triggered loops.
- Do not expose GitHub write credentials, model-provider keys, or user secrets
  to untrusted repository execution.
- Do not claim review-quality superiority from one model or one fixture.

## Notes

- This is the Harness Kit slice of the report. Olympus/Argus may later consume
  the outcome, but Olympus remains responsible for PR webhooks, GitHub App
  posting, durable run state, Sprite isolation, and Habitat writeback.
- The report's security lesson is non-negotiable: no write token or model key
  goes into a sandbox that can run repository-controlled code.
