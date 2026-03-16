use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::widgets::{Block, Borders};

use crate::commands::chat::app::App;
use crate::theme::Theme;

pub(super) fn draw(frame: &mut Frame, app: &mut App, area: Rect) {
    let theme = Theme::default();

    let title = if app.status().starts_with("Streaming") {
        " Composer (locked) "
    } else {
        " Composer "
    };
    let block = Block::new()
        .borders(Borders::ALL)
        .border_style(theme.border_focused)
        .title(title);
    app.input_mut().set_block(block);
    frame.render_widget(app.input(), area);
}
