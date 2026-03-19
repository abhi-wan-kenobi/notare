use crate::cli::{ConnectProvider, ConnectionType};

const STT_PROVIDERS: &[ConnectProvider] = &[
    ConnectProvider::Deepgram,
    ConnectProvider::Soniox,
    ConnectProvider::Assemblyai,
    ConnectProvider::Openai,
    ConnectProvider::Gladia,
    ConnectProvider::Elevenlabs,
    ConnectProvider::Mistral,
    ConnectProvider::Fireworks,
    #[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
    ConnectProvider::Cactus,
    ConnectProvider::Custom,
];

const LLM_PROVIDERS: &[ConnectProvider] = &[
    ConnectProvider::Openai,
    ConnectProvider::Anthropic,
    ConnectProvider::Openrouter,
    ConnectProvider::GoogleGenerativeAi,
    ConnectProvider::Mistral,
    ConnectProvider::AzureOpenai,
    ConnectProvider::AzureAi,
    ConnectProvider::Ollama,
    ConnectProvider::Lmstudio,
    ConnectProvider::Custom,
];

const CAL_PROVIDERS: &[ConnectProvider] = &[
    #[cfg(target_os = "macos")]
    ConnectProvider::AppleCalendar,
    ConnectProvider::GoogleCalendar,
    ConnectProvider::OutlookCalendar,
];

const DISABLED_PROVIDERS: &[ConnectProvider] = &[
    ConnectProvider::GoogleCalendar,
    ConnectProvider::OutlookCalendar,
];

pub(crate) const ALL_PROVIDERS: &[ConnectProvider] = &[
    // STT-only
    ConnectProvider::Deepgram,
    ConnectProvider::Soniox,
    ConnectProvider::Assemblyai,
    ConnectProvider::Gladia,
    ConnectProvider::Elevenlabs,
    ConnectProvider::Fireworks,
    // Dual (STT + LLM)
    ConnectProvider::Openai,
    ConnectProvider::Mistral,
    // LLM-only
    ConnectProvider::Anthropic,
    ConnectProvider::Openrouter,
    ConnectProvider::GoogleGenerativeAi,
    ConnectProvider::AzureOpenai,
    ConnectProvider::AzureAi,
    // Local
    #[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
    ConnectProvider::Cactus,
    ConnectProvider::Ollama,
    ConnectProvider::Lmstudio,
    // Custom
    ConnectProvider::Custom,
    // Calendar
    #[cfg(target_os = "macos")]
    ConnectProvider::AppleCalendar,
    ConnectProvider::GoogleCalendar,
    ConnectProvider::OutlookCalendar,
];

impl std::fmt::Display for ConnectionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Stt => write!(f, "stt"),
            Self::Llm => write!(f, "llm"),
            Self::Cal => write!(f, "cal"),
        }
    }
}

impl std::fmt::Display for ConnectProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.id())
    }
}

impl ConnectProvider {
    pub(crate) fn id(&self) -> &'static str {
        match self {
            Self::Deepgram => "deepgram",
            Self::Soniox => "soniox",
            Self::Assemblyai => "assemblyai",
            Self::Openai => "openai",
            Self::Gladia => "gladia",
            Self::Elevenlabs => "elevenlabs",
            Self::Mistral => "mistral",
            Self::Fireworks => "fireworks",
            #[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
            Self::Cactus => "cactus",
            Self::Anthropic => "anthropic",
            Self::Openrouter => "openrouter",
            Self::GoogleGenerativeAi => "google_generative_ai",
            Self::AzureOpenai => "azure_openai",
            Self::AzureAi => "azure_ai",
            Self::Ollama => "ollama",
            Self::Lmstudio => "lmstudio",
            Self::Custom => "custom",
            #[cfg(target_os = "macos")]
            Self::AppleCalendar => "apple_calendar",
            Self::GoogleCalendar => "google_calendar",
            Self::OutlookCalendar => "outlook_calendar",
        }
    }

    pub(crate) fn display_name(&self) -> &'static str {
        match self {
            Self::Deepgram => "Deepgram",
            Self::Soniox => "Soniox",
            Self::Assemblyai => "AssemblyAI",
            Self::Openai => "OpenAI",
            Self::Gladia => "Gladia",
            Self::Elevenlabs => "ElevenLabs",
            Self::Mistral => "Mistral",
            Self::Fireworks => "Fireworks",
            #[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
            Self::Cactus => "Cactus",
            Self::Anthropic => "Anthropic",
            Self::Openrouter => "OpenRouter",
            Self::GoogleGenerativeAi => "Google Generative AI",
            Self::AzureOpenai => "Azure OpenAI",
            Self::AzureAi => "Azure AI",
            Self::Ollama => "Ollama",
            Self::Lmstudio => "LM Studio",
            Self::Custom => "Custom",
            #[cfg(target_os = "macos")]
            Self::AppleCalendar => "Apple Calendar",
            Self::GoogleCalendar => "Google Calendar",
            Self::OutlookCalendar => "Outlook Calendar",
        }
    }

    pub(crate) fn capabilities(&self) -> Vec<ConnectionType> {
        let mut caps = Vec::new();
        if STT_PROVIDERS.contains(self) {
            caps.push(ConnectionType::Stt);
        }
        if LLM_PROVIDERS.contains(self) {
            caps.push(ConnectionType::Llm);
        }
        if CAL_PROVIDERS.contains(self) {
            caps.push(ConnectionType::Cal);
        }
        caps
    }

    pub(crate) fn is_disabled(&self) -> bool {
        DISABLED_PROVIDERS.contains(self)
    }

    pub(crate) fn is_local(&self) -> bool {
        match self {
            #[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
            Self::Cactus => true,
            Self::Ollama | Self::Lmstudio => true,
            #[cfg(target_os = "macos")]
            Self::AppleCalendar => true,
            _ => false,
        }
    }

    pub(crate) fn default_base_url(&self) -> Option<&'static str> {
        match self {
            Self::Deepgram => Some("https://api.deepgram.com/v1"),
            Self::Soniox => Some("https://api.soniox.com"),
            Self::Assemblyai => Some("https://api.assemblyai.com"),
            Self::Openai => Some("https://api.openai.com/v1"),
            Self::Gladia => Some("https://api.gladia.io"),
            Self::Elevenlabs => Some("https://api.elevenlabs.io"),
            Self::Mistral => Some("https://api.mistral.ai/v1"),
            Self::Fireworks => Some("https://api.fireworks.ai"),
            Self::Anthropic => Some("https://api.anthropic.com/v1"),
            Self::Openrouter => Some("https://openrouter.ai/api/v1"),
            Self::GoogleGenerativeAi => Some("https://generativelanguage.googleapis.com/v1beta"),
            Self::Ollama => Some("http://127.0.0.1:11434/v1"),
            Self::Lmstudio => Some("http://127.0.0.1:1234/v1"),
            #[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
            Self::Cactus => None,
            #[cfg(target_os = "macos")]
            Self::AppleCalendar => None,
            Self::GoogleCalendar | Self::OutlookCalendar => None,
            Self::AzureOpenai | Self::AzureAi | Self::Custom => None,
        }
    }

    pub(crate) fn valid_for(&self, ct: ConnectionType) -> bool {
        self.capabilities().contains(&ct)
    }
}
