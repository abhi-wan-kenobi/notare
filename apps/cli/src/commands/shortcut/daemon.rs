use std::io::Write;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

use crate::error::{CliError, CliResult};

use super::hotkey::{self, HotkeyEvent};

pub async fn run() -> CliResult<()> {
    tracing::info!("Shortcut daemon starting");

    let ui_binary = resolve_ui_binary()?;
    tracing::info!(path = %ui_binary.display(), "UI binary resolved");

    let mut rx = hotkey::listen();
    let mut ui_process: Option<UiProcess> = None;

    while let Some(event) = rx.recv().await {
        match event {
            HotkeyEvent::RecordStart => {
                tracing::info!("Hotkey: record start");

                // Kill any existing UI
                if let Some(mut proc) = ui_process.take() {
                    proc.dismiss();
                }

                match UiProcess::spawn(&ui_binary) {
                    Ok(proc) => ui_process = Some(proc),
                    Err(e) => tracing::error!("Failed to spawn UI: {e}"),
                }

                // TODO: start audio capture here (separate PR)
            }
            HotkeyEvent::RecordStop => {
                tracing::info!("Hotkey: record stop");

                if let Some(mut proc) = ui_process.take() {
                    proc.dismiss();
                }

                // TODO: stop capture, transcribe, copy to clipboard (separate PR)
            }
        }
    }

    Ok(())
}

fn resolve_ui_binary() -> CliResult<PathBuf> {
    let exe = std::env::current_exe()
        .and_then(|p| p.canonicalize())
        .map_err(|e| CliError::operation_failed("resolve current exe", e.to_string()))?;

    let dir = exe
        .parent()
        .ok_or_else(|| CliError::operation_failed("resolve binary dir", "no parent"))?;

    let ui_path = dir.join("char-cli-ui");
    if !ui_path.exists() {
        return Err(CliError::operation_failed(
            "find UI binary",
            format!(
                "char-cli-ui not found at {}. Re-run `char shortcut install` or reinstall char.",
                ui_path.display()
            ),
        ));
    }

    Ok(ui_path)
}

struct UiProcess {
    child: Child,
}

impl UiProcess {
    fn spawn(binary: &PathBuf) -> CliResult<Self> {
        let child = Command::new(binary)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| CliError::operation_failed("spawn UI process", e.to_string()))?;

        let mut proc = Self { child };
        proc.send(r#"{"type":"state","recording":true}"#);
        Ok(proc)
    }

    fn send(&mut self, json_line: &str) {
        if let Some(stdin) = self.child.stdin.as_mut() {
            let _ = writeln!(stdin, "{json_line}");
            let _ = stdin.flush();
        }
    }

    #[allow(dead_code)]
    fn send_levels(&mut self, left: f32, _right: f32) {
        self.send(&format!(r#"{{"type":"levels","left":{left},"right":0.0}}"#));
    }

    fn dismiss(&mut self) {
        self.send(r#"{"type":"dismiss"}"#);
        let _ = self.child.wait();
    }
}

impl Drop for UiProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}
