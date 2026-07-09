# Glass Status Beats

Every roster-dispatched lane publishes four one-line Glass beats in one Glass
session: session start, milestone, blocked, and shipped. Glass is one-way; it
is never a place to wait for operator feedback. If a lane is blocked on the
operator, publish the blocked beat and also use Powder `request_input` with the
same ask.

## Resolution

Set these once per lane:

```sh
export GLASS_URL="${GLASS_URL:-https://sanctum.tail5f5eb4.ts.net:10003}"
export GLASS_AGENT="${GLASS_AGENT:-<lane-agent-id>}"
export GLASS_SESSION_TITLE="${GLASS_SESSION_TITLE:-<work-ref>: <lane title>}"
```

Use `glass publish` when the `glass` binary is on PATH and its environment is
wired to the live Glass store. The current CLI publish path writes through the
local Glass store (`GLASS_DB` or the CLI default), while the HTTP forms below
publish to `${GLASS_URL}`. Glass requires no secret; if a future deployment adds
a token, pass only an environment reference such as `GLASS_TOKEN`.

Reuse the session id from the first publish for later beats. Both CLI and HTTP
publish responses return it at `.post.session_id`:

```sh
export GLASS_SESSION_ID="$(jq -r '.post.session_id' < start-beat-response.json)"
```

Keep `summary` to one line. Put command output, diffs, or long notes in linked
artifacts or separate typed surfaces.

## CLI Beats

Session start, `feedKind: note`:

```sh
glass publish --json \
  --title "session start: <work-ref>" \
  --agent "$GLASS_AGENT" \
  --session-title "$GLASS_SESSION_TITLE" \
  --surfaces-json - <<'JSON'
[
  {
    "kind": "markdown",
    "markdown": "Started <work-ref>: <objective>.",
    "feedKind": "note",
    "summary": "Started <work-ref>: <objective>.",
    "detail": "state-as-of: <ISO-8601>; repo: <repo>; branch: <branch>",
    "evidenceLinks": [
      { "label": "Powder card", "url": "<powder-card-url>" },
      { "label": "lane card", "url": "<lane-card-or-brief-url>" }
    ]
  }
]
JSON
```

Milestone, `feedKind: report`:

```sh
glass publish --json \
  --title "milestone: <work-ref>" \
  --agent "$GLASS_AGENT" \
  --session "$GLASS_SESSION_ID" \
  --surfaces-json - <<'JSON'
[
  {
    "kind": "markdown",
    "markdown": "Milestone <name>: <one-line result>.",
    "feedKind": "report",
    "summary": "Milestone <name>: <one-line result>.",
    "detail": "proof read: <command/path/artifact>; next: <next step>",
    "evidenceLinks": [
      { "label": "evidence", "url": "<artifact-or-command-log-url>" }
    ]
  }
]
JSON
```

Blocked, `feedKind: blocked`:

```sh
glass publish --json \
  --title "blocked: <work-ref>" \
  --agent "$GLASS_AGENT" \
  --session "$GLASS_SESSION_ID" \
  --surfaces-json - <<'JSON'
[
  {
    "kind": "markdown",
    "markdown": "Blocked on operator: <specific ask>.",
    "feedKind": "blocked",
    "summary": "Blocked on operator: <specific ask>.",
    "detail": "Also sent the same ask through Powder request_input; Glass is one-way and has no reply channel.",
    "evidenceLinks": [
      { "label": "Powder request", "url": "<powder-run-or-card-url>" }
    ]
  }
]
JSON
```

Shipped, `feedKind: shipped`:

```sh
glass publish --json \
  --title "shipped: <work-ref>" \
  --agent "$GLASS_AGENT" \
  --session "$GLASS_SESSION_ID" \
  --surfaces-json - <<'JSON'
[
  {
    "kind": "markdown",
    "markdown": "Shipped <work-ref>: <one-line outcome>.",
    "feedKind": "shipped",
    "summary": "Shipped <work-ref>: <one-line outcome>.",
    "detail": "verified: <exact gate/route/artifact>; residual risk: <none-or-one-line>",
    "evidenceLinks": [
      { "label": "diff", "url": "<pr-or-commit-url>" },
      { "label": "verification", "url": "<ci-artifact-or-report-url>" }
    ]
  }
]
JSON
```

## HTTP Fallback

For MCP-compatible consumers, post the same `title`, `agent`,
`session_title` or `session_id`, and `surfaces` payload through `/mcp`:

```sh
curl -fsS -X POST "${GLASS_URL%/}/mcp" \
  -H 'content-type: application/json' \
  --data @- <<'JSON'
{
  "jsonrpc": "2.0",
  "id": "glass-status-beat",
  "method": "tools/call",
  "params": {
    "name": "publish_post",
    "arguments": {
      "agent": "<lane-agent-id>",
      "session_title": "<work-ref>: <lane title>",
      "title": "session start: <work-ref>",
      "surfaces": [
        {
          "kind": "markdown",
          "markdown": "Started <work-ref>: <objective>.",
          "feedKind": "note",
          "summary": "Started <work-ref>: <objective>.",
          "detail": "state-as-of: <ISO-8601>; repo: <repo>; branch: <branch>",
          "evidenceLinks": [
            { "label": "Powder card", "url": "<powder-card-url>" },
            { "label": "lane card", "url": "<lane-card-or-brief-url>" }
          ]
        }
      ]
    }
  }
}
JSON
```

For curl-only environments, post the same shape directly to `/api/posts`:

```sh
curl -fsS -X POST "${GLASS_URL%/}/api/posts" \
  -H 'content-type: application/json' \
  --data @- <<'JSON'
{
  "agent": "<lane-agent-id>",
  "sessionTitle": "<work-ref>: <lane title>",
  "title": "session start: <work-ref>",
  "surfaces": [
    {
      "kind": "markdown",
      "markdown": "Started <work-ref>: <objective>.",
      "feedKind": "note",
      "summary": "Started <work-ref>: <objective>.",
      "detail": "state-as-of: <ISO-8601>; repo: <repo>; branch: <branch>",
      "evidenceLinks": [
        { "label": "Powder card", "url": "<powder-card-url>" },
        { "label": "lane card", "url": "<lane-card-or-brief-url>" }
      ]
    }
  ]
}
JSON
```

For milestone, blocked, and shipped HTTP beats, reuse the returned
`post.session_id` as `session_id`/`sessionId` and swap in the matching title,
`feedKind`, one-line `summary`, `detail`, and `evidenceLinks` from the CLI
beats above.
