use std::time::SystemTime;

use ratatui_image::{picker::Picker, protocol::StatefulProtocol};

const LOGO_PNG_BYTES: &[u8] = include_bytes!("../../../../assets/char.png");

const TIPS_UNCONFIGURED: &[&str] = &["Run /connect to set up a provider"];

const TIPS_READY: &[&str] = &["Type /listen to start a live transcription session"];

#[derive(Clone, Copy)]
pub(crate) struct SlashCommand {
    pub(crate) name: &'static str,
    pub(crate) description: &'static str,
    pub(crate) group: &'static str,
}

pub(crate) struct CommandEntry {
    pub(crate) name: &'static str,
    pub(crate) description: &'static str,
    pub(crate) group: &'static str,
    pub(crate) disabled_reason: Option<&'static str>,
}

pub(crate) const COMMANDS: &[SlashCommand] = &[
    SlashCommand {
        name: "/listen",
        description: "Start live transcription",
        group: "Session",
    },
    SlashCommand {
        name: "/chat",
        description: "Start a chat",
        group: "Session",
    },
    SlashCommand {
        name: "/chat resume",
        description: "Resume an existing chat",
        group: "Session",
    },
    SlashCommand {
        name: "/sessions",
        description: "Browse past sessions",
        group: "Session",
    },
    SlashCommand {
        name: "/connect",
        description: "Connect provider",
        group: "Setup",
    },
    SlashCommand {
        name: "/auth",
        description: "Open auth in browser",
        group: "Setup",
    },
    SlashCommand {
        name: "/bug",
        description: "Report a bug on GitHub",
        group: "App",
    },
    SlashCommand {
        name: "/hello",
        description: "Open char.com",
        group: "App",
    },
    SlashCommand {
        name: "/desktop",
        description: "Open desktop app or download page",
        group: "App",
    },
    SlashCommand {
        name: "/model paths",
        description: "Show model storage paths",
        group: "Model",
    },
    SlashCommand {
        name: "/model current",
        description: "Show current model config",
        group: "Model",
    },
    SlashCommand {
        name: "/model list",
        description: "List available models",
        group: "Model",
    },
    SlashCommand {
        name: "/exit",
        description: "Exit",
        group: "App",
    },
];

pub(crate) fn pick_tip(
    stt_provider: &Option<String>,
    llm_provider: &Option<String>,
) -> &'static str {
    let tips = if stt_provider.is_none() || llm_provider.is_none() {
        TIPS_UNCONFIGURED
    } else {
        TIPS_READY
    };
    let index = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_millis() as usize % tips.len())
        .unwrap_or(0);
    tips[index]
}

pub(crate) fn load_logo_protocol() -> Option<StatefulProtocol> {
    let picker = Picker::from_query_stdio().unwrap_or_else(|_| Picker::halfblocks());
    let image = image::load_from_memory(LOGO_PNG_BYTES).ok()?;
    Some(picker.new_resize_protocol(image))
}
