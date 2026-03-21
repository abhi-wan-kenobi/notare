pub use hypr_db_app::human_cli::Commands;

use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use sqlx::SqlitePool;

use crate::error::{CliError, CliResult};
use crate::widgets::InlineBox;

pub async fn run(pool: &SqlitePool, command: Option<Commands>) -> CliResult<()> {
    match command {
        Some(Commands::Show { id, format }) => match format {
            hypr_db_app::human_cli::OutputFormat::Json => {
                Ok(hypr_db_app::human_cli::show(pool, &id, format).await?)
            }
            _ => show_pretty(pool, &id).await,
        },
        other => Ok(hypr_db_app::human_cli::run(pool, other).await?),
    }
}

async fn show_pretty(pool: &SqlitePool, id: &str) -> CliResult<()> {
    let dim = Style::default().add_modifier(Modifier::DIM);
    let bold = Style::default().add_modifier(Modifier::BOLD);

    match hypr_db_app::get_human(pool, id).await {
        Ok(Some(h)) => {
            let mut lines = vec![
                Line::from(Span::styled(h.name, bold)),
                Line::from(vec![Span::styled("email      ", dim), Span::raw(h.email)]),
                Line::from(vec![Span::styled("org        ", dim), Span::raw(h.org_id)]),
                Line::from(vec![
                    Span::styled("title      ", dim),
                    Span::raw(h.job_title),
                ]),
                Line::from(vec![
                    Span::styled("created    ", dim),
                    Span::raw(h.created_at),
                ]),
            ];

            if let Ok(events) = hypr_db_app::list_events_by_human(pool, id).await {
                if !events.is_empty() {
                    lines.push(Line::raw(""));
                    lines.push(Line::from(Span::styled("Recent events", bold)));
                    for event in events.into_iter().take(10) {
                        let date = if event.started_at.len() >= 16 {
                            event.started_at[..16].replace('T', " ")
                        } else {
                            event.started_at.replace('T', " ")
                        };
                        lines.push(Line::from(vec![
                            Span::styled(format!("{date}  "), dim),
                            Span::raw(event.title),
                        ]));
                    }
                }
            }

            if let Ok(participants) = hypr_db_app::list_meetings_by_human(pool, id).await {
                if !participants.is_empty() {
                    lines.push(Line::raw(""));
                    lines.push(Line::from(Span::styled("Meetings", bold)));
                    for p in participants {
                        let title = hypr_db_app::get_meeting(pool, &p.meeting_id)
                            .await
                            .ok()
                            .flatten()
                            .and_then(|m| m.title)
                            .unwrap_or_default();
                        lines.push(Line::from(vec![
                            Span::styled(format!("{}  ", p.meeting_id), dim),
                            Span::raw(title),
                        ]));
                    }
                }
            }

            render_info(lines)
        }
        Ok(None) => Err(CliError::msg(format!("human '{id}' not found"))),
        Err(e) => Err(CliError::operation_failed("query", e.to_string())),
    }
}

fn render_info(lines: Vec<Line<'static>>) -> CliResult<()> {
    let height = InlineBox::viewport_height(lines.len() as u16);
    hypr_cli_tui::render_inline(height, |frame| {
        let inner = InlineBox::render(frame);
        frame.render_widget(ratatui::widgets::Paragraph::new(lines), inner);
    })
    .map_err(|e| CliError::operation_failed("render", e.to_string()))
}
