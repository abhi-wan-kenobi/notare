mod calendar;

use apalis::prelude::{WorkerBuilder, WorkerBuilderExt, WorkerFactoryFn};
use std::str::FromStr;

use crate::state::WorkerState;

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
