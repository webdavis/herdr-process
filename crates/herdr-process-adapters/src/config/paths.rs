use std::path::{Path, PathBuf};
#[derive(Debug, PartialEq, Eq)]
pub struct ConfigurationPaths {
    pub profiles: PathBuf,
    pub herdr: PathBuf,
}
pub fn resolve_configuration_paths(
    profiles_override: Option<&Path>,
    herdr_override: Option<&Path>,
    xdg_config_home: Option<&Path>,
    home: &Path,
) -> ConfigurationPaths {
    let herdr = herdr_override.map(Path::to_path_buf).unwrap_or_else(|| {
        xdg_config_home
            .map(Path::to_path_buf)
            .unwrap_or_else(|| home.join(".config"))
            .join("herdr/config.toml")
    });
    ConfigurationPaths {
        profiles: profiles_override.map(Path::to_path_buf).unwrap_or_else(|| {
            herdr
                .parent()
                .unwrap_or(Path::new(""))
                .join("processes.toml")
        }),
        herdr,
    }
}
