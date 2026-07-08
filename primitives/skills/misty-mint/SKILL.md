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

mint is the fleet's agent credential broker. Founding principle: **agents cause
authorized effects; they never possess authority.** Credential bytes never enter
an agent's runtime. mint owns *use*, not storage — custody stays in a backing
secret store. mint is not a secret store and does not hand agents secret values,
ever, through any surface.

One execution mode ships: the **egress credential proxy**. An agent sends the
request it would have sent straight to the vendor to mint instead; mint
authenticates it, checks policy/budget/circuit-breaker, resolves credential
placeholders against the real secret *after the request leaves the sandbox*,
forwards the call, scrubs the response, writes an audit event, and returns the
result. Honest scope of what is and isn't built:
`references/errors-and-scope.md`.

## Call mint (the agent path)

Send the exact request you would have sent to the vendor to mint's proxy route,
with the vendor scheme/host/path folded into the URL and a capability header
added:

```
{METHOD} {MINT_BASE_URL}/proxy/{scheme}/{host}/{*rest}
X-Mint-Capability: <capability-token>
<any forwarded header that would carry a real credential>: ...__mint.<service>.<name>__...
```

Concrete — calling `https://api.openai.com/v1/responses` through the
`openai.default` alias:

```sh
curl -H "X-Mint-Capability: $MINT_CAPABILITY_TOKEN" \
     -H "Authorization: Bearer __mint.openai.default__" \
     -X POST "${MINT_BASE_URL}/proxy/https/api.openai.com/v1/responses" \
     -d '{"model": "...", "input": "..."}'
```

- **`X-Mint-Capability`** carries a token scoped to mint itself (`aud: mint`,
  short TTL, budgeted) — **not** a vendor credential. Missing or invalid → `401`.
- **`__mint.<service>.<name>__`** goes anywhere a real credential value would sit
  inside a *forwarded header*. mint swaps in the real secret after the request
  leaves the sandbox — the agent never holds the value. It is the resolvable
  form of the operator-facing alias `secret://<service>/<name>`.
- **`MINT_BASE_URL`** — resolve from the environment at call time, never
  hardcode. As of 2026-07-07 mint is the private Fly app `misty-mint` (org
  misty-step): from another org Fly app, `http://misty-mint.internal:4949` over
  6PN; from a laptop, `fly proxy 4949:4949 --app misty-mint` then
  `http://127.0.0.1:4949`; local `mint serve` defaults to `http://127.0.0.1:4949`.
  A stable tailnet URL is pending — keep resolving from the environment.
- If there's no placeholder for the service you need, ask the operator to
  declare the alias and a matching policy rule — never fall back to an inline
  key "just this once."

Non-2xx status codes each mean something specific — read the code, don't retry
blind: `references/errors-and-scope.md`. Operator CLI and the read-only MCP face
(not the agent call path): `references/operator-surfaces.md`.

## Red lines

- Never accept, request, or echo a real credential value. If one reaches your
  context from any source, that is a mint-bypass bug — stop and flag it, don't
  route around mint to "fix" the call.
- Never hardcode `MINT_BASE_URL`. Resolve it from the environment at call time.
- The capability token is sensitive despite being scoped and short-lived. Handle
  it as a secret reference — never log it or paste it into code, commits, or
  reports.

## Verification

In the mint repo, `scripts/mint-probe.sh` is mint's own definition of done: it
proves an agent-shaped caller never sees the real secret, the audit log never
contains it, and a policy-denied call reaches the vendor zero times. Trust that
script and its CI job over this skill's prose if the two ever disagree.
