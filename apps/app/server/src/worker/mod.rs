mod calendar;

use apalis::prelude::{Error, WorkerBuilder, WorkerBuilderExt, WorkerFactoryFn};
use std::str::FromStr;

use crate::state::WorkerState;

fn err_from(e: impl Into<String>) -> Error {
    Error::Failed(std::sync::Arc::new(Box::new(std::io::Error::new(
        std::io::ErrorKind::Other,
        e.into(),
    ))))
}

pub async fn monitor(state: WorkerState) -> Result<(), std::io::Error> {
    let calendar_schedule = apalis_cron::Schedule::from_str("0 0 * * * *").unwrap();

    let calendar_ctx = state.clone();

    apalis::prelude::Monitor::new()
        .register(
            WorkerBuilder::new("calendar")
                .concurrency(1)
                .data(calendar_ctx)
                .backend(apalis_cron::CronStream::new(calendar_schedule))
                .build_fn(calendar::perform),
        )
        .run()
        .await
}
