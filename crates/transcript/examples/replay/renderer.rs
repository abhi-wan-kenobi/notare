use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, Paragraph, Wrap},
};

use crate::App;

const DEBUG_PANEL_WIDTH: u16 = 32;

pub fn render(frame: &mut Frame, app: &App) {
    let [header_area, body_area, timeline_area, hint_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Fill(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(frame.area());

    let [transcript_area, debug_area] =
        Layout::horizontal([Constraint::Fill(1), Constraint::Length(DEBUG_PANEL_WIDTH)])
            .areas(body_area);

    render_header(frame, app, header_area);
    render_transcript(frame, app, transcript_area);
    render_debug(frame, app, debug_area);
    render_timeline(frame, app, timeline_area);
    render_hints(frame, hint_area);
}

fn render_header(frame: &mut Frame, app: &App, area: Rect) {
    let status = if app.paused {
        "⏸ PAUSED"
    } else {
        "▶ PLAYING"
    };
    let text = format!(
        " {} | {} | {}ms/event ",
        app.fixture_name, status, app.speed_ms
    );
    frame.render_widget(
        Paragraph::new(text).style(Style::default().fg(Color::DarkGray)),
        area,
    );
}

fn render_transcript(frame: &mut Frame, app: &App, area: Rect) {
    let frame_data = app.view.frame();
    let mut spans: Vec<Span> = Vec::new();

    for word in &frame_data.final_words {
        spans.push(Span::raw(word.text.clone()));
    }

    for word in &frame_data.partial_words {
        spans.push(Span::styled(
            word.text.clone(),
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC),
        ));
    }

    if !frame_data.partial_words.is_empty() {
        spans.push(Span::styled("▏", Style::default().fg(Color::DarkGray)));
    }

    let lines = if spans.is_empty() {
        vec![]
    } else {
        vec![Line::from(spans)]
    };

    frame.render_widget(
        Paragraph::new(lines)
            .block(Block::default())
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn render_debug(frame: &mut Frame, app: &App, area: Rect) {
    let dbg = app.view.pipeline_debug();
    let frame_data = app.view.frame();

    let block = Block::default()
        .borders(Borders::LEFT)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(Span::styled(
            " pipeline ",
            Style::default().fg(Color::DarkGray),
        ));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let [promotion_area, postprocess_area] =
        Layout::vertical([Constraint::Fill(1), Constraint::Length(4)]).areas(inner);

    render_promotion_section(frame, &dbg.partial_stability, &frame_data, promotion_area);
    render_postprocess_section(frame, dbg.postprocess_applied, postprocess_area);
}

fn render_promotion_section(
    frame: &mut Frame,
    stability: &[(String, u32)],
    frame_data: &transcript::types::TranscriptFrame,
    area: Rect,
) {
    let mut lines = vec![Line::from(Span::styled(
        "promotion",
        Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::UNDERLINED),
    ))];

    if stability.is_empty() {
        lines.push(Line::from(Span::styled(
            "no partials",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        for (text, seen) in stability {
            let bar_width = area.width.saturating_sub(6) as usize;
            let word_display = truncate(text.trim(), bar_width);
            lines.push(Line::from(vec![
                Span::styled(
                    format!("{:<width$}", word_display, width = bar_width),
                    Style::default()
                        .fg(Color::DarkGray)
                        .add_modifier(Modifier::ITALIC),
                ),
                Span::raw(" "),
                Span::styled(
                    format!("×{seen}"),
                    Style::default().fg(if *seen >= 3 {
                        Color::Yellow
                    } else {
                        Color::DarkGray
                    }),
                ),
            ]));
        }
    }

    lines.push(Line::raw(""));
    lines.push(Line::from(vec![
        Span::styled("finals ", Style::default().fg(Color::DarkGray)),
        Span::raw(frame_data.final_words.len().to_string()),
        Span::styled("  partials ", Style::default().fg(Color::DarkGray)),
        Span::raw(frame_data.partial_words.len().to_string()),
    ]));

    if !frame_data.speaker_hints.is_empty() {
        lines.push(Line::from(vec![
            Span::styled("speakers ", Style::default().fg(Color::DarkGray)),
            Span::raw(frame_data.speaker_hints.len().to_string()),
        ]));
    }

    frame.render_widget(Paragraph::new(lines), area);
}

fn render_postprocess_section(frame: &mut Frame, postprocess_applied: usize, area: Rect) {
    let status_style = if postprocess_applied > 0 {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let lines = vec![
        Line::from(Span::styled(
            "postprocess",
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::UNDERLINED),
        )),
        Line::from(vec![
            Span::styled("batches ", Style::default().fg(Color::DarkGray)),
            Span::styled(postprocess_applied.to_string(), status_style),
        ]),
    ];

    frame.render_widget(Paragraph::new(lines), area);
}

fn render_timeline(frame: &mut Frame, app: &App, area: Rect) {
    let total = app.total();
    let ratio = if total == 0 {
        0.0
    } else {
        app.position as f64 / total as f64
    };
    let label = format!("{}/{}", app.position, total);
    let gauge = Gauge::default()
        .gauge_style(Style::default().fg(Color::White).bg(Color::DarkGray))
        .ratio(ratio)
        .label(label);
    frame.render_widget(gauge, area);
}

fn render_hints(frame: &mut Frame, area: Rect) {
    frame.render_widget(
        Paragraph::new(
            " [Space] pause/resume  [←/→] seek  [↑/↓] speed  [Home/End] jump  [q] quit ",
        )
        .style(Style::default().fg(Color::DarkGray)),
        area,
    );
}

fn truncate(s: &str, max_chars: usize) -> &str {
    if s.chars().count() <= max_chars {
        return s;
    }
    let mut end = 0;
    for (i, _) in s.char_indices().take(max_chars) {
        end = i;
    }
    &s[..end]
}
