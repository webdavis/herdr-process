use super::paste::START;
use herdr_process_domain::Binding;
use std::{fmt, time::Duration};
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RouterError {
    EmptyPrefix,
    InterruptConflict { binding: Option<usize> },
    ZeroTimeout,
    EmptyChord { binding: usize },
    PrefixConflict { binding: usize },
    AmbiguousBindings { first: usize, second: usize },
    PasteConflict { binding: Option<usize> },
}
impl fmt::Display for RouterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InterruptConflict { binding } => write!(
                f,
                "prefix or binding {binding:?} conflicts with ordinary Ctrl+C interrupt"
            ),
            Self::EmptyPrefix => f.write_str("input prefix must not be empty"),
            Self::ZeroTimeout => f.write_str("input timeout must be positive"),
            Self::EmptyChord { binding } => write!(f, "binding {binding} has an empty chord"),
            Self::PrefixConflict { binding } => write!(
                f,
                "binding {binding} conflicts with the literal prefix escape"
            ),
            Self::AmbiguousBindings { first, second } => write!(
                f,
                "bindings {first} and {second} have duplicate or ambiguous byte encodings"
            ),
            Self::PasteConflict {
                binding: Some(binding),
            } => write!(
                f,
                "binding {binding} conflicts with bracketed paste detection"
            ),
            Self::PasteConflict { binding: None } => {
                f.write_str("input prefix conflicts with bracketed paste detection")
            }
        }
    }
}
impl std::error::Error for RouterError {}
fn overlaps(a: &[u8], b: &[u8]) -> bool {
    a.starts_with(b) || b.starts_with(a)
}
fn paste_conflict(bytes: &[u8]) -> bool {
    overlaps(bytes, START) || bytes.windows(START.len()).any(|window| window == START)
}
pub(super) fn validate(
    prefix: &[u8],
    bindings: &[Binding],
    timeout: Duration,
) -> Result<(), RouterError> {
    if prefix.is_empty() {
        return Err(RouterError::EmptyPrefix);
    }
    if timeout.is_zero() {
        return Err(RouterError::ZeroTimeout);
    }
    if prefix.contains(&3) {
        return Err(RouterError::InterruptConflict { binding: None });
    }
    if paste_conflict(prefix) {
        return Err(RouterError::PasteConflict { binding: None });
    }
    for (index, binding) in bindings.iter().enumerate() {
        if binding.chord.is_empty() {
            return Err(RouterError::EmptyChord { binding: index });
        }
        if binding.chord.contains(&3) {
            return Err(RouterError::InterruptConflict {
                binding: Some(index),
            });
        }
        if overlaps(prefix, &binding.chord) {
            return Err(RouterError::PrefixConflict { binding: index });
        }
        if paste_conflict(&binding.chord) {
            return Err(RouterError::PasteConflict {
                binding: Some(index),
            });
        }
        for (previous, other) in bindings[..index].iter().enumerate() {
            if overlaps(&binding.chord, &other.chord) {
                return Err(RouterError::AmbiguousBindings {
                    first: previous,
                    second: index,
                });
            }
        }
    }
    Ok(())
}
