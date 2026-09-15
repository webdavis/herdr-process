use crate::Configuration;
use anyhow::Result;
use herdr_process_domain::Action;
use serde::Serialize;
use std::{
    fs::{File, OpenOptions},
    io::{self, Write},
    os::unix::fs::OpenOptionsExt,
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT_MANIFEST: AtomicU64 = AtomicU64::new(0);

#[derive(Serialize)]
struct Manifest {
    id: &'static str,
    name: &'static str,
    version: &'static str,
    min_herdr_version: &'static str,
    platforms: [&'static str; 1],
    build: Vec<Command>,
    actions: Vec<Entry>,
    panes: Vec<Entry>,
}
#[derive(Serialize)]
struct Command {
    command: Vec<String>,
}
#[derive(Serialize)]
struct Entry {
    id: String,
    title: String,
    command: Vec<String>,
}
impl Configuration {
    pub fn write_manifest(&self, plugin_directory: &Path) -> Result<()> {
        let manifest = self.render_manifest()?;
        replace_manifest(plugin_directory, manifest.as_bytes(), |file, bytes| {
            file.write_all(bytes)
        })?;
        Ok(())
    }
    pub fn render_manifest(&self) -> Result<String> {
        let mut actions = Vec::new();
        for profile in self.profiles().keys() {
            for action in [
                Action::SplitRight,
                Action::SplitBelow,
                Action::ToggleFloat,
                Action::Kill,
            ] {
                actions.push(Entry {
                    id: format!("{profile}:{action}"),
                    title: format!("{profile}: {action}"),
                    command: vec![
                        "./target/release/herdr-process".into(),
                        "action".into(),
                        profile.clone(),
                        action.to_string(),
                    ],
                });
            }
        }
        let manifest = Manifest {
            id: "herdr-process",
            name: "Herdr process",
            version: env!("CARGO_PKG_VERSION"),
            min_herdr_version: "0.9.0",
            platforms: ["macos"],
            build: vec![Command {
                command: vec![
                    "cargo".into(),
                    "build".into(),
                    "--release".into(),
                    "--locked".into(),
                ],
            }],
            actions,
            panes: vec![Entry {
                id: "attach".into(),
                title: "Process attachment".into(),
                command: vec!["./target/release/herdr-process".into(), "attach".into()],
            }],
        };
        Ok(toml::to_string(&manifest)?)
    }
}
fn replace_manifest(
    directory: &Path,
    bytes: &[u8],
    write: impl FnOnce(&mut File, &[u8]) -> io::Result<()>,
) -> io::Result<()> {
    let temporary = directory.join(format!(
        ".herdr-plugin.toml.{}-{}.tmp",
        std::process::id(),
        NEXT_MANIFEST.fetch_add(1, Ordering::Relaxed)
    ));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temporary)?;
    let result = (|| {
        write(&mut file, bytes)?;
        file.sync_all()?;
        std::fs::rename(&temporary, directory.join("herdr-plugin.toml"))
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result
}
#[cfg(test)]
mod tests;
