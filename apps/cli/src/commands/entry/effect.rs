#[derive(Clone, Copy)]
pub enum EntryAction {
    Listen,
    Connect,
    Quit,
}

pub(crate) enum Effect {
    LaunchListen,
    LaunchConnect,
    OpenAuth,
    OpenDesktop,
    Exit,
}
