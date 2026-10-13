use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Target {
    pub workspace: String,
    pub pane: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Right,
    Below,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Placement {
    Floating,
    Docked {
        target: Target,
        direction: Direction,
        pane: String,
    },
}

impl Placement {
    pub fn docked_at(&self, target: &Target) -> Option<&Target> {
        match self {
            Self::Docked {
                target: anchor,
                pane,
                ..
            } if anchor.workspace == target.workspace && *pane == target.pane => Some(anchor),
            _ => None,
        }
    }
}
