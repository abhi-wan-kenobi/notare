use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::pin::pin;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use hypr_audio::AudioProvider;
use hypr_audio_actual::ActualAudio;
use hypr_audio_utils::chunk_size_for_stt;
use tokio_stream::StreamExt;

use crate::error::{CliError, CliResult};

use super::hotkey::{self, HotkeyEvent};

enum DaemonEvent {
    Hotkey(HotkeyEvent),
    UiAction(UiAction),
}

enum UiAction {
    Cancel,
    Stop,
}

const SAMPLE_RATE: u32 = 16_000;
const LEVEL_TICK: Duration = Duration::from_millis(100);

pub async fn run() -> CliResult<()> {
    tracing::info!("Shortcut daemon starting");

    let ui_binary = resolve_ui_binary()?;
    tracing::info!(path = %ui_binary.display(), "UI binary resolved");

    let audio = ActualAudio;
    let chunk_size = chunk_size_for_stt(SAMPLE_RATE);

    let mut hotkey_rx = hotkey::listen();
    let (ui_tx, mut ui_rx) = tokio::sync::mpsc::unbounded_channel::<UiAction>();
    let mut ui_process: Option<UiProcess> = None;

    loop {
        let event = tokio::select! {
            Some(hk) = hotkey_rx.recv() => DaemonEvent::Hotkey(hk),
            Some(action) = ui_rx.recv() => DaemonEvent::UiAction(action),
            else => break,
        };

        match event {
            DaemonEvent::Hotkey(HotkeyEvent::RecordStart) => {
                tracing::info!("Hotkey: record start");

                if let Some(mut proc) = ui_process.take() {
                    proc.dismiss();
                }

                match UiProcess::spawn(&ui_binary, ui_tx.clone()) {
                    Ok(proc) => ui_process = Some(proc),
                    Err(e) => {
                        tracing::error!("Failed to spawn UI: {e}");
                        continue;
                    }
                }

                let stream = audio.open_mic_capture(None, SAMPLE_RATE, chunk_size);
                match stream {
                    Ok(stream) => {
                        run_capture(
                            stream,
                            ui_process.as_mut().unwrap(),
                            &mut hotkey_rx,
                            &mut ui_rx,
                        )
                        .await;
                    }
                    Err(e) => {
                        tracing::error!("Failed to open mic capture: {e}");
                    }
                }

                if let Some(mut proc) = ui_process.take() {
                    proc.dismiss();
                }

                // TODO: transcribe, copy to clipboard (separate PR)
            }
            DaemonEvent::Hotkey(HotkeyEvent::RecordStop)
            | DaemonEvent::UiAction(UiAction::Cancel)
            | DaemonEvent::UiAction(UiAction::Stop) => {
                tracing::info!("Recording stopped (no active capture)");

                if let Some(mut proc) = ui_process.take() {
                    proc.dismiss();
                }
            }
        }
    }

    Ok(())
}

async fn run_capture(
    stream: hypr_audio::CaptureStream,
    ui: &mut UiProcess,
    hotkey_rx: &mut tokio::sync::mpsc::UnboundedReceiver<HotkeyEvent>,
    ui_rx: &mut tokio::sync::mpsc::UnboundedReceiver<UiAction>,
) {
    let mut stream = pin!(stream);
    let mut last_level = Instant::now() - LEVEL_TICK;

    loop {
        tokio::select! {
            frame = stream.next() => {
                let Some(result) = frame else { break };
                let Ok(frame) = result else {
                    tracing::error!("Audio capture error");
                    break;
                };

                let now = Instant::now();
                if now.duration_since(last_level) >= LEVEL_TICK {
                    last_level = now;
                    let raw = frame.preferred_mic();
                    let level = peak_level(&raw);
                    ui.send_levels(level, 0.0);
                }
            }
            Some(hk) = hotkey_rx.recv() => {
                if matches!(hk, HotkeyEvent::RecordStop) {
                    tracing::info!("Hotkey: record stop");
                    break;
                }
            }
            Some(action) = ui_rx.recv() => {
                tracing::info!("UI action: {:?}", match &action {
                    UiAction::Cancel => "cancel",
                    UiAction::Stop => "stop",
                });
                break;
            }
        }
    }
}

fn peak_level(samples: &[f32]) -> f32 {
    samples
        .iter()
        .map(|s| s.abs())
        .fold(0.0_f32, f32::max)
        .clamp(0.0, 1.0)
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
    fn spawn(
        binary: &PathBuf,
        ui_tx: tokio::sync::mpsc::UnboundedSender<UiAction>,
    ) -> CliResult<Self> {
        let mut child = Command::new(binary)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| CliError::operation_failed("spawn UI process", e.to_string()))?;

        // Read stdout from UI process for cancel/stop actions
        if let Some(stdout) = child.stdout.take() {
            let tx = ui_tx;
            std::thread::spawn(move || {
                let reader = BufReader::new(stdout);
                for line in reader.lines() {
                    let Ok(line) = line else { break };
                    if let Some(action) = parse_ui_action(&line) {
                        let _ = tx.send(action);
                    }
                }
            });
        }

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

fn parse_ui_action(line: &str) -> Option<UiAction> {
    let v: serde_json::Value = serde_json::from_str(line).ok()?;
    if v.get("type")?.as_str()? != "action" {
        return None;
    }
    match v.get("action")?.as_str()? {
        "cancel" => Some(UiAction::Cancel),
        "stop" => Some(UiAction::Stop),
        _ => None,
    }
}
