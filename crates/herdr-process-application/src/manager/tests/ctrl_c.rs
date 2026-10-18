use super::*;

#[test]
fn ctrl_c_in_a_floating_view_hides_it_when_the_profile_says_hide() {
    let mut world = World::new();
    let (_, attached) = world.open_float("shell", "ticket-1");
    attached.take_sent();
    attached.push(Request::Input { bytes: vec![3] });
    world.tick();
    assert_eq!(attached.sent(), vec![Sent::Retire]);
    assert_eq!(world.processes.process(0).written(), b"");
    assert!(!world.processes.process(0).terminated());
}

#[test]
fn ctrl_c_in_a_floating_view_kills_the_process_when_the_profile_says_quit() {
    let mut world = World::new();
    let (_, attached) = world.open_float("quit", "ticket-1");
    attached.take_sent();
    attached.push(Request::Input { bytes: vec![3] });
    world.tick();
    assert_eq!(attached.sent(), vec![Sent::Retire]);
    assert!(world.processes.process(0).terminated());
}

#[test]
fn ctrl_c_in_a_docked_view_reaches_the_process() {
    let mut world = World::new();
    let (_, attached) = world.open_split("shell", "pane-9");
    attached.take_sent();
    attached.push(Request::Input { bytes: vec![3] });
    world.tick();
    world.tick();
    assert_eq!(world.processes.process(0).written(), vec![3]);
    assert_eq!(attached.sent(), vec![]);
}

#[test]
fn plain_keys_reach_the_process_in_either_placement() {
    let mut world = World::new();
    let (_, attached) = world.open_float("shell", "ticket-1");
    attached.push(Request::Input {
        bytes: b"ls\r".to_vec(),
    });
    world.tick();
    world.tick();
    assert_eq!(world.processes.process(0).written(), b"ls\r");
}
