use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Block, StatefulWidget},
};
use textwrap::wrap;

use super::scrollable::{ScrollState, Scrollable};

pub struct TranscriptEntry {
    pub label: String,
    pub content: String,
    pub label_style: Style,
    pub content_style: Style,
}

pub struct Transcript<'a> {
    entries: Vec<TranscriptEntry>,
    placeholder: Option<Span<'a>>,
    block: Option<Block<'a>>,
}

impl<'a> Transcript<'a> {
    pub fn new(entries: Vec<TranscriptEntry>) -> Self {
        Self {
            entries,
            placeholder: None,
            block: None,
        }
    }

    pub fn placeholder(mut self, span: Span<'a>) -> Self {
        self.placeholder = Some(span);
        self
    }

    pub fn block(mut self, block: Block<'a>) -> Self {
        self.block = Some(block);
        self
    }
}

impl StatefulWidget for Transcript<'_> {
    type State = ScrollState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let block = self.block.unwrap_or_default();
        let inner = block.inner(area);
        let width = inner.width.saturating_sub(2) as usize;
        let wrap_width = width.max(8);

        let mut lines = Vec::new();

        for entry in &self.entries {
            if !lines.is_empty() {
                lines.push(Line::default());
            }
            lines.extend(render_entry(entry, wrap_width));
        }

        if lines.is_empty() {
            if let Some(placeholder) = self.placeholder {
                lines.push(Line::from(placeholder));
            }
        }

        Scrollable::new(lines).block(block).render(area, buf, state);
    }
}

fn render_entry(entry: &TranscriptEntry, width: usize) -> Vec<Line<'static>> {
    let mut lines = vec![Line::from(vec![
        Span::styled(format!("{}: ", entry.label), entry.label_style),
        Span::styled(String::new(), entry.content_style),
    ])];

    let wrapped = wrap(&entry.content, width.saturating_sub(2).max(8));
    if wrapped.is_empty() {
        lines.push(Line::from(Span::styled("  ", entry.content_style)));
    } else {
        lines.extend(
            wrapped
                .into_iter()
                .map(|line| Line::from(Span::styled(format!("  {line}"), entry.content_style))),
        );
    }

    lines
}
