mod paths;
pub use paths::{ConfigurationPaths, resolve_configuration_paths};
mod bindings;
mod keys;
mod profiles;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CtrlC {
    Hide,
    Quit,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Profile {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub width: u8,
    pub height: u8,
    pub ctrl_c: CtrlC,
}
#[derive(Debug)]
pub struct Configuration {
    profiles: BTreeMap<String, Profile>,
    prefix: Vec<u8>,
    bindings: Vec<herdr_process_domain::Binding>,
    identity: String,
}
impl Configuration {
    pub(super) fn parse(profiles: &str, herdr: &str, home: &Path) -> Result<Self> {
        let profiles = profiles::parse(profiles, home)?;
        let (prefix, bindings) = bindings::parse(herdr, &profiles)?;
        let mapping: Vec<_> = bindings
            .iter()
            .map(|b| (&b.chord, &b.profile, b.action.to_string()))
            .collect();
        let identity = serde_json::to_string(&(&profiles, herdr, &prefix, mapping))?;
        Ok(Self {
            profiles,
            prefix,
            bindings,
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
    pub fn prefix(&self) -> &[u8] {
        &self.prefix
    }
    pub fn bindings(&self) -> &[herdr_process_domain::Binding] {
        &self.bindings
    }
    pub fn profiles(&self) -> &BTreeMap<String, Profile> {
        &self.profiles
    }
}
#[cfg(test)]
mod tests;
