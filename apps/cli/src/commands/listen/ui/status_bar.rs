use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::Span,
};

use crate::commands::listen::app::{App, Mode};
use crate::theme::Theme;
use crate::widgets::{CommandBar, KeyHints};

pub(super) fn draw_status_bar(frame: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    match app.mode() {
        Mode::Command => {
            frame.render_widget(CommandBar::new(app.command_buffer()), area);
        }
        Mode::Insert => {
            frame.render_widget(
                KeyHints::new(theme)
                    .badge(" INSERT ", Style::new().fg(Color::Black).bg(Color::Green))
                    .hints(vec![
                        ("esc", "normal"),
                        ("tab", "normal"),
                        ("ctrl+z/y", "undo/redo"),
                        ("ctrl+u", "clear"),
                    ]),
                area,
            );
        }
        Mode::Normal => {
            frame.render_widget(
                KeyHints::new(theme)
                    .badge(" NORMAL ", Style::new().fg(Color::Black).bg(Color::Cyan))
                    .hints(vec![
                        (":q", "quit"),
                        ("j/k", "scroll"),
                        ("i", "notepad"),
                        ("G/g", "bottom/top"),
                    ])
                    .suffix(Span::styled(
                        format!("{} words", app.word_count()),
                        theme.muted,
                    )),
                area,
            );
        }
    }
}
