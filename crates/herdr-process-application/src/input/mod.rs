mod validation;
pub use validation::RouterError;
mod paste;
use herdr_process_domain::{Action, Binding};
use paste::{Paste, PasteInput};
use std::time::Duration;
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Effect {
    Forward(Vec<u8>),
    Interrupt,
    Invoke { profile: String, action: Action },
}
pub struct InputRouter {
    paste: Paste,
    prefix: Vec<u8>,
    bindings: Vec<Binding>,
    pending: Vec<u8>,
    timeout: Duration,
    last: Duration,
}
impl InputRouter {
    pub fn new(
        prefix: Vec<u8>,
        bindings: Vec<Binding>,
        timeout: Duration,
    ) -> Result<Self, RouterError> {
        validation::validate(&prefix, &bindings, timeout)?;
        Ok(Self {
            paste: Paste::default(),
            prefix,
            bindings,
            pending: Vec::new(),
            timeout,
            last: Duration::ZERO,
        })
    }
    pub fn feed(&mut self, bytes: &[u8], now: Duration) -> Vec<Effect> {
        let mut effects = self.expire(now);
        if !bytes.is_empty() {
            self.last = now;
        }
        for &byte in bytes {
            match self.paste.push(byte) {
                PasteInput::Hold => {}
                PasteInput::Keys(keys) => {
                    if keys == [3] && self.pending.is_empty() {
                        effects.push(Effect::Interrupt);
                    } else {
                        for key in keys {
                            self.route(key, &mut effects);
                        }
                    }
                }
                PasteInput::Start(bytes) => {
                    forward(&mut effects, &std::mem::take(&mut self.pending));
                    forward(&mut effects, &bytes);
                }
                PasteInput::Data(byte) => forward(&mut effects, &[byte]),
            }
        }
        effects
    }
    pub fn expire(&mut self, now: Duration) -> Vec<Effect> {
        let mut effects = Vec::new();
        if now.saturating_sub(self.last) >= self.timeout {
            forward(&mut effects, &std::mem::take(&mut self.pending));
            forward(&mut effects, &self.paste.flush());
        }
        effects
    }
    fn route(&mut self, byte: u8, effects: &mut Vec<Effect>) {
        self.pending.push(byte);
        loop {
            if key_prefix(&self.prefix, &self.pending) {
                return;
            }
            let prefix_len = self.prefix.len();
            if self.pending.len() >= prefix_len
                && key_prefix(&self.prefix, &self.pending[..prefix_len])
            {
                let chord = &self.pending[prefix_len..];
                if chord.len() == prefix_len && key_prefix(&self.prefix, chord) {
                    forward(effects, &self.pending[..prefix_len]);
                    self.pending.clear();
                    return;
                }
                if key_prefix(&self.prefix, chord) {
                    return;
                }
                if let Some(binding) = self
                    .bindings
                    .iter()
                    .find(|b| b.chord.len() == chord.len() && key_prefix(&b.chord, chord))
                {
                    effects.push(Effect::Invoke {
                        profile: binding.profile.clone(),
                        action: binding.action,
                    });
                    self.pending.clear();
                    return;
                }
                if self.bindings.iter().any(|b| key_prefix(&b.chord, chord)) {
                    return;
                }
            }
            forward(effects, &[self.pending.remove(0)]);
            if self.pending.is_empty() {
                return;
            }
        }
    }
}
fn key_prefix(key: &[u8], input: &[u8]) -> bool {
    key.starts_with(input)
        || (matches!(key, [27, b'[', b'A'..=b'D']) && [27, b'O', key[2]].starts_with(input))
}

fn forward(effects: &mut Vec<Effect>, bytes: &[u8]) {
    if bytes.is_empty() {
        return;
    }
    if let Some(Effect::Forward(previous)) = effects.last_mut() {
        previous.extend_from_slice(bytes);
    } else {
        effects.push(Effect::Forward(bytes.to_vec()));
    }
}
#[cfg(test)]
mod tests;
