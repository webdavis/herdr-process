use super::*;

#[test]
fn a_busy_host_retries_the_float_with_a_fresh_ticket_after_ten_milliseconds() {
    let mut world = World::new();
    world
        .host
        .reply(Err(HostFailure::new(HostFailureCode::UiBusy)));
    world.host.reply(Ok(HostReply::Opened(None)));
    let controller = world.action("shell", Action::ToggleFloat, target("w1", "p1"));
    world.tick();
    assert_eq!(world.host.opened().len(), 1);
    world.advance(9);
    assert_eq!(world.host.opened().len(), 1);
    world.advance(1);
    let tickets: Vec<_> = world
        .host
        .opened()
        .iter()
        .map(|v| v.ticket.clone())
        .collect();
    assert_eq!(tickets, vec!["ticket-1".to_owned(), "ticket-2".to_owned()]);
    assert_eq!(controller.sent(), vec![]);
    let attached = world.attach("shell", "ticket-2");
    world.tick();
    assert_eq!(attached.sent().last(), Some(&Sent::Ack));
    assert_eq!(controller.sent(), vec![Sent::Ack]);
}

#[test]
fn a_busy_host_fails_a_split_at_once() {
    let mut world = World::new();
    world
        .host
        .reply(Err(HostFailure::new(HostFailureCode::UiBusy)));
    let controller = world.action("shell", Action::SplitRight, target("w1", "p1"));
    world.tick();
    assert_eq!(
        controller.sent(),
        vec![Sent::Error(
            "shell split-right: Herdr host call failed (UiBusy)".into()
        )]
    );
    assert_eq!(world.host.opened().len(), 1);
}

#[test]
fn a_rejected_host_call_fails_the_float() {
    let mut world = World::new();
    world
        .host
        .reply(Err(HostFailure::new(HostFailureCode::Rejected)));
    let controller = world.action("shell", Action::ToggleFloat, target("w1", "p1"));
    world.tick();
    assert_eq!(
        controller.sent(),
        vec![Sent::Error(
            "shell toggle-float: Herdr host call failed (Rejected)".into()
        )]
    );
}
