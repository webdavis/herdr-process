use herdr_process_domain::{Action, Direction, Placement, Target};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewIntent {
    OpenFloat,
    OpenSplit(Direction),
    Focus,
    Hide,
    Kill,
}

pub fn view_intent(action: Action, current: Option<&Placement>, target: &Target) -> ViewIntent {
    let direction = match action {
        Action::Kill => return ViewIntent::Kill,
        Action::ToggleFloat => {
            return if current == Some(&Placement::Floating) {
                ViewIntent::Hide
            } else {
                ViewIntent::OpenFloat
            };
        }
        Action::SplitRight => Direction::Right,
        Action::SplitBelow => Direction::Below,
    };
    if let Some(Placement::Docked {
        target: anchor,
        direction: existing,
        pane,
    }) = current
        && *existing == direction
        && anchor.workspace == target.workspace
        && (anchor.pane == target.pane || *pane == target.pane)
    {
        return ViewIntent::Focus;
    }
    ViewIntent::OpenSplit(direction)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target() -> Target {
        Target {
            workspace: "work".into(),
            pane: "source".into(),
        }
    }

    #[test]
    fn actions_start_hidden_sessions_in_the_requested_orientation() {
        assert_eq!(
            view_intent(Action::SplitRight, None, &target()),
            ViewIntent::OpenSplit(Direction::Right)
        );
        assert_eq!(
            view_intent(Action::SplitBelow, None, &target()),
            ViewIntent::OpenSplit(Direction::Below)
        );
        assert_eq!(
            view_intent(Action::ToggleFloat, None, &target()),
            ViewIntent::OpenFloat
        );
        assert_eq!(view_intent(Action::Kill, None, &target()), ViewIntent::Kill);
    }

    #[test]
    fn float_toggle_hides_and_either_split_docks_the_same_session() {
        let current = Some(&Placement::Floating);
        assert_eq!(
            view_intent(Action::ToggleFloat, current, &target()),
            ViewIntent::Hide
        );
        assert_eq!(
            view_intent(Action::SplitRight, current, &target()),
            ViewIntent::OpenSplit(Direction::Right)
        );
        assert_eq!(
            view_intent(Action::SplitBelow, current, &target()),
            ViewIntent::OpenSplit(Direction::Below)
        );
        assert_eq!(
            view_intent(Action::Kill, current, &target()),
            ViewIntent::Kill
        );
    }

    #[test]
    fn correct_split_focuses_but_another_orientation_or_workspace_repositions() {
        let current = Placement::Docked {
            target: target(),
            direction: Direction::Right,
            pane: "owned".into(),
        };
        assert_eq!(
            view_intent(Action::SplitRight, Some(&current), &target()),
            ViewIntent::Focus
        );
        let own = Target {
            workspace: "work".into(),
            pane: "owned".into(),
        };
        assert_eq!(
            view_intent(Action::SplitRight, Some(&current), &own),
            ViewIntent::Focus
        );
        assert_eq!(
            view_intent(Action::SplitBelow, Some(&current), &own),
            ViewIntent::OpenSplit(Direction::Below)
        );
        assert_eq!(
            view_intent(Action::ToggleFloat, Some(&current), &own),
            ViewIntent::OpenFloat
        );
        let other = Target {
            workspace: "other".into(),
            pane: "source".into(),
        };
        assert_eq!(
            view_intent(Action::SplitRight, Some(&current), &other),
            ViewIntent::OpenSplit(Direction::Right)
        );
    }
}
