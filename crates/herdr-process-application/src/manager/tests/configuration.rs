use super::*;

#[test]
fn an_action_from_a_changed_configuration_says_to_kill_the_processes() {
    let mut world = World::new();
    let controller = world.connect(vec![Request::Action {
        profile: "shell".into(),
        action: Action::ToggleFloat,
        configuration: "config-2".into(),
        target: target("w1", "p1"),
    }]);
    world.tick();
    assert_eq!(
        controller.sent(),
        vec![Sent::Error(
            "configuration changed; kill the running processes to load it".into()
        )]
    );
    assert_eq!(world.processes.commands(), vec![]);
}
