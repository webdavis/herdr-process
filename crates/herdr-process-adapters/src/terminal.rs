pub struct Terminal {
    parser: vt100::Parser<Responses>,
}

impl Terminal {
    pub fn new(rows: u16, cols: u16) -> Self {
        Self {
            parser: vt100::Parser::new_with_callbacks(
                rows.max(1),
                cols.max(1),
                1000,
                Responses::default(),
            ),
        }
    }

    pub fn feed(&mut self, bytes: &[u8]) {
        self.parser.process(bytes);
    }

    pub fn resize(&mut self, rows: u16, cols: u16) {
        self.parser.screen_mut().set_size(rows.max(1), cols.max(1));
    }

    pub fn snapshot(&self) -> Vec<u8> {
        let screen = self.parser.screen();
        // The attachment owns paste framing; child input modes are projected separately.
        let mut bytes =
            b"\x1b[?2004h\x1b[?9l\x1b[?1000l\x1b[?1002l\x1b[?1003l\x1b[?1005l\x1b[?1006l".to_vec();
        bytes.extend_from_slice(if screen.application_keypad() {
            b"\x1b="
        } else {
            b"\x1b>"
        });
        bytes.extend_from_slice(if screen.application_cursor() {
            b"\x1b[?1h"
        } else {
            b"\x1b[?1l"
        });
        bytes.extend_from_slice(match screen.mouse_protocol_mode() {
            vt100::MouseProtocolMode::None => b"",
            vt100::MouseProtocolMode::Press => b"\x1b[?9h",
            vt100::MouseProtocolMode::PressRelease => b"\x1b[?1000h",
            vt100::MouseProtocolMode::ButtonMotion => b"\x1b[?1002h",
            vt100::MouseProtocolMode::AnyMotion => b"\x1b[?1003h",
        });
        bytes.extend_from_slice(match screen.mouse_protocol_encoding() {
            vt100::MouseProtocolEncoding::Default => b"",
            vt100::MouseProtocolEncoding::Utf8 => b"\x1b[?1005h",
            vt100::MouseProtocolEncoding::Sgr => b"\x1b[?1006h",
        });
        bytes.extend(screen.contents_formatted());
        bytes
    }

    pub fn take_responses(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.parser.callbacks_mut().bytes)
    }
}

#[derive(Default)]
struct Responses {
    bytes: Vec<u8>,
}

impl vt100::Callbacks for Responses {
    fn unhandled_csi(
        &mut self,
        screen: &mut vt100::Screen,
        first: Option<u8>,
        second: Option<u8>,
        params: &[&[u16]],
        command: char,
    ) {
        if first.is_some() || second.is_some() || command != 'n' || params.len() != 1 {
            return;
        }
        match params[0] {
            [5] => self.bytes.extend_from_slice(b"\x1b[0n"),
            [6] => {
                let (row, col) = screen.cursor_position();
                self.bytes
                    .extend_from_slice(format!("\x1b[{};{}R", row + 1, col + 1).as_bytes());
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests;
