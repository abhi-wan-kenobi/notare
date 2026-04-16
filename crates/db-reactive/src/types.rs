use std::collections::HashSet;

pub trait QueryEventSink: Clone + Send + 'static {
    fn send_result(&self, rows: Vec<serde_json::Value>) -> std::result::Result<(), String>;
    fn send_error(&self, error: String) -> std::result::Result<(), String>;
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum DependencyTarget {
    Table(String),
    VirtualTable(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DependencyAnalysis {
    Reactive { targets: HashSet<DependencyTarget> },
    NonReactive { reason: String },
}

pub struct SubscriptionRegistration {
    pub id: String,
    pub analysis: DependencyAnalysis,
}
