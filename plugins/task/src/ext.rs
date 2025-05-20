pub trait TaskPluginExt<R: tauri::Runtime> {
    fn ping(&self) -> Result<String, crate::Error>;
}

impl<R: tauri::Runtime, T: tauri::Manager<R>> TaskPluginExt<R> for T {
    fn ping(&self) -> Result<String, crate::Error> {
        Ok("pong".to_string())
    }
}
