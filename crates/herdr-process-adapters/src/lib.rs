mod terminal;

pub use terminal::Terminal;
mod transport;
pub use transport::Peer;
mod endpoint;
pub use endpoint::{Endpoint, Listener};

mod config;
mod manifest;
pub use config::{Configuration, ConfigurationPaths, CtrlC, Profile, resolve_configuration_paths};
mod process;
pub use portable_pty::PtySize;
pub use process::{ProcessSpec, SupervisedProcess, supervise};
mod herdr;
pub use herdr::{Herdr, HostCall, HostFailure, HostFailureCode, HostReply, OpenView};
mod attachment_terminal;
pub use attachment_terminal::AttachmentTerminal;

#[cfg(any(test, feature = "test-support"))]
pub mod test_support;
