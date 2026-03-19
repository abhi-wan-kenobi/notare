use crossterm::event::KeyEvent;

pub(crate) enum Action {
    Key(KeyEvent),
    Loaded(Vec<hypr_db_app::MeetingRow>),
    LoadError(String),
}
