use super::validation::{KeymapError, validate};
use herdr_process_domain::Binding;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Keymap {
    prefix: Vec<u8>,
    bindings: Vec<Binding>,
}

impl Keymap {
    pub fn new(prefix: Vec<u8>, bindings: Vec<Binding>) -> Result<Self, KeymapError> {
        validate(&prefix, &bindings)?;
        Ok(Self { prefix, bindings })
    }

    pub fn prefix(&self) -> &[u8] {
        &self.prefix
    }

    pub fn bindings(&self) -> &[Binding] {
        &self.bindings
    }
}
