use super::keys;
use anyhow::{Context, Result, ensure};
use herdr_process_application::Keymap;
use herdr_process_domain::{Action, Binding, Profile};
use registry::Registry;
use std::collections::BTreeMap;
mod registry;

enum Entry<'a> {
    Owned {
        profile: String,
        action: Action,
        keys: Vec<&'a str>,
    },
    Foreign {
        keys: Vec<&'a str>,
    },
}

pub(super) fn parse(text: &str, profiles: &BTreeMap<String, Profile>) -> Result<Keymap> {
    let root: toml::Value = toml::from_str(text)?;
    let table = root
        .get("keys")
        .and_then(toml::Value::as_table)
        .context("missing herdr [keys] table and prefix")?;
    let prefix = keys::encode(
        table
            .get("prefix")
            .and_then(toml::Value::as_str)
            .context("missing herdr keys.prefix")?,
    )?;
    let mut registered = Registry::default();
    registered.prefix(&prefix);
    for (field, value) in table {
        match field.as_str() {
            "prefix" | "command" => {}
            "indexed" => registered.indexed(value)?,
            _ => {
                let labels = labels(value)?;
                if is_navigate_mode(field) {
                    continue;
                }
                for label in labels {
                    registered.native(field, label)?;
                }
            }
        }
    }
    let mut bindings = Vec::new();
    for command in commands(table)? {
        match entry(command, profiles)? {
            Entry::Owned {
                profile,
                action,
                keys,
            } => {
                ensure!(!keys.is_empty(), "owned command must have a key");
                for label in keys {
                    let label = label.trim();
                    ensure!(!label.is_empty(), "owned command has an empty key");
                    let chord = owned_chord(label)?;
                    registered.owned(chord.clone(), label)?;
                    bindings.push(Binding {
                        chord,
                        profile: profile.clone(),
                        action,
                    });
                }
            }
            Entry::Foreign { keys } => {
                for label in keys {
                    registered.foreign(label)?;
                }
            }
        }
    }
    Ok(Keymap::new(prefix, bindings)?)
}

fn commands(table: &toml::Table) -> Result<&[toml::Value]> {
    match table.get("command") {
        Some(commands) => commands
            .as_array()
            .map(Vec::as_slice)
            .context("keys.command must be an array of tables"),
        None => Ok(&[]),
    }
}

fn entry<'a>(command: &'a toml::Value, profiles: &BTreeMap<String, Profile>) -> Result<Entry<'a>> {
    let command_text = command
        .get("command")
        .and_then(toml::Value::as_str)
        .context("keys.command command must be a string")?
        .trim();
    ensure!(!command_text.is_empty(), "custom command must not be empty");
    let kind = command.get("type").and_then(toml::Value::as_str);
    if command.get("type").is_some() {
        ensure!(
            matches!(kind, Some("shell" | "pane" | "popup" | "plugin_action")),
            "unsupported herdr command type"
        );
    }
    let key = command.get("key").context("command is missing key");
    if !command_text.starts_with("herdr-process.") {
        return Ok(Entry::Foreign {
            keys: labels(key?)?,
        });
    }
    ensure!(
        kind == Some("plugin_action"),
        "owned binding must have type='plugin_action'"
    );
    let (profile, action) = resolve_action(command_text, profiles)?;
    Ok(Entry::Owned {
        profile,
        action,
        keys: labels(key?)?,
    })
}

fn labels(value: &toml::Value) -> Result<Vec<&str>> {
    if let Some(label) = value.as_str() {
        return Ok(vec![label]);
    }
    value
        .as_array()
        .context("key must be a string or string array")?
        .iter()
        .map(|v| v.as_str().context("key array entries must be strings"))
        .collect()
}

pub(super) fn resolve_action(
    command: &str,
    profiles: &BTreeMap<String, Profile>,
) -> Result<(String, Action)> {
    let local = command
        .trim()
        .strip_prefix("herdr-process.")
        .context("expected qualified herdr-process action")?;
    let (profile, action) = local
        .split_once(':')
        .context("use herdr-process.<profile>:<action>; herdr rejects dots in local action IDs")?;
    ensure!(
        profiles.contains_key(profile),
        "unknown process profile {profile:?}"
    );
    Ok((profile.to_string(), action.parse()?))
}

fn is_navigate_mode(field: &str) -> bool {
    field.starts_with("navigate_")
}

fn owned_chord(label: &str) -> Result<Vec<u8>> {
    let chord = label
        .strip_prefix("prefix+")
        .context("owned shortcut must use prefix+")?;
    keys::encode(chord).with_context(|| format!("key {label:?}"))
}
