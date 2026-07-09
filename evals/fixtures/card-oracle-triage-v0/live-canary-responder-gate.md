# Gate external responders behind least privilege and audited context

Status: done · Priority: p0

Make Canary safe for arbitrary-user auto-triage by ensuring external responders receive only authorized, minimized, replayable, and auditable context.

## Acceptance
- Given a responder needs to claim or annotate one service's incident, then Canary can issue a narrow responder-write authority that permits claims and annotations for that service without broad admin access.
- Given an incident, error group, target, or monitor is serialized for a responder, then the payload is produced through a redacted context schema that includes tenant, project, service, subject, retention, and privacy policy.
- Given telemetry attributes, annotation metadata, and evidence links contain sensitive-looking data, then responder context either redacts/minimizes it server-side or excludes it by schema.
- Given a webhook receiver is registered as an automation responder, then a conformance fixture proves timestamp validation, delivery-id dedupe, and timeline replay before action.
- Given a responder reads rich incident detail, then Canary records a durable read-audit event with responder identity, subject, context envelope, and timestamp.
- Given browser capture is enabled for a consuming app, then it uses a public-ingest token or relay design that cannot read, administer, claim, or expose a secret API key.
- Given MCP or CLI tools expose responder actions, then their manifest scopes and runtime enforcement match the HTTP authority model.
