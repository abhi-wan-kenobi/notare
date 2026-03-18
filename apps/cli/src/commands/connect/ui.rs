use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Position, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, ListItem, Paragraph};

use crate::theme::Theme;
use crate::widgets::{CenteredDialog, KeyHints, SelectList};

use super::app::{App, Step};

pub(crate) fn draw(frame: &mut Frame, app: &mut App) {
    let theme = Theme::DEFAULT;

    let inner = CenteredDialog::new("Connect a provider", &theme).render(frame);

    let [header_area, content_area, status_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(1),
        Constraint::Length(1),
    ])
    .areas(inner);

    draw_header(frame, app, header_area);

    match app.step() {
        Step::SelectType => draw_type_list(frame, app, content_area, &theme),
        Step::SelectProvider => draw_provider_list(frame, app, content_area, &theme),
        Step::InputBaseUrl | Step::InputApiKey => draw_input(frame, app, content_area, &theme),
        Step::Done => {}
    }

    draw_status(frame, app, status_area, &theme);
}

fn draw_header(frame: &mut Frame, app: &App, area: Rect) {
    let breadcrumb = app.breadcrumb();
    if breadcrumb.is_empty() {
        return;
    }
    frame.render_widget(
        Line::from(Span::styled(
            format!("  {breadcrumb}"),
            Style::new().fg(Color::DarkGray),
        )),
        area,
    );
}

fn draw_type_list(frame: &mut Frame, app: &mut App, area: Rect, theme: &Theme) {
    let current_llm = app.current_llm_provider().map(str::to_string);
    let current_stt = app.current_stt_provider().map(str::to_string);

    let items: Vec<ListItem> = [("LLM", current_llm), ("STT", current_stt)]
        .into_iter()
        .map(|(label, current)| {
            let mut spans = vec![Span::raw(label)];
            if let Some(provider) = current {
                spans.push(Span::styled(format!("  {provider}"), theme.muted));
            }
            ListItem::new(Line::from(spans))
        })
        .collect();

    frame.render_stateful_widget(SelectList::new(items, theme), area, app.list_state_mut());
}

fn draw_provider_list(frame: &mut Frame, app: &mut App, area: Rect, theme: &Theme) {
    let entries = app.provider_entries();
    let items: Vec<ListItem> = entries
        .iter()
        .map(|p| ListItem::new(p.id().to_string()))
        .collect();

    frame.render_stateful_widget(SelectList::new(items, theme), area, app.list_state_mut());
}

// --- Data layer: describe what to render ---

enum Section {
    Label(String),
    Input { text: String, cursor_x: u16 },
    Default(String),
    Error(String),
}

fn input_sections(app: &App) -> Vec<Section> {
    let display_text = if app.input_masked() && !app.input().is_empty() {
        "*".repeat(app.input().chars().count())
    } else {
        app.input().to_string()
    };

    #[allow(clippy::cast_possible_truncation)]
    let cursor_x = app.cursor_pos() as u16;

    let mut out = vec![
        Section::Label(format!("  {}:", app.input_label())),
        Section::Input {
            text: display_text,
            cursor_x,
        },
    ];

    if let Some(default) = app.input_default() {
        out.push(Section::Default(format!("  default: {default}")));
    }

    if let Some(error) = app.error() {
        out.push(Section::Error(format!("  {error}")));
    }

    out
}

// --- View layer: how to render each section ---

fn section_constraint(section: &Section) -> Constraint {
    match section {
        Section::Input { .. } => Constraint::Length(3),
        _ => Constraint::Length(1),
    }
}

fn render_section(frame: &mut Frame, section: &Section, area: Rect, theme: &Theme) {
    match section {
        Section::Label(text) => {
            frame.render_widget(Span::styled(text.as_str(), Style::new().bold()), area);
        }
        Section::Input { text, cursor_x } => {
            let input_block = Block::bordered().border_style(Style::new().fg(Color::Cyan));
            let inner = input_block.inner(area);
            frame.render_widget(Paragraph::new(text.as_str()).block(input_block), area);
            frame.set_cursor_position(Position::new(inner.x + cursor_x, inner.y));
        }
        Section::Default(text) => {
            frame.render_widget(
                Span::styled(text.as_str(), Style::new().fg(Color::DarkGray)),
                area,
            );
        }
        Section::Error(text) => {
            frame.render_widget(Span::styled(text.as_str(), theme.error), area);
        }
    }
}

fn draw_input(frame: &mut Frame, app: &mut App, area: Rect, theme: &Theme) {
    let sections = input_sections(app);

    let mut constraints: Vec<Constraint> = sections.iter().map(section_constraint).collect();
    constraints.push(Constraint::Min(0));

    let areas = Layout::vertical(constraints).split(area);

    for (section, &area) in sections.iter().zip(areas.iter()) {
        render_section(frame, section, area, theme);
    }
}

fn draw_status(frame: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    let hints = match app.step() {
        Step::SelectType | Step::SelectProvider => vec![],
        Step::InputBaseUrl | Step::InputApiKey => {
            vec![("Enter", "confirm"), ("Esc", "quit")]
        }
        Step::Done => vec![],
    };

    frame.render_widget(KeyHints::new(theme).hints(hints), area);
}
