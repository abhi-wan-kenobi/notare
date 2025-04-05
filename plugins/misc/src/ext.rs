use tauri::{Manager, Runtime};

pub trait MiscPluginExt<R: Runtime> {
    fn get_git_hash(&self) -> String;
    fn get_fingerprint(&self) -> String;
    fn opinionated_md_to_html(&self, text: impl AsRef<str>) -> Result<String, String>;
}

impl<R: Runtime, T: Manager<R>> MiscPluginExt<R> for T {
    fn get_git_hash(&self) -> String {
        env!("VERGEN_GIT_SHA").to_string()
    }

    fn get_fingerprint(&self) -> String {
        hypr_host::fingerprint()
    }

    fn opinionated_md_to_html(&self, text: impl AsRef<str>) -> Result<String, String> {
        hypr_buffer::opinionated_md_to_html(text.as_ref()).map_err(|e| e.to_string())
    }
}
