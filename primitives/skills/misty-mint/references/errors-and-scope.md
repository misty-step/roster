# mint — errors and honest scope

## Errors — read the status code, don't retry blind

| Status | Meaning | What it means for you |
|---|---|---|
| `401` | Missing/invalid `X-Mint-Capability` | Your capability token is missing, expired, or wrong. Do not retry with a guessed token. |
| `403` | No matching policy rule (deny-by-default), or the matched rule has `approval_required: true` | mint made **zero** upstream calls either way. Ask the operator for a policy rule (or for the human-tap approval mint doesn't implement yet) — don't retry. |
| `429` | Over budget (`max_calls` exceeded) | Session/rule call budget is exhausted. Stop calling this service until budget resets or is raised. |
| `503` | Circuit breaker open for that host | The upstream host has been failing repeatedly; mint is short-circuiting. Back off. |
| `504` | Upstream request timed out | The vendor didn't respond in time. Safe to retry with backoff. |

Every response body — including denials — is scrubbed of known-sensitive JSON
fields (`access_token`, `refresh_token`, `private_key`, `client_secret`,
`api_key`) before it reaches you.

## What mint doesn't do yet (honest scope)

See mint's own `VISION.md` before assuming a capability beyond what ships today.

- Only the egress HTTP proxy mode exists — no typed-action broker, no
  secretless protocol proxy (both declared in VISION.md, neither built).
- Backing stores today: the macOS keychain (local) and env-var custody for
  the deployed broker (`MINT_SECRET_*`, provisioned as Fly secrets). 1Password/
  OpenBao are still future work.
- Only shared-secret capability auth today — tailnet-WhoIs auth is future.
- Only flat two-segment aliases (`secret://<service>/<name>`) — no
  hierarchical names.
- `approval_required` policy rules currently **deny** the call outright. There
  is no human-tap escalation yet; it comes back as a `403`, never a
  pending/approval response.
- No hot reload — policy and capability changes require restarting `mint serve`.
- No SDK face yet (the MCP face shipped — see `operator-surfaces.md`).
