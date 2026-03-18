use ratatui::{Frame, layout::Rect};

use crate::commands::listen::app::App;
use crate::theme::Theme;

pub(super) fn draw_notepad(frame: &mut Frame, app: &mut App, area: Rect, theme: &Theme) {
    if area.width < 3 || area.height < 3 {
        return;
    }

    let block = theme.bordered_block(app.memo_focused()).title(" Notepad ");
    app.set_memo_block(block);
    frame.render_widget(app.memo(), area);
}
