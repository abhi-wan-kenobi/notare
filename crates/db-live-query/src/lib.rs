#![forbid(unsafe_code)]

mod error;
mod explain;
mod query;
mod runtime;
mod schema;
mod subscriptions;
mod types;
mod watch;

pub use error::{Error, Result};
pub use explain::extract_dependencies;
pub use hypr_db_change::{ChangeNotifier, TableChange, TableChangeKind};
pub use runtime::DbRuntime;
pub use schema::DependencyResolutionError;
pub use types::{
    DependencyAnalysis, DependencyTarget, ProxyQueryMethod, ProxyQueryResult, QueryEventSink,
    SubscriptionRegistration,
};
