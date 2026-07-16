use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum Phase {
    Recording,
    Processing,
}

/// Lifecycle of the persistent dictation orb (Windows/Linux webview path).
/// Distinct from [`Phase`], which drives the native macOS mini-panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum DictationPhase {
    /// Orb visible, not dictating.
    Idle,
    /// Mic streaming to the local STT server; recognized text is being typed.
    Listening,
    /// Stop requested; waiting for the server to flush the final segments.
    Processing,
    /// The session died (mic/server/injection failure). Cleared on next start.
    Error,
}

/// Where recognized speech goes (mirrors the `dictation_output_mode` setting;
/// serialized as `"type"` / `"batch-paste"` so the two representations match).
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "kebab-case")]
pub enum DictationOutputMode {
    /// Final transcript segments are typed into the focused app as they
    /// arrive (the original behavior).
    #[default]
    Type,
    /// Nothing is typed while dictating; on stop the accumulated transcript
    /// is cleaned, copied to the clipboard and pasted once (terminal-friendly;
    /// the clipboard intentionally keeps the text for repeated pastes).
    BatchPaste,
}

/// Broadcast by the Rust dictation session to every webview (the orb window
/// renders it; the main window can observe it to track session state).
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, tauri_specta::Event)]
#[serde(rename_all = "camelCase")]
pub struct DictationStateEvent {
    pub phase: DictationPhase,
    pub amplitude: f32,
    /// Output mode of the running session (the orb shows a subtle hint while
    /// batch mode records). `type` when idle.
    pub mode: DictationOutputMode,
}

/// Emitted by the orb webview when the user clicks the orb; the main window
/// host toggles the dictation session in response.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, tauri_specta::Event)]
#[serde(rename_all = "camelCase")]
pub struct DictationOrbClicked {}

/// A final transcript segment that was delivered: typed into the focused app
/// (`type` mode) or accumulated for the paste-on-stop buffer (`batch-paste`).
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, tauri_specta::Event)]
#[serde(rename_all = "camelCase")]
pub struct DictationTranscriptEvent {
    pub text: String,
}
