pub mod decision;
pub mod hotkey;
pub mod key_event;
pub mod processor;

pub use decision::*;
pub use hotkey::*;
pub use key_event::*;
pub use processor::*;

#[cfg(target_os = "macos")]
pub mod permission;
#[cfg(target_os = "macos")]
pub mod tap;

#[cfg(target_os = "macos")]
pub use tap::{EventTap, TapError, TapEvent};
