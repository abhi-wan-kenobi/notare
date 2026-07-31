use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "lowercase")]
pub enum Modifier {
    Command,
    Option,
    Shift,
    Control,
    Fn,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
pub struct HotKey {
    pub key: Option<u16>,
    pub modifiers: Vec<Modifier>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Options {
    #[serde(default)]
    pub use_double_tap_only: bool,
    #[serde(default = "default_true")]
    pub double_tap_lock_enabled: bool,
    #[serde(default = "default_min_key_time_ms")]
    pub minimum_key_time_ms: u64,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            use_double_tap_only: false,
            double_tap_lock_enabled: true,
            minimum_key_time_ms: default_min_key_time_ms(),
        }
    }
}

fn default_true() -> bool {
    true
}

fn default_min_key_time_ms() -> u64 {
    150
}

#[macro_export]
macro_rules! common_event_derives {
    ($item:item) => {
        #[derive(
            serde::Serialize, serde::Deserialize, Clone, specta::Type, tauri_specta::Event,
        )]
        $item
    };
}

common_event_derives! {
    #[serde(tag = "type", rename_all = "camelCase")]
    pub enum ShortcutEvent {
        Pressed,
        Released,
        Cancelled,
        Discarded,
    }
}

/// Whether a `GlobalHotkeyTriggered` event marks the key going down or coming
/// back up. Push-to-talk needs both edges (hold to record, release to stop);
/// toggle-style consumers act on `Pressed` only and ignore `Released`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "lowercase")]
pub enum HotkeyState {
    Pressed,
    Released,
}

common_event_derives! {
    /// Fired when a keyed global hotkey registered via `register_global_hotkey`
    /// changes state, backed by `tauri-plugin-global-shortcut` on every
    /// platform. `id` is the caller-chosen registration key (e.g. the dictation
    /// toggle vs. paste-last hotkeys), so a single listener can route each event
    /// to the right action; `shortcut` is the accelerator string it was bound
    /// to; `state` is whether the key was pressed (down) or released (up) - both
    /// edges are emitted so a push-to-talk consumer can hold-to-record, while a
    /// toggle consumer simply ignores `released`.
    /// Distinct from `ShortcutEvent`, which is the macOS-only native
    /// push-to-talk event-tap path (unused today - see `handler::push_to_talk`).
    #[serde(rename_all = "camelCase")]
    pub struct GlobalHotkeyTriggered {
        pub id: String,
        pub shortcut: String,
        pub state: HotkeyState,
    }
}
