use crate::Action;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Command {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: PathBuf,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CtrlC {
    Hide,
    Quit,
}

impl CtrlC {
    pub fn action(self) -> Action {
        match self {
            Self::Hide => Action::ToggleFloat,
            Self::Quit => Action::Kill,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Profile {
    #[serde(flatten)]
    pub command: Command,
    pub width: u8,
    pub height: u8,
    pub ctrl_c: CtrlC,
}
