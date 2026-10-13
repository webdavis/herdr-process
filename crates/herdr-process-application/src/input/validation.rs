use super::paste::START;
use herdr_process_domain::Binding;
use std::fmt;
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KeymapError {
    EmptyPrefix,
    InterruptConflict { binding: Option<Binding> },
    EmptyChord { binding: Binding },
    PrefixConflict { binding: Binding },
    AmbiguousBindings { first: Binding, second: Binding },
    PasteConflict { binding: Option<Binding> },
}
impl fmt::Display for KeymapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyPrefix => f.write_str("input prefix must not be empty"),
            Self::EmptyChord { binding } => {
                write!(f, "binding {} has an empty chord", name(binding))
            }
            Self::PrefixConflict { binding } => {
                write!(f, "binding {} overlaps the prefix key", name(binding))
            }
            Self::AmbiguousBindings { first, second } => write!(
                f,
                "bindings {} and {} use the same or overlapping keys",
                name(first),
                name(second)
            ),
            Self::InterruptConflict { binding } => write!(
                f,
                "{} conflicts with ordinary Ctrl+C interrupt",
                subject(binding)
            ),
            Self::PasteConflict { binding } => write!(
                f,
                "{} conflicts with bracketed paste detection",
                subject(binding)
            ),
        }
    }
}
fn name(binding: &Binding) -> String {
    format!("{}:{}", binding.profile, binding.action)
}
fn subject(binding: &Option<Binding>) -> String {
    binding
        .as_ref()
        .map_or("input prefix".into(), |b| format!("binding {}", name(b)))
}
impl std::error::Error for KeymapError {}
fn overlaps(a: &[u8], b: &[u8]) -> bool {
    a.starts_with(b) || b.starts_with(a)
}
fn paste_conflict(bytes: &[u8]) -> bool {
    overlaps(bytes, START) || bytes.windows(START.len()).any(|window| window == START)
}
pub(super) fn validate(prefix: &[u8], bindings: &[Binding]) -> Result<(), KeymapError> {
    if prefix.is_empty() {
        return Err(KeymapError::EmptyPrefix);
    }
    if prefix.contains(&3) {
        return Err(KeymapError::InterruptConflict { binding: None });
    }
    if paste_conflict(prefix) {
        return Err(KeymapError::PasteConflict { binding: None });
    }
    for (index, binding) in bindings.iter().enumerate() {
        if binding.chord.is_empty() {
            return Err(KeymapError::EmptyChord {
                binding: binding.clone(),
            });
        }
        if binding.chord.contains(&3) {
            return Err(KeymapError::InterruptConflict {
                binding: Some(binding.clone()),
            });
        }
        if overlaps(prefix, &binding.chord) {
            return Err(KeymapError::PrefixConflict {
                binding: binding.clone(),
            });
        }
        if paste_conflict(&binding.chord) {
            return Err(KeymapError::PasteConflict {
                binding: Some(binding.clone()),
            });
        }
        if let Some(other) = bindings[..index]
            .iter()
            .find(|other| overlaps(&binding.chord, &other.chord))
        {
            return Err(KeymapError::AmbiguousBindings {
                first: other.clone(),
                second: binding.clone(),
            });
        }
    }
    Ok(())
}
