#!/usr/bin/env python3
"""generate-agents-page — roster's UI face: one self-contained page from the
live declarations (operator ask 2026-07-06: visualize the agents, their
instructions, skills, etc.).

Koan-compliant: the roster IS declarations, so the UI is a generator, not an
app. Reads agents/*/role.yaml + instructions.md + primitives/models.yaml +
primitives/subagent-pool.yaml, emits an HTML fragment for the house artifact
template (publish via harness-kit's artifact_create.py --html-file). No
server, no JS, <details> for drill-down. Regenerate whenever declarations
change:

    python3 scripts/generate-agents-page.py > /tmp/agents-page.html
"""
import html
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path

try:
    import yaml
except ImportError:
    print("needs pyyaml: pip3 install pyyaml", file=sys.stderr)
    sys.exit(1)

ROOT = Path(__file__).resolve().parent.parent
E = lambda s: html.escape(str(s), quote=True)


def load_models():
    p = ROOT / "primitives" / "models.yaml"
    if not p.exists():
        return {}
    return (yaml.safe_load(p.read_text()) or {}).get("models", {})


def load_pool():
    p = ROOT / "primitives" / "subagent-pool.yaml"
    if not p.exists():
        return []
    return (yaml.safe_load(p.read_text()) or {}).get("pool", [])


def resolve(models, model_id):
    row = models.get(model_id)
    if not row:
        return f"no translation needed: {model_id}"
    return " · ".join(f"{h} → {m}" for h, m in row.items())


def chip(text, cls=""):
    return f'<span class="pill" style="{cls}">{E(text)}</span>'


def fmt_entry(entry):
    return f"{entry.get('model', '?')} (reasoning: {entry.get('reasoning', '?')})"


def agent_card(name, role, instructions, models):
    mp = role.get("model_policy", {})
    preferred = mp.get("preferred", {}) or {}
    fallbacks = mp.get("fallbacks", []) or []
    perms = role.get("permissions", {})
    rights = role.get("subagent_rights", {})
    skills = role.get("skills", []) or []
    mcps = role.get("mcps", []) or []
    mcpc = role.get("mcps_contextual", []) or []
    writes = "write" in str(perms.get("filesystem", ""))
    tone = "border-left:4px solid var(--red)" if writes else "border-left:4px solid var(--teal)"
    skill_rows = "".join(
        f"<tr><td><b>{E(s.get('name'))}</b></td><td>{E(s.get('reason',''))}</td>"
        f"<td class='mono' style='font-size:.7rem;opacity:.6'>{E(Path(s.get('path','')).parent.name)}</td></tr>"
        for s in skills)
    ee = "".join(f"<li>{E(x)}</li>" for x in role.get("evidence_expectations", []) or [])
    rights_txt = " · ".join(k.replace("may_", "").replace("_", " ")
                            for k, v in rights.items() if v) or "none (leaf lane)"
    return f"""
<details class="call" style="{tone};margin:.7rem 0">
<summary style="cursor:pointer">
  <b style="font-size:1.05rem">{E(name)}</b>
  {chip(preferred.get('model', '?'))} {chip('reasoning: ' + str(preferred.get('reasoning', '?')))}
  {chip(str(perms.get('filesystem', '?')))} {chip(f"{len(skills)} skills")}
  <div style="opacity:.8;margin-top:.3rem;font-weight:400">{E(role.get('description', ''))}</div>
</summary>
<h3>Model policy</h3>
<p>preferred <b>{E(fmt_entry(preferred))}</b> — resolves: {E(resolve(models, preferred.get('model', '')))}<br>
fallbacks: {E(', '.join(fmt_entry(f) for f in fallbacks) or '—')}</p>
<h3>Permissions</h3>
<p>{' '.join(chip(f"{k}: {v}") for k, v in perms.items())}</p>
<p><b>May:</b> {E(rights_txt)}</p>
<h3>Skills</h3>
<div class="scroll"><table><thead><tr><th>skill</th><th>why</th><th>home</th></tr></thead>
<tbody>{skill_rows or '<tr><td colspan=3>none</td></tr>'}</tbody></table></div>
<h3>MCP servers</h3>
<p>required: <b>{E(', '.join(mcps) or '—')}</b> · contextual: {E(', '.join(mcpc) or '—')}</p>
<h3>Evidence contract</h3>
<ul>{ee or '<li>—</li>'}</ul>
<h3>Instructions (the identity, verbatim)</h3>
<blockquote style="white-space:pre-wrap;font-size:.88rem">{E(instructions.strip())}</blockquote>
</details>"""


def main():
    models = load_models()
    pool = load_pool()
    sha = subprocess.run(["git", "-C", str(ROOT), "rev-parse", "--short=12", "HEAD"],
                         capture_output=True, text=True).stdout.strip()
    agents = sorted(p for p in (ROOT / "agents").iterdir() if (p / "role.yaml").exists())
    cards = []
    spawners = 0
    for a in agents:
        role = yaml.safe_load((a / "role.yaml").read_text())
        instructions = (a / "instructions.md").read_text()
        cards.append(agent_card(a.name, role, instructions, models))
        if role.get("subagent_rights", {}).get("may_spawn_subagents"):
            spawners += 1
    now = datetime.now(timezone.utc).strftime("%Y-%m-%d %H:%M UTC")
    writers = sum(1 for a in agents
                  if "write" in str(yaml.safe_load((a / "role.yaml").read_text())
                                    .get("permissions", {}).get("filesystem", "")))
    pool_txt = ", ".join(fmt_entry(p) if p.get("reasoning") else p.get("model", "?") for p in pool)
    print(f"""
<p class="lede">{len(agents)} agents declared · {writers} with write access ·
{spawners} may spawn ad hoc subagents · generated {E(now)} from roster @
<code>{E(sha)}</code> — the declarations are the source of truth; this page
is a rendering. Tap an agent to open its full identity.</p>
<p class="lede">Default ad hoc subagent pool (<code>primitives/subagent-pool.yaml</code>):
{E(pool_txt) or '—'}</p>
{''.join(cards)}
<footer>Regenerate: <code>python3 scripts/generate-agents-page.py</code> in the
roster repo, then republish. Identity changes go through role.yaml — never
this page.</footer>""")


if __name__ == "__main__":
    main()
