use std::{
    io,
    ops::RangeInclusive,
    path::Path,
    time::{Duration, Instant, SystemTime},
};

use crossterm::{
    event::{
        self, Event as CrosstermEvent, KeyCode, KeyEvent, KeyEventKind, MouseEvent, MouseEventKind,
    },
    execute,
};
use hypr_activity_capture::{
    ActivityCapture, Capabilities, EventCoalescer, PlatformCapture, Transition,
};
use ratatui::{DefaultTerminal, Frame, layout::Rect, widgets::ListState};

use crate::{
    event_row::EventRow,
    export::{ExportScope, RawRecord, copy_records, save_records},
    options::Options,
    theme::Theme,
    widgets::ActivityScreen,
};

const UI_IDLE_POLL: Duration = Duration::from_millis(250);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum View {
    List,
    Details,
}

pub(crate) fn run(options: Options, color_enabled: bool) -> io::Result<()> {
    let _mouse_capture = MouseCaptureGuard::enable()?;
    ratatui::run(|terminal| {
        let app = ActivityApp::new(options, Theme::new(color_enabled))?;
        app.run(terminal)
    })
}

struct MouseCaptureGuard;

impl MouseCaptureGuard {
    fn enable() -> io::Result<Self> {
        execute!(std::io::stdout(), crossterm::event::EnableMouseCapture)?;
        Ok(Self)
    }
}

impl Drop for MouseCaptureGuard {
    fn drop(&mut self) {
        let _ = execute!(std::io::stdout(), crossterm::event::DisableMouseCapture);
    }
}

struct ActivityApp {
    capture: PlatformCapture,
    options: Options,
    capabilities: Capabilities,
    theme: Theme,
    coalescer: EventCoalescer,
    events: Vec<EventRow>,
    raw_records: Vec<RawRecord>,
    list_state: ListState,
    selection_anchor: Option<usize>,
    status_message: Option<String>,
    view: View,
    should_exit: bool,
    next_capture_at: Option<Instant>,
    list_inner_area: Rect,
}

impl ActivityApp {
    fn new(options: Options, theme: Theme) -> io::Result<Self> {
        let capture = PlatformCapture::with_policy(options.policy());
        let capabilities = capture.capabilities();
        let mut app = Self {
            capture,
            options,
            capabilities,
            theme,
            coalescer: EventCoalescer::default(),
            events: Vec::new(),
            raw_records: Vec::new(),
            list_state: ListState::default(),
            selection_anchor: None,
            status_message: None,
            view: View::List,
            should_exit: false,
            next_capture_at: None,
            list_inner_area: Rect::default(),
        };

        app.capture_once()?;
        app.next_capture_at =
            (!app.options.once).then_some(Instant::now() + app.options.poll_interval());
        Ok(app)
    }

    fn run(mut self, terminal: &mut DefaultTerminal) -> io::Result<()> {
        while !self.should_exit {
            terminal.draw(|frame| self.render(frame))?;

            let timeout = self.poll_timeout();
            if event::poll(timeout)? {
                self.handle_terminal_event(event::read()?);
            } else if self.next_capture_at.is_some() {
                self.capture_once()?;
                self.next_capture_at = Some(Instant::now() + self.options.poll_interval());
            }
        }

        Ok(())
    }

    fn poll_timeout(&self) -> Duration {
        match self.next_capture_at {
            Some(deadline) => deadline.saturating_duration_since(Instant::now()),
            None => UI_IDLE_POLL,
        }
    }

    fn capture_once(&mut self) -> io::Result<()> {
        let snapshot = self
            .capture
            .snapshot()
            .map_err(|error| io::Error::other(error.to_string()))?;

        match snapshot {
            Some(snapshot) => {
                if let Some(transition) = self.coalescer.push(Some(snapshot)) {
                    self.push_transition(transition);
                }
            }
            None if self.events.is_empty() => {
                let row = EventRow::idle(SystemTime::now(), None);
                let record = RawRecord::placeholder(&row, "capture returned no snapshot");
                self.push_row(row, record);
            }
            None => {
                if let Some(transition) = self.coalescer.push(None) {
                    self.push_transition(transition);
                }
            }
        }

        Ok(())
    }

    fn handle_terminal_event(&mut self, event: CrosstermEvent) {
        match event {
            CrosstermEvent::Key(key) if key.kind == KeyEventKind::Press => self.handle_key(key),
            CrosstermEvent::Mouse(mouse) => self.handle_mouse(mouse),
            CrosstermEvent::Resize(_, _) => {}
            _ => {}
        }
    }

    fn handle_key(&mut self, key: KeyEvent) {
        match self.view {
            View::List => self.handle_list_key(key),
            View::Details => self.handle_details_key(key),
        }
    }

    fn handle_list_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => self.should_exit = true,
            KeyCode::Char('j') | KeyCode::Down => self.select_next(),
            KeyCode::Char('k') | KeyCode::Up => self.select_previous(),
            KeyCode::Char('g') | KeyCode::Home => self.select_first(),
            KeyCode::Char('G') | KeyCode::End => self.select_last(),
            KeyCode::Char('v') => self.toggle_selection_anchor(),
            KeyCode::Char('y') => self.copy_selection(),
            KeyCode::Char('s') => self.save_selection(),
            KeyCode::Char('S') => self.save_session(),
            KeyCode::Char('l') | KeyCode::Right | KeyCode::Enter => self.view = View::Details,
            _ => {}
        }
    }

    fn handle_details_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('q') => self.should_exit = true,
            KeyCode::Esc | KeyCode::Backspace | KeyCode::Left | KeyCode::Char('h') => {
                self.view = View::List;
            }
            KeyCode::Char('j') | KeyCode::Down => self.select_next(),
            KeyCode::Char('k') | KeyCode::Up => self.select_previous(),
            KeyCode::Char('g') | KeyCode::Home => self.select_first(),
            KeyCode::Char('G') | KeyCode::End => self.select_last(),
            KeyCode::Char('y') => self.copy_selection(),
            KeyCode::Char('s') => self.save_selection(),
            KeyCode::Char('S') => self.save_session(),
            _ => {}
        }
    }

    fn handle_mouse(&mut self, mouse: MouseEvent) {
        if self.view != View::List {
            return;
        }

        match mouse.kind {
            MouseEventKind::Down(_) => {
                if let Some(index) = self.row_at(mouse.column, mouse.row) {
                    self.select_index(index);
                    self.view = View::Details;
                }
            }
            MouseEventKind::ScrollDown => {
                if self.row_hit_area(mouse.column, mouse.row) {
                    self.select_next();
                }
            }
            MouseEventKind::ScrollUp => {
                if self.row_hit_area(mouse.column, mouse.row) {
                    self.select_previous();
                }
            }
            _ => {}
        }
    }

    fn row_hit_area(&self, column: u16, row: u16) -> bool {
        let area = self.list_inner_area;
        column >= area.x
            && column < area.x.saturating_add(area.width)
            && row >= area.y
            && row < area.y.saturating_add(area.height)
    }

    fn row_at(&self, column: u16, row: u16) -> Option<usize> {
        if !self.row_hit_area(column, row) {
            return None;
        }

        let relative_row = usize::from(row.saturating_sub(self.list_inner_area.y));
        let index = self.list_state.offset() + relative_row;
        (index < self.events.len()).then_some(index)
    }

    fn push_transition(&mut self, transition: Transition) {
        if let Some(row) = EventRow::from_transition(&transition) {
            let record = RawRecord::from_transition(&row, transition);
            self.push_row(row, record);
        }
    }

    fn push_row(&mut self, row: EventRow, raw_record: RawRecord) {
        let should_follow_tail = matches!(self.view, View::List)
            && self
                .selected_index()
                .is_none_or(|index| index + 1 >= self.events.len());

        self.events.push(row);
        self.raw_records.push(raw_record);

        if should_follow_tail || self.events.len() == 1 {
            self.select_last();
        }
    }

    fn selected_index(&self) -> Option<usize> {
        self.list_state
            .selected()
            .filter(|index| *index < self.events.len())
    }

    fn select_index(&mut self, index: usize) {
        if self.events.is_empty() {
            self.list_state.select(None);
        } else {
            self.list_state
                .select(Some(index.min(self.events.len() - 1)));
        }
    }

    fn select_first(&mut self) {
        if !self.events.is_empty() {
            self.list_state.select(Some(0));
        }
    }

    fn select_last(&mut self) {
        if !self.events.is_empty() {
            self.list_state.select(Some(self.events.len() - 1));
        }
    }

    fn select_next(&mut self) {
        if self.events.is_empty() {
            return;
        }

        let next = self
            .selected_index()
            .map_or(0, |index| (index + 1).min(self.events.len() - 1));
        self.select_index(next);
    }

    fn select_previous(&mut self) {
        if self.events.is_empty() {
            return;
        }

        let previous = self
            .selected_index()
            .map_or(0, |index| index.saturating_sub(1));
        self.select_index(previous);
    }

    fn toggle_selection_anchor(&mut self) {
        match (self.selection_anchor, self.selected_index()) {
            (Some(_), _) => {
                self.selection_anchor = None;
                self.status_message = Some("selection cleared".to_string());
            }
            (None, Some(index)) => {
                self.selection_anchor = Some(index);
                self.status_message = Some(format!("selection started at row {}", index + 1));
            }
            (None, None) => {
                self.status_message = Some("nothing selected".to_string());
            }
        }
    }

    fn copy_selection(&mut self) {
        let Some(range) = self.export_range() else {
            self.status_message = Some("nothing to copy".to_string());
            return;
        };

        let count = range_len(&range);
        match copy_records(&self.raw_records, range, ExportScope::Selection) {
            Ok(_) => {
                self.status_message = Some(format!("copied {count} raw record(s) as JSON"));
            }
            Err(error) => {
                self.status_message = Some(format!("copy failed: {error}"));
            }
        }
    }

    fn save_selection(&mut self) {
        let Some(range) = self.export_range() else {
            self.status_message = Some("nothing to save".to_string());
            return;
        };

        let count = range_len(&range);
        match save_records(&self.raw_records, range, ExportScope::Selection) {
            Ok(path) => {
                self.status_message = Some(format!(
                    "saved {count} raw record(s) to {}",
                    file_label(&path)
                ));
            }
            Err(error) => {
                self.status_message = Some(format!("save failed: {error}"));
            }
        }
    }

    fn save_session(&mut self) {
        if self.raw_records.is_empty() {
            self.status_message = Some("nothing to save".to_string());
            return;
        }

        let range = 0..=self.raw_records.len() - 1;
        match save_records(&self.raw_records, range.clone(), ExportScope::Session) {
            Ok(path) => {
                self.status_message = Some(format!(
                    "saved {} raw record(s) to {}",
                    range_len(&range),
                    file_label(&path)
                ));
            }
            Err(error) => {
                self.status_message = Some(format!("save failed: {error}"));
            }
        }
    }

    fn export_range(&self) -> Option<RangeInclusive<usize>> {
        let current = self.selected_index()?;
        let anchor = self.selection_anchor.unwrap_or(current);
        Some(anchor.min(current)..=anchor.max(current))
    }

    fn selection_summary(&self) -> Option<String> {
        let range = self.export_range()?;
        self.selection_anchor.map(|_| {
            format!(
                "range={}..{} ({} rows)",
                range.start() + 1,
                range.end() + 1,
                range_len(&range)
            )
        })
    }

    fn render(&mut self, frame: &mut Frame) {
        let selected_index = self.selected_index();
        frame.render_widget(
            ActivityScreen::new(
                &self.options,
                self.capabilities,
                self.theme,
                self.view,
                &self.events,
                selected_index,
                self.export_range(),
                self.selection_summary().as_deref(),
                self.status_message.as_deref(),
                &mut self.list_state,
                &mut self.list_inner_area,
            ),
            frame.area(),
        );
    }
}

fn range_len(range: &RangeInclusive<usize>) -> usize {
    range.end() - range.start() + 1
}

fn file_label(path: &Path) -> String {
    path.file_name()
        .and_then(|value| value.to_str())
        .map(str::to_owned)
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}
