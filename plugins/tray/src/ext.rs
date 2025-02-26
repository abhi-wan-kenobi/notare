use tauri::{
    image::Image,
    menu::{Menu, MenuId, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    AppHandle, Manager, Result,
};

pub enum TrayItem {
    Open,
    Quit,
}

impl From<TrayItem> for MenuId {
    fn from(value: TrayItem) -> Self {
        match value {
            TrayItem::Open => "open_hypr",
            TrayItem::Quit => "quit_hypr",
        }
        .into()
    }
}

impl From<MenuId> for TrayItem {
    fn from(id: MenuId) -> Self {
        let id = id.0.as_str();
        match id {
            "open_hypr" => TrayItem::Open,
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
                &MenuItem::with_id(app, TrayItem::Open, "Open", true, None::<&str>)?,
                &PredefinedMenuItem::separator(app)?,
                &MenuItem::with_id(app, TrayItem::Quit, "Quit", true, None::<&str>)?,
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
                        if let Some(window) = app.get_webview_window("main") {
                            window.show().unwrap();
                            window.set_focus().unwrap();
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
