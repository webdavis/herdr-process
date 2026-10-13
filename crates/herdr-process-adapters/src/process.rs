use herdr_process_domain::Command;

#[cfg(target_os = "macos")]
#[path = "process/control.rs"]
mod control;
#[cfg(target_os = "macos")]
#[path = "process/directory.rs"]
mod directory;
#[cfg(target_os = "macos")]
#[path = "process/guardian.rs"]
mod guardian;
#[cfg(target_os = "macos")]
#[path = "process/launch.rs"]
mod launch;
#[cfg(target_os = "macos")]
#[path = "process/native.rs"]
mod native;
#[cfg(target_os = "macos")]
#[path = "process/owner.rs"]
mod owner;
#[cfg(target_os = "macos")]
#[path = "process/startup.rs"]
mod startup;
#[cfg(target_os = "macos")]
pub use {guardian::supervise, owner::SupervisedProcess};

#[cfg(not(target_os = "macos"))]
#[path = "process/unsupported.rs"]
mod unsupported;
#[cfg(not(target_os = "macos"))]
pub use unsupported::{SupervisedProcess, supervise};
