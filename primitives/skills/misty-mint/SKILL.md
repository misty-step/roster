---
name: mint
description: |
  Use when an agent needs to make an outbound HTTP call to a vendor API
  (OpenAI, GitHub, Stripe, or any third-party service) and would otherwise
  need an API key, token, or other secret. mint is the fleet's agent
  credential broker: route the call through mint's egress proxy with a credential
  placeholder — your tailnet identity IS the auth; never request, embed, or
  expect real credential bytes in agent context. Trigger phrases: "mint",
  "credential broker", "egress proxy", "call the API through mint", "I need
  an API key", "OpenRouter key", "secret placeholder", "__mint.",
  "proxy call".
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

You hold NOTHING — no token, no key, no auth header for mint itself
(mint-924: identity is your machine's tailnet peer address, resolved
server-side via `tailscale whois`; anything you put in a header cannot
change who mint thinks you are). Send the exact request you would have sent
to the vendor to mint's proxy route, with the vendor scheme/host/path folded
into the URL and a placeholder where the credential would sit:

```
{METHOD} {MINT_BASE_URL}/proxy/{scheme}/{host}/{*rest}
<any forwarded header that would carry a real credential>: ...__mint.<service>.<name>__...
```

Concrete — calling OpenRouter through the `openrouter.default` alias:

```sh
curl -H "Authorization: Bearer __mint.openrouter.default__" \
     -X POST "${MINT_BASE_URL:-http://mint.tail5f5eb4.ts.net:4949}/proxy/https/openrouter.ai/api/v1/chat/completions" \
     -d '{"model": "...", "messages": [...]}'
```

- **`MINT_BASE_URL`** — prefer the environment; the deployed default is
  `http://mint.tail5f5eb4.ts.net:4949` (dedicated tailnet droplet since
  2026-07-08, do-migration-105; plain HTTP is correct — WireGuard is the
  transport encryption). Local `mint serve` uses `http://127.0.0.1:4949`.
  You must be a tailnet peer: calls from off-tailnet (or through `tailscale
  serve`/443, which launders the peer address) are refused by design.
- **`__mint.<service>.<name>__`** goes anywhere a real credential value would
  sit inside a *forwarded header*. mint swaps in the real secret after the
  request leaves your sandbox — you never hold the value. It is the
  resolvable form of the operator-facing alias `secret://<service>/<name>`.
- **Live aliases** (policy-gated per caller identity; deploy/policy.yaml in
  the mint repo is the source of truth): `openrouter.default`
  (openrouter.ai), `powder.default` (Powder on the bastion box),
  `canary.default` (canary-obs). Fleet `tag:server` boxes and the operator's
  own devices (`phrazzld@github`) hold rules today.
- **`X-Mint-Capability`** is dev/loopback-only since mint-924 (local `mint
  serve` smoke) — it is not the deployed agent path and mint refuses it from
  anywhere but 127.0.0.1.
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
- Prefer `MINT_BASE_URL` from the environment; the tailnet default above is
  the documented fallback, not something to bake into committed code.
- The dev-only capability token (loopback `mint serve`) is still sensitive.
  Handle it as a secret reference — never log it or paste it into code,
  commits, or reports.

## Verification

In the mint repo, `scripts/mint-probe.sh` is mint's own definition of done: it
proves an agent-shaped caller never sees the real secret, the audit log never
contains it, and a policy-denied call reaches the vendor zero times. Trust that
script and its CI job over this skill's prose if the two ever disagree.
