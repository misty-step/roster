//! Roster's deterministic spine: discover one configuration, resolve one agent,
//! and materialize exactly that agent's declared primitives.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    path::{Path, PathBuf},
    str::FromStr,
};

const CONFIG_SCHEMA: &str = "roster.config.v1";
const ROLE_SCHEMA: &str = "roster.role.v2";
const PACK_SCHEMA: &str = "roster.pack.v1";
const MCP_SCHEMA: &str = "roster.mcp_registry.v1";
const MANIFEST_SCHEMA: &str = "roster.bundle.v2";

pub fn discover_config(start: &Path, home: &Path) -> Result<PathBuf, RosterError> {
    let start = absolute(start)?;
    for directory in start.ancestors() {
        let candidate = directory.join(".roster/config.yaml");
        if candidate.is_file() {
            return Ok(candidate);
        }
    }
    let fallback = home.join(".roster/config.yaml");
    if fallback.is_file() {
        Ok(fallback)
    } else {
        Err(RosterError::ConfigNotFound { start, fallback })
    }
}

pub fn discover_from(start: &Path) -> Result<PathBuf, RosterError> {
    let home = env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or(RosterError::HomeNotSet)?;
    discover_config(start, &home)
}

#[derive(Debug)]
pub struct Roster {
    config_path: PathBuf,
    config: Config,
}

impl Roster {
    pub fn load_config(path: impl AsRef<Path>) -> Result<Self, RosterError> {
        let path = absolute(path.as_ref())?;
        let mut visited = BTreeSet::new();
        let config = load_config_tree(&path, &mut visited)?;
        validate_config(&path, &config)?;
        Ok(Self {
            config_path: path,
            config,
        })
    }

    pub fn discover(start: impl AsRef<Path>) -> Result<Self, RosterError> {
        Self::load_config(discover_from(start.as_ref())?)
    }

    pub fn config_path(&self) -> &Path {
        &self.config_path
    }

    pub fn agents(&self) -> &BTreeMap<String, Agent> {
        &self.config.agents
    }

    pub fn source_roots(&self) -> impl Iterator<Item = &Path> {
        self.config.sources.values().map(PathBuf::as_path)
    }

    pub fn agent(&self, name: &str) -> Option<&Agent> {
        self.config.agents.get(name)
    }

    pub fn authority(&self) -> Option<&Authority> {
        self.config.authority.as_ref()
    }

    pub fn resolve(&self, name: &str) -> Result<ResolvedAgent, RosterError> {
        let agent = self
            .agent(name)
            .cloned()
            .ok_or_else(|| RosterError::UnknownAgent(name.to_owned()))?;
        let role_identity = Identity::from_str(&agent.role)?;
        if role_identity.kind != PrimitiveKind::Role {
            return Err(RosterError::Validation(format!(
                "agent {name:?} role must identify a role, got {}",
                agent.role
            )));
        }
        let source_root = self.source_root(&role_identity.source)?;
        let role_path = source_root
            .join("roles")
            .join(format!("{}.yaml", role_identity.name));
        let role: Role = read_yaml(&role_path)?;
        require_schema(&role_path, &role.schema_version, ROLE_SCHEMA)?;
        if role.name != role_identity.name {
            return Err(RosterError::Validation(format!(
                "{} declares role {:?}, expected {:?}",
                role_path.display(),
                role.name,
                role_identity.name
            )));
        }

        self.resolve_includes(
            name,
            name,
            &agent.description,
            &role.name,
            &role.description,
            &role.include,
            agent.clone(),
            role_identity.to_string(),
        )
    }

    pub fn resolve_ad_hoc(
        &self,
        binding: &str,
        name: &str,
        purpose: &str,
        include: &[String],
    ) -> Result<ResolvedAgent, RosterError> {
        if !is_slug(name) {
            return Err(RosterError::Validation(format!(
                "unsafe ad-hoc agent name {name:?}"
            )));
        }
        if self.agent(name).is_some() {
            return Err(RosterError::Validation(format!(
                "ad-hoc agent name {name:?} conflicts with a declared agent"
            )));
        }
        let purpose = purpose.trim();
        if purpose.is_empty() {
            return Err(RosterError::Validation(
                "ad-hoc purpose must not be empty".to_owned(),
            ));
        }
        if include.is_empty() {
            return Err(RosterError::Validation(
                "ad-hoc composition must include at least one primitive or pack".to_owned(),
            ));
        }
        let agent = self
            .agent(binding)
            .cloned()
            .ok_or_else(|| RosterError::UnknownAgent(binding.to_owned()))?;
        self.resolve_includes(
            name,
            binding,
            purpose,
            "ad-hoc",
            purpose,
            include,
            agent,
            format!("ad-hoc/role:{name}"),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn resolve_includes(
        &self,
        name: &str,
        binding: &str,
        description: &str,
        role_name: &str,
        role_description: &str,
        include: &[String],
        agent: Agent,
        via_root: String,
    ) -> Result<ResolvedAgent, RosterError> {
        let mut flattened = Vec::new();
        let mut pack_stack = BTreeSet::new();
        for include in include {
            self.expand(
                include,
                &mut flattened,
                &mut pack_stack,
                std::slice::from_ref(&via_root),
            )?;
        }

        let mut unique = Vec::<Expanded>::new();
        let mut positions = BTreeMap::<String, usize>::new();
        for expanded in flattened {
            let key = expanded.identity.to_string();
            if let Some(position) = positions.get(&key).copied() {
                if !unique[position].via.contains(&expanded.via) {
                    unique[position].via.push(expanded.via);
                }
            } else {
                positions.insert(key, unique.len());
                unique.push(Expanded {
                    identity: expanded.identity,
                    via: vec![expanded.via],
                });
            }
        }

        let mut guidance = Vec::new();
        let mut skills = Vec::new();
        let mut skill_names = BTreeMap::<String, String>::new();
        let mut mcp_names = BTreeMap::<String, String>::new();
        let mut mcps = Vec::new();
        for expanded in unique {
            let identity = expanded.identity;
            let root = self.source_root(&identity.source)?;
            match identity.kind {
                PrimitiveKind::Guidance => {
                    let path = root
                        .join("primitives/guidance")
                        .join(format!("{}.md", identity.name));
                    guidance.push(ResolvedGuidance {
                        identity: identity.to_string(),
                        body: read_text(&path)?,
                        path,
                        via: expanded.via,
                    });
                }
                PrimitiveKind::Skill => {
                    if let Some(existing) =
                        skill_names.insert(identity.name.clone(), identity.to_string())
                    {
                        return Err(RosterError::Validation(format!(
                            "skill name collision: {existing} and {identity} both project as {:?}",
                            identity.name
                        )));
                    }
                    let path = resolve_skill_path(&root, &identity.name)?;
                    skills.push(ResolvedSkill {
                        identity: identity.to_string(),
                        name: identity.name,
                        path,
                        via: expanded.via,
                    });
                }
                PrimitiveKind::Mcp => {
                    if let Some(existing) =
                        mcp_names.insert(identity.name.clone(), identity.to_string())
                    {
                        return Err(RosterError::Validation(format!(
                            "MCP name collision: {existing} and {identity} both project as {:?}",
                            identity.name
                        )));
                    }
                    let registry_path = root.join("primitives/mcps/registry.yaml");
                    let registry: McpRegistry = read_yaml(&registry_path)?;
                    require_schema(&registry_path, &registry.schema_version, MCP_SCHEMA)?;
                    validate_mcp_registry(&registry_path, &registry)?;
                    let mcp = registry
                        .mcps
                        .into_iter()
                        .find(|mcp| mcp.id == identity.name)
                        .ok_or_else(|| RosterError::MissingPrimitive(identity.to_string()))?;
                    if mcp.status != "available" {
                        return Err(RosterError::Validation(format!(
                            "{} selects MCP {:?} with non-launchable status {:?}",
                            identity, mcp.id, mcp.status
                        )));
                    }
                    mcps.push(ResolvedMcp {
                        identity: identity.to_string(),
                        mcp,
                        source: registry_path,
                        via: expanded.via,
                    });
                }
                PrimitiveKind::Pack | PrimitiveKind::Role => unreachable!("expanded above"),
            }
        }

        Ok(ResolvedAgent {
            name: name.to_owned(),
            binding: binding.to_owned(),
            description: description.to_owned(),
            role: role_name.to_owned(),
            role_description: role_description.to_owned(),
            model: agent.model,
            reasoning: agent.reasoning,
            harness: agent.harness,
            args: agent.args,
            guidance,
            skills,
            mcps,
            authority: self.config.authority.clone(),
            config_path: self.config_path.clone(),
        })
    }

    fn source_root(&self, source: &str) -> Result<PathBuf, RosterError> {
        let declared = self
            .config
            .sources
            .get(source)
            .ok_or_else(|| RosterError::UnknownSource(source.to_owned()))?;
        let base = self
            .config_path
            .parent()
            .expect("a config file has a parent");
        absolute(&base.join(declared))
    }

    fn expand(
        &self,
        raw: &str,
        output: &mut Vec<ExpandedPath>,
        pack_stack: &mut BTreeSet<String>,
        via: &[String],
    ) -> Result<(), RosterError> {
        let identity = Identity::from_str(raw)?;
        if identity.kind != PrimitiveKind::Pack {
            if identity.kind == PrimitiveKind::Role {
                return Err(RosterError::Validation(format!(
                    "roles cannot include roles: {identity}"
                )));
            }
            output.push(ExpandedPath {
                identity,
                via: via.to_vec(),
            });
            return Ok(());
        }

        let key = identity.to_string();
        if !pack_stack.insert(key.clone()) {
            return Err(RosterError::Validation(format!(
                "pack cycle detected at {key}"
            )));
        }
        let path = self
            .source_root(&identity.source)?
            .join("packs")
            .join(format!("{}.yaml", identity.name));
        let pack: Pack = read_yaml(&path)?;
        require_schema(&path, &pack.schema_version, PACK_SCHEMA)?;
        if pack.name != identity.name {
            return Err(RosterError::Validation(format!(
                "{} declares pack {:?}, expected {:?}",
                path.display(),
                pack.name,
                identity.name
            )));
        }
        for include in &pack.include {
            let mut next_via = via.to_vec();
            next_via.push(key.clone());
            self.expand(include, output, pack_stack, &next_via)?;
        }
        pack_stack.remove(&key);
        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Config {
    schema_version: String,
    #[serde(default)]
    imports: Vec<PathBuf>,
    sources: BTreeMap<String, PathBuf>,
    agents: BTreeMap<String, Agent>,
    #[serde(default)]
    authority: Option<Authority>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Agent {
    pub description: String,
    pub role: String,
    pub model: String,
    #[serde(default)]
    pub reasoning: Option<String>,
    pub harness: Harness,
    #[serde(default)]
    pub args: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Authority {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Harness {
    Codex,
    Claude,
    Omp,
}

impl Harness {
    pub fn command(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::Claude => "claude",
            Self::Omp => "omp",
        }
    }
}

impl FromStr for Harness {
    type Err = RosterError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "codex" => Ok(Self::Codex),
            "claude" => Ok(Self::Claude),
            "omp" => Ok(Self::Omp),
            other => Err(RosterError::Validation(format!(
                "unsupported Harness {other:?}; expected codex, claude, or omp"
            ))),
        }
    }
}

impl std::fmt::Display for Harness {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.command())
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Role {
    schema_version: String,
    name: String,
    description: String,
    include: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Pack {
    schema_version: String,
    name: String,
    #[serde(default)]
    #[serde(rename = "description")]
    _description: Option<String>,
    include: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
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

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct McpRegistry {
    schema_version: String,
    #[serde(rename = "provenance")]
    _provenance: String,
    mcps: Vec<Mcp>,
}

#[derive(Debug, Clone)]
pub struct ResolvedGuidance {
    pub identity: String,
    pub body: String,
    pub path: PathBuf,
    pub via: Vec<Vec<String>>,
}

#[derive(Debug, Clone)]
pub struct ResolvedSkill {
    pub identity: String,
    pub name: String,
    pub path: PathBuf,
    pub via: Vec<Vec<String>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResolvedMcp {
    pub identity: String,
    #[serde(flatten)]
    pub mcp: Mcp,
    #[serde(skip)]
    pub source: PathBuf,
    #[serde(skip)]
    pub via: Vec<Vec<String>>,
}

impl std::ops::Deref for ResolvedMcp {
    type Target = Mcp;
    fn deref(&self) -> &Self::Target {
        &self.mcp
    }
}

#[derive(Debug, Clone)]
pub struct ResolvedAgent {
    pub name: String,
    pub binding: String,
    pub description: String,
    pub role: String,
    pub role_description: String,
    pub model: String,
    pub reasoning: Option<String>,
    pub harness: Harness,
    pub args: Vec<String>,
    pub guidance: Vec<ResolvedGuidance>,
    pub skills: Vec<ResolvedSkill>,
    pub mcps: Vec<ResolvedMcp>,
    pub authority: Option<Authority>,
    pub config_path: PathBuf,
}

impl ResolvedAgent {
    pub fn write_bundle(
        &self,
        destination: &Path,
        workspace: &Path,
    ) -> Result<BundleManifest, RosterError> {
        if destination.exists() {
            return Err(RosterError::DestinationExists(destination.to_path_buf()));
        }
        for skill in &self.skills {
            validate_skill_tree(&skill.path)?;
        }
        fs::create_dir_all(destination).map_err(|source| RosterError::Io {
            path: destination.to_path_buf(),
            source,
        })?;

        let workspace = absolute(workspace)?;
        let (agents_markdown, context) = self.agents_markdown_with_context(&workspace)?;
        let agents_path = destination.join("AGENTS.md");
        write_text(&agents_path, &agents_markdown)?;
        for skill in &self.skills {
            copy_tree(&skill.path, &destination.join("skills").join(&skill.name))?;
        }
        let mcps_path = destination.join("mcps.yaml");
        let mcp_document = McpProjection {
            schema_version: "roster.mcps.v1",
            mcps: self.mcps.clone(),
        };
        write_text(&mcps_path, &to_yaml(&mcp_document)?)?;

        let mut files = BTreeMap::new();
        collect_hashes(destination, destination, &mut files)?;
        let manifest = BundleManifest {
            schema_version: MANIFEST_SCHEMA.to_owned(),
            agent: self.name.clone(),
            binding: self.binding.clone(),
            role: self.role.clone(),
            purpose: self.description.clone(),
            model: self.model.clone(),
            reasoning: self.reasoning.clone(),
            harness: self.harness,
            args: self.args.clone(),
            config: self.config_path.clone(),
            workspace,
            context,
            guidance: self
                .guidance
                .iter()
                .map(|item| ManifestPrimitive {
                    identity: item.identity.clone(),
                    source: item.path.clone(),
                    via: item.via.clone(),
                })
                .collect(),
            skills: self
                .skills
                .iter()
                .map(|item| ManifestPrimitive {
                    identity: item.identity.clone(),
                    source: item.path.clone(),
                    via: item.via.clone(),
                })
                .collect(),
            mcps: self
                .mcps
                .iter()
                .map(|item| ManifestPrimitive {
                    identity: item.identity.clone(),
                    source: item.source.clone(),
                    via: item.via.clone(),
                })
                .collect(),
            files,
        };
        write_text(&destination.join("manifest.yaml"), &to_yaml(&manifest)?)?;
        Ok(manifest)
    }

    pub fn agents_markdown(&self) -> String {
        let mut output = format!(
            "<!-- roster-bundle:{} -->\n# {}\n\n{}\n",
            MANIFEST_SCHEMA, self.name, self.role_description
        );
        for item in &self.guidance {
            output.push('\n');
            output.push_str(item.body.trim());
            output.push('\n');
        }
        if !self.skills.is_empty() {
            output.push_str("\n## Skills\n\nUse these progressive-disclosure skills when their descriptions fit the work:\n");
            for skill in &self.skills {
                output.push_str(&format!(
                    "\n- `{}`: `skills/{}/SKILL.md`",
                    skill.name, skill.name
                ));
            }
            output.push('\n');
        }
        if !self.mcps.is_empty() {
            output.push_str("\n## MCP tools\n\nOnly these role-selected MCP servers are projected into this session:\n");
            for mcp in &self.mcps {
                output.push_str(&format!("\n- `{}`", mcp.id));
            }
            output.push('\n');
        }
        output.push_str(
            "\n## Runtime authority\n\nIf an operation needs authority not already available, request only that operation with `roster authority request <capability>`. The configured provider may grant, proxy, ask the operator, or deny it; denial affects that operation, not this session. Never print or persist credential bytes.\n",
        );
        output
    }

    fn agents_markdown_with_context(
        &self,
        workspace: &Path,
    ) -> Result<(String, Vec<ManifestContext>), RosterError> {
        let mut output = self.agents_markdown();
        let home = env::var_os("HOME").map(PathBuf::from);
        let mut paths = workspace
            .ancestors()
            .take_while(|directory| home.as_deref() != Some(*directory))
            .map(|directory| directory.join("AGENTS.md"))
            .filter(|path| path.is_file())
            .collect::<Vec<_>>();
        paths.reverse();
        let mut context = Vec::new();
        for path in paths {
            let body = read_text(&path)?;
            output.push_str(&format!(
                "\n\n---\n\n# Project context: {}\n\n{body}",
                path.display()
            ));
            context.push(ManifestContext {
                source: path,
                sha256: format!("sha256:{:x}", Sha256::digest(body.as_bytes())),
            });
        }
        Ok((output, context))
    }
}

#[derive(Debug, Serialize)]
struct McpProjection {
    schema_version: &'static str,
    mcps: Vec<ResolvedMcp>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BundleManifest {
    pub schema_version: String,
    pub agent: String,
    pub binding: String,
    pub role: String,
    pub purpose: String,
    pub model: String,
    pub reasoning: Option<String>,
    pub harness: Harness,
    pub args: Vec<String>,
    pub config: PathBuf,
    pub workspace: PathBuf,
    pub context: Vec<ManifestContext>,
    pub guidance: Vec<ManifestPrimitive>,
    pub skills: Vec<ManifestPrimitive>,
    pub mcps: Vec<ManifestPrimitive>,
    pub files: BTreeMap<PathBuf, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestContext {
    pub source: PathBuf,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestPrimitive {
    pub identity: String,
    pub source: PathBuf,
    pub via: Vec<Vec<String>>,
}

#[derive(Debug, Clone)]
struct Expanded {
    identity: Identity,
    via: Vec<Vec<String>>,
}

#[derive(Debug, Clone)]
struct ExpandedPath {
    identity: Identity,
    via: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Identity {
    source: String,
    kind: PrimitiveKind,
    name: String,
}

impl FromStr for Identity {
    type Err = RosterError;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let (source, primitive) = value
            .split_once('/')
            .ok_or_else(|| RosterError::InvalidIdentity(value.to_owned()))?;
        let (kind, name) = primitive
            .split_once(':')
            .ok_or_else(|| RosterError::InvalidIdentity(value.to_owned()))?;
        if !is_slug(source) || !is_slug(name) {
            return Err(RosterError::InvalidIdentity(value.to_owned()));
        }
        Ok(Self {
            source: source.to_owned(),
            kind: PrimitiveKind::from_str(kind)?,
            name: name.to_owned(),
        })
    }
}

impl std::fmt::Display for Identity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}:{}", self.source, self.kind, self.name)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PrimitiveKind {
    Role,
    Pack,
    Guidance,
    Skill,
    Mcp,
}

impl FromStr for PrimitiveKind {
    type Err = RosterError;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "role" => Ok(Self::Role),
            "pack" => Ok(Self::Pack),
            "guidance" => Ok(Self::Guidance),
            "skill" => Ok(Self::Skill),
            "mcp" => Ok(Self::Mcp),
            _ => Err(RosterError::InvalidPrimitiveKind(value.to_owned())),
        }
    }
}

impl std::fmt::Display for PrimitiveKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Role => "role",
            Self::Pack => "pack",
            Self::Guidance => "guidance",
            Self::Skill => "skill",
            Self::Mcp => "mcp",
        })
    }
}

fn load_config_tree(path: &Path, visited: &mut BTreeSet<PathBuf>) -> Result<Config, RosterError> {
    if !visited.insert(path.to_path_buf()) {
        return Err(RosterError::Validation(format!(
            "config import cycle at {}",
            path.display()
        )));
    }
    let mut local: Config = read_yaml(path)?;
    require_schema(path, &local.schema_version, CONFIG_SCHEMA)?;
    let config_directory = path.parent().expect("config parent");
    for source in local.sources.values_mut() {
        if source.is_relative() {
            *source = config_directory.join(&*source);
        }
    }
    let imports = std::mem::take(&mut local.imports);
    let mut combined = Config {
        schema_version: CONFIG_SCHEMA.to_owned(),
        imports: Vec::new(),
        sources: BTreeMap::new(),
        agents: BTreeMap::new(),
        authority: None,
    };
    for import in imports {
        let imported_path = absolute(&path.parent().expect("parent").join(import))?;
        let imported = load_config_tree(&imported_path, visited)?;
        merge_config(&mut combined, imported, &imported_path)?;
    }
    merge_config(&mut combined, local, path)?;
    visited.remove(path);
    Ok(combined)
}

fn merge_config(target: &mut Config, source: Config, origin: &Path) -> Result<(), RosterError> {
    for (id, path) in source.sources {
        if target.sources.insert(id.clone(), path).is_some() {
            return Err(RosterError::Validation(format!(
                "duplicate source {id:?} while importing {}",
                origin.display()
            )));
        }
    }
    for (name, agent) in source.agents {
        if target.agents.insert(name.clone(), agent).is_some() {
            return Err(RosterError::Validation(format!(
                "duplicate agent {name:?} while importing {}",
                origin.display()
            )));
        }
    }
    if let Some(authority) = source.authority
        && target.authority.replace(authority).is_some()
    {
        return Err(RosterError::Validation(format!(
            "duplicate authority while importing {}",
            origin.display()
        )));
    }
    Ok(())
}

fn validate_config(path: &Path, config: &Config) -> Result<(), RosterError> {
    if config.sources.is_empty() {
        return Err(RosterError::Validation(format!(
            "{} has no sources",
            path.display()
        )));
    }
    if config.agents.is_empty() {
        return Err(RosterError::Validation(format!(
            "{} has no agents",
            path.display()
        )));
    }
    for source in config.sources.keys() {
        if !is_slug(source) {
            return Err(RosterError::Validation(format!(
                "{} has unsafe source name {source:?}",
                path.display()
            )));
        }
    }
    for (name, agent) in &config.agents {
        if !is_slug(name) {
            return Err(RosterError::Validation(format!(
                "{} has unsafe agent name {name:?}",
                path.display()
            )));
        }
        if agent.description.trim().is_empty() || agent.model.trim().is_empty() {
            return Err(RosterError::Validation(format!(
                "{} has an incomplete agent {name:?}",
                path.display()
            )));
        }
        Identity::from_str(&agent.role)?;
        validate_agent_args(name, agent)?;
    }
    Ok(())
}

fn is_slug(value: &str) -> bool {
    let mut characters = value.chars();
    characters
        .next()
        .is_some_and(|character| character.is_ascii_lowercase() || character.is_ascii_digit())
        && characters.all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || matches!(character, '-' | '_')
        })
}

fn validate_agent_args(name: &str, agent: &Agent) -> Result<(), RosterError> {
    let mut index = 0;
    while index < agent.args.len() {
        let flag = agent.args[index].as_str();
        let value = |index: usize| {
            agent.args.get(index).map(String::as_str).ok_or_else(|| {
                RosterError::Validation(format!(
                    "agent {name:?} argument {flag:?} requires a value"
                ))
            })
        };
        let consumed = match agent.harness {
            Harness::Codex => match flag {
                "--search" | "--dangerously-bypass-approvals-and-sandbox" | "--no-alt-screen" => 1,
                "--sandbox" | "-s" => {
                    let selected = value(index + 1)?;
                    require_argument_value(
                        name,
                        flag,
                        selected,
                        &["read-only", "workspace-write", "danger-full-access"],
                    )?;
                    2
                }
                "--ask-for-approval" | "-a" => {
                    let selected = value(index + 1)?;
                    require_argument_value(
                        name,
                        flag,
                        selected,
                        &["untrusted", "on-request", "never"],
                    )?;
                    2
                }
                _ => return Err(unsafe_argument(name, flag)),
            },
            Harness::Claude => match flag {
                "--dangerously-skip-permissions"
                | "--allow-dangerously-skip-permissions"
                | "--no-chrome"
                | "--chrome"
                | "--verbose" => 1,
                "--permission-mode" => {
                    let selected = value(index + 1)?;
                    require_argument_value(
                        name,
                        flag,
                        selected,
                        &[
                            "acceptEdits",
                            "auto",
                            "bypassPermissions",
                            "manual",
                            "dontAsk",
                            "plan",
                        ],
                    )?;
                    2
                }
                _ => return Err(unsafe_argument(name, flag)),
            },
            Harness::Omp => match flag {
                "--auto-approve" | "--advisor" | "--no-pty" => 1,
                "--approval-mode" => {
                    let selected = value(index + 1)?;
                    require_argument_value(name, flag, selected, &["always-ask", "write", "yolo"])?;
                    2
                }
                _ => return Err(unsafe_argument(name, flag)),
            },
        };
        index += consumed;
    }
    Ok(())
}

fn require_argument_value(
    agent: &str,
    flag: &str,
    actual: &str,
    allowed: &[&str],
) -> Result<(), RosterError> {
    if allowed.contains(&actual) {
        Ok(())
    } else {
        Err(RosterError::Validation(format!(
            "agent {agent:?} has invalid {flag} value {actual:?}"
        )))
    }
}

fn unsafe_argument(agent: &str, flag: &str) -> RosterError {
    RosterError::Validation(format!(
        "agent {agent:?} uses unsafe or topology-changing Harness argument {flag:?}"
    ))
}

fn validate_mcp_registry(path: &Path, registry: &McpRegistry) -> Result<(), RosterError> {
    if registry._provenance.trim().is_empty() {
        return Err(RosterError::Validation(format!(
            "{} has empty provenance",
            path.display()
        )));
    }
    let mut ids = BTreeSet::new();
    for mcp in &registry.mcps {
        if mcp.id.trim().is_empty() || !ids.insert(mcp.id.clone()) {
            return Err(RosterError::Validation(format!(
                "{} has empty or duplicate MCP id {:?}",
                path.display(),
                mcp.id
            )));
        }
        if !["available", "external", "disabled", "not_applicable"].contains(&mcp.status.as_str()) {
            return Err(RosterError::Validation(format!(
                "{} MCP {:?} has unsupported status {:?}",
                path.display(),
                mcp.id,
                mcp.status
            )));
        }
        if mcp.status == "available" {
            match mcp.transport.as_deref() {
                Some("stdio")
                    if mcp
                        .command
                        .as_deref()
                        .is_some_and(|command| !command.trim().is_empty()) => {}
                Some("http") if mcp.url.as_deref().is_some_and(|url| !url.trim().is_empty()) => {}
                _ => {
                    return Err(RosterError::Validation(format!(
                        "{} available MCP {:?} requires stdio command or http URL",
                        path.display(),
                        mcp.id
                    )));
                }
            }
        }
        if mcp.env_refs.iter().any(|value| value.trim().is_empty()) {
            return Err(RosterError::Validation(format!(
                "{} MCP {:?} has an empty env ref",
                path.display(),
                mcp.id
            )));
        }
    }
    Ok(())
}

fn resolve_skill_path(source_root: &Path, name: &str) -> Result<PathBuf, RosterError> {
    let direct = source_root.join("primitives/skills").join(name);
    let direct_file = direct.join("SKILL.md");
    let index_path = source_root.join("primitives/skills/skills-index.yaml");
    let mut indexed_file = None;
    if index_path.is_file() {
        let index: SkillsIndex = read_yaml(&index_path)?;
        if index.schema_version != "roster.skills_index.v1" {
            return Err(RosterError::Validation(format!(
                "{} uses unsupported schema {:?}",
                index_path.display(),
                index.schema_version
            )));
        }
        let mut names = BTreeSet::new();
        for entry in index.skills {
            if !names.insert(entry.name.clone()) {
                return Err(RosterError::Validation(format!(
                    "{} declares duplicate skill name {:?}",
                    index_path.display(),
                    entry.name
                )));
            }
            if entry.name == name {
                if entry.path.is_absolute()
                    || entry.path.components().any(|component| {
                        !matches!(
                            component,
                            std::path::Component::Normal(_) | std::path::Component::CurDir
                        )
                    })
                {
                    return Err(RosterError::Validation(format!(
                        "{} skill {:?} escapes its declared source via {}",
                        index_path.display(),
                        name,
                        entry.path.display()
                    )));
                }
                indexed_file = Some(source_root.join(entry.path));
            }
        }
    }
    if direct_file.is_file() {
        if let Some(indexed) = &indexed_file
            && indexed != &direct_file
        {
            return Err(RosterError::Validation(format!(
                "skill:{name} conflicts between {} and {}",
                direct_file.display(),
                indexed.display()
            )));
        }
        return Ok(direct);
    }
    if let Some(skill_file) = indexed_file {
        if skill_file.file_name().and_then(|name| name.to_str()) != Some("SKILL.md")
            || !skill_file.is_file()
        {
            return Err(RosterError::MissingPrimitive(format!(
                "skill:{name} at {}",
                skill_file.display()
            )));
        }
        let canonical_root = fs::canonicalize(source_root).map_err(|source| RosterError::Io {
            path: source_root.to_path_buf(),
            source,
        })?;
        let canonical_file = fs::canonicalize(&skill_file).map_err(|source| RosterError::Io {
            path: skill_file.clone(),
            source,
        })?;
        if !canonical_file.starts_with(&canonical_root) {
            return Err(RosterError::Validation(format!(
                "skill:{name} resolves outside source {}",
                source_root.display()
            )));
        }
        return Ok(skill_file.parent().expect("SKILL.md parent").to_path_buf());
    }
    Err(RosterError::MissingPrimitive(format!("skill:{name}")))
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SkillsIndex {
    schema_version: String,
    #[serde(rename = "phase")]
    _phase: String,
    #[serde(rename = "note")]
    _note: String,
    skills: Vec<SkillIndexEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SkillIndexEntry {
    name: String,
    path: PathBuf,
}

fn copy_tree(source: &Path, destination: &Path) -> Result<(), RosterError> {
    fs::create_dir_all(destination).map_err(|error| RosterError::Io {
        path: destination.to_path_buf(),
        source: error,
    })?;
    let entries = fs::read_dir(source).map_err(|error| RosterError::Io {
        path: source.to_path_buf(),
        source: error,
    })?;
    for entry in entries {
        let entry = entry.map_err(|error| RosterError::Io {
            path: source.to_path_buf(),
            source: error,
        })?;
        let source_path = entry.path();
        let file_type = entry.file_type().map_err(|error| RosterError::Io {
            path: source_path.clone(),
            source: error,
        })?;
        if file_type.is_symlink() {
            return Err(RosterError::Validation(format!(
                "skill payload contains symlink {}",
                source_path.display()
            )));
        }
        if ignore_bundle_entry(&source_path, file_type.is_dir()) {
            continue;
        }
        let destination_path = destination.join(entry.file_name());
        if file_type.is_dir() {
            copy_tree(&source_path, &destination_path)?;
        } else {
            fs::copy(&source_path, &destination_path).map_err(|error| RosterError::Io {
                path: destination_path,
                source: error,
            })?;
        }
    }
    Ok(())
}

fn ignore_bundle_entry(path: &Path, directory: bool) -> bool {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    if directory && matches!(name, ".git" | "__pycache__" | "node_modules" | "target") {
        return true;
    }
    matches!(name, ".DS_Store" | "Thumbs.db")
        || name.ends_with('~')
        || ["pyc", "pyo", "swp", "tmp"]
            .iter()
            .any(|extension| path.extension().and_then(|value| value.to_str()) == Some(extension))
}

fn validate_skill_tree(directory: &Path) -> Result<(), RosterError> {
    for entry in fs::read_dir(directory).map_err(|source| RosterError::Io {
        path: directory.to_path_buf(),
        source,
    })? {
        let entry = entry.map_err(|source| RosterError::Io {
            path: directory.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        let file_type = entry.file_type().map_err(|source| RosterError::Io {
            path: path.clone(),
            source,
        })?;
        if file_type.is_symlink() {
            return Err(RosterError::Validation(format!(
                "skill payload contains symlink {}",
                path.display()
            )));
        }
        if ignore_bundle_entry(&path, file_type.is_dir()) {
            continue;
        }
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        if (name == ".env" || name.starts_with(".env.")) && !name.starts_with(".env.example") {
            return Err(RosterError::Validation(format!(
                "skill payload contains secret-shaped file {}",
                path.display()
            )));
        }
        if file_type.is_dir() {
            validate_skill_tree(&path)?;
        }
    }
    Ok(())
}

fn collect_hashes(
    root: &Path,
    directory: &Path,
    hashes: &mut BTreeMap<PathBuf, String>,
) -> Result<(), RosterError> {
    for entry in fs::read_dir(directory).map_err(|source| RosterError::Io {
        path: directory.to_path_buf(),
        source,
    })? {
        let entry = entry.map_err(|source| RosterError::Io {
            path: directory.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        if entry
            .file_type()
            .map_err(|source| RosterError::Io {
                path: path.clone(),
                source,
            })?
            .is_dir()
        {
            collect_hashes(root, &path, hashes)?;
        } else if path.file_name().and_then(|name| name.to_str()) != Some("manifest.yaml") {
            let bytes = fs::read(&path).map_err(|source| RosterError::Io {
                path: path.clone(),
                source,
            })?;
            hashes.insert(
                path.strip_prefix(root).expect("inside root").to_path_buf(),
                format!("sha256:{:x}", Sha256::digest(bytes)),
            );
        }
    }
    Ok(())
}

fn read_text(path: &Path) -> Result<String, RosterError> {
    fs::read_to_string(path).map_err(|source| RosterError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn read_yaml<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, RosterError> {
    let text = read_text(path)?;
    serde_yaml::from_str(&text).map_err(|source| RosterError::Yaml {
        path: path.to_path_buf(),
        source,
    })
}

fn to_yaml<T: Serialize>(value: &T) -> Result<String, RosterError> {
    serde_yaml::to_string(value).map_err(RosterError::Serialize)
}

fn write_text(path: &Path, body: &str) -> Result<(), RosterError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| RosterError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    fs::write(path, body).map_err(|source| RosterError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn absolute(path: &Path) -> Result<PathBuf, RosterError> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        env::current_dir()
            .map(|cwd| cwd.join(path))
            .map_err(|source| RosterError::Io {
                path: path.to_path_buf(),
                source,
            })
    }
}

fn require_schema(path: &Path, actual: &str, expected: &str) -> Result<(), RosterError> {
    if actual == expected {
        Ok(())
    } else {
        Err(RosterError::Validation(format!(
            "{} uses schema {:?}; expected {:?}",
            path.display(),
            actual,
            expected
        )))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RosterError {
    #[error("HOME is not set")]
    HomeNotSet,
    #[error("no .roster/config.yaml found from {start}; fallback {fallback} is absent")]
    ConfigNotFound { start: PathBuf, fallback: PathBuf },
    #[error("I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("invalid YAML at {path}: {source}")]
    Yaml {
        path: PathBuf,
        #[source]
        source: serde_yaml::Error,
    },
    #[error("could not serialize YAML: {0}")]
    Serialize(serde_yaml::Error),
    #[error("unknown agent {0:?}")]
    UnknownAgent(String),
    #[error("unknown source {0:?}")]
    UnknownSource(String),
    #[error("invalid primitive identity {0:?}; expected source/kind:name")]
    InvalidIdentity(String),
    #[error("invalid primitive kind {0:?}")]
    InvalidPrimitiveKind(String),
    #[error("missing primitive {0}")]
    MissingPrimitive(String),
    #[error("bundle destination already exists: {0}")]
    DestinationExists(PathBuf),
    #[error("validation failed: {0}")]
    Validation(String),
}
