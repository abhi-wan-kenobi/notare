use std::ops::Range;

use ratatui::style::{Color, Modifier, Style};
use regex::Regex;

pub(crate) struct Highlighter {
    heading: Regex,
    bold: Regex,
    italic: Regex,
    inline_code: Regex,
    list_marker: Regex,
    code_fence: Regex,
}

impl Highlighter {
    pub fn new() -> Self {
        Self {
            heading: Regex::new(r"^#{1,6}\s").unwrap(),
            bold: Regex::new(r"\*\*[^*]+\*\*").unwrap(),
            italic: Regex::new(r"\*[^*]+\*").unwrap(),
            inline_code: Regex::new(r"`[^`]+`").unwrap(),
            list_marker: Regex::new(r"^(\s*[-*]\s|\s*\d+\.\s)").unwrap(),
            code_fence: Regex::new(r"^```").unwrap(),
        }
    }

    pub fn highlight(&self, lines: &[String]) -> Vec<Vec<(Range<usize>, Style)>> {
        let mut result = Vec::with_capacity(lines.len());
        let mut in_code_block = false;

        for line in lines {
            let mut spans = Vec::new();

            if self.code_fence.is_match(line) {
                in_code_block = !in_code_block;
                if !line.is_empty() {
                    spans.push((0..line.len(), style_code()));
                }
                result.push(spans);
                continue;
            }

            if in_code_block {
                if !line.is_empty() {
                    spans.push((0..line.len(), style_code()));
                }
                result.push(spans);
                continue;
            }

            if self.heading.is_match(line) {
                let level = line.chars().take_while(|&c| c == '#').count();
                spans.push((0..line.len(), heading_style(level)));
                result.push(spans);
                continue;
            }

            let mut used = vec![false; line.len()];

            if line.starts_with("> ") {
                spans.push((0..2, style_blockquote()));
                mark_used(&mut used, 0, 2);
            }

            if let Some(m) = self.list_marker.find(line) {
                if !any_used(&used, m.start(), m.end()) {
                    spans.push((m.start()..m.end(), style_list_marker()));
                    mark_used(&mut used, m.start(), m.end());
                }
            }

            for m in self.inline_code.find_iter(line) {
                if !any_used(&used, m.start(), m.end()) {
                    spans.push((m.start()..m.end(), style_code()));
                    mark_used(&mut used, m.start(), m.end());
                }
            }

            for m in self.bold.find_iter(line) {
                if !any_used(&used, m.start(), m.end()) {
                    spans.push((m.start()..m.end(), style_bold()));
                    mark_used(&mut used, m.start(), m.end());
                }
            }

            for m in self.italic.find_iter(line) {
                if !any_used(&used, m.start(), m.end()) {
                    spans.push((m.start()..m.end(), style_italic()));
                    mark_used(&mut used, m.start(), m.end());
                }
            }

            result.push(spans);
        }

        result
    }
}

fn mark_used(used: &mut [bool], start: usize, end: usize) {
    for b in &mut used[start..end] {
        *b = true;
    }
}

fn any_used(used: &[bool], start: usize, end: usize) -> bool {
    used[start..end].iter().any(|&u| u)
}

fn heading_style(level: usize) -> Style {
    let color = match level {
        1 => Color::Cyan,
        2 => Color::Blue,
        3 => Color::Green,
        4 => Color::Yellow,
        _ => Color::Magenta,
    };
    Style::new().fg(color).add_modifier(Modifier::BOLD)
}

fn style_bold() -> Style {
    Style::new().add_modifier(Modifier::BOLD)
}

fn style_italic() -> Style {
    Style::new().add_modifier(Modifier::ITALIC)
}

fn style_code() -> Style {
    Style::new().fg(Color::Gray)
}

fn style_list_marker() -> Style {
    Style::new().fg(Color::Cyan)
}

fn style_blockquote() -> Style {
    Style::new()
        .fg(Color::DarkGray)
        .add_modifier(Modifier::ITALIC)
}
