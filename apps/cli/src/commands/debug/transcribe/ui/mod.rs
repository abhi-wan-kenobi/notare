pub mod shell;

pub use super::tracing::TracingCapture;

use super::app::App;

pub(crate) fn draw(frame: &mut ratatui::Frame, app: &mut App) {
    app.draw(frame);
}
