pub(super) const START: &[u8] = b"\x1b[200~";
const END: &[u8] = b"\x1b[201~";
#[derive(Default)]
pub(super) struct Paste {
    candidate: Vec<u8>,
    sent: usize,
    active: bool,
    end: usize,
}
pub(super) enum PasteInput {
    Hold,
    Keys(Vec<u8>),
    Start(Vec<u8>),
    Data(u8),
}
impl Paste {
    pub(super) fn push(&mut self, byte: u8) -> PasteInput {
        if self.active {
            self.end = if byte == END[self.end] {
                self.end + 1
            } else {
                usize::from(byte == END[0])
            };
            if self.end == END.len() {
                self.active = false;
                self.end = 0;
            }
            return PasteInput::Data(byte);
        }
        self.candidate.push(byte);
        if self.candidate == START {
            self.active = true;
            let remaining = self.candidate[self.sent..].to_vec();
            self.candidate.clear();
            self.sent = 0;
            return PasteInput::Start(remaining);
        }
        if START.starts_with(&self.candidate) {
            return PasteInput::Hold;
        }
        let keep = usize::from(byte == START[0]);
        let count = self.candidate.len() - keep;
        let remaining = self.candidate[self.sent..count].to_vec();
        self.candidate.drain(..count);
        self.sent = 0;
        PasteInput::Keys(remaining)
    }
    pub(super) fn flush(&mut self) -> Vec<u8> {
        let remaining = self.candidate[self.sent..].to_vec();
        self.sent = self.candidate.len();
        remaining
    }
}
