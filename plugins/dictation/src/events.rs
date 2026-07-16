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

/// Broadcast by the Rust dictation session to every webview (the orb window
/// renders it; the main window can observe it to track session state).
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, tauri_specta::Event)]
#[serde(rename_all = "camelCase")]
pub struct DictationStateEvent {
    pub phase: DictationPhase,
    pub amplitude: f32,
}

/// Emitted by the orb webview when the user clicks the orb; the main window
/// host toggles the dictation session in response.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, tauri_specta::Event)]
#[serde(rename_all = "camelCase")]
pub struct DictationOrbClicked {}

/// A final transcript segment that was injected into the focused app.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, tauri_specta::Event)]
#[serde(rename_all = "camelCase")]
pub struct DictationTranscriptEvent {
    pub text: String,
}
