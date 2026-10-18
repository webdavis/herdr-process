use anyhow::{Result, bail, ensure};

const NAMED: &[(&str, char)] = &[
    ("space", ' '),
    ("enter", '\r'),
    ("return", '\r'),
    ("esc", '\x1b'),
    ("escape", '\x1b'),
    ("tab", '\t'),
    ("backspace", '\x7f'),
    ("bs", '\x7f'),
    ("minus", '-'),
    ("comma", ','),
    ("period", '.'),
    ("slash", '/'),
    ("backslash", '\\'),
    ("quote", '\''),
    ("double_quote", '"'),
    ("double-quote", '"'),
    ("semicolon", ';'),
    ("colon", ':'),
    ("percent", '%'),
    ("ampersand", '&'),
    ("backtick", '`'),
    ("plus", '+'),
];
const CURSOR: &[(&str, char)] = &[("up", 'A'), ("down", 'B'), ("right", 'C'), ("left", 'D')];
const FUNCTION_CODES: [u8; 8] = [15, 17, 18, 19, 20, 21, 23, 24];

#[derive(Default)]
struct Modifiers {
    ctrl: bool,
    shift: bool,
    alt: bool,
}

impl Modifiers {
    fn code(&self) -> u8 {
        1 + u8::from(self.shift) + 2 * u8::from(self.alt) + 4 * u8::from(self.ctrl)
    }
    fn csi(&self, prefix: &str, end: char, plain: String) -> Vec<u8> {
        if self.code() == 1 {
            plain
        } else {
            format!("\x1b[{prefix};{}{end}", self.code())
        }
        .into_bytes()
    }
}

pub(super) fn encode(raw: &str) -> Result<Vec<u8>> {
    let (modifiers, key) = modifiers(raw)?;
    let lower = key.to_ascii_lowercase();
    if let Some(bytes) = cursor(&lower, &modifiers) {
        return Ok(bytes);
    }
    if let Some(bytes) = function(key, &lower, &modifiers)? {
        return Ok(bytes);
    }
    character(raw, key, &lower, modifiers)
}

fn modifiers(raw: &str) -> Result<(Modifiers, &str)> {
    let mut modifiers = Modifiers::default();
    let mut key = None;
    for token in raw.split('+').map(str::trim) {
        ensure!(!token.is_empty(), "empty key token in {raw:?}");
        match token.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => modifiers.ctrl = true,
            "shift" => modifiers.shift = true,
            "alt" | "option" | "meta" => modifiers.alt = true,
            "cmd" | "command" | "super" | "hyper" => {
                bail!("unsupported legacy modifier in {raw:?}")
            }
            _ => {
                ensure!(key.is_none(), "multiple key names in {raw:?}");
                key = Some(token);
            }
        }
    }
    let key = key.ok_or_else(|| anyhow::anyhow!("missing key in {raw:?}"))?;
    Ok((modifiers, key))
}

fn cursor(lower: &str, modifiers: &Modifiers) -> Option<Vec<u8>> {
    let end = lookup(CURSOR, lower)?;
    Some(modifiers.csi("1", end, format!("\x1b[{end}")))
}

fn function(key: &str, lower: &str, modifiers: &Modifiers) -> Result<Option<Vec<u8>>> {
    if lower.len() < 2 || !lower.starts_with('f') {
        return Ok(None);
    }
    let n: usize = lower[1..].parse()?;
    ensure!(
        (1..=12).contains(&n),
        "unsupported legacy function key {key}"
    );
    Ok(Some(if n <= 4 {
        let end = char::from(b'P' + n as u8 - 1);
        modifiers.csi("1", end, format!("\x1bO{end}"))
    } else {
        let code = FUNCTION_CODES[n - 5].to_string();
        modifiers.csi(&code, '~', format!("\x1b[{code}~"))
    }))
}

fn character(raw: &str, key: &str, lower: &str, mut modifiers: Modifiers) -> Result<Vec<u8>> {
    let ch = match lookup(NAMED, lower) {
        Some(ch) => ch,
        None => {
            let mut chars = key.chars();
            let ch = chars.next().ok_or_else(|| anyhow::anyhow!("empty key"))?;
            ensure!(
                chars.next().is_none() && !ch.is_control(),
                "unsupported native key {key:?}"
            );
            ch
        }
    };
    modifiers.shift |= ch.is_ascii_uppercase();
    let mut bytes = if ch == '\t' && modifiers.shift && !modifiers.ctrl && !modifiers.alt {
        b"\x1b[Z".to_vec()
    } else if modifiers.ctrl {
        ensure!(
            !modifiers.shift && ch.is_ascii_lowercase() && !matches!(ch, 'i' | 'm'),
            "ambiguous or unsupported legacy Ctrl chord {raw:?}"
        );
        vec![ch as u8 - b'a' + 1]
    } else if modifiers.shift {
        ensure!(
            ch.is_ascii_alphabetic(),
            "ambiguous legacy Shift chord {raw:?}"
        );
        vec![ch.to_ascii_uppercase() as u8]
    } else {
        ch.to_string().into_bytes()
    };
    if modifiers.alt {
        bytes.insert(0, 27);
    }
    Ok(bytes)
}

fn lookup(table: &[(&str, char)], name: &str) -> Option<char> {
    table
        .iter()
        .find(|(candidate, _)| *candidate == name)
        .map(|(_, ch)| *ch)
}
