#[cfg(feature = "outlook")]
pub mod outlook;

#[cfg(feature = "google")]
pub mod google;

#[cfg(feature = "apple")]
#[cfg(target_os = "macos")]
pub mod apple;

pub use hypr_calendar_interface::*;
