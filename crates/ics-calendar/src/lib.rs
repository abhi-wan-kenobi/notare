//! Local `.ics` calendar-file support for Notare.
//!
//! Two halves:
//! - [`store`]: manages imported `.ics` files copied into an app-owned
//!   directory (so the original file can vanish), with an `index.json`
//!   remembering original names. Each imported file is one calendar.
//! - [`parse`] / [`expand`]: parses VCALENDAR/VEVENT data (via the `ical`
//!   crate) and expands recurring events into concrete occurrences inside a
//!   query window (via the `rrule` crate).

mod expand;
mod parse;
mod store;

pub use expand::{ExpandOptions, IcsOccurrence, expand_events};
pub use parse::{
    IcsAttendee, IcsAttendeeRole, IcsAttendeeStatus, IcsCalendar, IcsEvent, IcsEventStatus,
    IcsPerson, parse_ics,
};
pub use store::{IcsFileInfo, IcsStore};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to parse ics data: {0}")]
    Parse(String),
    #[error("ics file contains no VEVENT components")]
    NoEvents,
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("unknown imported calendar id: {0}")]
    UnknownCalendar(String),
    #[error("invalid ics index: {0}")]
    Index(String),
}
