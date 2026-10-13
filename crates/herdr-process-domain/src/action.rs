use serde::{Deserialize, Serialize};
use std::{fmt, str::FromStr};
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Action {
    SplitRight,
    SplitBelow,
    ToggleFloat,
    Kill,
}
impl Action {
    pub const ALL: [Action; 4] = [
        Self::SplitRight,
        Self::SplitBelow,
        Self::ToggleFloat,
        Self::Kill,
    ];
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseActionError(pub String);
impl fmt::Display for ParseActionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unknown action: {}", self.0)
    }
}
impl std::error::Error for ParseActionError {}
impl FromStr for Action {
    type Err = ParseActionError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "split-right" => Ok(Self::SplitRight),
            "split-below" => Ok(Self::SplitBelow),
            "toggle-float" => Ok(Self::ToggleFloat),
            "kill" => Ok(Self::Kill),
            _ => Err(ParseActionError(s.into())),
        }
    }
}
impl fmt::Display for Action {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::SplitRight => "split-right",
            Self::SplitBelow => "split-below",
            Self::ToggleFloat => "toggle-float",
            Self::Kill => "kill",
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn command_spellings_round_trip_and_reject_unknown_actions() {
        for (s, a) in [
            ("split-right", Action::SplitRight),
            ("split-below", Action::SplitBelow),
            ("toggle-float", Action::ToggleFloat),
            ("kill", Action::Kill),
        ] {
            assert_eq!(s.parse(), Ok(a));
            assert_eq!(a.to_string(), s);
        }
        for s in ["hide", "SplitRight", "split_right", " kill", ""] {
            assert!(s.parse::<Action>().is_err());
        }
    }
}
