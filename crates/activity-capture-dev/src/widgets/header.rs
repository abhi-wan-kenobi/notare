use hypr_activity_capture::Capabilities;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    text::{Line, Span, Text},
    widgets::{Block, Paragraph, Widget},
};

use crate::{app::View, options::Options, theme::Theme};

pub(super) struct SessionHeader<'a> {
    options: &'a Options,
    capabilities: Capabilities,
    theme: Theme,
    view: View,
    selection_summary: Option<&'a str>,
    status_message: Option<&'a str>,
}

impl<'a> SessionHeader<'a> {
    pub(super) fn new(
        options: &'a Options,
        capabilities: Capabilities,
        theme: Theme,
        view: View,
        selection_summary: Option<&'a str>,
        status_message: Option<&'a str>,
    ) -> Self {
        Self {
            options,
            capabilities,
            theme,
            view,
            selection_summary,
            status_message,
        }
    }
}

impl Widget for SessionHeader<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let note = match self.selection_summary {
            Some(summary) => format!("  one line per event; press v to mark a range  {summary}"),
            None => "  one line per event; press v to mark a range".to_string(),
        };
        let export = self.status_message.map(str::to_string).unwrap_or_else(|| {
            "  y copy JSON  s save selection/current  S save full session".to_string()
        });

        let lines = vec![
            Line::from(vec![
                Span::styled("activity-capture", self.theme.title()),
                Span::raw(format!("  poll={}ms", self.options.poll_ms)),
                Span::raw(format!("  policy={}", self.options.policy_label())),
                Span::raw(match self.view {
                    View::List => "  view=list",
                    View::Details => "  view=details",
                }),
            ]),
            Line::from(vec![
                Span::styled("capabilities", self.theme.label()),
                Span::raw(format!(
                    "  watch={} text={} url={} ax={}",
                    yes_no(self.capabilities.can_watch),
                    yes_no(self.capabilities.can_capture_visible_text),
                    yes_no(self.capabilities.can_capture_browser_url),
                    yes_no(self.capabilities.requires_accessibility_permission),
                )),
            ]),
            Line::from(vec![
                Span::styled("note", self.theme.label()),
                Span::raw(note),
            ]),
            Line::from(vec![
                Span::styled("export", self.theme.label()),
                Span::raw(export),
            ]),
        ];

        Paragraph::new(Text::from(lines))
            .block(Block::bordered().title("Session"))
            .render(area, buf);
    }
}

fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}
