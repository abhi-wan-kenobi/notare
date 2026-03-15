use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::commands::listen::app::{App, Mode};
use crate::theme::Theme;

pub(super) fn draw_status_bar(frame: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    let mode = app.mode();

    let line = match mode {
        Mode::Command => {
            let cmd_display = format!(":{}", app.command_buffer());
            Line::from(vec![
                Span::styled(" COMMAND ", Style::new().fg(Color::Black).bg(Color::Yellow)),
                Span::raw(" "),
                Span::styled(cmd_display, Style::new().fg(Color::White)),
                Span::styled("\u{2588}", Style::new().fg(Color::Gray)),
            ])
        }
        Mode::Insert => Line::from(vec![
            Span::styled(" INSERT ", Style::new().fg(Color::Black).bg(Color::Green)),
            Span::raw(" "),
            Span::styled("[esc]", theme.shortcut_key),
            Span::raw(" normal  "),
            Span::styled("[tab]", theme.shortcut_key),
            Span::raw(" normal  "),
            Span::styled("[ctrl+z/y]", theme.shortcut_key),
            Span::raw(" undo/redo  "),
            Span::styled("[ctrl+u]", theme.shortcut_key),
            Span::raw(" clear"),
        ]),
        Mode::Normal => Line::from(vec![
            Span::styled(" NORMAL ", Style::new().fg(Color::Black).bg(Color::Cyan)),
            Span::raw(" "),
            Span::styled("[:q]", theme.shortcut_key),
            Span::raw(" quit  "),
            Span::styled("[j/k]", theme.shortcut_key),
            Span::raw(" scroll  "),
            Span::styled("[i]", theme.shortcut_key),
            Span::raw(" notepad  "),
            Span::styled("[G/g]", theme.shortcut_key),
            Span::raw(" bottom/top  "),
            Span::styled(format!("{} words", app.word_count()), theme.muted),
        ]),
    };

    frame.render_widget(Paragraph::new(line), area);
}
