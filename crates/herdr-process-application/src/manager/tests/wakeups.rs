use super::*;

#[test]
fn an_idle_attached_manager_has_no_wakeup_deadline() {
    let mut world = World::new();
    world.open_float("shell", "ticket-1");
    world.tick();
    assert_eq!(world.manager.timeout(), None);
}

#[test]
fn a_pending_chord_wakes_the_manager_when_it_times_out() {
    let mut world = World::new();
    let (_, attached) = world.open_float("shell", "ticket-1");
    attached.push(Request::Input { bytes: vec![1] });
    world.tick();
    assert_eq!(world.manager.timeout(), Some(Duration::from_millis(100)));
    world.advance(100);
    assert_eq!(world.manager.timeout(), None);
}

#[test]
fn a_retiring_client_wakes_the_manager_when_its_grace_ends() {
    let mut world = World::new();
    let (_, attached) = world.open_float("shell", "ticket-1");
    attached.push(Request::Detach {});
    world.tick();
    assert_eq!(world.manager.timeout(), Some(Duration::from_millis(500)));
    world.advance(500);
    assert_eq!(world.manager.timeout(), None);
}
