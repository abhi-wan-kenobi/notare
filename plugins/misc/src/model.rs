#[derive(strum::Display, specta::Type, serde::Serialize, serde::Deserialize)]
pub enum TemplateName {
    #[specta(rename = "misc:create-title-system")]
    #[strum(serialize = "misc:create-title-system")]
    CreateTitleSystem,
    #[specta(rename = "misc:create-title-user")]
    #[strum(serialize = "misc:create-title-user")]
    CreateTitleUser,
    #[specta(rename = "misc:enhance-system")]
    #[strum(serialize = "misc:enhance-system")]
    EnhanceSystem,
    #[specta(rename = "misc:enhance-user")]
    #[strum(serialize = "misc:enhance-user")]
    EnhanceUser,
    #[specta(rename = "misc:postprocess-enhance-system")]
    #[strum(serialize = "misc:postprocess-enhance-system")]
    PostprocessEnhanceSystem,
    #[specta(rename = "misc:postprocess-enhance-user")]
    #[strum(serialize = "misc:postprocess-enhance-user")]
    PostprocessEnhanceUser,
}

pub const TEMPLATES: &[(TemplateName, &str)] = &[
    (
        TemplateName::CreateTitleSystem,
        include_str!("../templates/create_title.system.jinja"),
    ),
    (
        TemplateName::CreateTitleUser,
        include_str!("../templates/create_title.user.jinja"),
    ),
    (
        TemplateName::EnhanceSystem,
        include_str!("../templates/enhance.system.jinja"),
    ),
    (
        TemplateName::EnhanceUser,
        include_str!("../templates/enhance.user.jinja"),
    ),
    (
        TemplateName::PostprocessEnhanceSystem,
        include_str!("../templates/postprocess_enhance.system.jinja"),
    ),
    (
        TemplateName::PostprocessEnhanceUser,
        include_str!("../templates/postprocess_enhance.user.jinja"),
    ),
];
