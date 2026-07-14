use std::{collections::BTreeMap, env, ffi::OsStr, process::Command};

const SAFE_PARENT_ENV: &[&str] = &[
    "HOME",
    "USER",
    "LOGNAME",
    "PATH",
    "SHELL",
    "TERM",
    "COLORTERM",
    "TERM_PROGRAM",
    "TERM_PROGRAM_VERSION",
    "TMPDIR",
    "LANG",
    "LC_ALL",
    "LC_CTYPE",
    "NO_COLOR",
    "FORCE_COLOR",
    "SSH_AUTH_SOCK",
    "XDG_CONFIG_HOME",
    "XDG_CACHE_HOME",
    "XDG_DATA_HOME",
];

const DECLARED_CHILD_ENV_PREFIX: &str = "ROSTER_CHILD_ENV_";

pub fn visible_parent_environment() -> BTreeMap<String, String> {
    SAFE_PARENT_ENV
        .iter()
        .filter_map(|name| env::var(name).ok().map(|value| ((*name).to_owned(), value)))
        .collect()
}

/// Construct every Roster child from the same empty, explicit environment.
///
/// Operators may deliberately supply value-free runtime routing through
/// `ROSTER_CHILD_ENV_<NAME>`; the child receives it as `<NAME>`. Roster-owned
/// projection values are applied last and therefore cannot be overridden by
/// ambient state.
pub fn isolated(program: impl AsRef<OsStr>, projection: &BTreeMap<String, String>) -> Command {
    let mut command = Command::new(program);
    command.env_clear();
    for name in SAFE_PARENT_ENV {
        if let Some(value) = env::var_os(name) {
            command.env(name, value);
        }
    }
    for (name, value) in env::vars_os() {
        let Some(name) = name.to_str() else {
            continue;
        };
        let Some(child_name) = name.strip_prefix(DECLARED_CHILD_ENV_PREFIX) else {
            continue;
        };
        if !child_name.is_empty() {
            command.env(child_name, value);
        }
    }
    command.envs(projection);
    command
}
