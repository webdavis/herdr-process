use super::*;

#[test]
fn opening_a_float_retires_the_floating_view_of_another_profile_first() {
    let mut world = World::new();
    let (_, attached) = world.open_float("shell", "ticket-1");
    attached.take_sent();
    world.host.reply(Ok(HostReply::Opened(None)));
    let controller = world.action("quit", Action::ToggleFloat, target("w1", "p1"));
    world.tick();
    assert_eq!(attached.sent(), vec![Sent::Retire]);
    assert_eq!(world.host.opened().len(), 1);
    world.tick();
    assert_eq!(world.host.opened().len(), 1);
    attached.close();
    world.tick();
    assert_eq!(world.host.opened()[1].profile, "quit");
    assert_eq!(world.host.opened()[1].ticket, "ticket-2");
    let replacement = world.attach("quit", "ticket-2");
    world.tick();
    assert_eq!(replacement.sent().last(), Some(&Sent::Ack));
    assert_eq!(controller.sent(), vec![Sent::Ack]);
    assert_eq!(world.processes.commands().len(), 2);
}

#[test]
fn a_retired_view_that_never_disconnects_times_the_replacement_out() {
    let mut world = World::new();
    let (_, attached) = world.open_float("shell", "ticket-1");
    world.host.reply(Ok(HostReply::Opened(None)));
    let controller = world.action("quit", Action::ToggleFloat, target("w1", "p1"));
    world.tick();
    world.advance(600);
    assert_eq!(
        controller.sent(),
        vec![Sent::Error(
            "quit toggle-float: view replacement timed out; retry".into()
        )]
    );
    assert_eq!(world.host.opened().len(), 1);
    assert!(attached.sent().contains(&Sent::Retire));
}
