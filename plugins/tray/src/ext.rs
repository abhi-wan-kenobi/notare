use tauri::{
    image::Image,
    menu::{Menu, MenuId, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    AppHandle, Result,
};

pub enum TrayItem {
    Info,
    Open,
    Github,
    Twitter,
    Discord,
    Quit,
}

impl From<TrayItem> for MenuId {
    fn from(value: TrayItem) -> Self {
        match value {
            TrayItem::Open => "open_hypr",
            TrayItem::Github => "github_hypr",
            TrayItem::Twitter => "twitter_hypr",
            TrayItem::Discord => "discord_hypr",
            TrayItem::Quit => "quit_hypr",
            TrayItem::Info => "info_hypr",
        }
        .into()
    }
}

impl From<MenuId> for TrayItem {
    fn from(id: MenuId) -> Self {
        let id = id.0.as_str();
        match id {
            "info_hypr" => TrayItem::Info,
            "open_hypr" => TrayItem::Open,
            "github_hypr" => TrayItem::Github,
            "twitter_hypr" => TrayItem::Twitter,
            "discord_hypr" => TrayItem::Discord,
            "quit_hypr" => TrayItem::Quit,
            _ => unreachable!(),
        }
    }
}

pub trait TrayPluginExt<R: tauri::Runtime> {
    fn create_tray(&self) -> Result<()>;
}

impl<T: tauri::Manager<tauri::Wry>> TrayPluginExt<tauri::Wry> for T {
    fn create_tray(&self) -> Result<()> {
        let app = self.app_handle();

        let menu = Menu::with_items(
            app,
            &[
                &info_menu(app)?,
                &open_menu(app)?,
                &PredefinedMenuItem::separator(app)?,
                &github_menu(app)?,
                &twitter_menu(app)?,
                &discord_menu(app)?,
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
                    TrayItem::Open => {
                        use tauri_plugin_windows::HyprWindow;
                        let _ = HyprWindow::Main.show(app);
                    }
                    TrayItem::Github => {
                        let _ = webbrowser::open("https://github.com/fastrepl/hyprnote");
                    }
                    TrayItem::Twitter => {
                        let _ = webbrowser::open("https://hyprnote.com/x");
                    }
                    TrayItem::Discord => {
                        let _ = webbrowser::open("https://hyprnote.com/discord");
                    }
                    TrayItem::Quit => {
                        app.exit(0);
                    }
                    TrayItem::Info => {}
                }
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

fn open_menu<R: tauri::Runtime>(app: &AppHandle<R>) -> Result<MenuItem<R>> {
    MenuItem::with_id(app, TrayItem::Open, "Open", true, None::<&str>)
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
    MenuItem::with_id(app, TrayItem::Quit, "Quit completely", true, Some("cmd+q"))
}
