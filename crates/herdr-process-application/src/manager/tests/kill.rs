use super::*;

#[test]
fn kill_terminates_the_process_and_retires_the_view() {
    let mut world = World::new();
    let (_, attached) = world.open_float("shell", "ticket-1");
    attached.take_sent();
    let controller = world.action("shell", Action::Kill, target("w1", "p1"));
    world.tick();
    assert_eq!(attached.sent(), vec![Sent::Retire]);
    assert_eq!(controller.sent(), vec![Sent::Ack]);
    assert!(world.processes.process(0).terminated());
}

#[test]
fn kill_without_a_session_acks_and_starts_nothing() {
    let mut world = World::new();
    let controller = world.action("shell", Action::Kill, target("w1", "p1"));
    world.tick();
    assert_eq!(controller.sent(), vec![Sent::Ack]);
    assert_eq!(world.processes.commands(), vec![]);
    assert_eq!(world.host.opened(), vec![]);
}

#[test]
fn a_process_that_exits_retires_its_view_and_the_manager_finishes() {
    let mut world = World::new();
    let (controller, attached) = world.open_float("shell", "ticket-1");
    attached.take_sent();
    controller.close();
    world.processes.process(0).prints(b"bye");
    world.processes.process(0).exits(0);
    world.tick();
    assert_eq!(
        attached.sent(),
        vec![Sent::Screen(b"bye".to_vec()), Sent::Retire]
    );
    attached.close();
    world.tick();
    assert!(world.manager.finished());
}
