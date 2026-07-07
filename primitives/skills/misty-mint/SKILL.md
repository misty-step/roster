---
name: mint
description: |
  Use when an agent needs to make an outbound HTTP call to a vendor API
  (OpenAI, GitHub, Stripe, or any third-party service) and would otherwise
  need an API key, token, or other secret. mint is the fleet's agent
  credential broker: route the call through mint's egress proxy carrying a
  capability token and credential placeholders — never request, embed, or
  expect real credential bytes in agent context. Trigger phrases: "mint",
  "credential broker", "egress proxy", "call the API through mint", "I need
  an API key", "capability token", "X-Mint-Capability", "secret
  placeholder", "__mint.", "proxy call".
argument-hint: "[proxy|serve|policy-check|audit-tail|alias-list]"
---

# mint

mint is the fleet's agent credential broker. Founding principle: **agents
cause authorized effects; they never possess authority.** Credential bytes
never enter an agent's runtime. mint owns *use*, not storage — custody stays
in a backing secret store (the macOS keychain today; Fly secrets, 1Password,
or OpenBao later). mint is not a secret store and does not hand agents
secret values, ever, through any surface.

See mint's own `VISION.md` before assuming a capability beyond what is
shipped today — "What mint doesn't do yet" below is the honest boundary.

## What mint does today

One execution mode ships: the **egress credential proxy**. An agent sends
the request it would have sent straight to the vendor to mint instead; mint
authenticates it, checks policy, checks budget, checks the per-host circuit
breaker, resolves any credential placeholders in the forwarded headers
against the real secret, forwards the call, scrubs the response, writes an
audit event, and returns the result. Two other modes (a typed-action broker;
a secretless protocol proxy) are declared in mint's VISION.md but **not
built** — do not assume either exists.

## How an agent calls mint

Send the exact request you would have sent to the vendor to mint's proxy
route instead, with the vendor scheme/host/path folded into the URL and a
capability header added:

```
{METHOD} {MINT_BASE_URL}/proxy/{scheme}/{host}/{*rest}
X-Mint-Capability: <capability-token>
<any forwarded header that would carry a real credential>: ...__mint.<service>.<name>__...
```

Concrete example — calling `https://api.openai.com/v1/responses` through the
`openai.default` alias:

```sh
curl -H "X-Mint-Capability: $MINT_CAPABILITY_TOKEN" \
     -H "Authorization: Bearer __mint.openai.default__" \
     -X POST "${MINT_BASE_URL}/proxy/https/api.openai.com/v1/responses" \
     -d '{"model": "...", "input": "..."}'
```

`MINT_BASE_URL` — resolve from the environment at call time, never hardcode
it. As of 2026-07-07 mint is deployed as the private Fly app `misty-mint`
(org misty-step): from another Fly app in the org it is reachable at
`http://misty-mint.internal:4949` over the private 6PN; from a laptop over
`fly proxy 4949:4949 --app misty-mint` (then `http://127.0.0.1:4949`); locally
`mint serve` defaults to `http://127.0.0.1:4949`. A stable tailnet URL is the
pending follow-up (the app joins the tailnet once its `TAILSCALE_AUTHKEY` is
set) — so keep resolving `MINT_BASE_URL` from the environment rather than
pinning any of these.

- `X-Mint-Capability` carries a token scoped to mint itself (`aud: mint`,
  short TTL, budgeted) — **not** a vendor credential. Missing or invalid →
  `401`.
- Put `__mint.<service>.<name>__` anywhere a real credential value would go
  inside a *forwarded header*. mint resolves it against the real secret
  after the request has left the sandbox and swaps it in — the agent never
  holds the value. This placeholder is the resolvable form of the
  operator-facing alias `secret://<service>/<name>`.
- Never hand-construct, guess, or reuse a credential you happen to see
  elsewhere. If there's no placeholder for the service you need, ask the
  operator to declare the alias and a matching policy rule — do not fall
  back to an inline key "just this once."

## Errors — read the status code, don't retry blind

| Status | Meaning | What it means for you |
|---|---|---|
| `401` | Missing/invalid `X-Mint-Capability` | Your capability token is missing, expired, or wrong. Do not retry with a guessed token. |
| `403` | No matching policy rule (deny-by-default), or the matched rule has `approval_required: true` | mint made **zero** upstream calls either way. Ask the operator for a policy rule (or for the human-tap approval mint doesn't implement yet) — don't retry. |
| `429` | Over budget (`max_calls` exceeded) | Session/rule call budget is exhausted. Stop calling this service until budget resets or is raised. |
| `503` | Circuit breaker open for that host | The upstream host has been failing repeatedly; mint is short-circuiting. Back off. |
| `504` | Upstream request timed out | The vendor didn't respond in time. Safe to retry with backoff. |

Every response body — including denials — is scrubbed of known-sensitive
JSON fields (`access_token`, `refresh_token`, `private_key`, `client_secret`,
`api_key`) before it reaches you.

## What mint doesn't do yet (honest scope)

- Only the egress HTTP proxy mode exists — no typed-action broker, no
  secretless protocol proxy.
- Backing stores today: the macOS keychain (local) and env-var custody for
  the deployed broker (`MINT_SECRET_*`, provisioned as Fly secrets). 1Password/
  OpenBao are still future work.
- Only shared-secret capability auth today — tailnet-WhoIs auth is future.
- Only flat two-segment aliases (`secret://<service>/<name>`) — no
  hierarchical names.
- `approval_required` policy rules currently **deny** the call outright.
  There is no human-tap escalation yet; do not expect a pending/approval
  response back, ever — it comes back as a `403`.
- No hot reload — policy and capability changes require restarting `mint
  serve`.
- No MCP server or SDK face yet (see below).

## Operator CLI (not the agent path)

Operators — not agents making calls — use the CLI to run and inspect the
broker locally:

```sh
mint serve                # start the broker
mint policy check <file>  # validate a policy YAML file, report rule count or the exact error
mint audit tail [-n N]    # print the last N audit events, oldest first (default 20)
mint alias list            # list declared aliases and descriptions — never values
```

An agent making a vendor call always goes through the HTTP proxy above, not
this CLI.

## MCP (declared, not yet built)

mint has no MCP server today. The declared shape is a stdio server
(`mint-mcp`) exposing **read-only verbs mirroring the CLI** — alias list,
policy check, audit tail — for operator-facing inspection only. It will
**not** expose a tool that resolves or returns a secret value; that would
defeat the broker's entire premise. Do not expect an MCP tool that hands you
a credential, now or later. Until `mint-mcp` ships, use the HTTP proxy
contract above for agent calls and the CLI for operator inspection.

When `mint-mcp` ships, its `primitives/mcps/factory-mcps.yaml` entry should
follow the file's existing `not_applicable` → `available` pattern (compare
the `landmark` and `aesthetic` entries, both `not_applicable` until their own
MCP servers exist):

```yaml
  - id: mint
    app: mint
    source_repo: misty-step/mint
    product_skill: misty-mint
    status: not_applicable   # flip to `available` once mint-mcp ships
    reason: mint has no MCP server yet; use the HTTP proxy contract (agents) or the CLI (operators).
    capabilities:
      - alias list (read-only)
      - policy check (read-only)
      - audit tail (read-only)
```

This is documentation only, showing the shape to add later —
`primitives/mcps/factory-mcps.yaml` lives outside this skill's directory and
has not been edited as part of authoring this skill.

## Red lines

- Never accept, request, or echo a real credential value. If one reaches
  your context from any source, that is a mint-bypass bug — stop and flag
  it, don't route around mint to "fix" the call.
- Never hardcode `MINT_BASE_URL`. Resolve it from the environment at call
  time; the fleet endpoint is not yet finalized.
- The capability token is itself sensitive, even though it is scoped and
  short-lived. Handle it as a secret reference — never log it or paste it
  into code, commits, or reports.

## Verification

In the mint repo, `scripts/mint-probe.sh` is mint's own definition of done:
it proves an agent-shaped caller never sees the real secret, the audit log
never contains it, and a policy-denied call reaches the vendor zero times.
Trust that script and its CI job over this skill's prose if the two ever
disagree.
