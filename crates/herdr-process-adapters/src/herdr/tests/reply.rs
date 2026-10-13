use super::*;

fn failure(expected: Expected, json: &str) -> HostFailure {
    expected.decode(json.as_bytes()).unwrap_err()
}

#[test]
fn popup_opens_without_a_pane_and_split_returns_its_pane() {
    assert_eq!(
        Expected::Popup.decode(POPUP.as_bytes()),
        Ok(HostReply::Opened(None))
    );
    assert_eq!(
        Expected::Split.decode(SPLIT.as_bytes()),
        Ok(HostReply::Opened(Some("pane-7".into())))
    );
}

#[test]
fn focus_reply_checks_identity() {
    let response = SPLIT.replace("plugin_pane_opened", "plugin_pane_focused");
    let focus = || Expected::Focus("pane-7".into());
    assert_eq!(focus().decode(response.as_bytes()), Ok(HostReply::Focused));
    for wrong in [
        response.replace("pane-7", "pane-8"),
        response.replace("attach", "other"),
        response.replace("herdr-process", "other"),
    ] {
        assert_eq!(failure(focus(), &wrong).code, HostFailureCode::Malformed);
    }
}

#[test]
fn busy_is_typed_and_diagnostics_are_private() {
    let json = r#"{"id":"r","error":{"code":"ui_busy","message":"password=secret"}}"#;
    let failure = failure(Expected::Popup, json);
    assert!(failure.is_busy());
    assert!(!format!("{failure:?} {failure}").contains("secret"));
}

#[test]
fn rejected_json_is_typed_without_secret_text() {
    let json = r#"{"id":"r","error":{"code":"denied","message":"secret"}}"#;
    let failure = failure(Expected::Popup, json);
    assert_eq!(failure.code, HostFailureCode::Rejected);
    assert!(!format!("{failure:?}").contains("secret"));
}

#[test]
fn rejects_malformed_wrong_shape_and_wrong_identity() {
    for json in [
        "secret",
        "{}",
        r#"{"id":"r","result":{"type":"plugin_pane_focused"}}"#,
        r#"{"id":"r","result":{"type":"ok"},"error":{"code":"ui_busy","message":"x"}}"#,
    ] {
        assert_eq!(
            failure(Expected::Popup, json).code,
            HostFailureCode::Malformed
        );
    }
    for json in [
        SPLIT.replace("herdr-process", "other"),
        SPLIT.replace("pane_id", "id"),
        POPUP.into(),
    ] {
        assert_eq!(
            failure(Expected::Split, &json).code,
            HostFailureCode::Malformed
        );
    }
}
