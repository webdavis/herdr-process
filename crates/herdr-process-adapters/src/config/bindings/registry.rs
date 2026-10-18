use super::keys;
use anyhow::{Context, Result, ensure};
use std::collections::BTreeMap;

#[derive(Default)]
pub(super) struct Registry(BTreeMap<(bool, Vec<u8>), String>);

impl Registry {
    pub fn prefix(&mut self, prefix: &[u8]) {
        self.0
            .insert((false, prefix.to_vec()), "keys.prefix".to_string());
    }
    pub fn owned(&mut self, chord: Vec<u8>, label: &str) -> Result<()> {
        self.insert((true, chord), label)
    }
    pub fn indexed(&mut self, value: &toml::Value) -> Result<()> {
        for (_, modifiers) in value.as_table().context("keys.indexed must be a table")? {
            let modifiers = modifiers
                .as_str()
                .context("indexed modifiers must be a string")?;
            if modifiers.trim().is_empty() {
                continue;
            }
            for n in 1..=9 {
                self.foreign(&format!("{modifiers}+{n}"))?;
            }
        }
        Ok(())
    }
    pub fn native(&mut self, field: &str, label: &str) -> Result<()> {
        if !label.contains("1..9") {
            return self.foreign(label);
        }
        ensure!(
            matches!(field, "switch_tab" | "switch_workspace" | "focus_agent"),
            "range only allowed for indexed native actions"
        );
        for n in 1..=9 {
            self.foreign(&label.replace("1..9", &n.to_string()))?;
        }
        Ok(())
    }
    pub fn foreign(&mut self, label: &str) -> Result<()> {
        let label = label.trim();
        match csi_u_only_or_legacy(label) {
            Some(key) if !label.is_empty() => self.insert(key, label),
            _ => Ok(()),
        }
    }
    fn insert(&mut self, key: (bool, Vec<u8>), label: &str) -> Result<()> {
        ensure!(
            self.0.insert(key, label.to_string()).is_none(),
            "duplicate or conflicting native chord {label:?}"
        );
        Ok(())
    }
}

fn csi_u_only_or_legacy(label: &str) -> Option<(bool, Vec<u8>)> {
    let prefixed = label.strip_prefix("prefix+");
    let bytes = keys::encode(prefixed.unwrap_or(label)).ok()?;
    Some((prefixed.is_some(), bytes))
}
