use std::future::Future;

use apalis_sql::{sqlite::SqliteStorage, sqlx::SqlitePool};

pub trait AppExt<R: tauri::Runtime> {
    fn setup_db(&self) -> impl Future<Output = Result<(), crate::Error>>;
    fn setup_worker(&self) -> impl Future<Output = Result<(), crate::Error>>;
}

impl<R: tauri::Runtime, T: tauri::Manager<R>> AppExt<R> for T {
    #[tracing::instrument(skip_all)]
    async fn setup_db(&self) -> Result<(), crate::Error> {
        Ok(())
    }

    #[tracing::instrument(skip_all)]
    async fn setup_worker(&self) -> Result<(), crate::Error> {
        let pool = SqlitePool::connect("sqlite::memory:").await?;
        SqliteStorage::setup(&pool).await.unwrap();

        Ok(())
    }
}
