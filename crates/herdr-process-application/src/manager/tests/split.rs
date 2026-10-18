use super::*;

#[test]
fn split_right_opens_a_docked_view_beside_the_target() {
    let mut world = World::new();
    world
        .host
        .reply(Ok(HostReply::Opened(Some("pane-9".into()))));
    let controller = world.action("shell", Action::SplitRight, target("w1", "p1"));
    world.tick();
    assert_eq!(
        world.host.opened(),
        vec![OpenView {
            profile: "shell".into(),
            ticket: "ticket-1".into(),
            manager_socket: "/run/manager.sock".into(),
            cwd: "/home".into(),
            width: 80,
            height: 70,
            split: Some((target("w1", "p1"), Direction::Right)),
        }]
    );
    let attached = world.attach("shell", "ticket-1");
    world.tick();
    assert_eq!(
        attached.sent(),
        vec![
            Sent::Screen(vec![]),
            Sent::Attached,
            Sent::Screen(vec![]),
            Sent::Ack
        ]
    );
    assert_eq!(controller.sent(), vec![Sent::Ack]);
}

#[test]
fn split_below_without_a_current_pane_is_refused_before_anything_starts() {
    let mut world = World::new();
    let controller = world.action("shell", Action::SplitBelow, target("", ""));
    world.tick();
    assert_eq!(
        controller.sent(),
        vec![Sent::Error(
            "shell split-below: split needs a workspace and pane; invoke from a current pane"
                .into()
        )]
    );
    assert_eq!(world.processes.commands(), vec![]);
    assert_eq!(world.host.opened(), vec![]);
}

#[test]
fn a_split_reply_without_a_pane_fails_the_action() {
    let mut world = World::new();
    world.host.reply(Ok(HostReply::Opened(None)));
    let controller = world.action("shell", Action::SplitRight, target("w1", "p1"));
    world.tick();
    assert_eq!(
        controller.sent(),
        vec![Sent::Error(
            "shell split-right: split did not return a pane".into()
        )]
    );
}
