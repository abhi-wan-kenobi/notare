#[cfg(feature = "dev")]
mod imp {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::layout::{Constraint, Direction, Flex, Layout, Rect};
    use ratatui::style::{Color, Style};
    use ratatui::text::{Line, Span};
    use ratatui::widgets::{Block, Borders, Clear, Padding};

    use crate::theme::Theme;
    use crate::widgets::{ScrollViewState, render_scrollable};

    pub(crate) struct Inspector {
        screen: &'static str,
        open: bool,
        scroll: ScrollViewState,
        autoscroll: bool,
    }

    impl Inspector {
        pub(crate) fn new(screen: &'static str) -> Self {
            crate::tui_trace::trace_status(screen, "inspector_ready");
            Self {
                screen,
                open: false,
                scroll: ScrollViewState::new(),
                autoscroll: true,
            }
        }

        pub(crate) fn handle_key(&mut self, key: KeyEvent) -> bool {
            if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('t') {
                self.open = !self.open;
                crate::tui_trace::trace_status(
                    self.screen,
                    if self.open {
                        "overlay_opened"
                    } else {
                        "overlay_closed"
                    },
                );
                if self.open {
                    self.autoscroll = true;
                    self.scroll.scroll_to_bottom();
                }
                return true;
            }

            if !self.open {
                return false;
            }

            match key.code {
                KeyCode::Esc => {
                    self.open = false;
                    crate::tui_trace::trace_status(self.screen, "overlay_closed");
                }
                KeyCode::Char('j') | KeyCode::Down => {
                    self.autoscroll = false;
                    self.scroll.scroll_down();
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    self.autoscroll = false;
                    self.scroll.scroll_up();
                }
                KeyCode::PageDown => {
                    self.autoscroll = false;
                    self.scroll.scroll_page_down();
                }
                KeyCode::PageUp => {
                    self.autoscroll = false;
                    self.scroll.scroll_page_up();
                }
                KeyCode::Char('g') => {
                    self.autoscroll = false;
                    self.scroll.scroll_to_top();
                }
                KeyCode::Char('G') => {
                    self.autoscroll = true;
                    self.scroll.scroll_to_bottom();
                }
                _ => {}
            }

            true
        }

        pub(crate) fn draw(&mut self, frame: &mut ratatui::Frame) {
            if !self.open {
                return;
            }

            let theme = Theme::DEFAULT;
            let backdrop = Block::new().style(Style::new().bg(theme.overlay_bg));
            frame.render_widget(backdrop, frame.area());

            let area = centered_overlay(frame.area());
            frame.render_widget(Clear, area);

            let block = Block::new()
                .style(Style::new().bg(theme.dialog_bg))
                .borders(Borders::ALL)
                .border_style(Style::new().fg(Color::Cyan))
                .title(format!(" Debug Inspector [{}] ", self.screen))
                .title_bottom(Line::from(vec![
                    Span::styled("Ctrl+T", Style::new().fg(Color::Cyan)),
                    Span::raw(" toggle  "),
                    Span::styled("Esc", Style::new().fg(Color::Cyan)),
                    Span::raw(" close  "),
                    Span::styled("j/k", Style::new().fg(Color::Cyan)),
                    Span::raw(" scroll"),
                ]))
                .padding(Padding::new(1, 1, 0, 0));

            let lines = crate::tui_trace::capture().snapshot_lines_for_screen(self.screen);
            if self.autoscroll {
                self.scroll.scroll_to_bottom();
            }
            render_scrollable(frame, lines, Some(block), area, &mut self.scroll);
        }
    }

    fn centered_overlay(area: Rect) -> Rect {
        let vertical = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(10),
                Constraint::Percentage(80),
                Constraint::Percentage(10),
            ])
            .flex(Flex::Center)
            .split(area);
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(8),
                Constraint::Percentage(84),
                Constraint::Percentage(8),
            ])
            .flex(Flex::Center)
            .split(vertical[1])[1]
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

        #[test]
        fn capture_evicts_oldest_records() {
            let capture = InteractionCapture::new();
            for idx in 0..(CAPACITY + 5) {
                capture.push("chat", InteractionKind::Input, format!("record {idx}"));
            }
            let lines = capture.snapshot();
            assert_eq!(lines.len(), CAPACITY);
        }

        #[test]
        fn ctrl_t_toggles_overlay() {
            let mut inspector = Inspector::new("chat");
            let toggle = KeyEvent::new(KeyCode::Char('t'), KeyModifiers::CONTROL);
            assert!(inspector.handle_key(toggle));
            assert!(inspector.open);
            assert!(inspector.handle_key(toggle));
            assert!(!inspector.open);
        }

        #[test]
        fn esc_only_closes_overlay() {
            let mut inspector = Inspector::new("chat");
            let toggle = KeyEvent::new(KeyCode::Char('t'), KeyModifiers::CONTROL);
            inspector.handle_key(toggle);
            assert!(inspector.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)));
            assert!(!inspector.open);
        }
    }
}

#[cfg(not(feature = "dev"))]
mod imp {
    use crossterm::event::KeyEvent;

    pub(crate) struct Inspector;

    impl Inspector {
        pub(crate) fn new(_screen: &'static str) -> Self {
            Self
        }

        pub(crate) fn handle_key(&mut self, _key: KeyEvent) -> bool {
            false
        }

        pub(crate) fn draw(&mut self, _frame: &mut ratatui::Frame) {}
    }
}

pub(crate) use imp::Inspector;
