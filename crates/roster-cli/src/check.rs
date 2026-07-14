//! `roster check` gates only what deterministic code consumes: skill
//! frontmatter shape, path references skill prose cites, index/disk parity,
//! and conflict markers. Semantic judgment (premise soundness, prose
//! quality) stays model work — see the ci and shape skills. Scope is
//! `primitives/`, including untracked non-ignored files so a new primitive is
//! gated before its first commit.

use crate::process;
use anyhow::{Context, Result, ensure};
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

pub fn run(root: &Path) -> Result<bool> {
    let tracked = tracked_files(root)?;
    let mut findings = Vec::new();
    let mut warnings = Vec::new();

    for path in &tracked {
        let Ok(content) = fs::read_to_string(root.join(path)) else {
            continue;
        };
        if content.lines().any(|line| line.starts_with("<<<<<<<")) {
            findings.push(format!("{}: conflict marker", path.display()));
        }
        check_review_due(path, &content, &mut warnings);
        if let Some((is_external, _)) = skill_md_name(path) {
            check_frontmatter(path, &content, is_external, &mut findings);
        }
        if is_first_party_prose(path) {
            for reference in extract_path_refs(&content) {
                if !root.join(&reference).exists() {
                    findings.push(format!("{}: dead reference {reference}", path.display()));
                }
            }
        }
    }
    check_index_drift(root, &tracked, &mut findings)?;
    check_external_provenance(root, &mut findings)?;

    for warning in &warnings {
        println!("WARN {warning}");
    }
    for finding in &findings {
        println!("FAIL {finding}");
    }
    if findings.is_empty() {
        println!("roster check: ok ({} primitive files)", tracked.len());
    }
    Ok(findings.is_empty())
}

/// Validate the local, offline half of the external-skill supply chain. The
/// registry is the declaration; vendored directories and their fetch receipts
/// must agree with it. Byte parity with GitHub remains an explicit online sync
/// operation, not a hidden network dependency of `roster check`.
fn check_external_provenance(root: &Path, findings: &mut Vec<String>) -> Result<()> {
    let external_root = root.join("primitives/skills/.external");
    let registry_path = external_root.join("registry.yaml");
    if !registry_path.is_file() {
        return Ok(());
    }

    let registry: serde_yaml::Value = serde_yaml::from_str(
        &fs::read_to_string(&registry_path).context("read external skill registry")?,
    )
    .context("parse external skill registry")?;
    let sources = registry
        .get("sources")
        .and_then(serde_yaml::Value::as_sequence)
        .cloned()
        .unwrap_or_default();
    let mut declared = std::collections::BTreeMap::<String, (String, String, String)>::new();

    for source in sources {
        let active = source
            .get("active")
            .and_then(serde_yaml::Value::as_bool)
            .unwrap_or(true);
        let default = source
            .get("default")
            .and_then(serde_yaml::Value::as_bool)
            .unwrap_or(false);
        if !active || default {
            continue;
        }
        let repo = yaml_string(&source, "repo");
        let pin = yaml_string(&source, "pin");
        let prefix = yaml_string(&source, "alias_prefix");
        if repo.is_empty() || !is_full_git_sha(&pin) || prefix.is_empty() {
            findings.push(format!(
                "primitives/skills/.external/registry.yaml: active external source {repo:?} requires repo, a full 40-hex pin, and alias_prefix"
            ));
            continue;
        }
        let mut names = source
            .get("include")
            .and_then(serde_yaml::Value::as_sequence)
            .into_iter()
            .flatten()
            .filter_map(serde_yaml::Value::as_str)
            .map(str::to_owned)
            .collect::<Vec<_>>();
        if let Some(name) = source.get("skill_name").and_then(serde_yaml::Value::as_str) {
            names.push(name.to_owned());
        }
        if names.is_empty() {
            findings.push(format!(
                "primitives/skills/.external/registry.yaml: active external source {repo} declares no include or skill_name"
            ));
            continue;
        }

        for source_name in names {
            let alias = format!("{prefix}{source_name}");
            if let Some((other_repo, _, _)) = declared.get(&alias) {
                findings.push(format!(
                    "primitives/skills/.external/registry.yaml: alias collision {alias} ({other_repo} and {repo})"
                ));
                continue;
            }
            declared.insert(alias, (repo.clone(), pin.clone(), source_name));
        }
    }

    for (alias, (repo, pin, source_name)) in &declared {
        let dir = external_root.join(alias);
        if !dir.join("SKILL.md").is_file() {
            findings.push(format!(
                "primitives/skills/.external/registry.yaml: declared alias {alias} has no vendored SKILL.md"
            ));
            continue;
        }
        let meta_path = dir.join(".sync-meta.json");
        let meta: serde_json::Value = match fs::read_to_string(&meta_path) {
            Ok(content) => match serde_json::from_str(&content) {
                Ok(meta) => meta,
                Err(error) => {
                    findings.push(format!(
                        "{}: invalid JSON: {error}",
                        meta_path.strip_prefix(root).unwrap_or(&meta_path).display()
                    ));
                    continue;
                }
            },
            Err(_) => {
                findings.push(format!(
                    "primitives/skills/.external/{alias}/.sync-meta.json: missing provenance receipt"
                ));
                continue;
            }
        };
        for (field, expected) in [("repo", repo.as_str()), ("sha", pin.as_str())] {
            let actual = meta.get(field).and_then(serde_json::Value::as_str);
            if actual != Some(expected) {
                findings.push(format!(
                    "primitives/skills/.external/{alias}/.sync-meta.json: {field} mismatch (expected {expected:?}, found {actual:?})"
                ));
            }
        }
        // Multi-root sources may record the complete nested path while flat
        // sources record only the leaf. In either case its final component is
        // the registry's included skill name.
        let suffix = meta
            .get("src_path_suffix")
            .and_then(serde_json::Value::as_str);
        let suffix_name = suffix
            .and_then(|value| Path::new(value).file_name())
            .and_then(|value| value.to_str());
        if suffix_name != Some(source_name) {
            findings.push(format!(
                "primitives/skills/.external/{alias}/.sync-meta.json: src_path_suffix mismatch (expected leaf {source_name:?}, found {suffix:?})"
            ));
        }
    }

    for entry in fs::read_dir(&external_root).context("read external skills directory")? {
        let entry = entry.context("read external skill entry")?;
        if !entry.file_type()?.is_dir() || !entry.path().join("SKILL.md").is_file() {
            continue;
        }
        let alias = entry.file_name().to_string_lossy().to_string();
        if !declared.contains_key(&alias) {
            findings.push(format!(
                "primitives/skills/.external/{alias}: vendored skill is not declared by an active registry source"
            ));
        }
    }
    Ok(())
}

fn yaml_string(value: &serde_yaml::Value, key: &str) -> String {
    value
        .get(key)
        .and_then(serde_yaml::Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

fn is_full_git_sha(value: &str) -> bool {
    value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

/// Non-fatal freshness tripwire: any frontmatter `*review_due: YYYY-MM-DD`
/// date in the past emits a WARN so the agent running the gate knows the
/// file's researched facts are stale and owes a refresh (/research or
/// /harness-engineering models). Warnings never fail the gate — a date
/// passing must not redden unrelated CI.
fn check_review_due(path: &Path, content: &str, warnings: &mut Vec<String>) {
    if path.extension().and_then(|ext| ext.to_str()) != Some("md") {
        return;
    }
    let Some(rest) = content.strip_prefix("---\n") else {
        return;
    };
    let Some((frontmatter, _)) = rest.split_once("\n---") else {
        return;
    };
    let today = chrono::Local::now().date_naive();
    for line in frontmatter.lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let (key, value) = (key.trim(), value.trim());
        if !key.ends_with("review_due") {
            continue;
        }
        if let Ok(due) = chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d")
            && due < today
        {
            warnings.push(format!(
                "{}: {key} {value} is past due — researched facts are stale; refresh via /research or /harness-engineering models",
                path.display()
            ));
        }
    }
}

fn tracked_files(root: &Path) -> Result<Vec<PathBuf>> {
    let output = process::isolated("git", &Default::default())
        .args([
            "ls-files",
            "--cached",
            "--others",
            "--exclude-standard",
            "--",
            "primitives",
        ])
        .current_dir(root)
        .output()
        .context("run git ls-files")?;
    ensure!(
        output.status.success(),
        "git ls-files failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(String::from_utf8(output.stdout)?
        .lines()
        .map(PathBuf::from)
        .filter(|path| root.join(path).exists())
        .collect())
}

/// `primitives/skills/<name>/SKILL.md` or `.../.external/<name>/SKILL.md`.
fn skill_md_name(path: &Path) -> Option<(bool, &str)> {
    let rest = path.to_str()?.strip_prefix("primitives/skills/")?;
    let (is_external, rest) = rest
        .strip_prefix(".external/")
        .map_or((false, rest), |r| (true, r));
    let name = rest.strip_suffix("/SKILL.md")?;
    (!name.contains('/')).then_some((is_external, name))
}

fn is_first_party_prose(path: &Path) -> bool {
    let raw = path.to_str().unwrap_or_default();
    raw.starts_with("primitives/skills/")
        && raw.ends_with(".md")
        && !raw.contains("/.external/")
        && !raw.contains("/evals/")
}

fn check_frontmatter(path: &Path, content: &str, is_external: bool, findings: &mut Vec<String>) {
    let Some(frontmatter) = content
        .strip_prefix("---\n")
        .and_then(|body| body.find("\n---").map(|end| &body[..end]))
    else {
        findings.push(format!("{}: no frontmatter block", path.display()));
        return;
    };
    let doc: serde_yaml::Value = match serde_yaml::from_str(frontmatter) {
        Ok(doc) => doc,
        Err(error) => {
            findings.push(format!(
                "{}: frontmatter does not parse: {error}",
                path.display()
            ));
            return;
        }
    };
    let empty = |key: &str| {
        doc.get(key)
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .trim()
            .is_empty()
    };
    if empty("name") {
        findings.push(format!("{}: missing/empty name", path.display()));
    }
    if empty("description") {
        findings.push(format!("{}: missing/empty description", path.display()));
    }
    let disabled = doc
        .get("disable-model-invocation")
        .and_then(serde_yaml::Value::as_bool)
        .unwrap_or(false);
    if !is_external && !disabled && empty("argument-hint") {
        findings.push(format!(
            "{}: missing argument-hint (first-party, model-invocable)",
            path.display()
        ));
    }
}

/// Scans prose for `primitives/skills/...` and `primitives/shared/...` path
/// tokens (backticked or bare) so this doesn't need a markdown parser.
fn extract_path_refs(content: &str) -> BTreeSet<String> {
    let mut refs = BTreeSet::new();
    for prefix in ["primitives/skills/", "primitives/shared/"] {
        let mut rest = content;
        while let Some(start) = rest.find(prefix) {
            let tail = &rest[start..];
            let end = tail
                .find(|c: char| !(c.is_alphanumeric() || "_./-".contains(c)))
                .unwrap_or(tail.len());
            let candidate = tail[..end].trim_end_matches('/');
            if candidate.len() > prefix.len() {
                refs.insert(candidate.to_string());
            }
            rest = &tail[end.max(1)..];
        }
    }
    refs
}

fn check_index_drift(root: &Path, tracked: &[PathBuf], findings: &mut Vec<String>) -> Result<()> {
    let index_rel = "primitives/skills/skills-index.yaml";
    let content = fs::read_to_string(root.join(index_rel)).context("read skills-index.yaml")?;
    let doc: serde_yaml::Value =
        serde_yaml::from_str(&content).context("parse skills-index.yaml")?;
    let entries = doc
        .get("skills")
        .and_then(|v| v.as_sequence())
        .cloned()
        .unwrap_or_default();

    let mut indexed = BTreeSet::new();
    for entry in &entries {
        let path = entry
            .get("path")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        // Index paths are repo-relative; absolute paths are a portability
        // failure (they only resolve on the machine that wrote them).
        if Path::new(path).is_absolute() {
            let name = entry.get("name").and_then(|v| v.as_str()).unwrap_or("?");
            findings.push(format!(
                "{index_rel}: absolute path for {name} (must be repo-relative) -> {path}"
            ));
        } else if !root.join(path).exists() {
            let name = entry.get("name").and_then(|v| v.as_str()).unwrap_or("?");
            findings.push(format!("{index_rel}: orphan entry {name} -> {path}"));
        }
        if let Some(dir) = path
            .split("primitives/skills/")
            .nth(1)
            .and_then(|r| r.split('/').next())
        {
            indexed.insert(dir.to_string());
        }
    }
    for (is_external, name) in tracked.iter().filter_map(|p| skill_md_name(p)) {
        if !is_external && !indexed.contains(name) {
            findings.push(format!(
                "{index_rel}: missing entry for primitives/skills/{name}"
            ));
        }
    }
    Ok(())
}
