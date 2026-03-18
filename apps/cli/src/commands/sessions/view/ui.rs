use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Padding, Paragraph},
};

use crate::theme::Theme;
use crate::widgets::{CommandBar, InfoLine, KeyHints, build_segment_lines, render_scrollable};

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

fn draw_header(frame: &mut Frame, app: &App, area: Rect, theme: &Theme) {
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

fn draw_memo(frame: &mut Frame, app: &mut App, area: Rect, theme: &Theme) {
    if area.width < 3 || area.height < 3 {
        return;
    }

    let block = theme.bordered_block(app.memo_focused()).title(" Memo ");
    app.set_memo_block(block);
    frame.render_widget(app.memo(), area);
}

fn draw_transcript(frame: &mut Frame, app: &mut App, area: Rect, theme: &Theme) {
    let segments = app.segments();

    let block = theme
        .bordered_block(app.transcript_focused())
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

    render_scrollable(frame, lines, Some(block), area, app.scroll_state_mut());
}

fn draw_status_bar(frame: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    match app.mode() {
        Mode::Command => {
            frame.render_widget(CommandBar::new(app.command_buffer()), area);
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
