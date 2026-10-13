mod paths;
pub use paths::{ConfigurationPaths, resolve_configuration_paths};
mod bindings;
mod keys;
mod profiles;
use anyhow::Result;
use herdr_process_application::Keymap;
use herdr_process_domain::Profile;
use std::{collections::BTreeMap, path::Path};
#[derive(Debug)]
pub struct Configuration {
    profiles: BTreeMap<String, Profile>,
    keymap: Keymap,
    identity: String,
}
impl Configuration {
    pub(super) fn parse(profiles: &str, herdr: &str, home: &Path) -> Result<Self> {
        let profiles = profiles::parse(profiles, home)?;
        let keymap = bindings::parse(herdr, &profiles)?;
        let mapping: Vec<_> = keymap
            .bindings()
            .iter()
            .map(|b| (&b.chord, &b.profile, b.action.to_string()))
            .collect();
        let identity = serde_json::to_string(&(&profiles, herdr, keymap.prefix(), mapping))?;
        Ok(Self {
            profiles,
            keymap,
            identity,
        })
    }
    pub fn load(profiles_path: &Path, herdr_path: &Path, home: &Path) -> Result<Self> {
        Self::parse(
            &std::fs::read_to_string(profiles_path)?,
            &std::fs::read_to_string(herdr_path)?,
            home,
        )
    }
    pub fn identity(&self) -> &str {
        &self.identity
    }
    pub fn resolve_action(
        &self,
        qualified: &str,
    ) -> Result<(String, herdr_process_domain::Action)> {
        bindings::resolve_action(qualified, &self.profiles)
    }
    pub fn keymap(&self) -> &Keymap {
        &self.keymap
    }
    pub fn profiles(&self) -> &BTreeMap<String, Profile> {
        &self.profiles
    }
}
#[cfg(test)]
mod tests;
