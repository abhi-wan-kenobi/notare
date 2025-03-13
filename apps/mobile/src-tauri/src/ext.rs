use std::future::Future;

pub trait AppExt<R: tauri::Runtime> {
    fn setup_db(&self) -> impl Future<Output = Result<(), crate::Error>>;
}

impl<R: tauri::Runtime, T: tauri::Manager<R>> AppExt<R> for T {
    #[tracing::instrument(skip_all)]
    async fn setup_db(&self) -> Result<(), crate::Error> {
        let app = self.app_handle();

        let db = hypr_db_core::DatabaseBuilder::default()
            .memory()
            .build()
            .await
            .unwrap();

        {
            use tauri_plugin_db::DatabasePluginExt;
            app.db_attach(db).await.unwrap();
        }

        Ok(())
    }
}
