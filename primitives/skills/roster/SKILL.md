---
name: roster
description: |
  Discover and dispatch the agent fleet. Any Roster agent may commission any
  other agent; there is no declared delegates list. Use when: "delegate this",
  "dispatch an agent", "who can do this", "hand this to a builder/reviewer/
  verifier", "launch a focused agent", or any time work should run in a fresh,
  independently composed context instead of this one.
  Trigger: /roster, /dispatch.
argument-hint: "[agent]"
---

# /roster

Every agent in the fleet can commission every other agent. Who exists, what
they carry, and how to launch them is discovered live from the CLI — never
from memory or a hardcoded list.

## See the fleet

```sh
roster list            # launchable agent definitions in the effective config
roster show <agent>    # one agent's exact resolved composition (role, model,
                       # harness, guidance, skills, MCPs)
```

Pick by role fit, not by name familiarity. An agent's role description is the
contract for what it will and won't do.

## Dispatch

```sh
roster dispatch <agent>
```

This resolves the agent's full graph into a temporary, isolated bundle,
launches its harness in the current workspace, and removes the projection
after exit. The dispatched agent gets its own role's skills and MCPs — not
yours.

Commission outcome-shaped lanes: give the agent a role-appropriate objective,
exact scope, an oracle it can run, the output shape you need back, and hard
boundaries. Give fresh critics (reviewer, verifier) only the artifact and the
oracle — never the author's reasoning trail.

## Ground rules

- One card, one lane: a dispatched agent works one commissioned outcome.
- Don't chain dispatches to simulate a pipeline the operator didn't ask for;
  each hop loses context and costs attention.
- If the right agent doesn't exist, record an agent-creation gap for the
  operator — do not improvise a role.
