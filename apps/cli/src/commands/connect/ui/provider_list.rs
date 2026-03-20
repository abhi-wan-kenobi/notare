use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Position, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{ListItem, Paragraph};

use crate::theme::Theme;
use crate::widgets::SelectList;

use super::super::app::App;
use super::super::{ConnectProvider, ConnectionType};

const SEARCH_HEIGHT: u16 = 3;
const NAME_TAG_BOUNDARY: usize = 2;
const MIN_NAME_TAG_SPACER: usize = 1;
const TAG_BOUNDARY: &str = " ";

pub(crate) fn draw(frame: &mut Frame, app: &mut App, area: Rect, theme: &Theme) {
    let [search_area, list_area] =
        Layout::vertical([Constraint::Length(SEARCH_HEIGHT), Constraint::Min(1)]).areas(area);

    draw_search_input(frame, app, search_area, theme);

    let filtered = app.filtered_providers();
    if filtered.is_empty() {
        draw_empty_state(frame, list_area, theme);
        return;
    }

    let configured = app.configured_providers();
    let current_stt = app.current_stt_provider();
    let current_llm = app.current_llm_provider();
    let available_width = list_area.width as usize;
    let items: Vec<ListItem> = filtered
        .iter()
        .map(|provider| {
            build_provider_item(
                provider,
                configured.contains(provider.id()),
                current_stt == Some(provider.id()),
                current_llm == Some(provider.id()),
                available_width,
                theme,
            )
        })
        .collect();

    frame.render_stateful_widget(
        SelectList::new(items, theme),
        list_area,
        app.list_state_mut(),
    );
}

fn draw_search_input(frame: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    let search_block = theme.bordered_block(true).title(" Search ");
    let search_inner = search_block.inner(area);

    frame.render_widget(Paragraph::new(app.search_query()).block(search_block), area);

    #[allow(clippy::cast_possible_truncation)]
    let cursor_x = app.search_query().chars().count() as u16;
    frame.set_cursor_position(Position::new(search_inner.x + cursor_x, search_inner.y));
}

fn draw_empty_state(frame: &mut Frame, area: Rect, theme: &Theme) {
    frame.render_widget(Span::styled("  No matches", theme.muted), area);
}

fn build_provider_item(
    provider: &ConnectProvider,
    is_configured: bool,
    is_active_stt: bool,
    is_active_llm: bool,
    available_width: usize,
    theme: &Theme,
) -> ListItem<'static> {
    let disabled = provider.is_disabled();
    let name = provider.display_name();
    let tag_spans = build_tag_spans(
        provider.capabilities(),
        disabled,
        is_configured,
        is_active_stt,
        is_active_llm,
        theme,
    );
    let tags_width: usize = tag_spans.iter().map(|span| span.width()).sum();
    let name_width = name.len();
    let spacer_width = available_width
        .saturating_sub(name_width + tags_width + NAME_TAG_BOUNDARY)
        .max(MIN_NAME_TAG_SPACER);
    let name_style = if disabled {
        theme.muted
    } else {
        Style::default()
    };

    let mut spans = vec![
        Span::styled(name.to_string(), name_style),
        Span::raw(" ".repeat(spacer_width)),
    ];
    spans.extend(tag_spans);

    ListItem::new(Line::from(spans))
}

fn build_tag_spans(
    capabilities: Vec<ConnectionType>,
    disabled: bool,
    is_configured: bool,
    is_active_stt: bool,
    is_active_llm: bool,
    theme: &Theme,
) -> Vec<Span<'static>> {
    let mut segments = Vec::new();

    if is_configured {
        push_tag_segment(&mut segments, Span::styled("✓", theme.status.active));
    }

    if disabled {
        push_tag_segment(&mut segments, Span::styled("soon", theme.muted));
    }

    for capability in capabilities {
        let is_active = match capability {
            ConnectionType::Stt => is_active_stt,
            ConnectionType::Llm => is_active_llm,
            ConnectionType::Cal => false,
        };
        push_tag_segment(
            &mut segments,
            Span::styled(
                capability_label(capability),
                capability_style(capability, disabled, is_active, theme),
            ),
        );
    }

    segments
}

fn push_tag_segment(segments: &mut Vec<Span<'static>>, span: Span<'static>) {
    if !segments.is_empty() {
        segments.push(Span::raw(TAG_BOUNDARY));
    }
    segments.push(span);
}

fn capability_label(capability: ConnectionType) -> &'static str {
    match capability {
        ConnectionType::Stt => "[STT]",
        ConnectionType::Llm => "[LLM]",
        ConnectionType::Cal => "[CAL]",
    }
}

fn capability_style(
    capability: ConnectionType,
    disabled: bool,
    is_active: bool,
    theme: &Theme,
) -> Style {
    if disabled {
        return theme.muted;
    }

    if !is_active {
        return theme.muted;
    }

    match capability {
        ConnectionType::Stt => theme.capability.stt,
        ConnectionType::Llm => theme.capability.llm,
        ConnectionType::Cal => theme.capability.cal,
    }
}
