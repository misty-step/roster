use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug)]
pub struct Roster {
    agents: Vec<Agent>,
    mcp_registry: McpRegistry,
}

impl Roster {
    pub fn load(root: impl AsRef<Path>) -> Result<Self, RosterError> {
        let root = root.as_ref();
        let agents_dir = root.join("agents");
        let entries = fs::read_dir(&agents_dir).map_err(|source| RosterError::Io {
            path: agents_dir.clone(),
            source,
        })?;

        let mut agents = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|source| RosterError::Io {
                path: agents_dir.clone(),
                source,
            })?;
            let file_type = entry.file_type().map_err(|source| RosterError::Io {
                path: entry.path(),
                source,
            })?;
            if file_type.is_dir() {
                agents.push(load_agent(entry.path())?);
            }
        }

        agents.sort_by(|left, right| left.role.name.cmp(&right.role.name));
        validate_unique_names(&agents)?;
        let mcp_registry = McpRegistry::load(root)?;
        validate_mcp_references(&agents, &mcp_registry)?;

        Ok(Self {
            agents,
            mcp_registry,
        })
    }

    pub fn agents(&self) -> &[Agent] {
        &self.agents
    }

    pub fn agent(&self, name: &str) -> Option<&Agent> {
        self.agents.iter().find(|agent| agent.role.name == name)
    }

    pub fn mcps(&self) -> &[Mcp] {
        &self.mcp_registry.mcps
    }

    pub fn mcp(&self, id: &str) -> Option<&Mcp> {
        self.mcps().iter().find(|mcp| mcp.id == id)
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct McpRegistry {
    pub schema_version: String,
    pub provenance: String,
    pub mcps: Vec<Mcp>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Mcp {
    pub id: String,
    pub status: String,
    #[serde(default)]
    pub transport: Option<String>,
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub env_refs: Vec<String>,
    #[serde(default)]
    pub app: Option<String>,
    #[serde(default)]
    pub source_repo: Option<String>,
    #[serde(default)]
    pub source_ref: Option<String>,
    #[serde(default)]
    pub product_skill: Option<String>,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
}

impl McpRegistry {
    pub fn load(root: impl AsRef<Path>) -> Result<Self, RosterError> {
        let path = root.as_ref().join("primitives/mcps/registry.yaml");
        let text = fs::read_to_string(&path).map_err(|source| RosterError::Io {
            path: path.clone(),
            source,
        })?;
        let registry: Self = serde_yaml::from_str(&text).map_err(|source| RosterError::Yaml {
            path: path.clone(),
            source,
        })?;

        if registry.schema_version != "roster.mcp_registry.v1" {
            return Err(RosterError::Validation(format!(
                "{} has unsupported schema_version {:?}",
                path.display(),
                registry.schema_version
            )));
        }
        require_non_empty(&registry.provenance, "provenance", &path)?;

        let mut ids = BTreeSet::new();
        for mcp in &registry.mcps {
            require_non_empty(&mcp.id, "mcps[].id", &path)?;
            require_non_empty(&mcp.status, "mcps[].status", &path)?;
            require_one_of(
                &mcp.status,
                "mcps[].status",
                &["available", "external", "disabled", "not_applicable"],
                &path,
            )?;
            if !ids.insert(&mcp.id) {
                return Err(RosterError::Validation(format!(
                    "{} has duplicate MCP id {:?}",
                    path.display(),
                    mcp.id
                )));
            }
            if let Some(reason) = &mcp.reason {
                require_non_empty(reason, "mcps[].reason", &path)?;
            }
            for env_ref in &mcp.env_refs {
                require_non_empty(env_ref, "mcps[].env_refs[]", &path)?;
            }
            match mcp.status.as_str() {
                "available" => match mcp.transport.as_deref() {
                    Some("stdio") => require_non_empty(
                        mcp.command.as_deref().unwrap_or_default(),
                        "mcps[].command",
                        &path,
                    )?,
                    Some("http") => require_non_empty(
                        mcp.url.as_deref().unwrap_or_default(),
                        "mcps[].url",
                        &path,
                    )?,
                    transport => {
                        return Err(RosterError::Validation(format!(
                            "{} MCP {:?} is available but has unsupported transport {:?}",
                            path.display(),
                            mcp.id,
                            transport
                        )));
                    }
                },
                "disabled" | "not_applicable" => require_non_empty(
                    mcp.reason.as_deref().unwrap_or_default(),
                    "mcps[].reason",
                    &path,
                )?,
                "external" => {}
                _ => unreachable!("status was validated above"),
            }
        }

        Ok(registry)
    }
}

/// Concrete model id -> per-harness invocable token, loaded from
/// `primitives/models.yaml`. Distinct from the pre-existing
/// `primitives/providers.yaml` (a peer-harness-CLI dispatch table migrated
/// from harness-kit's agents.yaml at P0 -- how to invoke codex/claude/pi/etc,
/// not consulted by this struct): two files, two concepts. Also distinct
/// from the retired `primitives/tiers.yaml`, which resolved an ABSTRACT tier
/// symbol (`fable-class`, `codex-class`, `openrouter-class`) per harness --
/// model policy v2 (roster-924) made `model_policy.preferred` always a
/// concrete, invocable id, so this table only translates that id into the
/// token a specific harness renderer needs, never a tier.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Models {
    pub schema_version: String,
    pub models: BTreeMap<String, ModelBinding>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ModelBinding {
    pub claude: String,
    pub bb: String,
}

impl Models {
    pub fn load(root: impl AsRef<Path>) -> Result<Self, RosterError> {
        let path = root.as_ref().join("primitives/models.yaml");
        let text = fs::read_to_string(&path).map_err(|source| RosterError::Io {
            path: path.clone(),
            source,
        })?;
        serde_yaml::from_str(&text).map_err(|source| RosterError::Yaml { path, source })
    }

    fn claude_for(&self, model: &str) -> Option<&str> {
        self.models
            .get(model)
            .map(|binding| binding.claude.as_str())
    }

    fn bb_for(&self, model: &str) -> Option<&str> {
        self.models.get(model).map(|binding| binding.bb.as_str())
    }
}

/// Default ad hoc subagent pool, loaded from `primitives/subagent-pool.yaml`.
/// Declared once; every agent with `subagent_rights.may_spawn_subagents`
/// true points at this file rather than each instructions.md re-listing the
/// pool. Not consumed by any renderer today -- validated here so a typo in
/// the pool file fails a test, not a silent drift from the instructions.md
/// line that names it.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SubagentPool {
    pub schema_version: String,
    pub pool: Vec<PoolEntry>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PoolEntry {
    pub model: String,
    #[serde(default)]
    pub reasoning: Option<String>,
}

impl SubagentPool {
    pub fn load(root: impl AsRef<Path>) -> Result<Self, RosterError> {
        let path = root.as_ref().join("primitives/subagent-pool.yaml");
        let text = fs::read_to_string(&path).map_err(|source| RosterError::Io {
            path: path.clone(),
            source,
        })?;
        serde_yaml::from_str(&text).map_err(|source| RosterError::Yaml { path, source })
    }
}

#[derive(Debug)]
pub struct Agent {
    pub directory: PathBuf,
    pub role: Role,
    pub instructions: String,
}

impl Agent {
    pub fn instruction_path(&self) -> PathBuf {
        self.directory.join("instructions.md")
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Role {
    pub schema_version: String,
    pub name: String,
    pub description: String,
    pub model_policy: ModelPolicy,
    pub permissions: Permissions,
    pub skills: Vec<SkillRef>,
    pub mcps: Vec<String>,
    #[serde(default)]
    pub mcps_contextual: Vec<String>,
    pub subagent_rights: SubagentRights,
    pub evidence_expectations: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ModelPolicy {
    pub preferred: ModelEntry,
    pub fallbacks: Vec<ModelEntry>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct ModelEntry {
    pub model: String,
    pub reasoning: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Permissions {
    pub filesystem: String,
    pub commands: String,
    pub network: String,
    pub secrets: String,
    pub mutations: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SkillRef {
    pub name: String,
    pub path: String,
    pub reason: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SubagentRights {
    pub may_dispatch: bool,
    pub may_spawn_subagents: bool,
    pub may_use_peer_harnesses: bool,
}

#[derive(Debug)]
pub struct PowderCardSnapshot {
    pub id: String,
    pub title: String,
    pub body: String,
    pub acceptance: Vec<String>,
    pub status: String,
    pub updated_at: i64,
    pub fetched_at: i64,
    pub claim: Option<PowderClaim>,
}

#[derive(Debug)]
pub struct PowderClaim {
    pub agent: String,
    pub run_id: String,
    pub expires_at: i64,
}

#[derive(Debug, thiserror::Error)]
pub enum RosterError {
    #[error("failed to read {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse {path}: {source}")]
    Yaml {
        path: PathBuf,
        #[source]
        source: serde_yaml::Error,
    },
    #[error("{0}")]
    Validation(String),
}

pub fn render_claude_agent(agent: &Agent, models: &Models) -> Result<String, String> {
    let role = &agent.role;
    let model = claude_model(role, models);
    let tools = claude_tools(role)?;

    Ok(format!(
        r#"---
name: {name}
description: {description}
model: {model}
tools: {tools}
---

# {name}

{instructions}

## Model Policy

- Preferred: {preferred}
- Fallbacks: {fallbacks}

## Skills To Read

{skills}

## MCP Servers

{mcp_servers}

## Permissions

- Filesystem: {filesystem}
- Commands: {commands}
- Network: {network}
- Secrets: {secrets}
- Mutations: {mutations}

## Evidence Expectations

{evidence}
"#,
        name = role.name,
        description = role.description,
        model = model,
        tools = tools,
        preferred = format_model_entry(&role.model_policy.preferred),
        skills = render_skills(&role.skills, &[]),
        instructions = agent.instructions.trim(),
        fallbacks = format_fallbacks(&role.model_policy.fallbacks),
        mcp_servers = render_mcp_servers(&role.mcps, &role.mcps_contextual, &[]),
        filesystem = role.permissions.filesystem,
        commands = role.permissions.commands,
        network = role.permissions.network,
        secrets = role.permissions.secrets,
        mutations = role.permissions.mutations,
        evidence = bullet_list(&role.evidence_expectations),
    ))
}

/// Codex custom agents are native config layers referenced from
/// `[agents.<name>]` in `$CODEX_HOME/config.toml`. The prompt-native brief is
/// carried as additive developer instructions; `roster brief` remains the
/// standalone dispatch-packet surface.
pub fn render_codex_agent(agent: &Agent) -> String {
    let role = &agent.role;
    let mut lines = vec!["# Generated from a Roster agent declaration.".to_string()];
    if let Some(model) = std::iter::once(&role.model_policy.preferred)
        .chain(role.model_policy.fallbacks.iter())
        .find(|entry| entry.model.starts_with("gpt-"))
    {
        lines.push(format!("model = {}", toml_string(&model.model)));
        lines.push(format!(
            "model_reasoning_effort = {}",
            toml_string(&model.reasoning)
        ));
    }
    let sandbox = match role.permissions.filesystem.as_str() {
        "read-only" => "read-only",
        _ => "workspace-write",
    };
    lines.push(format!("sandbox_mode = {}", toml_string(sandbox)));
    lines.push(format!(
        "developer_instructions = {}",
        toml_string(&render_brief(agent, &[], &[], None))
    ));
    format!("{}\n", lines.join("\n"))
}

fn toml_string(value: &str) -> String {
    serde_json::to_string(value).expect("JSON strings are valid TOML basic strings")
}

/// Composes the single doctrine file `roster sync` points every harness's
/// global doctrine link at: the shared operating doctrine verbatim, then
/// `agent`'s identity (instructions, skills, MCP bindings), so any default
/// agent session on the machine boots as the declared roster orchestrator
/// (operator ruling 2026-07-07) rather than a bare copy of shared AGENTS.md.
pub fn render_home_doctrine(root: &Path, agent: &Agent) -> Result<String, RosterError> {
    let role = &agent.role;
    let doctrine_path = root.join("primitives/shared/AGENTS.md");
    let doctrine = fs::read_to_string(&doctrine_path).map_err(|source| RosterError::Io {
        path: doctrine_path.clone(),
        source,
    })?;

    Ok(format!(
        r#"{doctrine}

# Session Identity: {name} (roster)

{instructions}

## Skills To Read

{skills}

## MCP Servers

{mcp_servers}

---
This is the composed home doctrine for the roster `{name}` agent, the
declared default orchestrator for every harness on this machine. See the
rest of the roster with `roster list`, `roster show <agent>`, or the roster
MCP; lane dispatch goes through `roster materialize` / `roster brief`.
"#,
        doctrine = doctrine.trim_end(),
        name = role.name,
        instructions = agent.instructions.trim(),
        skills = render_skills(&role.skills, &[]),
        mcp_servers = render_mcp_servers(&role.mcps, &role.mcps_contextual, &[]),
    ))
}

fn format_model_entry(entry: &ModelEntry) -> String {
    format!("{} (reasoning: {})", entry.model, entry.reasoning)
}

fn format_fallbacks(fallbacks: &[ModelEntry]) -> String {
    if fallbacks.is_empty() {
        "none".to_string()
    } else {
        fallbacks
            .iter()
            .map(format_model_entry)
            .collect::<Vec<_>>()
            .join(", ")
    }
}

fn claude_model(role: &Role, models: &Models) -> String {
    let preferred = &role.model_policy.preferred.model;

    if let Some(model) = models.claude_for(preferred) {
        return model.to_string();
    }

    // `preferred` isn't a known concrete id in primitives/models.yaml, so
    // treat it as a literal Claude model name. Pass Claude subagent model
    // names straight through; conservatively map the handful of literal
    // Claude ids role.yaml might carry to their subagent-frontmatter short
    // form; anything else (a codex/browser-only id, or an unrecognized
    // string) falls back to `inherit` — the subagent runs on the session's
    // own model — rather than guessing a wrong one.
    match preferred.as_str() {
        "sonnet" | "opus" | "haiku" | "inherit" => preferred.clone(),
        "claude-opus-4-8" => "opus".to_string(),
        "claude-sonnet-5" => "sonnet".to_string(),
        "claude-haiku-4-5" | "claude-haiku-4-5-20251001" => "haiku".to_string(),
        _ => "inherit".to_string(),
    }
}

fn claude_tools(role: &Role) -> Result<String, String> {
    let mut tools = vec!["Read".to_string()];

    // Mutating an external system through a scoped MCP does not grant file
    // writes. The filesystem declaration is the only source for Write/Edit.
    if allows_file_writes(role) {
        tools.push("Write".to_string());
        tools.push("Edit".to_string());
    }

    tools.push("Grep".to_string());
    tools.push("Glob".to_string());

    if allows_shell(role) {
        tools.push("Bash".to_string());
    }

    if role.permissions.network == "allowed" {
        tools.push("WebSearch".to_string());
    }

    for mcp in &role.mcps {
        match (mcp.as_str(), role.permissions.mutations.as_str()) {
            ("powder", "card-comments-and-answers-only") => {
                tools.push("mcp__powder__add_comment".to_string());
                tools.push("mcp__powder__answer_input".to_string());
            }
            ("powder" | "robinhood-trading", "with-explicit-scope") => {
                tools.push(format!("mcp__{mcp}__*"));
            }
            _ => {
                return Err(format!(
                    "claude has no safe required-MCP tool mapping for agent {:?}: server {mcp:?}, mutations {:?}",
                    role.name, role.permissions.mutations
                ));
            }
        }
    }

    Ok(tools.join(", "))
}

// bb (Bitterblossom) config has no MCP concept: `role.mcps`/`mcps_contextual`
// are not rendered into the generated TOML at all, required or contextual.
pub fn render_bb_agent(agent: &Agent, models: &Models) -> Result<String, String> {
    let role = &agent.role;
    require_no_mcps("bb", role)?;
    let model = bb_model(role, models)?;
    let authority = if allows_file_writes(role) {
        "edit"
    } else {
        "read"
    };
    let skills = toml_array(
        &role
            .skills
            .iter()
            .map(|skill| skill.name.clone())
            .collect::<Vec<_>>(),
    );

    Ok(format!(
        r#"# Generated from roster agent {name}.
# Roster preferred model: {preferred}
# Roster reasoning: {reasoning}
version = 1
harness = "pi"
model = "{model}"
provider = "openrouter"
auth = "api"
role = "{name}"
skills = {skills}
secrets = ["OPENROUTER_API_KEY"]

[policy]
authority = "{authority}"
model_allowlist = ["{model}"]
trigger_bindings = ["manual"]
iteration_cap = 24
turn_cap = 40
tool_action_cap = 80
output_bytes_cap = 120000
wall_clock_minutes = 30
side_effect_policy = "kill"
"#,
        name = toml_escape(&role.name),
        preferred = toml_escape(&role.model_policy.preferred.model),
        reasoning = toml_escape(&role.model_policy.preferred.reasoning),
        model = toml_escape(&model),
        skills = skills,
        authority = authority,
    ))
}

/// omp (`can1357/oh-my-pi`, a fork of pi) agent target (roster-915):
/// Markdown/YAML frontmatter, a schema superset of Claude Code's per the
/// roster-910 dispatch-mechanics receipt. Two fields the receipt found on
/// omp's own bundled agents are intentionally NOT emitted here: `spawns:`
/// (the agent-to-agent call graph) and `output:` (a JSON-Schema
/// structured-yield contract). See `docs/roster-915-schema-decision.md` for
/// why roster doesn't adopt an equivalent field of its own yet -- in short,
/// roster has no dispatch runtime to enforce either against, so declaring
/// them here would be unenforced decoration. `model:` is a list of omp's
/// confirmed alias vocabulary (`pi/slow` for high/xhigh reasoning, `pi/smol`
/// otherwise), resolved at omp's session level via `--slow`/`--smol`, not a
/// concrete model id -- so unlike `render_claude_agent`/`render_bb_agent`
/// this needs no `Models` lookup.
pub fn render_omp_agent(agent: &Agent) -> Result<String, String> {
    let role = &agent.role;
    let alias = omp_model_alias(&role.model_policy.preferred.reasoning);
    let tools = omp_tools(role);

    Ok(format!(
        r#"---
name: {name}
description: {description}
model: [{alias}]
tools: {tools}
---

# {name}

{instructions}

## Skills To Read

{skills}

## MCP Servers

{mcp_servers}

## Evidence Expectations

{evidence}
"#,
        name = role.name,
        description = role.description,
        alias = alias,
        tools = tools,
        instructions = agent.instructions.trim(),
        skills = render_skills(&role.skills, &[]),
        mcp_servers = render_mcp_servers(&role.mcps, &role.mcps_contextual, &[]),
        evidence = bullet_list(&role.evidence_expectations),
    ))
}

fn omp_model_alias(reasoning: &str) -> &'static str {
    // Only the two alias values the roster-910 receipt actually observed on
    // live bundled omp agents (reviewer.md: pi/slow, explore.md: pi/smol) --
    // omp's docs mention a third `--plan` session flag, but no bundled agent
    // sample confirmed a `pi/plan` model value, so it's not invented here.
    match reasoning {
        "high" | "xhigh" => "pi/slow",
        _ => "pi/smol",
    }
}

fn omp_tools(role: &Role) -> String {
    // Conservative subset of the tool vocabulary the receipt actually saw
    // (reviewer.md: read, grep, glob, bash, lsp, web_search, ast_grep,
    // yield). `lsp`/`ast_grep`/`yield` aren't included: `yield` pairs with
    // the `output:` schema this renderer doesn't emit, and there's no
    // evidenced per-role criterion for lsp/ast_grep beyond the two samples.
    let mut tools = vec!["read", "grep", "glob"];
    if allows_file_writes(role) {
        tools.push("write");
        tools.push("edit");
    }
    if allows_shell(role) {
        tools.push("bash");
    }
    if role.permissions.network == "allowed" {
        tools.push("web_search");
    }
    if !role.mcps.is_empty() || !role.mcps_contextual.is_empty() {
        // OMP task agents inherit the parent MCP manager. An explicit tools
        // list otherwise hides MCP tools, so expose OMP's native discovery
        // tool and let the declaration's required/contextual server ids guide
        // model selection without hard-coding server tool names here.
        tools.push("search_tool_bm25");
    }
    format!("[{}]", tools.join(", "))
}

fn allows_file_writes(role: &Role) -> bool {
    role.permissions.filesystem == "workspace-write"
}

fn allows_shell(role: &Role) -> bool {
    matches!(
        role.permissions.commands.as_str(),
        "allowed" | "verification-only"
    )
}

fn require_no_mcps(harness: &str, role: &Role) -> Result<(), String> {
    if role.mcps.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "{harness} cannot bind required MCP servers for agent {:?}: {}",
            role.name,
            role.mcps.join(", ")
        ))
    }
}

pub fn render_brief(
    agent: &Agent,
    add_skills: &[String],
    add_mcps: &[String],
    card: Option<&PowderCardSnapshot>,
) -> String {
    let role = &agent.role;
    let mut output = format!(
        r#"# Roster Brief: {name}

## Role

{description}

## Model Policy

- Preferred: {preferred}
- Fallbacks: {fallbacks}

## Instructions

Read: {instruction_path}

{instructions}

## Skills To Read

{skills}

## MCP Servers

{mcp_servers}

## Permissions

- Filesystem: {filesystem}
- Commands: {commands}
- Network: {network}
- Secrets: {secrets}
- Mutations: {mutations}

## Subagent Rights

- May dispatch: {may_dispatch}
- May spawn subagents: {may_spawn_subagents}
- May use peer harnesses: {may_use_peer_harnesses}

## Evidence Contract

{evidence}
"#,
        name = role.name,
        description = role.description,
        preferred = format_model_entry(&role.model_policy.preferred),
        fallbacks = format_fallbacks(&role.model_policy.fallbacks),
        instruction_path = agent.instruction_path().display(),
        instructions = agent.instructions.trim(),
        skills = render_skills(&role.skills, add_skills),
        mcp_servers = render_mcp_servers(&role.mcps, &role.mcps_contextual, add_mcps),
        filesystem = role.permissions.filesystem,
        commands = role.permissions.commands,
        network = role.permissions.network,
        secrets = role.permissions.secrets,
        mutations = role.permissions.mutations,
        may_dispatch = role.subagent_rights.may_dispatch,
        may_spawn_subagents = role.subagent_rights.may_spawn_subagents,
        may_use_peer_harnesses = role.subagent_rights.may_use_peer_harnesses,
        evidence = bullet_list(&role.evidence_expectations),
    );

    if let Some(card) = card {
        output.push_str("\n## Powder Card\n\n");
        output.push_str(&format!("- ID: {}\n", card.id));
        output.push_str(&format!("- Title: {}\n", card.title));
        output.push_str(&format!("- Status: {}\n", card.status));
        output.push_str(&format!("- Powder updated at: {}\n", card.updated_at));
        output.push_str(&format!("- Fetched at: {}\n", card.fetched_at));
        if let Some(claim) = &card.claim {
            output.push_str(&format!(
                "- Active claim: {} via {} until {}\n",
                claim.agent, claim.run_id, claim.expires_at
            ));
        } else {
            output.push_str("- Active claim: none\n");
        }
        output.push_str(
            "- Authority: Powder is authoritative; re-read this card at claim/start and refresh if its update or claim differs.\n",
        );
        if !card.acceptance.is_empty() {
            output.push_str("\n### Acceptance\n\n");
            output.push_str(&bullet_list(&card.acceptance));
            output.push('\n');
        }
        if !card.body.trim().is_empty() {
            output.push_str("\n### Body\n\n");
            output.push_str(card.body.trim());
            output.push('\n');
        }
    }

    output
}

fn bb_model(role: &Role, models: &Models) -> Result<String, String> {
    if let Some(model) = openrouter_model(&role.model_policy.preferred.model) {
        return Ok(model.to_string());
    }
    if let Some(model) = role
        .model_policy
        .fallbacks
        .iter()
        .find_map(|fallback| openrouter_model(&fallback.model))
    {
        return Ok(model.to_string());
    }

    // Neither `preferred` nor any fallback is a literal `openrouter/`-prefixed
    // model id. Resolve `preferred` through primitives/models.yaml instead of
    // ever emitting the bare concrete id as-is (a codex-only id like
    // `gpt-5.6-luna` is not an invocable OpenRouter model, so a bb config carrying
    // it verbatim would silently fail at dispatch time rather than at render
    // time).
    models
        .bb_for(&role.model_policy.preferred.model)
        .map(|model| openrouter_model(model).unwrap_or(model).to_string())
        .ok_or_else(|| {
            format!(
                "cannot resolve bb model for agent {:?}: preferred {:?} is not an \
                 openrouter/-prefixed literal, has no openrouter/-prefixed fallback, \
                 and is not a known model in primitives/models.yaml",
                role.name, role.model_policy.preferred.model
            )
        })
}

fn openrouter_model(value: &str) -> Option<&str> {
    value.strip_prefix("openrouter/")
}

pub fn render_show(agent: &Agent) -> String {
    let role = &agent.role;
    format!(
        r#"# {name}

{description}

- Directory: {directory}
- Instructions: {instruction_path}
- Preferred model: {preferred}
- Fallbacks: {fallbacks}
- Skills: {skill_count}
- MCPs: {mcps}
- Contextual MCPs: {mcps_contextual}

## Evidence Expectations

{evidence}
"#,
        name = role.name,
        description = role.description,
        directory = agent.directory.display(),
        instruction_path = agent.instruction_path().display(),
        preferred = format_model_entry(&role.model_policy.preferred),
        fallbacks = format_fallbacks(&role.model_policy.fallbacks),
        skill_count = role.skills.len(),
        mcps = if role.mcps.is_empty() {
            "none".to_string()
        } else {
            role.mcps.join(", ")
        },
        mcps_contextual = if role.mcps_contextual.is_empty() {
            "none".to_string()
        } else {
            role.mcps_contextual.join(", ")
        },
        evidence = bullet_list(&role.evidence_expectations),
    )
}

fn load_agent(directory: PathBuf) -> Result<Agent, RosterError> {
    let role_path = directory.join("role.yaml");
    let role_text = fs::read_to_string(&role_path).map_err(|source| RosterError::Io {
        path: role_path.clone(),
        source,
    })?;
    let role: Role = serde_yaml::from_str(&role_text).map_err(|source| RosterError::Yaml {
        path: role_path.clone(),
        source,
    })?;

    let instructions_path = directory.join("instructions.md");
    let instructions =
        fs::read_to_string(&instructions_path).map_err(|source| RosterError::Io {
            path: instructions_path.clone(),
            source,
        })?;

    validate_agent(&directory, &role, &instructions)?;

    Ok(Agent {
        directory,
        role,
        instructions,
    })
}

fn validate_agent(directory: &Path, role: &Role, instructions: &str) -> Result<(), RosterError> {
    if role.schema_version != "roster.role.v1" {
        return Err(RosterError::Validation(format!(
            "{} role.yaml has unsupported schema_version {:?}",
            directory.display(),
            role.schema_version
        )));
    }

    let directory_name = directory
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            RosterError::Validation(format!(
                "{} has no valid directory name",
                directory.display()
            ))
        })?;
    if role.name != directory_name {
        return Err(RosterError::Validation(format!(
            "{} role name {:?} does not match directory {:?}",
            directory.display(),
            role.name,
            directory_name
        )));
    }

    require_non_empty(&role.name, "name", directory)?;
    require_non_empty(&role.description, "description", directory)?;
    require_non_empty(
        &role.model_policy.preferred.model,
        "model_policy.preferred.model",
        directory,
    )?;
    require_non_empty(
        &role.model_policy.preferred.reasoning,
        "model_policy.preferred.reasoning",
        directory,
    )?;
    for (index, fallback) in role.model_policy.fallbacks.iter().enumerate() {
        require_non_empty(
            &fallback.model,
            &format!("model_policy.fallbacks[{index}].model"),
            directory,
        )?;
        require_non_empty(
            &fallback.reasoning,
            &format!("model_policy.fallbacks[{index}].reasoning"),
            directory,
        )?;
    }
    require_non_empty(
        &role.permissions.filesystem,
        "permissions.filesystem",
        directory,
    )?;
    require_one_of(
        &role.permissions.filesystem,
        "permissions.filesystem",
        &["read-only", "workspace-write"],
        directory,
    )?;
    require_non_empty(
        &role.permissions.commands,
        "permissions.commands",
        directory,
    )?;
    require_non_empty(&role.permissions.network, "permissions.network", directory)?;
    require_non_empty(&role.permissions.secrets, "permissions.secrets", directory)?;
    require_non_empty(
        &role.permissions.mutations,
        "permissions.mutations",
        directory,
    )?;
    require_non_empty(instructions, "instructions.md", directory)?;

    for skill in &role.skills {
        require_non_empty(&skill.name, "skills[].name", directory)?;
        require_non_empty(&skill.path, "skills[].path", directory)?;
        require_non_empty(&skill.reason, "skills[].reason", directory)?;
    }

    for mcp in &role.mcps {
        require_non_empty(mcp, "mcps[]", directory)?;
    }

    for mcp in &role.mcps_contextual {
        require_non_empty(mcp, "mcps_contextual[]", directory)?;
    }

    for expectation in &role.evidence_expectations {
        require_non_empty(expectation, "evidence_expectations[]", directory)?;
    }

    Ok(())
}

fn validate_unique_names(agents: &[Agent]) -> Result<(), RosterError> {
    let mut names = BTreeSet::new();
    for agent in agents {
        if !names.insert(&agent.role.name) {
            return Err(RosterError::Validation(format!(
                "duplicate agent name {:?}",
                agent.role.name
            )));
        }
    }
    Ok(())
}

fn validate_mcp_references(agents: &[Agent], registry: &McpRegistry) -> Result<(), RosterError> {
    let statuses = registry
        .mcps
        .iter()
        .map(|mcp| (mcp.id.as_str(), mcp.status.as_str()))
        .collect::<BTreeMap<_, _>>();

    for agent in agents {
        for (kind, mcps) in [
            ("mcps", agent.role.mcps.as_slice()),
            ("mcps_contextual", agent.role.mcps_contextual.as_slice()),
        ] {
            for mcp in mcps {
                let Some(status) = statuses.get(mcp.as_str()) else {
                    return Err(RosterError::Validation(format!(
                        "agent {:?} references unknown MCP {:?} in {kind}",
                        agent.role.name, mcp
                    )));
                };
                if matches!(*status, "disabled" | "not_applicable") {
                    return Err(RosterError::Validation(format!(
                        "agent {:?} references MCP {:?} with non-bindable status {:?} in {kind}",
                        agent.role.name, mcp, status
                    )));
                }
            }
        }
    }

    Ok(())
}

fn require_non_empty(value: &str, field: &str, directory: &Path) -> Result<(), RosterError> {
    if value.trim().is_empty() {
        return Err(RosterError::Validation(format!(
            "{} has empty {}",
            directory.display(),
            field
        )));
    }
    Ok(())
}

fn require_one_of(
    value: &str,
    field: &str,
    allowed: &[&str],
    directory: &Path,
) -> Result<(), RosterError> {
    if allowed.contains(&value) {
        Ok(())
    } else {
        Err(RosterError::Validation(format!(
            "{} has unsupported {} {:?}; expected one of {}",
            directory.display(),
            field,
            value,
            allowed.join(", ")
        )))
    }
}

fn render_skills(skills: &[SkillRef], add_skills: &[String]) -> String {
    let mut lines = Vec::new();
    for skill in skills {
        lines.push(format!(
            "- {}: {} ({})",
            skill.name, skill.path, skill.reason
        ));
    }
    for skill in add_skills {
        lines.push(format!("- override: {skill}"));
    }
    if lines.is_empty() {
        "- none".to_string()
    } else {
        lines.join("\n")
    }
}

fn render_mcp_servers(mcps: &[String], mcps_contextual: &[String], add_mcps: &[String]) -> String {
    let required = mcp_lines(mcps, &[]);
    let contextual = mcp_lines(mcps_contextual, add_mcps);
    format!("### Required\n\n{required}\n\n### Contextual (bind when present)\n\n{contextual}")
}

fn mcp_lines(mcps: &[String], overrides: &[String]) -> String {
    let mut lines = mcps
        .iter()
        .map(|mcp| format!("- {mcp}"))
        .collect::<Vec<_>>();
    for mcp in overrides {
        lines.push(format!("- override: {mcp}"));
    }
    if lines.is_empty() {
        "- none".to_string()
    } else {
        lines.join("\n")
    }
}

fn bullet_list(items: &[String]) -> String {
    if items.is_empty() {
        "- none".to_string()
    } else {
        items
            .iter()
            .map(|item| format!("- {item}"))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

fn toml_array(items: &[String]) -> String {
    let values = items
        .iter()
        .map(|item| format!("\"{}\"", toml_escape(item)))
        .collect::<Vec<_>>()
        .join(", ");
    format!("[{values}]")
}

fn toml_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}
