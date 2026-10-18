use super::*;

#[test]
fn toggle_float_spawns_the_profile_and_opens_a_floating_view() {
    let mut world = World::new();
    world.host.reply(Ok(HostReply::Opened(None)));
    let controller = world.action("shell", Action::ToggleFloat, target("w1", "p1"));
    world.tick();
    assert_eq!(
        world.processes.commands(),
        vec![herdr_process_domain::Command {
            program: "/bin/sh".into(),
            args: vec!["-l".into()],
            cwd: "/home".into(),
        }]
    );
    assert_eq!(
        world.host.opened(),
        vec![OpenView {
            profile: "shell".into(),
            ticket: "ticket-1".into(),
            manager_socket: "/run/manager.sock".into(),
            cwd: "/home".into(),
            width: 80,
            height: 70,
            split: None,
        }]
    );
    assert_eq!(controller.sent(), vec![]);
    world.processes.process(0).prints(b"$ ");
    let attached = world.attach("shell", "ticket-1");
    world.tick();
    assert_eq!(
        attached.sent(),
        vec![
            Sent::Screen(vec![]),
            Sent::Attached,
            Sent::Screen(b"$ ".to_vec()),
            Sent::Ack
        ]
    );
    assert_eq!(controller.sent(), vec![Sent::Ack]);
    assert_eq!(world.processes.process(0).size(), (24, 80));
}

#[test]
fn toggle_float_on_a_floating_view_hides_it_and_a_third_toggle_reopens_it() {
    let mut world = World::new();
    let (_, attached) = world.open_float("shell", "ticket-1");
    attached.take_sent();
    let controller = world.action("shell", Action::ToggleFloat, target("w1", "p1"));
    world.tick();
    assert_eq!(attached.sent(), vec![Sent::Retire]);
    assert_eq!(controller.sent(), vec![Sent::Ack]);
    assert_eq!(world.host.opened().len(), 1);
    assert!(!world.processes.process(0).terminated());
    attached.close();
    world.host.reply(Ok(HostReply::Opened(None)));
    world.action("shell", Action::ToggleFloat, target("w1", "p1"));
    world.tick();
    assert_eq!(world.host.opened()[1].ticket, "ticket-2");
    assert_eq!(world.processes.commands().len(), 1);
}

#[test]
fn an_attachment_with_the_wrong_ticket_is_refused() {
    let mut world = World::new();
    world.host.reply(Ok(HostReply::Opened(None)));
    world.action("shell", Action::ToggleFloat, target("w1", "p1"));
    world.tick();
    let attached = world.attach("shell", "ticket-9");
    world.tick();
    assert_eq!(
        attached.sent(),
        vec![
            Sent::Error("invalid attachment capability".into()),
            Sent::Error("attachment is not awaiting readiness".into())
        ]
    );
}
