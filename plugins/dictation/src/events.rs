use serde::{Deserialize, Serialize};

/// Drives the native macOS mini-panel (`hypr-dictation-ui-macos`), a minimal
/// NSPanel affordance with just two states.
///
/// DECISION (dictation `success` phase): this enum is deliberately NOT given a
/// `Success` variant. `success` is a transient positive flourish that belongs
/// to the cross-platform webview orb ([`DictationPhase::Success`]); the native
/// panel is a bare recording indicator, so folding a one-shot end-of-session
/// pulse into it would add native-only surface (new NSPanel look/trait) with no
/// cross-platform payoff and nothing Linux-verifiable. Keep `Phase` and its
/// NSPanel traits a no-diff; the success affordance lives entirely in the
/// webview path.
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
    /// One-shot positive flourish emitted the instant a session finishes
    /// successfully (`DictationFinishedEvent.failed == false`), immediately
    /// before the orb settles back to [`DictationPhase::Idle`]. Never a
    /// resting state - the orb only shows it for a beat.
    Success,
    /// The session died (mic/server/injection failure). Cleared on next start.
    Error,
}

/// Where recognized speech goes (mirrors the `dictation_output_mode` setting;
/// serialized as `"type"` / `"batch"` so the two representations match).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "kebab-case")]
pub enum DictationOutputMode {
    /// Final transcript segments are typed into the focused app as they
    /// arrive (the original behavior).
    #[default]
    Type,
    /// Nothing is typed while dictating; the transcript accumulates and is
    /// delivered once at the end of the session. Whether the delivery pastes
    /// at the cursor or only copies to the clipboard is the frontend's call
    /// (`dictation_paste_at_cursor` setting). The pre-rework name of this
    /// variant was `batch-paste`; the alias keeps old persisted values valid.
    #[serde(alias = "batch-paste")]
    Batch,
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
/// (`type` mode) or accumulated for the deliver-on-stop buffer (`batch`).
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, tauri_specta::Event)]
#[serde(rename_all = "camelCase")]
pub struct DictationTranscriptEvent {
    pub text: String,
}

/// Emitted exactly once when a dictation session ends, carrying the raw
/// accumulated transcript of the whole session (all final segments, both
/// modes). The main window finishes the job from here: cleanup per the
/// `dictation_cleanup` setting (basic/LLM), delivery in batch mode
/// (`deliver_text`) and the history entry. `failed` mirrors the session
/// outcome so batch delivery can degrade to copy-only.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, tauri_specta::Event)]
#[serde(rename_all = "camelCase")]
pub struct DictationFinishedEvent {
    pub raw_text: String,
    pub mode: DictationOutputMode,
    pub failed: bool,
}

#[cfg(test)]
mod tests {
    use super::DictationOutputMode;

    #[test]
    fn output_mode_serializes_kebab_case() {
        assert_eq!(
            serde_json::to_string(&DictationOutputMode::Type).unwrap(),
            "\"type\""
        );
        assert_eq!(
            serde_json::to_string(&DictationOutputMode::Batch).unwrap(),
            "\"batch\""
        );
    }

    #[test]
    fn output_mode_tolerates_the_legacy_batch_paste_value() {
        assert_eq!(
            serde_json::from_str::<DictationOutputMode>("\"batch-paste\"").unwrap(),
            DictationOutputMode::Batch
        );
    }
}
