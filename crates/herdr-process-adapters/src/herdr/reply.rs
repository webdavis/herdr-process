#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HostReply {
    Opened(Option<String>),
    Focused,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostFailureCode {
    UiBusy,
    Rejected,
    Spawn,
    Io,
    Exit,
    Malformed,
    Oversized,
    Timeout,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostFailure {
    pub code: HostFailureCode,
    pub message: &'static str,
}
impl HostFailure {
    pub fn is_busy(&self) -> bool {
        self.code == HostFailureCode::UiBusy
    }
    pub(super) fn new(code: HostFailureCode) -> Self {
        Self {
            code,
            message: "Herdr host call failed",
        }
    }
}
impl std::fmt::Display for HostFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({:?})", self.message, self.code)
    }
}
impl std::error::Error for HostFailure {}
pub(super) enum Expected {
    Popup,
    Split,
    Focus(String),
}

impl Expected {
    pub(super) fn decode(&self, bytes: &[u8]) -> Result<HostReply, HostFailure> {
        use serde_json::Value;
        let malformed = || HostFailure::new(HostFailureCode::Malformed);
        let envelope: Value = serde_json::from_slice(bytes).map_err(|_| malformed())?;
        let id = envelope
            .get("id")
            .and_then(Value::as_str)
            .ok_or_else(malformed)?;
        if id.is_empty() {
            return Err(malformed());
        }
        if let Some(error) = envelope.get("error") {
            if envelope.get("result").is_some() {
                return Err(malformed());
            }
            let code = error
                .get("code")
                .and_then(Value::as_str)
                .ok_or_else(malformed)?;
            error
                .get("message")
                .and_then(Value::as_str)
                .ok_or_else(malformed)?;
            return Err(HostFailure::new(if code == "ui_busy" {
                HostFailureCode::UiBusy
            } else {
                HostFailureCode::Rejected
            }));
        }
        let result = envelope.get("result").ok_or_else(malformed)?;
        let kind = result
            .get("type")
            .and_then(Value::as_str)
            .ok_or_else(malformed)?;
        match self {
            Self::Popup if kind == "ok" => Ok(HostReply::Opened(None)),
            Self::Split if kind == "plugin_pane_opened" => {
                Ok(HostReply::Opened(Some(pane_id(result)?.to_owned())))
            }
            Self::Focus(expected) if kind == "plugin_pane_focused" => {
                if pane_id(result)? != expected {
                    return Err(malformed());
                }
                Ok(HostReply::Focused)
            }
            _ => Err(malformed()),
        }
    }
}

fn pane_id(result: &serde_json::Value) -> Result<&str, HostFailure> {
    let malformed = || HostFailure::new(HostFailureCode::Malformed);
    let pane = result.get("plugin_pane").ok_or_else(malformed)?;
    if pane.get("plugin_id").and_then(serde_json::Value::as_str) != Some("herdr-process")
        || pane.get("entrypoint").and_then(serde_json::Value::as_str) != Some("attach")
    {
        return Err(malformed());
    }
    pane.get("pane")
        .and_then(|p| p.get("pane_id"))
        .and_then(serde_json::Value::as_str)
        .filter(|id| !id.is_empty() && !id.chars().any(char::is_control))
        .ok_or_else(malformed)
}
