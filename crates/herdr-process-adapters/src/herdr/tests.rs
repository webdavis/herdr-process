use super::*;
mod arguments;
mod call;
mod reply;

const POPUP: &str = r#"{"id":"request","result":{"type":"ok"}}"#;
const SPLIT: &str = r#"{"id":"request","result":{"type":"plugin_pane_opened","plugin_pane":{"plugin_id":"herdr-process","entrypoint":"attach","pane":{"pane_id":"pane-7"}}}}"#;

fn view() -> OpenView {
    OpenView {
        profile: "a profile; $HOME".into(),
        ticket: "private-ticket".into(),
        manager_socket: "/private/tmp/dotfiles-modernization/manager socket".into(),
        cwd: "/private/tmp/dotfiles-modernization/exact cwd".into(),
        width: 80,
        height: 70,
        split: None,
    }
}
