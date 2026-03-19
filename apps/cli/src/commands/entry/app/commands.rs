use std::time::SystemTime;

use ratatui_image::{picker::Picker, protocol::StatefulProtocol};

const LOGO_PNG_BYTES: &[u8] = include_bytes!("../../../../assets/char.png");

const TIPS_UNCONFIGURED: &[&str] = &["Run /connect to set up a provider"];

const TIPS_READY: &[&str] = &["Type /listen to start a live transcription session"];

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Command {
    Listen,
    Chat,
    ChatResume,
    Sessions,
    Timeline,
    Connect,
    Auth,
    Bug,
    Hello,
    Desktop,
    Models,
    ModelsDownload,
    ModelsDelete,
    ModelsPaths,
    Exit,
}

pub(crate) const ALL_COMMANDS: &[Command] = &[
    Command::Listen,
    Command::Chat,
    Command::ChatResume,
    Command::Sessions,
    Command::Timeline,
    Command::Connect,
    Command::Auth,
    Command::Bug,
    Command::Hello,
    Command::Desktop,
    Command::Models,
    Command::ModelsDownload,
    Command::ModelsPaths,
    Command::Exit,
];

impl Command {
    pub(crate) fn name(&self) -> &'static str {
        match self {
            Self::Listen => "/listen",
            Self::Chat => "/chat",
            Self::ChatResume => "/chat resume",
            Self::Sessions => "/sessions",
            Self::Timeline => "/timeline",
            Self::Connect => "/connect",
            Self::Auth => "/auth",
            Self::Bug => "/bug",
            Self::Hello => "/hello",
            Self::Desktop => "/desktop",
            Self::Models => "/models",
            Self::ModelsDownload => "/models download",
            Self::ModelsDelete => "/models delete",
            Self::ModelsPaths => "/models paths",
            Self::Exit => "/exit",
        }
    }

    pub(crate) fn description(&self) -> &'static str {
        match self {
            Self::Listen => "Start live transcription",
            Self::Chat => "Start a chat",
            Self::ChatResume => "Resume an existing chat",
            Self::Sessions => "Browse past sessions",
            Self::Timeline => "CRM timeline view",
            Self::Connect => "Connect provider",
            Self::Auth => "Open auth in browser",
            Self::Bug => "Report a bug on GitHub",
            Self::Hello => "Open char.com",
            Self::Desktop => "Open desktop app or download page",
            Self::Models => "List available models",
            Self::ModelsDownload => "Download a model",
            Self::ModelsDelete => "Delete a model",
            Self::ModelsPaths => "Show model storage paths",
            Self::Exit => "Exit",
        }
    }

    pub(crate) fn group(&self) -> &'static str {
        match self {
            Self::Listen | Self::Chat | Self::ChatResume | Self::Sessions | Self::Timeline => {
                "Session"
            }
            Self::Connect | Self::Auth => "Setup",
            Self::Bug | Self::Hello | Self::Desktop | Self::Exit => "App",
            Self::Models | Self::ModelsDownload | Self::ModelsDelete | Self::ModelsPaths => {
                "Models"
            }
        }
    }

    pub(crate) fn aliases(&self) -> &'static [&'static str] {
        match self {
            Self::Exit => &["quit"],
            _ => &[],
        }
    }

    pub(crate) fn disabled_reason(
        &self,
        stt: &Option<String>,
        llm: &Option<String>,
    ) -> Option<&'static str> {
        match self {
            Self::Listen if stt.is_none() => Some("no STT provider"),
            Self::Chat | Self::ChatResume if llm.is_none() => Some("no LLM provider"),
            _ => None,
        }
    }
}

const ALL_VARIANTS: &[Command] = &[
    Command::Listen,
    Command::Chat,
    Command::ChatResume,
    Command::Sessions,
    Command::Timeline,
    Command::Connect,
    Command::Auth,
    Command::Bug,
    Command::Hello,
    Command::Desktop,
    Command::Models,
    Command::ModelsDownload,
    Command::ModelsDelete,
    Command::ModelsPaths,
    Command::Exit,
];

pub(crate) fn lookup(input: &str) -> Option<(Command, &str)> {
    for cmd in ALL_VARIANTS {
        let name = cmd.name().trim_start_matches('/');
        if let Some(rest) = input.strip_prefix(name) {
            if rest.is_empty() || rest.starts_with(' ') {
                return Some((*cmd, rest.trim_start()));
            }
        }
    }

    for cmd in ALL_VARIANTS {
        for alias in cmd.aliases() {
            if let Some(rest) = input.strip_prefix(alias) {
                if rest.is_empty() || rest.starts_with(' ') {
                    return Some((*cmd, rest.trim_start()));
                }
            }
        }
    }

    None
}

pub(crate) struct CommandEntry {
    pub(crate) name: &'static str,
    pub(crate) description: &'static str,
    pub(crate) group: &'static str,
    pub(crate) disabled_reason: Option<&'static str>,
}

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
