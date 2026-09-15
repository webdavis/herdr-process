use super::{CtrlC, Profile};
use anyhow::{Result, ensure};
use serde::Deserialize;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Profiles {
    windows: BTreeMap<String, RawProfile>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawProfile {
    program: String,
    #[serde(default)]
    args: Vec<String>,
    cwd: String,
    width: Percentage,
    height: Percentage,
    ctrl_c: CtrlC,
}
#[derive(Deserialize)]
#[serde(untagged)]
enum Percentage {
    Text(String),
    Integer(i64),
}
impl Percentage {
    fn value(self) -> Result<u8> {
        let number = match self {
            Self::Integer(n) => n,
            Self::Text(s) => {
                let digits = s
                    .strip_suffix('%')
                    .filter(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
                    .ok_or_else(|| anyhow::anyhow!("expected an integer percentage such as 80%"))?;
                digits.parse()?
            }
        };
        ensure!(
            (1..=100).contains(&number),
            "percentage must be between 1 and 100"
        );
        Ok(number as u8)
    }
}
pub(super) fn parse(text: &str, home: &Path) -> Result<BTreeMap<String, Profile>> {
    let raw: Profiles = toml::from_str(text)?;
    raw.windows
        .into_iter()
        .map(|(name, profile)| {
            let profile = profile.resolve(&name, home)?;
            Ok((name, profile))
        })
        .collect()
}
impl RawProfile {
    fn resolve(self, name: &str, home: &Path) -> Result<Profile> {
        ensure!(
            !name.is_empty()
                && name.len() <= 107
                && name
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-'),
            "profile name {name:?} must use ASCII letters, digits, underscore or hyphen and fit a 120-character action ID"
        );
        ensure!(
            !self.program.trim().is_empty(),
            "profile {name}: program must not be empty"
        );
        ensure!(
            !self.program.contains('\0')
                && !self.cwd.contains('\0')
                && !self.args.iter().any(|a| a.contains('\0')),
            "profile {name}: program, cwd and args must not contain NUL"
        );
        let cwd = if let Some(relative) = self.cwd.strip_prefix("~/") {
            home.join(relative)
        } else {
            PathBuf::from(&self.cwd)
        };
        ensure!(
            cwd.is_absolute(),
            "profile {name}: cwd must be absolute or start with ~/"
        );
        Ok(Profile {
            program: self.program,
            args: self.args,
            cwd,
            width: self.width.value()?,
            height: self.height.value()?,
            ctrl_c: self.ctrl_c,
        })
    }
}
