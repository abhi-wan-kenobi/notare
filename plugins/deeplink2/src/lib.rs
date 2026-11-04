mod commands;
mod error;
mod pending;
pub mod server;
mod types;

#[cfg(test)]
mod docs;

pub use error::{Error, Result};
pub use types::{
    AuthCallbackSearch, BillingRefreshSearch, DeepLink, DeepLinkEvent, IntegrationCallbackSearch,
    PairSearch,
};

use std::str::FromStr;

use tauri::Manager;
use tauri_plugin_deep_link::DeepLinkExt;
use tauri_specta::Event;

const PLUGIN_NAME: &str = "deeplink2";

fn redact_url(url_str: &str) -> String {
    match url::Url::parse(url_str) {
        Ok(parsed) => {
            let scheme = parsed.scheme();
            let host = parsed.host_str().unwrap_or("");
            let path = parsed.path();
            format!("{}://{}{}", scheme, host, path)
        }
        Err(_) => "[invalid_url]".to_string(),
    }
}

fn make_specta_builder<R: tauri::Runtime>() -> tauri_specta::Builder<R> {
    tauri_specta::Builder::<R>::new()
        .plugin_name(PLUGIN_NAME)
        .commands(tauri_specta::collect_commands![
            commands::start_callback_server::<tauri::Wry>,
            commands::stop_callback_server::<tauri::Wry>,
            commands::take_pending_pair::<tauri::Wry>,
        ])
        .events(tauri_specta::collect_events![types::DeepLinkEvent])
        .typ::<types::DeepLink>()
        .error_handling(tauri_specta::ErrorHandlingMode::Result)
}

fn handle_url<R: tauri::Runtime>(app: &tauri::AppHandle<R>, url_str: &str) {
    let redacted = redact_url(url_str);
    tracing::info!(url = %redacted, "deeplink_received");

    match DeepLink::from_str(url_str) {
        Ok(deep_link) => {
            tracing::info!(path = deep_link.path(), "deeplink_parsed");
            if let DeepLink::Pair(search) = &deep_link {
                app.state::<pending::PendingPair>().set(search.clone());
            }
            if let Err(e) = DeepLinkEvent(deep_link).emit(app) {
                tracing::error!(error = ?e, "deeplink_event_emit_failed");
            }
        }
        Err(e) => {
            tracing::debug!(error = ?e, url = %redacted, "deeplink_parse_failed");
        }
    }
}

pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    let specta_builder = make_specta_builder();

    tauri::plugin::Builder::new(PLUGIN_NAME)
        .invoke_handler(specta_builder.invoke_handler())
        .setup(move |app, _api| {
            specta_builder.mount_events(app);
            app.manage(server::CallbackServerState::new());

            app.manage(pending::PendingPair::default());

            let app_handle = app.clone();
            app.deep_link().on_open_url(move |event| {
                for url in event.urls() {
                    handle_url(&app_handle, url.as_str());
                }
            });

            // Windows/Linux cold start: the deep-link plugin emitted the launch
            // URL during its own init, before the listener above existed.
            if let Ok(Some(urls)) = app.deep_link().get_current() {
                for url in urls {
                    handle_url(app, url.as_str());
                }
            }

            Ok(())
        })
        .build()
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn pair_link_is_parked_until_taken_once() {
        let app = tauri::test::mock_app();
        make_specta_builder::<tauri::test::MockRuntime>().mount_events(app.handle());
        app.manage(pending::PendingPair::default());

        handle_url(app.handle(), "notare://pair#first");
        handle_url(app.handle(), "notare://pair#second");

        let pending = app.state::<pending::PendingPair>();
        assert_eq!(pending.take().unwrap().link, "notare://pair#second");
        assert!(pending.take().is_none());
    }

    #[test]
    fn non_pair_urls_never_park_a_link() {
        let app = tauri::test::mock_app();
        make_specta_builder::<tauri::test::MockRuntime>().mount_events(app.handle());
        app.manage(pending::PendingPair::default());

        for url in [
            "notare://billing/refresh",
            "notare://auth/callback?access_token=a&refresh_token=b",
            "notare://pair",
            "https://pair#abc",
            "not a url",
        ] {
            handle_url(app.handle(), url);
        }

        assert!(app.state::<pending::PendingPair>().take().is_none());
    }

    #[test]
    fn export() {
        export_types();
        export_docs();
    }

    fn export_types() {
        const OUTPUT_FILE: &str = "./js/bindings.gen.ts";

        make_specta_builder::<tauri::Wry>()
            .export(
                specta_typescript::Typescript::default()
                    .formatter(specta_typescript::formatter::prettier)
                    .bigint(specta_typescript::BigIntExportBehavior::Number),
                OUTPUT_FILE,
            )
            .unwrap();

        let content = std::fs::read_to_string(OUTPUT_FILE).unwrap();
        std::fs::write(OUTPUT_FILE, format!("// @ts-nocheck\n{content}")).unwrap();
    }

    fn export_docs() {
        let source_code = std::fs::read_to_string("./js/bindings.gen.ts").unwrap();
        let deeplinks = docs::parse_deeplinks(&source_code).unwrap();
        assert!(!deeplinks.is_empty());

        let output_dir = std::path::Path::new("../../apps/web/content/deeplinks");
        std::fs::create_dir_all(output_dir).unwrap();

        for deeplink in &deeplinks {
            let filepath = output_dir.join(deeplink.doc_path());
            let content = deeplink.doc_render();
            std::fs::write(&filepath, content).unwrap();
        }
    }
}
