use std::future::Future;

pub trait AppExt<R: tauri::Runtime> {
    fn setup_db(&self) -> impl Future<Output = Result<(), String>>;
}

impl<R: tauri::Runtime, T: tauri::Manager<R>> AppExt<R> for T {
    #[tracing::instrument(skip_all)]
    async fn setup_db(&self) -> Result<(), String> {
        Ok(())
    }
}
