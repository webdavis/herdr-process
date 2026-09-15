#[derive(Clone, Debug, PartialEq, Eq)]
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
