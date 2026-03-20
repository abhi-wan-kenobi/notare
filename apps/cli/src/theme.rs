use hypr_cli_editor::StyleSheet;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders};

#[derive(Debug, Clone, Copy)]
pub struct BorderStyles {
    pub default: Style,
    pub focused: Style,
}

#[derive(Debug, Clone, Copy)]
pub struct StatusStyles {
    pub active: Style,
    pub degraded: Style,
    pub inactive: Style,
}

#[derive(Debug, Clone, Copy)]
pub struct CapabilityStyles {
    pub stt: Style,
    pub llm: Style,
    pub cal: Style,
}

#[derive(Debug, Clone, Copy)]
pub struct WaveformStyles {
    pub normal: Style,
    pub hot: Style,
    pub silent: Style,
}

#[derive(Debug, Clone, Copy)]
pub struct TranscriptStyles {
    pub confirmed: Style,
    pub partial: Style,
}

#[derive(Debug, Clone, Copy)]
pub struct RawStyles {
    pub mic_confirmed: Style,
    pub mic_partial: Style,
    pub speaker_confirmed: Style,
    pub speaker_partial: Style,
}

#[derive(Debug, Clone, Copy)]
pub struct DialogStyles {
    pub overlay_bg: Color,
    pub bg: Color,
    pub title_fg: Color,
}

#[derive(Debug, Clone, Copy)]
pub struct ModeStyles {
    pub insert: Style,
    pub normal: Style,
    pub command: Style,
}

#[derive(Debug, Clone, Copy)]
pub struct Theme {
    pub bg: Color,
    pub accent: Style,
    pub input_bg: Color,
    pub error: Style,
    pub muted: Style,
    pub placeholder: Style,
    pub highlight_bg: Color,
    pub disabled_bg: Color,

    pub border: BorderStyles,
    pub status: StatusStyles,
    pub capability: CapabilityStyles,
    pub waveform: WaveformStyles,
    pub transcript: TranscriptStyles,
    pub raw: RawStyles,
    pub dialog: DialogStyles,
    pub mode: ModeStyles,

    pub panel_heading: Style,
    pub tab_active: Style,
    pub shortcut_key: Style,
    pub speaker_label: Style,
    pub timestamp: Style,
    pub user_bar: Style,
}

impl Theme {
    pub fn bordered_block(&self, focused: bool) -> Block<'static> {
        let style = if focused {
            self.border.focused
        } else {
            self.border.default
        };
        Block::new().borders(Borders::ALL).border_style(style)
    }

    pub const TRANSPARENT: Self = Self {
        bg: Color::Reset,
        ..Self::DEFAULT
    };

    pub const DEFAULT: Self = Self {
        bg: Color::Rgb(13, 17, 22),
        accent: Style::new().fg(Color::Yellow),
        input_bg: Color::Rgb(22, 27, 34),
        error: Style::new().fg(Color::Red),
        muted: Style::new().fg(Color::DarkGray),
        placeholder: Style::new()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::ITALIC),
        highlight_bg: Color::Rgb(30, 60, 100),
        disabled_bg: Color::Rgb(22, 22, 30),

        border: BorderStyles {
            default: Style::new().fg(Color::DarkGray),
            focused: Style::new().fg(Color::Yellow),
        },
        status: StatusStyles {
            active: Style::new().fg(Color::Green),
            degraded: Style::new().fg(Color::Yellow),
            inactive: Style::new().fg(Color::Red),
        },
        capability: CapabilityStyles {
            stt: Style::new().fg(Color::Cyan),
            llm: Style::new().fg(Color::Yellow),
            cal: Style::new().fg(Color::Magenta),
        },
        waveform: WaveformStyles {
            normal: Style::new().fg(Color::Red),
            hot: Style::new().fg(Color::LightRed),
            silent: Style::new().fg(Color::DarkGray),
        },
        transcript: TranscriptStyles {
            confirmed: Style::new(),
            partial: Style::new()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC),
        },
        raw: RawStyles {
            mic_confirmed: Style::new()
                .fg(Color::Rgb(255, 190, 190))
                .add_modifier(Modifier::BOLD),
            mic_partial: Style::new().fg(Color::Rgb(128, 95, 95)),
            speaker_confirmed: Style::new()
                .fg(Color::Rgb(190, 200, 255))
                .add_modifier(Modifier::BOLD),
            speaker_partial: Style::new().fg(Color::Rgb(95, 100, 128)),
        },
        dialog: DialogStyles {
            overlay_bg: Color::Rgb(2, 4, 10),
            bg: Color::Rgb(18, 22, 28),
            title_fg: Color::White,
        },
        mode: ModeStyles {
            insert: Style::new().fg(Color::Black).bg(Color::Green),
            normal: Style::new().fg(Color::Black).bg(Color::Cyan),
            command: Style::new().fg(Color::Black).bg(Color::Yellow),
        },

        panel_heading: Style::new().fg(Color::White),
        tab_active: Style::new()
            .fg(Color::Rgb(13, 17, 22))
            .bg(Color::White)
            .add_modifier(Modifier::BOLD),
        shortcut_key: Style::new().fg(Color::DarkGray),
        speaker_label: Style::new().fg(Color::Yellow),
        timestamp: Style::new().fg(Color::DarkGray),
        user_bar: Style::new().fg(Color::Indexed(69)),
    };
}

impl StyleSheet for Theme {
    fn heading(&self, level: u8) -> Style {
        match level {
            1 => self.accent.add_modifier(Modifier::BOLD),
            2 => Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            3 => Style::new().fg(Color::Green).add_modifier(Modifier::BOLD),
            _ => Style::new().fg(Color::Blue).add_modifier(Modifier::BOLD),
        }
    }

    fn strong(&self) -> Style {
        Style::new().add_modifier(Modifier::BOLD)
    }

    fn emphasis(&self) -> Style {
        Style::new().add_modifier(Modifier::ITALIC)
    }

    fn strikethrough(&self) -> Style {
        Style::new().add_modifier(Modifier::CROSSED_OUT)
    }

    fn code_inline(&self) -> Style {
        self.muted
    }

    fn code_fence(&self) -> Style {
        self.muted
    }

    fn link(&self) -> Style {
        Style::new()
            .fg(Color::Blue)
            .add_modifier(Modifier::UNDERLINED)
    }

    fn blockquote(&self) -> Style {
        self.muted.add_modifier(Modifier::ITALIC)
    }

    fn list_marker(&self) -> Style {
        self.accent
    }
}
