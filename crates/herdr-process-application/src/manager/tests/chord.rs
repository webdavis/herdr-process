use super::*;

#[test]
fn a_failed_in_pane_chord_reports_the_error_to_the_pane_it_came_from() {
    let mut world = World::new();
    let (controller, attached) = world.open_float("shell", "ticket-1");
    attached.take_sent();
    world
        .host
        .reply(Err(HostFailure::new(HostFailureCode::Rejected)));
    attached.push(Request::Input {
        bytes: vec![1, b'r'],
    });
    world.tick();
    assert_eq!(
        attached.sent(),
        vec![Sent::Error(
            "shell split-right: Herdr host call failed (Rejected)".into()
        )]
    );
    assert_eq!(controller.sent(), vec![Sent::Ack]);
    assert_eq!(world.host.opened().len(), 2);
    assert_eq!(world.processes.process(0).written(), b"");
}
