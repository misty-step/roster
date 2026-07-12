//! The persistent roster UI (roster-928): a served page reading the LIVE
//! roster checkout on every request, no regenerate step. This is a faithful
//! Rust port of `scripts/generate-agents-page.py`'s content and the house
//! shelf template it published through (`primitives/skills/artifact/scripts/
//! artifact_create.py`'s `HOUSE_CSS`/`HOUSE_JS`/`HEADER`/`wrap`) -- the
//! visual design already shipped and was approved as the "koan-minimal v1"
//! (card body, roster-928); this only changes WHEN it renders, from
//! generate-then-publish to live-per-request, so it never needs a fresh
//! design pass.

use roster_core::{Agent, Models, Permissions, Role, Roster, SubagentPool, SubagentRights};
use std::path::Path;

/// Verbatim from `artifact_create.py`'s `HOUSE_CSS` -- the shelf's one
/// house stylesheet, so this page matches every other published artifact.
const HOUSE_CSS: &str = r#"
img{cursor:zoom-in}

  :root{--bg:#f5f0e4;--panel:#fbf7ed;--ink:#241f19;--muted:#6b6053;--line:#e2d9c6;--red:#b4392a;--teal:#2c7a68;--gold:#bd8f2e;--shadow:0 1px 0 rgba(0,0,0,.04),0 12px 30px -22px rgba(40,30,20,.5);--col:52rem;}
  html[data-theme="dark"]{--bg:#11141a;--panel:#171b22;--ink:#e9e3d5;--muted:#9aa0ab;--line:#282e38;--red:#e3674f;--teal:#54bba4;--gold:#d8b25a;--shadow:0 1px 0 rgba(0,0,0,.3),0 16px 40px -26px rgba(0,0,0,.9);}
  @media (prefers-color-scheme:dark){html[data-theme="auto"]{--bg:#11141a;--panel:#171b22;--ink:#e9e3d5;--muted:#9aa0ab;--line:#282e38;--red:#e3674f;--teal:#54bba4;--gold:#d8b25a;--shadow:0 1px 0 rgba(0,0,0,.3),0 16px 40px -26px rgba(0,0,0,.9);}}
  *{box-sizing:border-box} html{scroll-behavior:smooth}
  body{margin:0;background:var(--bg);color:var(--ink);font:16px/1.62 -apple-system,BlinkMacSystemFont,"Segoe UI",Roboto,Helvetica,Arial,sans-serif;-webkit-font-smoothing:antialiased;padding:0 1.1rem 5rem;background-image:radial-gradient(circle at 12% -5%,rgba(189,143,46,.07),transparent 36%),radial-gradient(circle at 92% 4%,rgba(44,122,104,.07),transparent 34%);}
  .wrap{max-width:var(--col);margin:0 auto}
  .mono{font-family:ui-monospace,SFMono-Regular,Menlo,Consolas,monospace}
  a{color:var(--red);text-underline-offset:2px;overflow-wrap:anywhere}
  h1,h2,h3{line-height:1.2;font-weight:680}
  h1{font-family:Georgia,"Times New Roman",serif;font-weight:600;font-size:clamp(1.85rem,5.2vw,2.6rem);letter-spacing:-.01em;margin:.2em 0 .15em}
  h2{font-size:1.32rem;margin:2.3em 0 .3em} h2 .k{font:600 .72rem/1 ui-monospace,monospace;color:var(--red);vertical-align:.5em;margin-right:.55em}
  h3{font-size:1.04rem;margin:1.3em 0 .25em}
  p{margin:.55em 0} small{color:var(--muted)}
  header.bar{position:sticky;top:0;z-index:20;background:color-mix(in srgb,var(--bg) 86%,transparent);backdrop-filter:blur(8px);border-bottom:1px solid var(--line);margin:0 -1.1rem 1.6rem;padding:.55rem 1.1rem}
  .bar .row{max-width:var(--col);margin:0 auto;display:flex;align-items:center;gap:.6rem}
  .tag{font:600 .68rem/1 ui-monospace,monospace;letter-spacing:.08em;text-transform:uppercase;color:var(--muted)}
  .dot{width:.5rem;height:.5rem;border-radius:50%;background:var(--teal);animation:pulse 2.6s infinite}
  @keyframes pulse{0%{box-shadow:0 0 0 0 color-mix(in srgb,var(--teal) 60%,transparent)}70%{box-shadow:0 0 0 7px transparent}100%{box-shadow:0 0 0 0 transparent}}
  .spacer{flex:1}
  .toggle{cursor:pointer;border:1px solid var(--line);background:var(--panel);color:var(--ink);font:600 .72rem/1 ui-monospace,monospace;padding:.42rem .7rem;border-radius:2em;display:inline-flex;gap:.4rem;align-items:center}
  .toggle:hover{border-color:var(--red)}
  .sanctum-home{position:fixed;left:max(.85rem,env(safe-area-inset-left));bottom:max(.85rem,env(safe-area-inset-bottom));z-index:19;width:38px;height:38px;display:inline-flex;align-items:center;justify-content:center;border:1px solid var(--line);border-radius:999px;background:color-mix(in srgb,var(--panel) 88%,transparent);color:var(--ink);text-decoration:none;box-shadow:var(--shadow);backdrop-filter:blur(10px);-webkit-backdrop-filter:blur(10px)}
  .sanctum-home svg{width:17px;height:17px;fill:none;stroke:currentColor;stroke-width:1.7;stroke-linecap:round;stroke-linejoin:round}
  .sanctum-home:focus-visible{outline:2px solid var(--ink);outline-offset:3px}
  .sanctum-home:hover{border-color:var(--red);color:var(--red)}
  .label{font:600 .72rem/1 ui-monospace,monospace;letter-spacing:.14em;text-transform:uppercase;color:var(--red)}
  .lede{font-size:1.12rem;color:var(--muted);margin:.4em 0 1em}
  .pill{font:600 .68rem/1.4 ui-monospace,monospace;border:1px solid var(--line);border-radius:2em;padding:.28em .7em;color:var(--muted);background:var(--panel)}
  .call{border:1px solid var(--line);border-left:4px solid var(--teal);background:color-mix(in srgb,var(--teal) 7%,var(--panel));border-radius:.6rem;padding:.8rem 1.05rem;margin:1.1rem 0}
  .call.red{border-left-color:var(--red);background:color-mix(in srgb,var(--red) 7%,var(--panel))}
  .call.gold{border-left-color:var(--gold);background:color-mix(in srgb,var(--gold) 8%,var(--panel))}
  .scroll{overflow-x:auto;-webkit-overflow-scrolling:touch;margin:1rem 0;border:1px solid var(--line);border-radius:.6rem}
  table{border-collapse:collapse;width:100%;font-size:.88rem;background:var(--panel)}
  th,td{text-align:left;padding:.55rem .65rem;border-bottom:1px solid var(--line);vertical-align:top}
  th{font:600 .66rem/1.3 ui-monospace,monospace;letter-spacing:.04em;text-transform:uppercase;color:var(--muted);background:color-mix(in srgb,var(--panel) 70%,var(--bg))}
  tr:last-child td{border-bottom:0}
  code{font-family:ui-monospace,monospace;font-size:.85em;background:color-mix(in srgb,var(--ink) 8%,transparent);padding:.1em .35em;border-radius:.3em;overflow-wrap:anywhere}
  pre{background:var(--panel);border:1px solid var(--line);border-radius:.6rem;padding:.9rem;overflow-x:auto;font:600 .78rem/1.5 ui-monospace,monospace;color:var(--ink)}
  blockquote{border-left:3px solid var(--gold);margin:.8em 0;padding:.2em 0 .2em 1rem;color:var(--muted)}
  footer{margin-top:2.6rem;border-top:1px solid var(--line);padding-top:1rem;color:var(--muted);font-size:.85rem}
  @media (prefers-reduced-motion:reduce){.dot{animation:none} html{scroll-behavior:auto}}
"#;

/// Verbatim from `artifact_create.py`'s `HOUSE_JS` -- theme toggle + copy-page.
const HOUSE_JS: &str = r##"
(function(){
  var root=document.documentElement,k="hk-artifact-theme";
  var order=["auto","light","dark"],icons={auto:"◐",light:"☀",dark:"☾"},names={auto:"System",light:"Light",dark:"Dark"};
  var saved=null;try{saved=localStorage.getItem(k)}catch(e){}
  var cur=order.indexOf(saved)>=0?saved:"auto";
  function apply(){root.setAttribute("data-theme",cur);var i=document.getElementById("tgicon"),t=document.getElementById("tgtxt");if(i)i.textContent=icons[cur];if(t)t.textContent=names[cur];}
  apply();
  var tg=document.getElementById("tg");if(tg)tg.addEventListener("click",function(){cur=order[(order.indexOf(cur)+1)%order.length];try{localStorage.setItem(k,cur)}catch(e){}apply();});
  var cp=document.getElementById("cp"),cpt=document.getElementById("cptxt");
  document.addEventListener("click",function(e){
    var t=e.target;
    if(t.tagName==="IMG"&&!t.closest("a")&&!t.closest("#hk-lb")){
      var o=document.createElement("div");o.id="hk-lb";
      o.style.cssText="position:fixed;inset:0;background:rgba(0,0,0,.85);display:flex;align-items:center;justify-content:center;z-index:999;cursor:zoom-out;padding:2vh 2vw";
      var im=document.createElement("img");im.src=t.src;
      im.style.cssText="max-width:96vw;max-height:96vh;width:auto;height:auto;box-shadow:0 8px 40px rgba(0,0,0,.6)";
      o.appendChild(im);o.addEventListener("click",function(){o.remove()});
      document.addEventListener("keydown",function esc(ev){if(ev.key==="Escape"){o.remove();document.removeEventListener("keydown",esc)}});
      document.body.appendChild(o);
    }
  });
  if(cp)cp.addEventListener("click",function(){var doc="<!doctype html>\n"+document.documentElement.outerHTML;
    function ok(){if(cpt){cpt.textContent="Copied ✓";setTimeout(function(){cpt.textContent="Copy page"},1600);}}
    function fallback(){var ta=document.createElement("textarea");ta.value=doc;ta.style.position="fixed";ta.style.opacity="0";document.body.appendChild(ta);ta.focus();ta.select();try{document.execCommand("copy");ok()}catch(e){if(cpt)cpt.textContent="Copy failed"}document.body.removeChild(ta);}
    if(navigator.clipboard&&navigator.clipboard.writeText){navigator.clipboard.writeText(doc).then(ok,fallback)}else{fallback()}});
})();
"##;

const HOME_SNIPPET: &str = r#"<a data-sanctum-home class="sanctum-home" href="/" aria-label="Back to Sanctum" title="Back to Sanctum"><svg viewBox="0 0 24 24" aria-hidden="true"><path d="M15 21v-8a1 1 0 0 0-1-1h-4a1 1 0 0 0-1 1v8"/><path d="M3 10a2 2 0 0 1 .709-1.528l7-6a2 2 0 0 1 2.582 0l7 6A2 2 0 0 1 21 10v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/></svg></a>"#;

fn escape(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            other => out.push(other),
        }
    }
    out
}

fn chip(text: &str) -> String {
    format!(r#"<span class="pill">{}</span>"#, escape(text))
}

fn fmt_entry(model: &str, reasoning: &str) -> String {
    format!("{model} (reasoning: {reasoning})")
}

fn resolve(models: &Models, model_id: &str) -> String {
    match models.models.get(model_id) {
        Some(binding) => format!("claude → {} · bb → {}", binding.claude, binding.bb),
        None => format!("no translation needed: {model_id}"),
    }
}

fn rights_text(rights: &SubagentRights) -> String {
    let mut parts = Vec::new();
    if rights.may_dispatch {
        parts.push("dispatch");
    }
    if rights.may_spawn_subagents {
        parts.push("spawn subagents");
    }
    if rights.may_use_peer_harnesses {
        parts.push("use peer harnesses");
    }
    if parts.is_empty() {
        "none (leaf lane)".to_string()
    } else {
        parts.join(" · ")
    }
}

fn permission_chips(perms: &Permissions) -> String {
    [
        ("filesystem", &perms.filesystem),
        ("commands", &perms.commands),
        ("network", &perms.network),
        ("secrets", &perms.secrets),
        ("mutations", &perms.mutations),
    ]
    .iter()
    .map(|(key, value)| chip(&format!("{key}: {value}")))
    .collect::<Vec<_>>()
    .join(" ")
}

fn skill_path_home(path: &str) -> String {
    Path::new(path)
        .parent()
        .and_then(|parent| parent.file_name())
        .and_then(|name| name.to_str())
        .unwrap_or("")
        .to_string()
}

fn agent_card(role: &Role, instructions: &str, models: &Models) -> String {
    let preferred = &role.model_policy.preferred;
    let writes = role.permissions.filesystem.contains("write");
    let tone = if writes {
        "border-left:4px solid var(--red)"
    } else {
        "border-left:4px solid var(--teal)"
    };

    let skill_rows = role
        .skills
        .iter()
        .map(|skill| {
            format!(
                "<tr><td><b>{}</b></td><td>{}</td><td class='mono' style='font-size:.7rem;opacity:.6'>{}</td></tr>",
                escape(&skill.name),
                escape(&skill.reason),
                escape(&skill_path_home(&skill.path)),
            )
        })
        .collect::<String>();
    let skill_rows = if skill_rows.is_empty() {
        "<tr><td colspan=3>none</td></tr>".to_string()
    } else {
        skill_rows
    };

    let evidence = role
        .evidence_expectations
        .iter()
        .map(|item| format!("<li>{}</li>", escape(item)))
        .collect::<String>();
    let evidence = if evidence.is_empty() {
        "<li>—</li>".to_string()
    } else {
        evidence
    };

    let fallbacks = if role.model_policy.fallbacks.is_empty() {
        "—".to_string()
    } else {
        role.model_policy
            .fallbacks
            .iter()
            .map(|entry| fmt_entry(&entry.model, &entry.reasoning))
            .collect::<Vec<_>>()
            .join(", ")
    };

    let mcps = if role.mcps.is_empty() {
        "—".to_string()
    } else {
        role.mcps.join(", ")
    };
    let mcps_contextual = if role.mcps_contextual.is_empty() {
        "—".to_string()
    } else {
        role.mcps_contextual.join(", ")
    };

    format!(
        r#"<details class="call" style="{tone};margin:.7rem 0">
<summary style="cursor:pointer">
  <b style="font-size:1.05rem">{name}</b>
  {model_chip} {reasoning_chip}
  {fs_chip} {skill_count_chip}
  <div style="opacity:.8;margin-top:.3rem;font-weight:400">{description}</div>
</summary>
<h3>Model policy</h3>
<p>preferred <b>{preferred_entry}</b> — resolves: {resolved}<br>
fallbacks: {fallbacks}</p>
<h3>Permissions</h3>
<p>{permission_chips}</p>
<p><b>May:</b> {rights}</p>
<h3>Skills</h3>
<div class="scroll"><table><thead><tr><th>skill</th><th>why</th><th>home</th></tr></thead>
<tbody>{skill_rows}</tbody></table></div>
<h3>MCP servers</h3>
<p>required: <b>{mcps}</b> · contextual: {mcps_contextual}</p>
<h3>Evidence contract</h3>
<ul>{evidence}</ul>
<h3>Instructions (the identity, verbatim)</h3>
<blockquote style="white-space:pre-wrap;font-size:.88rem">{instructions}</blockquote>
</details>"#,
        tone = tone,
        name = escape(&role.name),
        model_chip = chip(&preferred.model),
        reasoning_chip = chip(&format!("reasoning: {}", preferred.reasoning)),
        fs_chip = chip(&role.permissions.filesystem),
        skill_count_chip = chip(&format!("{} skills", role.skills.len())),
        description = escape(&role.description),
        preferred_entry = escape(&fmt_entry(&preferred.model, &preferred.reasoning)),
        resolved = escape(&resolve(models, &preferred.model)),
        fallbacks = escape(&fallbacks),
        permission_chips = permission_chips(&role.permissions),
        rights = escape(&rights_text(&role.subagent_rights)),
        skill_rows = skill_rows,
        mcps = escape(&mcps),
        mcps_contextual = escape(&mcps_contextual),
        evidence = evidence,
        instructions = escape(instructions.trim()),
    )
}

fn pool_text(pool: &SubagentPool) -> String {
    if pool.pool.is_empty() {
        return "—".to_string();
    }
    pool.pool
        .iter()
        .map(|entry| match &entry.reasoning {
            Some(reasoning) => fmt_entry(&entry.model, reasoning),
            None => entry.model.clone(),
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// Best-effort `git rev-parse --short=12 HEAD` against the live checkout,
/// re-run on every request (not cached at boot) so it never drifts from
/// what a `role.yaml` edit on disk actually reflects. Failure is non-fatal
/// -- an empty sha just means the attribution line omits it, matching this
/// codebase's established "absent state degrades quietly" convention (see
/// Sanctum's `portal.rs` registry/links loaders).
fn git_sha(root: &Path) -> String {
    std::process::Command::new("git")
        .args([
            "-C",
            &root.to_string_lossy(),
            "rev-parse",
            "--short=12",
            "HEAD",
        ])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|sha| sha.trim().to_string())
        .unwrap_or_default()
}

/// Render the full live page: house chrome (verbatim shelf template) wrapping
/// one card per declared agent, read straight off disk this request.
pub fn render_agents_page(
    root: &Path,
    roster: &Roster,
    models: &Models,
    pool: &SubagentPool,
) -> String {
    let agents: Vec<&Agent> = roster.agents().iter().collect();
    let writers = agents
        .iter()
        .filter(|agent| agent.role.permissions.filesystem.contains("write"))
        .count();
    let spawners = agents
        .iter()
        .filter(|agent| agent.role.subagent_rights.may_spawn_subagents)
        .count();
    let sha = git_sha(root);
    let sha_clause = if sha.is_empty() {
        String::new()
    } else {
        format!(" from roster @ <code>{}</code>", escape(&sha))
    };

    let cards = agents
        .iter()
        .map(|agent| agent_card(&agent.role, &agent.instructions, models))
        .collect::<String>();

    let body = format!(
        r#"<p class="lede">{count} agents declared · {writers} with write access ·
{spawners} may spawn ad hoc subagents{sha_clause} — read live off disk on every request; the
declarations are the source of truth, this page only renders them. Tap an agent to open its
full identity.</p>
<p class="lede">Default ad hoc subagent pool (<code>primitives/subagent-pool.yaml</code>): {pool}</p>
{cards}
<footer>Read-only. Identity changes go through <code>role.yaml</code> +
<code>instructions.md</code> in git — never this page.</footer>"#,
        count = agents.len(),
        writers = writers,
        spawners = spawners,
        sha_clause = sha_clause,
        pool = escape(&pool_text(pool)),
        cards = cards,
    );

    format!(
        r#"<!doctype html>
<html lang="en" data-theme="auto">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Roster — Agents</title>
<style>{house_css}</style>
</head>
<body>
<header class="bar"><div class="row">
<span class="dot" aria-hidden="true"></span>
<span class="tag">ROSTER</span><span class="spacer"></span>
<button class="toggle" id="cp" aria-label="Copy page HTML"><span id="cpicon">⧉</span><span id="cptxt">Copy page</span></button>
<button class="toggle" id="tg" aria-label="Toggle theme"><span id="tgicon">◐</span><span id="tgtxt">System</span></button>
</div></header>
{home_snippet}
<div class="wrap">
<h1>Roster — Agents</h1>
<p class="lede">Every declared agent, identity, model policy, permissions, skills, and subagent pool — read live, no regenerate step.</p>
{body}
</div>
<script>{house_js}</script>
</body>
</html>"#,
        house_css = HOUSE_CSS,
        home_snippet = HOME_SNIPPET,
        body = body,
        house_js = HOUSE_JS,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn workspace_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|path| path.parent())
            .expect("workspace root")
            .to_path_buf()
    }

    #[test]
    fn renders_every_declared_agent_with_identity_model_permissions_skills_and_pool() {
        let root = workspace_root();
        let roster = Roster::load(&root).expect("load roster");
        let models = Models::load(&root).expect("load models");
        let pool = SubagentPool::load(&root).expect("load pool");
        let page = render_agents_page(&root, &roster, &models, &pool);

        assert!(page.contains("<meta name=\"viewport\""));
        assert!(page.contains("Roster — Agents"));
        for agent in roster.agents() {
            assert!(
                page.contains(&escape(&agent.role.name)),
                "missing agent {}",
                agent.role.name
            );
            assert!(page.contains(&escape(&agent.role.model_policy.preferred.model)));
        }
        assert!(page.contains("Permissions"));
        assert!(page.contains("Skills"));
        assert!(page.contains("subagent-pool.yaml"));
        assert!(page.contains(&format!("{} agents declared", roster.agents().len())));
    }

    #[test]
    fn escapes_html_in_declaration_content() {
        assert_eq!(escape("<script>&\"'"), "&lt;script&gt;&amp;&quot;&#39;");
    }

    #[test]
    fn rights_text_lists_only_granted_permissions_or_leaf_lane() {
        let none = SubagentRights {
            may_dispatch: false,
            may_spawn_subagents: false,
            may_use_peer_harnesses: false,
        };
        assert_eq!(rights_text(&none), "none (leaf lane)");

        let some = SubagentRights {
            may_dispatch: true,
            may_spawn_subagents: false,
            may_use_peer_harnesses: true,
        };
        assert_eq!(rights_text(&some), "dispatch · use peer harnesses");
    }

    #[test]
    fn resolve_reports_no_translation_needed_for_unknown_model() {
        let models = Models {
            schema_version: "test".to_string(),
            models: Default::default(),
        };
        assert_eq!(
            resolve(&models, "some-model"),
            "no translation needed: some-model"
        );
    }

    #[test]
    fn skill_path_home_extracts_parent_directory_name() {
        assert_eq!(
            skill_path_home("primitives/skills/deliver/SKILL.md"),
            "deliver"
        );
        assert_eq!(skill_path_home(""), "");
    }

    #[test]
    fn pool_text_falls_back_to_model_only_without_reasoning() {
        use roster_core::PoolEntry;
        let pool = SubagentPool {
            schema_version: "test".to_string(),
            pool: vec![PoolEntry {
                model: "gpt-5.6-luna".to_string(),
                reasoning: None,
            }],
        };
        assert_eq!(pool_text(&pool), "gpt-5.6-luna");
    }
}
