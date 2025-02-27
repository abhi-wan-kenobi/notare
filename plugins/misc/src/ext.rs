use crate::model::TemplateName;
use tauri::{Manager, Runtime};

pub trait UtilsPluginExt<R: Runtime> {
    fn get_fingerprint(&self) -> String;
    fn opinionated_md_to_html(&self, text: impl AsRef<str>) -> Result<String, String>;
    fn list_template_names(&self) -> Vec<TemplateName>;
}

impl<R: Runtime, T: Manager<R>> UtilsPluginExt<R> for T {
    fn get_fingerprint(&self) -> String {
        hypr_host::fingerprint()
    }

    fn opinionated_md_to_html(&self, text: impl AsRef<str>) -> Result<String, String> {
        hypr_buffer::opinionated_md_to_html(text.as_ref()).map_err(|e| e.to_string())
    }

    fn list_template_names(&self) -> Vec<TemplateName> {
        vec![
            TemplateName::CreateTitleSystem,
            TemplateName::CreateTitleUser,
            TemplateName::EnhanceSystem,
            TemplateName::EnhanceUser,
            TemplateName::PostprocessEnhanceSystem,
            TemplateName::PostprocessEnhanceUser,
        ]
    }
}
