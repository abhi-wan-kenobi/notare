use std::io::IsTerminal;
use std::time::Duration;

use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};

use crate::error::{CliError, CliResult};
use crate::tui::{InlineViewport, InputAction};

use super::plist;

const HIGHLIGHT: Color = Color::Rgb(0xFD, 0xE6, 0xAE);

#[derive(Clone, Copy, PartialEq)]
enum DaemonStatus {
    Running,
    NotInstalled,
}

struct ScreenState {
    selected: usize,
    status: DaemonStatus,
    accessibility: bool,
}

fn detect_status() -> DaemonStatus {
    let running = std::process::Command::new("launchctl")
        .args(["list", plist::LAUNCHD_LABEL])
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false);

    if running {
        DaemonStatus::Running
    } else {
        DaemonStatus::NotInstalled
    }
}

fn check_accessibility() -> bool {
    unsafe extern "C" {
        fn AXIsProcessTrusted() -> bool;
    }
    unsafe { AXIsProcessTrusted() }
}

fn render_lines(state: &ScreenState) -> Vec<Line<'static>> {
    let (status_dot, status_text, status_color) = match state.status {
        DaemonStatus::Running => ("●", "Running", Color::Green),
        DaemonStatus::NotInstalled => ("○", "Not installed", Color::DarkGray),
    };

    let acc_span = if state.accessibility {
        Span::styled("✓", Style::default().fg(Color::Green))
    } else {
        Span::styled("✗", Style::default().fg(Color::Red))
    };

    let install_label = if state.status == DaemonStatus::Running {
        "Reinstall"
    } else {
        "Install"
    };

    let actions: &[&str] = &[install_label, "Uninstall"];

    let line0 = Line::from(vec![
        Span::styled(
            format!("{status_dot} {status_text}"),
            Style::default().fg(status_color),
        ),
        Span::raw("  "),
        Span::styled("access ", Style::default().fg(Color::DarkGray)),
        acc_span,
    ]);

    let mut action_spans = Vec::new();
    for (i, label) in actions.iter().enumerate() {
        let is_selected = i == state.selected;
        let is_disabled = i == 1 && state.status == DaemonStatus::NotInstalled;

        let marker = if is_selected { "> " } else { "  " };
        let style = if is_disabled {
            Style::default().fg(Color::Rgb(0x44, 0x44, 0x44))
        } else if is_selected {
            Style::default().fg(HIGHLIGHT)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        action_spans.push(Span::styled(format!("{marker}{label}"), style));
        if i < actions.len() - 1 {
            action_spans.push(Span::raw("  "));
        }
    }

    let line1 = Line::from(vec![
        Span::styled("hotkey ", Style::default().fg(Color::DarkGray)),
        Span::styled("⌥⌥", Style::default().fg(Color::White)),
        Span::styled(" double-tap", Style::default().fg(Color::DarkGray)),
    ]);

    let line2 = Line::from(action_spans);

    vec![line0, line1, line2]
}

pub fn run() -> CliResult<()> {
    if !std::io::stderr().is_terminal() {
        return Err(CliError::operation_failed(
            "shortcut",
            "interactive mode requires a terminal; use `char shortcut install` or `char shortcut status`".to_string(),
        ));
    }

    let mut state = ScreenState {
        selected: 0,
        status: detect_status(),
        accessibility: check_accessibility(),
    };

    let mut viewport = InlineViewport::stderr_interactive(5, None, true)
        .map_err(|e| CliError::operation_failed("create viewport", e.to_string()))?;

    viewport.draw(&render_lines(&state));

    loop {
        std::thread::sleep(Duration::from_millis(30));

        for action in viewport.poll_input() {
            match action {
                InputAction::Up | InputAction::Down => {
                    state.selected = if state.selected == 0 { 1 } else { 0 };
                }
                InputAction::Confirm => {
                    viewport
                        .clear()
                        .map_err(|e| CliError::operation_failed("clear viewport", e.to_string()))?;

                    match state.selected {
                        0 => return super::install::run(),
                        1 => {
                            if state.status == DaemonStatus::NotInstalled {
                                continue;
                            }
                            return super::uninstall::run();
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }

        viewport.draw(&render_lines(&state));
    }
}
