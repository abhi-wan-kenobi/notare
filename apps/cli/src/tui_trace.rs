#[cfg(feature = "dev")]
mod imp {
    use std::collections::{BTreeMap, VecDeque};
    use std::sync::{Arc, Mutex, OnceLock};

    use chrono::Local;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::style::{Color, Modifier, Style};
    use ratatui::text::{Line, Span};
    use tracing::field::{Field, Visit};
    use tracing::{Event, Subscriber};
    use tracing_subscriber::filter::LevelFilter;
    use tracing_subscriber::layer::{Context, Layer, SubscriberExt};
    use tracing_subscriber::prelude::*;
    use tracing_subscriber::registry::LookupSpan;
    use tracing_subscriber::{EnvFilter, fmt};

    const CAPACITY: usize = 2_000;
    const TUI_TARGET: &str = "char.tui";

    static CAPTURE: OnceLock<Arc<TraceCapture>> = OnceLock::new();

    #[derive(Clone)]
    pub(crate) struct TraceCapture {
        records: Arc<Mutex<VecDeque<TraceRecord>>>,
    }

    #[derive(Clone)]
    struct TraceRecord {
        timestamp: String,
        level: tracing::Level,
        target: String,
        screen: Option<String>,
        phase: Option<String>,
        name: Option<String>,
        message: Option<String>,
        detail: Option<String>,
    }

    impl TraceCapture {
        fn new() -> Self {
            Self {
                records: Arc::new(Mutex::new(VecDeque::with_capacity(CAPACITY))),
            }
        }

        fn push(&self, record: TraceRecord) {
            let mut records = self.records.lock().unwrap();
            if records.len() >= CAPACITY {
                records.pop_front();
            }
            records.push_back(record);
        }

        pub(crate) fn snapshot_lines_for_screen(&self, screen: &str) -> Vec<Line<'static>> {
            let records = self.records.lock().unwrap();
            records
                .iter()
                .filter(|record| record.screen.as_deref() == Some(screen))
                .map(stylize_record)
                .collect()
        }

        pub(crate) fn snapshot_lines_all(&self) -> Vec<Line<'static>> {
            let records = self.records.lock().unwrap();
            records.iter().map(stylize_record).collect()
        }
    }

    pub(crate) fn capture() -> Arc<TraceCapture> {
        CAPTURE
            .get_or_init(|| Arc::new(TraceCapture::new()))
            .clone()
    }

    pub(crate) fn init(tui_command: bool, default_level: LevelFilter) {
        let stderr_default = if tui_command {
            LevelFilter::OFF
        } else {
            default_level
        };
        let capture_default = if tui_command {
            LevelFilter::DEBUG
        } else {
            LevelFilter::OFF
        };

        tracing_subscriber::registry()
            .with(
                fmt::layer().with_writer(std::io::stderr).with_filter(
                    EnvFilter::builder()
                        .with_default_directive(stderr_default.into())
                        .from_env_lossy(),
                ),
            )
            .with(
                CaptureLayer::new(capture()).with_filter(
                    EnvFilter::builder()
                        .with_default_directive(capture_default.into())
                        .from_env_lossy(),
                ),
            )
            .init();
    }

    pub(crate) fn trace_input_key(screen: &'static str, key: &KeyEvent) {
        tracing::debug!(
            target: TUI_TARGET,
            screen,
            phase = "input",
            name = "Key",
            key = %format_key(key)
        );
    }

    pub(crate) fn trace_input_paste(screen: &'static str, len: usize) {
        tracing::debug!(
            target: TUI_TARGET,
            screen,
            phase = "input",
            name = "Paste",
            len
        );
    }

    pub(crate) fn trace_action(screen: &'static str, name: &'static str) {
        tracing::debug!(target: TUI_TARGET, screen, phase = "action", name);
    }

    pub(crate) fn trace_effect(screen: &'static str, name: &'static str) {
        tracing::debug!(target: TUI_TARGET, screen, phase = "effect", name);
    }

    pub(crate) fn trace_external(screen: &'static str, name: &'static str) {
        tracing::debug!(target: TUI_TARGET, screen, phase = "external", name);
    }

    pub(crate) fn trace_status(screen: &'static str, name: &'static str) {
        tracing::debug!(target: TUI_TARGET, screen, phase = "status", name);
    }

    struct CaptureLayer {
        capture: Arc<TraceCapture>,
    }

    impl CaptureLayer {
        fn new(capture: Arc<TraceCapture>) -> Self {
            Self { capture }
        }
    }

    impl<S> Layer<S> for CaptureLayer
    where
        S: Subscriber + for<'a> LookupSpan<'a>,
    {
        fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
            let metadata = event.metadata();
            let mut visitor = EventVisitor::default();
            event.record(&mut visitor);

            let message = visitor.fields.remove("message");
            let screen = visitor.fields.remove("screen");
            let phase = visitor.fields.remove("phase");
            let name = visitor.fields.remove("name");
            let detail = format_detail(&visitor.fields);

            self.capture.push(TraceRecord {
                timestamp: Local::now().format("%H:%M:%S%.3f").to_string(),
                level: *metadata.level(),
                target: metadata.target().to_string(),
                screen,
                phase,
                name,
                message,
                detail,
            });
        }
    }

    #[derive(Default)]
    struct EventVisitor {
        fields: BTreeMap<String, String>,
    }

    impl Visit for EventVisitor {
        fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
            self.fields
                .insert(field.name().to_string(), format!("{value:?}"));
        }

        fn record_i64(&mut self, field: &Field, value: i64) {
            self.fields
                .insert(field.name().to_string(), value.to_string());
        }

        fn record_u64(&mut self, field: &Field, value: u64) {
            self.fields
                .insert(field.name().to_string(), value.to_string());
        }

        fn record_bool(&mut self, field: &Field, value: bool) {
            self.fields
                .insert(field.name().to_string(), value.to_string());
        }

        fn record_str(&mut self, field: &Field, value: &str) {
            self.fields
                .insert(field.name().to_string(), value.to_string());
        }
    }

    fn format_detail(fields: &BTreeMap<String, String>) -> Option<String> {
        if fields.is_empty() {
            return None;
        }

        let detail = fields
            .iter()
            .map(|(key, value)| format!("{key}={value}"))
            .collect::<Vec<_>>()
            .join(" ");
        Some(detail)
    }

    fn stylize_record(record: &TraceRecord) -> Line<'static> {
        let label = record
            .phase
            .as_deref()
            .map(|phase| phase.to_ascii_uppercase())
            .unwrap_or_else(|| record.level.as_str().to_string());
        let label_style = match record.phase.as_deref() {
            Some("input") => Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            Some("action") => Style::new().fg(Color::Green).add_modifier(Modifier::BOLD),
            Some("effect") => Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            Some("external") => Style::new().fg(Color::Magenta).add_modifier(Modifier::BOLD),
            Some("status") => Style::new()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
            _ => level_style(record.level),
        };

        let body = format_body(record);
        Line::from(vec![
            Span::styled(record.timestamp.clone(), Style::new().fg(Color::DarkGray)),
            Span::raw(" "),
            Span::styled(format!("{label:<8}"), label_style),
            Span::styled(
                format!("{:<12}", record.screen.as_deref().unwrap_or(&record.target)),
                Style::new().fg(Color::DarkGray),
            ),
            Span::raw(" "),
            Span::raw(body),
        ])
    }

    fn format_body(record: &TraceRecord) -> String {
        match (&record.name, &record.message, &record.detail) {
            (Some(name), None, Some(detail)) => format!("{name} {detail}"),
            (Some(name), Some(message), Some(detail)) => format!("{name} {message} {detail}"),
            (Some(name), Some(message), None) => format!("{name} {message}"),
            (Some(name), None, None) => name.clone(),
            (None, Some(message), Some(detail)) => format!("{message} {detail}"),
            (None, Some(message), None) => message.clone(),
            (None, None, Some(detail)) => detail.clone(),
            (None, None, None) => record.target.clone(),
        }
    }

    fn level_style(level: tracing::Level) -> Style {
        match level {
            tracing::Level::ERROR => Style::new().fg(Color::Red).add_modifier(Modifier::BOLD),
            tracing::Level::WARN => Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            tracing::Level::INFO => Style::new().fg(Color::Green),
            tracing::Level::DEBUG => Style::new().fg(Color::Blue),
            tracing::Level::TRACE => Style::new().fg(Color::DarkGray),
        }
    }

    fn format_key(key: &KeyEvent) -> String {
        let mut parts = Vec::new();
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            parts.push("Ctrl".to_string());
        }
        if key.modifiers.contains(KeyModifiers::ALT) {
            parts.push("Alt".to_string());
        }
        if key.modifiers.contains(KeyModifiers::SHIFT) {
            parts.push("Shift".to_string());
        }

        let code = match key.code {
            KeyCode::Backspace => "Backspace".to_string(),
            KeyCode::Enter => "Enter".to_string(),
            KeyCode::Left => "Left".to_string(),
            KeyCode::Right => "Right".to_string(),
            KeyCode::Up => "Up".to_string(),
            KeyCode::Down => "Down".to_string(),
            KeyCode::Home => "Home".to_string(),
            KeyCode::End => "End".to_string(),
            KeyCode::PageUp => "PageUp".to_string(),
            KeyCode::PageDown => "PageDown".to_string(),
            KeyCode::Tab => "Tab".to_string(),
            KeyCode::BackTab => "BackTab".to_string(),
            KeyCode::Delete => "Delete".to_string(),
            KeyCode::Insert => "Insert".to_string(),
            KeyCode::Esc => "Esc".to_string(),
            KeyCode::Char(c) => c.to_string(),
            KeyCode::F(n) => format!("F{n}"),
            other => format!("{other:?}"),
        };
        parts.push(code);
        parts.join("+")
    }
}

#[cfg(not(feature = "dev"))]
#[allow(dead_code)]
mod imp {
    use crossterm::event::KeyEvent;
    use ratatui::text::Line;
    use tracing_subscriber::filter::LevelFilter;

    #[derive(Clone, Default)]
    pub(crate) struct TraceCapture;

    impl TraceCapture {
        pub(crate) fn snapshot_lines_for_screen(&self, _screen: &str) -> Vec<Line<'static>> {
            Vec::new()
        }

        pub(crate) fn snapshot_lines_all(&self) -> Vec<Line<'static>> {
            Vec::new()
        }
    }

    pub(crate) fn capture() -> std::sync::Arc<TraceCapture> {
        std::sync::Arc::new(TraceCapture)
    }

    pub(crate) fn init(tui_command: bool, default_level: LevelFilter) {
        let stderr_default = if tui_command {
            LevelFilter::OFF
        } else {
            default_level
        };

        tracing_subscriber::fmt()
            .with_env_filter(
                tracing_subscriber::EnvFilter::builder()
                    .with_default_directive(stderr_default.into())
                    .from_env_lossy(),
            )
            .with_writer(std::io::stderr)
            .init();
    }

    pub(crate) fn trace_input_key(_screen: &'static str, _key: &KeyEvent) {}
    pub(crate) fn trace_input_paste(_screen: &'static str, _len: usize) {}
    pub(crate) fn trace_action(_screen: &'static str, _name: &'static str) {}
    pub(crate) fn trace_effect(_screen: &'static str, _name: &'static str) {}
    pub(crate) fn trace_external(_screen: &'static str, _name: &'static str) {}
    pub(crate) fn trace_status(_screen: &'static str, _name: &'static str) {}
}

#[allow(unused_imports)]
pub(crate) use imp::{
    TraceCapture, capture, init, trace_action, trace_effect, trace_external, trace_input_key,
    trace_input_paste, trace_status,
};
