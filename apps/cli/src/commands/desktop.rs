const DOWNLOAD_URL: &str = "https://char.com/download";
const DESKTOP_DEEPLINKS: &[&str] = &[
    "hyprnote://focus",
    "hyprnote-nightly://focus",
    "hyprnote-staging://focus",
    "hypr://focus",
];

pub enum DesktopAction {
    OpenedApp,
    OpenedDownloadPage,
}

pub fn run() -> DesktopAction {
    for deeplink in DESKTOP_DEEPLINKS {
        if open::that(deeplink).is_ok() {
            return DesktopAction::OpenedApp;
        }
    }

    if let Err(e) = open::that(DOWNLOAD_URL) {
        eprintln!("Failed to open desktop app or browser: {e}");
        eprintln!("Please visit: {DOWNLOAD_URL}");
        std::process::exit(1);
    }

    DesktopAction::OpenedDownloadPage
}
