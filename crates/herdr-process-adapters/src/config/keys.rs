use anyhow::{Result, bail, ensure};

// Legacy encodings verified against Herdr b99002ac input/{parse,encode}.rs.
pub(super) fn encode(raw: &str) -> Result<Vec<u8>> {
    let (mut ctrl, mut shift, mut alt) = (false, false, false);
    let mut key = None;
    for token in raw.split('+').map(str::trim) {
        ensure!(!token.is_empty(), "empty key token in {raw:?}");
        match token.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => ctrl = true,
            "shift" => shift = true,
            "alt" | "option" | "meta" => alt = true,
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
    let lower = key.to_ascii_lowercase();
    let modifier = 1 + u8::from(shift) + 2 * u8::from(alt) + 4 * u8::from(ctrl);
    let special = match lower.as_str() {
        "up" => Some('A'),
        "down" => Some('B'),
        "right" => Some('C'),
        "left" => Some('D'),
        _ => None,
    };
    if let Some(end) = special {
        return Ok(if modifier == 1 {
            format!("\x1b[{end}")
        } else {
            format!("\x1b[1;{modifier}{end}")
        }
        .into_bytes());
    }
    if lower.len() > 1 && lower.starts_with('f') {
        let n: usize = lower[1..].parse()?;
        ensure!(
            (1..=12).contains(&n),
            "unsupported legacy function key {key}"
        );
        if n <= 4 {
            let end = char::from(b'P' + n as u8 - 1);
            return Ok(if modifier == 1 {
                format!("\x1bO{end}")
            } else {
                format!("\x1b[1;{modifier}{end}")
            }
            .into_bytes());
        }
        let code = [15, 17, 18, 19, 20, 21, 23, 24][n - 5];
        return Ok(if modifier == 1 {
            format!("\x1b[{code}~")
        } else {
            format!("\x1b[{code};{modifier}~")
        }
        .into_bytes());
    }
    let ch = match lower.as_str() {
        "space" => ' ',
        "enter" | "return" => '\r',
        "esc" | "escape" => '\x1b',
        "tab" => '\t',
        "backspace" | "bs" => '\x7f',
        "minus" => '-',
        "comma" => ',',
        "period" => '.',
        "slash" => '/',
        "backslash" => '\\',
        "quote" => '\'',
        "double_quote" | "double-quote" => '"',
        "semicolon" => ';',
        "colon" => ':',
        "percent" => '%',
        "ampersand" => '&',
        "backtick" => '`',
        "plus" => '+',
        _ => {
            let mut chars = key.chars();
            let ch = chars.next().ok_or_else(|| anyhow::anyhow!("empty key"))?;
            ensure!(
                chars.next().is_none() && !ch.is_control(),
                "unsupported native key {key:?}"
            );
            ch
        }
    };
    shift |= ch.is_ascii_uppercase();
    let mut bytes = if ch == '\t' && shift && !ctrl && !alt {
        b"\x1b[Z".to_vec()
    } else if ctrl {
        ensure!(
            !shift && ch.is_ascii_lowercase() && !matches!(ch, 'i' | 'm'),
            "ambiguous or unsupported legacy Ctrl chord {raw:?}"
        );
        vec![ch as u8 - b'a' + 1]
    } else if shift {
        ensure!(
            ch.is_ascii_alphabetic(),
            "ambiguous legacy Shift chord {raw:?}"
        );
        vec![ch.to_ascii_uppercase() as u8]
    } else {
        ch.to_string().into_bytes()
    };
    if alt {
        bytes.insert(0, 27);
    }
    Ok(bytes)
}
