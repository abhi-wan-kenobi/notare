use tauri::Wry;

mod commands;
mod ext;

const PLUGIN_NAME: &str = "utils";

// NOTE: template name must match js/index.ts
pub const TEMPLATES: &[(&str, &str)] = &[
    (
        "misc:create-title-system",
        include_str!("../templates/create_title.system.jinja"),
    ),
    (
        "misc:create-title-user",
        include_str!("../templates/create_title.user.jinja"),
    ),
    (
        "misc:enhance-system",
        include_str!("../templates/enhance.system.jinja"),
    ),
    (
        "misc:enhance-user",
        include_str!("../templates/enhance.user.jinja"),
    ),
    (
        "misc:postprocess-enhance-system",
        include_str!("../templates/postprocess_enhance.system.jinja"),
    ),
    (
        "misc:postprocess-enhance-user",
        include_str!("../templates/postprocess_enhance.user.jinja"),
    ),
];

fn make_specta_builder() -> tauri_specta::Builder<Wry> {
    tauri_specta::Builder::new()
        .plugin_name(PLUGIN_NAME)
        .commands(tauri_specta::collect_commands![
            commands::opinionated_md_to_html::<Wry>
        ])
        .error_handling(tauri_specta::ErrorHandlingMode::Throw)
}

pub fn init() -> tauri::plugin::TauriPlugin<Wry> {
    let specta_builder = make_specta_builder();

    tauri::plugin::Builder::new(PLUGIN_NAME)
        .invoke_handler(specta_builder.invoke_handler())
        .build()
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn export_types() {
        make_specta_builder()
            .export(
                specta_typescript::Typescript::default()
                    .header("// @ts-nocheck\n\n")
                    .formatter(specta_typescript::formatter::prettier)
                    .bigint(specta_typescript::BigIntExportBehavior::Number),
                "./js/bindings.gen.ts",
            )
            .unwrap()
    }
}
