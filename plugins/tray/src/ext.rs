use crate::PLUGIN_NAME;
use tauri::{
    image::Image,
    menu::{CheckMenuItem, Menu, MenuId, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    AppHandle, Result,
};

pub enum TrayItem {
    Info,
    Github,
    Twitter,
    Discord,
    Quit,
    AlwaysOnTop,
}

impl From<TrayItem> for MenuId {
    fn from(value: TrayItem) -> Self {
        match value {
            TrayItem::Info => "info_hypr",
            TrayItem::Github => "github_hypr",
            TrayItem::Twitter => "twitter_hypr",
            TrayItem::Discord => "discord_hypr",
            TrayItem::Quit => "quit_hypr",
            TrayItem::AlwaysOnTop => "always_on_top_hypr",
        }
        .into()
    }
}

impl From<MenuId> for TrayItem {
    fn from(id: MenuId) -> Self {
        let id = id.0.as_str();
        match id {
            "info_hypr" => TrayItem::Info,
            "github_hypr" => TrayItem::Github,
            "twitter_hypr" => TrayItem::Twitter,
            "discord_hypr" => TrayItem::Discord,
            "quit_hypr" => TrayItem::Quit,
            "always_on_top_hypr" => TrayItem::AlwaysOnTop,
            _ => unreachable!(),
        }
    }
}

pub trait TrayPluginExt<R: tauri::Runtime> {
    fn create_tray(&self) -> Result<()>;
}

#[derive(Debug, PartialEq, Eq, Hash, strum::Display)]
enum StoreKey {
    MainWindowAlwaysOnTop,
}

impl tauri_plugin_store2::ScopedStoreKey for StoreKey {}

impl<T: tauri::Manager<tauri::Wry>> TrayPluginExt<tauri::Wry> for T {
    fn create_tray(&self) -> Result<()> {
        let app = self.app_handle();

        let store = {
            use tauri_plugin_store2::StorePluginExt;
            app.scoped_store::<StoreKey>(PLUGIN_NAME).unwrap()
        };

        let always_on_top = always_on_top_menu(app, always_on_top_state(&store))?;

        let menu = Menu::with_items(
            app,
            &[
                &info_menu(app)?,
                &github_menu(app)?,
                &twitter_menu(app)?,
                &discord_menu(app)?,
                &PredefinedMenuItem::separator(app)?,
                &always_on_top,
                &PredefinedMenuItem::separator(app)?,
                &quit_menu(app)?,
            ],
        )?;

        TrayIconBuilder::with_id("hypr-tray")
            .icon(Image::from_bytes(include_bytes!(
                "../icons/tray_default.png"
            ))?)
            .icon_as_template(true)
            .menu(&menu)
            .show_menu_on_left_click(true)
            .on_menu_event({
                move |app: &AppHandle, event| match TrayItem::from(event.id.clone()) {
                    TrayItem::Info => {}
                    TrayItem::Github => {
                        let _ = webbrowser::open("https://github.com/fastrepl/hyprnote");
                    }
                    TrayItem::Twitter => {
                        let _ = webbrowser::open("https://hyprnote.com/x");
                    }
                    TrayItem::Discord => {
                        let _ = webbrowser::open("https://hyprnote.com/discord");
                    }
                    TrayItem::AlwaysOnTop => {
                        let next_always_on_top = !always_on_top_state(&store);

                        let toggled = {
                            use tauri_plugin_windows::HyprWindow;
                            if let Some(window) = HyprWindow::Main.get(app) {
                                window.set_always_on_top(next_always_on_top).is_ok()
                            } else {
                                false
                            }
                        };

                        if toggled {
                            store
                                .set(StoreKey::MainWindowAlwaysOnTop, next_always_on_top)
                                .unwrap();

                            always_on_top.set_checked(next_always_on_top).unwrap();
                        }
                    }
                    TrayItem::Quit => {
                        app.exit(0);
                    }
                }
            })
            .on_tray_icon_event({
                let _app_handle = app.clone();
                move |_tray, _event| {}
            })
            .build(app)?;

        Ok(())
    }
}

fn info_menu<R: tauri::Runtime>(app: &AppHandle<R>) -> Result<MenuItem<R>> {
    let info = app.package_info();

    let display_name = match info.name.as_str() {
        "Hyprnote" => format!("Hyprnote v{}", &info.version),
        "Hyprnote Dev" => format!("Hyprnote v{} (dev)", &info.version),
        "Hyprnote Nightly" => format!("Hyprnote v{} (nightly)", &info.version),
        _ => unreachable!(),
    };

    MenuItem::with_id(app, TrayItem::Info, &display_name, false, None::<&str>)
}

fn github_menu<R: tauri::Runtime>(app: &AppHandle<R>) -> Result<MenuItem<R>> {
    MenuItem::with_id(app, TrayItem::Github, "GitHub", true, None::<&str>)
}

fn twitter_menu<R: tauri::Runtime>(app: &AppHandle<R>) -> Result<MenuItem<R>> {
    MenuItem::with_id(app, TrayItem::Twitter, "Twitter", true, None::<&str>)
}

fn discord_menu<R: tauri::Runtime>(app: &AppHandle<R>) -> Result<MenuItem<R>> {
    MenuItem::with_id(app, TrayItem::Discord, "Discord", true, None::<&str>)
}

fn quit_menu<R: tauri::Runtime>(app: &AppHandle<R>) -> Result<MenuItem<R>> {
    MenuItem::with_id(app, TrayItem::Quit, "Quit Completely", true, Some("cmd+q"))
}

fn always_on_top_menu<R: tauri::Runtime>(
    app: &AppHandle<R>,
    initial_state: bool,
) -> Result<CheckMenuItem<R>> {
    CheckMenuItem::with_id(
        app,
        TrayItem::AlwaysOnTop,
        "Always on top",
        true,
        initial_state,
        Some("cmd+shift+t"),
    )
}

fn always_on_top_state<R: tauri::Runtime>(
    store: &tauri_plugin_store2::ScopedStore<R, StoreKey>,
) -> bool {
    store
        .get::<bool>(StoreKey::MainWindowAlwaysOnTop)
        .unwrap_or(Some(false))
        .unwrap_or(false)
}
