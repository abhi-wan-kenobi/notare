use tauri::{Manager, Runtime};

pub trait UtilsPluginExt<R: Runtime> {
    fn opinionated_md_to_html(&self, text: impl AsRef<str>) -> Result<String, String>;
}

impl<R: Runtime, T: Manager<R>> UtilsPluginExt<R> for T {
    fn opinionated_md_to_html(&self, text: impl AsRef<str>) -> Result<String, String> {
        hypr_buffer::opinionated_md_to_html(text.as_ref()).map_err(|e| e.to_string())
    }
}
