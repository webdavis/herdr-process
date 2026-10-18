use std::time::{Duration, Instant};

const SHOWN_FOR: Duration = Duration::from_secs(5);
const RED: &str = "\x1b[38;2;243;139;168m";

pub(super) struct Notice {
    message: String,
    until: Instant,
}

impl Notice {
    pub fn new(message: String, now: Instant) -> Self {
        Self {
            message,
            until: now + SHOWN_FOR,
        }
    }
    pub fn render(&self, now: Instant, (rows, cols): (u16, u16)) -> Vec<u8> {
        if now >= self.until {
            return Vec::new();
        }
        let text: String = self.message.chars().take(cols.into()).collect();
        format!("\x1b7\x1b[{rows};1H\x1b[2K{RED}{text}\x1b[0m\x1b8").into_bytes()
    }
}
