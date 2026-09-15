use crate::arguments::Options;
use anyhow::{Context, Result, ensure};
use herdr_process_adapters::{
    Configuration, ConfigurationPaths, Herdr, resolve_configuration_paths,
};
use herdr_process_protocol::Target;
use std::{
    collections::hash_map::DefaultHasher,
    ffi::OsString,
    hash::{Hash, Hasher},
    path::PathBuf,
};

pub struct Environment {
    pub home: PathBuf,
    pub paths: ConfigurationPaths,
}
impl Environment {
    pub fn read(options: &Options, get: impl Fn(&str) -> Option<OsString>) -> Result<Self> {
        let home = PathBuf::from(get("HOME").context("HOME must be supplied and absolute")?);
        ensure!(home.is_absolute(), "HOME must be supplied and absolute");
        let selected = options
            .herdr
            .clone()
            .or_else(|| get("HERDR_CONFIG_PATH").map(PathBuf::from));
        let xdg = get("XDG_CONFIG_HOME").map(PathBuf::from);
        let paths = resolve_configuration_paths(
            options.profiles.as_deref(),
            selected.as_deref(),
            xdg.as_deref(),
            &home,
        );
        Ok(Self { home, paths })
    }
    pub fn load(&self) -> Result<Configuration> {
        Configuration::load(&self.paths.profiles, &self.paths.herdr, &self.home)
    }
}
pub fn host(get: impl Fn(&str) -> Option<OsString>) -> Result<Herdr> {
    let binary = get("HERDR_BIN_PATH")
        .filter(|v| !v.is_empty())
        .context("HERDR_BIN_PATH is required")?;
    let socket = get("HERDR_SOCKET_PATH")
        .filter(|v| !v.is_empty())
        .context("HERDR_SOCKET_PATH is required")?;
    Ok(Herdr {
        binary: binary.into(),
        socket: socket.into(),
    })
}
pub fn target(get: impl Fn(&str) -> Option<OsString>) -> Result<Target> {
    let context: serde_json::Value = match get("HERDR_PLUGIN_CONTEXT_JSON") {
        Some(value) => {
            let context: serde_json::Value =
                serde_json::from_str(value.to_str().context("invalid Herdr context encoding")?)
                    .context("invalid Herdr context")?;
            ensure!(context.is_object(), "invalid Herdr context object");
            context
        }
        None => serde_json::Value::Null,
    };
    let field = |name: &str, fallback: &str| -> Result<String> {
        match context.get(name) {
            Some(serde_json::Value::String(value)) => Ok(value.clone()),
            None | Some(serde_json::Value::Null) => get(fallback)
                .unwrap_or_default()
                .into_string()
                .map_err(|_| anyhow::anyhow!("invalid Herdr target encoding")),
            _ => anyhow::bail!("invalid Herdr target field"),
        }
    };
    Ok(Target {
        workspace: field("workspace_id", "HERDR_WORKSPACE_ID")?,
        pane: field("focused_pane_id", "HERDR_PANE_ID")?,
    })
}
pub fn runtime(host: &Herdr, paths: &ConfigurationPaths) -> PathBuf {
    let mut hash = DefaultHasher::new();
    (&host.socket, &paths.herdr, &paths.profiles).hash(&mut hash);
    // Relative selections are interpreted in the invoking directory.
    if !host.socket.is_absolute() || !paths.herdr.is_absolute() || !paths.profiles.is_absolute() {
        std::env::current_dir().ok().hash(&mut hash);
    }
    // Reading the effective user identifier has no mutable native state.
    let uid = unsafe { libc::geteuid() };
    PathBuf::from(format!("/tmp/herdr-process-{uid}-{:016x}", hash.finish()))
}
#[cfg(test)]
mod tests;
