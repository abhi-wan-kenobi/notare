use crossterm::event::KeyEvent;
use hypr_db_app::{NoteRow, SessionRow};
use hypr_transcript::Segment;

pub(crate) enum Action {
    Key(KeyEvent),
    Paste(String),
    Loaded {
        session: SessionRow,
        segments: Vec<Segment>,
        memo: Option<NoteRow>,
    },
    LoadError(String),
    Saved,
    SaveError(String),
}
