use super::*;
#[test]
fn identity_tracks_validated_profile_and_binding_sources() {
    let home = Path::new("/private/home");
    let original = Configuration::parse(PROFILES, HERDR, home).unwrap();
    assert!(!original.identity().is_empty());
    assert_eq!(
        original.identity(),
        Configuration::parse(PROFILES, HERDR, home)
            .unwrap()
            .identity()
    );
    for changed in [
        PROFILES.replace("80%", "81%"),
        PROFILES.replace("editor", "other"),
        PROFILES.replace("hide", "quit"),
        PROFILES.replace("projects", "elsewhere"),
    ] {
        assert_ne!(
            original.identity(),
            Configuration::parse(&changed, HERDR, home)
                .unwrap()
                .identity()
        );
    }
    assert_ne!(
        original.identity(),
        Configuration::parse(PROFILES, "[keys]\nprefix='ctrl+d'", home)
            .unwrap()
            .identity()
    );
    assert_ne!(
        original.identity(),
        Configuration::parse(PROFILES, HERDR, Path::new("/private/other-home"))
            .unwrap()
            .identity()
    );
}

#[test]
fn an_unrelated_herdr_config_edit_keeps_the_identity() {
    let home = Path::new("/private/home");
    let themed = format!("{HERDR}[theme]\nname = 'catppuccin-mocha'\n");
    assert_eq!(
        Configuration::parse(PROFILES, HERDR, home)
            .unwrap()
            .identity(),
        Configuration::parse(PROFILES, &themed, home)
            .unwrap()
            .identity()
    );
}
