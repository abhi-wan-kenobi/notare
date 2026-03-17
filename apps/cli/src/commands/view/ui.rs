use ratatui::{
    Frame,
    layout::{Constraint, Layout},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Padding, Paragraph},
};

use crate::theme::Theme;
use crate::widgets::{InfoLine, KeyHints, Scrollable, build_segment_lines};

use super::app::{App, Mode};

pub(crate) fn draw(frame: &mut Frame, app: &mut App) {
    let theme = Theme::TRANSPARENT;

    frame.render_widget(
        Block::default().style(Style::default().bg(theme.bg)),
        frame.area(),
    );

    let [header_area, body_area, status_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(3),
        Constraint::Length(1),
    ])
    .areas(frame.area());

    draw_header(frame, app, header_area, &theme);

    if app.loading() {
        let msg = Paragraph::new(Line::from(Span::styled("  Loading...", theme.muted)));
        frame.render_widget(msg, body_area);
    } else if let Some(error) = app.error() {
        let msg = Paragraph::new(Line::from(Span::styled(format!("  {error}"), theme.error)));
        frame.render_widget(msg, body_area);
    } else {
        let [memo_area, transcript_area] = Layout::horizontal([
            Constraint::Percentage(app.notepad_width_percent()),
            Constraint::Percentage(100 - app.notepad_width_percent()),
        ])
        .areas(body_area);

        draw_memo(frame, app, memo_area, &theme);
        draw_transcript(frame, app, transcript_area, &theme);
    }

    draw_status_bar(frame, app, status_area, &theme);
}

fn draw_header(frame: &mut Frame, app: &App, area: ratatui::layout::Rect, theme: &Theme) {
    let title = if app.title().is_empty() {
        "Untitled"
    } else {
        app.title()
    };

    let date = app.created_at();
    let short_date = date.get(..10).unwrap_or(date);

    let info = InfoLine::new(theme)
        .item(Span::styled(title, Style::new().fg(Color::Yellow)))
        .item(Span::raw(short_date));

    frame.render_widget(info, area);
}

fn draw_memo(frame: &mut Frame, app: &mut App, area: ratatui::layout::Rect, theme: &Theme) {
    if area.width < 3 || area.height < 3 {
        return;
    }

    let border_style = if app.memo_focused() {
        theme.border_focused
    } else {
        theme.border
    };

    let block = Block::new()
        .borders(Borders::ALL)
        .border_style(border_style)
        .title(" Memo ");

    app.set_memo_block(block);
    frame.render_widget(app.memo(), area);
}

fn draw_transcript(frame: &mut Frame, app: &mut App, area: ratatui::layout::Rect, theme: &Theme) {
    let segments = app.segments();

    let border_style = if app.transcript_focused() {
        theme.border_focused
    } else {
        theme.border
    };

    let block = Block::new()
        .borders(Borders::ALL)
        .border_style(border_style)
        .title(" Transcript ")
        .padding(Padding::new(1, 1, 0, 0));

    if segments.is_empty() {
        let lines = vec![Line::from(Span::styled("No transcript", theme.placeholder))];
        let paragraph = Paragraph::new(lines).block(block);
        frame.render_widget(paragraph, area);
        return;
    }

    let content_width = area.width.saturating_sub(4) as usize;
    let lines = build_segment_lines(segments, theme, content_width, None);

    let scrollable = Scrollable::new(lines).block(block);
    let scroll_state = app.scroll_state_mut();
    frame.render_stateful_widget(scrollable, area, scroll_state);
}

fn draw_status_bar(frame: &mut Frame, app: &App, area: ratatui::layout::Rect, theme: &Theme) {
    match app.mode() {
        Mode::Command => {
            let cmd_display = format!(":{}", app.command_buffer());
            let line = Line::from(vec![
                Span::styled(" COMMAND ", Style::new().fg(Color::Black).bg(Color::Yellow)),
                Span::raw(" "),
                Span::styled(cmd_display, Style::new().fg(Color::White)),
                Span::styled("\u{2588}", Style::new().fg(Color::Gray)),
            ]);
            frame.render_widget(Paragraph::new(line), area);
        }
        Mode::Insert => {
            let mut hints_widget = KeyHints::new(theme)
                .badge(" INSERT ", Style::new().fg(Color::Black).bg(Color::Green))
                .hints(vec![
                    ("esc", "normal"),
                    ("tab", "normal"),
                    ("ctrl+z/y", "undo/redo"),
                ]);
            if app.memo_dirty() {
                hints_widget = hints_widget.suffix(Span::styled("[modified]", theme.muted));
            }
            frame.render_widget(hints_widget, area);
        }
        Mode::Normal => {
            let mut hints = vec![
                (":q", "quit"),
                (":w", "save"),
                ("j/k", "scroll"),
                ("i", "memo"),
            ];
            if app.memo_dirty() {
                hints.push(("", "[modified]"));
            }
            let mut hints_widget = KeyHints::new(theme)
                .badge(" NORMAL ", Style::new().fg(Color::Black).bg(Color::Cyan))
                .hints(hints);
            if let Some(msg) = app.save_message() {
                hints_widget = hints_widget.suffix(Span::styled(msg, theme.status_active));
            }
            frame.render_widget(hints_widget, area);
        }
    }
}
