use super::{Profile, keys};
use anyhow::{Context, Result, ensure};
use herdr_process_application::InputRouter;
use herdr_process_domain::{Action, Binding};
use std::{collections::BTreeMap, time::Duration};

pub(super) fn parse(
    text: &str,
    profiles: &BTreeMap<String, Profile>,
) -> Result<(Vec<u8>, Vec<Binding>)> {
    let root: toml::Value = toml::from_str(text)?;
    let table = root
        .get("keys")
        .and_then(toml::Value::as_table)
        .context("missing Herdr [keys] table and prefix")?;
    let prefix = keys::encode(
        table
            .get("prefix")
            .and_then(toml::Value::as_str)
            .context("missing Herdr keys.prefix")?,
    )?;
    let mut bindings = Vec::new();
    let mut registered = BTreeMap::new();
    registered.insert((false, prefix.clone()), "keys.prefix".to_string());
    for (field, value) in table {
        if matches!(field.as_str(), "prefix" | "command") {
            continue;
        }
        if field == "indexed" {
            for (_, modifiers) in value.as_table().context("keys.indexed must be a table")? {
                let modifiers = modifiers
                    .as_str()
                    .context("indexed modifiers must be a string")?;
                if modifiers.trim().is_empty() {
                    continue;
                }
                for n in 1..=9 {
                    register(&format!("{modifiers}+{n}"), &mut registered)?;
                }
            }
        } else {
            for label in labels(value)? {
                // Navigate mode has its own native registry.
                if field.starts_with("navigate_") {
                    continue;
                }
                if label.contains("1..9") {
                    ensure!(
                        matches!(
                            field.as_str(),
                            "switch_tab" | "switch_workspace" | "focus_agent"
                        ),
                        "range only allowed for indexed native actions"
                    );
                    for n in 1..=9 {
                        register(&label.replace("1..9", &n.to_string()), &mut registered)?;
                    }
                } else {
                    register(label, &mut registered)?;
                }
            }
        }
    }
    if let Some(commands) = table.get("command") {
        for command in commands
            .as_array()
            .context("keys.command must be an array of tables")?
        {
            let command_text = command
                .get("command")
                .and_then(toml::Value::as_str)
                .context("keys.command command must be a string")?
                .trim();
            ensure!(!command_text.is_empty(), "custom command must not be empty");
            if let Some(kind) = command.get("type") {
                ensure!(
                    matches!(
                        kind.as_str(),
                        Some("shell" | "pane" | "popup" | "plugin_action")
                    ),
                    "unsupported Herdr command type"
                );
            }
            let own = command_text.starts_with("herdr-process.");
            let action = if own {
                ensure!(
                    command.get("type").and_then(toml::Value::as_str) == Some("plugin_action"),
                    "owned binding must have type='plugin_action'"
                );
                Some(resolve_action(command_text, profiles)?)
            } else {
                None
            };
            let labels = labels(command.get("key").context("command is missing key")?)?;
            ensure!(!own || !labels.is_empty(), "owned command must have a key");
            for label in labels {
                let label = label.trim();
                if label.is_empty() {
                    ensure!(!own, "owned command has an empty key");
                    continue;
                }
                let prefixed = label.strip_prefix("prefix+");
                ensure!(
                    !own || prefixed.is_some(),
                    "owned shortcut must use prefix+"
                );
                let chord = prefixed.unwrap_or(label);
                let encoded = if own {
                    Some(keys::encode(chord).with_context(|| format!("key {label:?}"))?)
                } else {
                    encode_foreign_chord(chord)
                };
                let Some(bytes) = encoded else {
                    continue;
                };
                ensure!(
                    registered
                        .insert((prefixed.is_some(), bytes.clone()), label.to_string())
                        .is_none(),
                    "duplicate or conflicting native chord {label:?}"
                );
                if let Some((profile, action)) = &action {
                    bindings.push(Binding {
                        chord: bytes,
                        profile: profile.clone(),
                        action: *action,
                    });
                }
            }
        }
    }
    InputRouter::new(prefix.clone(), bindings.clone(), Duration::from_millis(100))?;
    Ok((prefix, bindings))
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
        .context("use herdr-process.<profile>:<action>; Herdr rejects dots in local action IDs")?;
    ensure!(
        profiles.contains_key(profile),
        "unknown process profile {profile:?}"
    );
    Ok((profile.to_string(), action.parse()?))
}

/// A chord the legacy encoder cannot represent is one Herdr's own legacy encoder cannot represent
/// either, so Herdr binds it through its native (CSI-u) path, whose byte sequences no legacy chord
/// encoding equals. Such a chord cannot collide with this plugin's prefix-mode byte matching, so it
/// is not a conflict. Only a chord this plugin owns must still fail loudly.
fn encode_foreign_chord(chord: &str) -> Option<Vec<u8>> {
    keys::encode(chord).ok()
}

fn register(label: &str, registered: &mut BTreeMap<(bool, Vec<u8>), String>) -> Result<()> {
    let label = label.trim();
    if label.is_empty() {
        return Ok(());
    }
    let prefixed = label.strip_prefix("prefix+");
    let Some(bytes) = encode_foreign_chord(prefixed.unwrap_or(label)) else {
        return Ok(());
    };
    ensure!(
        registered
            .insert((prefixed.is_some(), bytes), label.to_string())
            .is_none(),
        "duplicate or conflicting native chord {label:?}"
    );
    Ok(())
}
