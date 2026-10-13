use herdr_process_domain::{Action, Target};
use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Request {
    Action {
        profile: String,
        action: Action,
        configuration: String,
        target: Target,
    },
    Attach {
        profile: String,
        ticket: String,
        rows: u16,
        cols: u16,
    },
    Input {
        bytes: Vec<u8>,
    },
    Resize {
        rows: u16,
        cols: u16,
    },
    Ready {},
    Detach {},
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Response {
    Ack {},
    Error { message: String },
    Screen { bytes: Vec<u8> },
    Attached {},
    Retire {},
}
