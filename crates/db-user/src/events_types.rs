use chrono::{DateTime, Utc};

use crate::user_common_derives;

user_common_derives! {
    pub struct Event {
        pub id: String,
        pub user_id: String,
        pub tracking_id: String,
        pub calendar_id: String,
        pub name: String,
        pub note: String,
        pub start_date: DateTime<Utc>,
        pub end_date: DateTime<Utc>,
        pub google_event_url: Option<String>,
    }
}

user_common_derives! {
    pub enum ListEventFilter {
        #[serde(rename = "userId")]
        UserId(String),
        #[serde(rename = "dateRange")]
        DateRange {
            #[serde(rename = "userId")]
            user_id: String,
            range: (DateTime<Utc>, DateTime<Utc>),
        },
    }
}
