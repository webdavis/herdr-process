use super::*;

#[test]
fn split_right_on_the_docked_view_focuses_its_pane() {
    let mut world = World::new();
    let (_, attached) = world.open_split("shell", "pane-9");
    attached.take_sent();
    world.host.reply(Ok(HostReply::Focused));
    let controller = world.action("shell", Action::SplitRight, target("w1", "p1"));
    world.tick();
    assert_eq!(*world.host.focused.borrow(), vec!["pane-9".to_owned()]);
    assert_eq!(controller.sent(), vec![Sent::Ack]);
    assert_eq!(attached.sent(), vec![]);
    assert_eq!(world.host.opened().len(), 1);
}

#[test]
fn split_below_on_a_right_docked_view_opens_a_new_split_and_retires_the_old_one() {
    let mut world = World::new();
    let (_, attached) = world.open_split("shell", "pane-9");
    attached.take_sent();
    world
        .host
        .reply(Ok(HostReply::Opened(Some("pane-10".into()))));
    let controller = world.action("shell", Action::SplitBelow, target("w1", "p1"));
    world.tick();
    assert_eq!(
        world.host.opened()[1].split,
        Some((target("w1", "p1"), Direction::Below))
    );
    let replacement = world.attach("shell", "ticket-2");
    world.tick();
    assert_eq!(attached.sent(), vec![Sent::Retire]);
    assert_eq!(replacement.sent().last(), Some(&Sent::Ack));
    assert_eq!(controller.sent(), vec![Sent::Ack]);
    assert_eq!(world.processes.commands().len(), 1);
}
