use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{ListItem, ListState};

use crate::theme::Theme;
use crate::widgets::{
    CenteredDialog, KeyHints, MultiSelect, MultiSelectEntry, MultiSelectState, SelectList,
};

use super::app::{App, LanguageFocus, LanguageTab, ProviderTab, Tab};
use super::runtime::CalendarPermissionState;

const THEME: Theme = Theme::DEFAULT;

enum Section<'a> {
    Label(&'a str),
    Message(&'a str),
    CurrentProvider {
        name: &'a str,
    },
    ProviderList {
        items: Vec<ListItem<'a>>,
        state: &'a mut ListState,
    },
    CalendarList {
        entries: Vec<MultiSelectEntry<'a>>,
        state: MultiSelectState,
    },
    Gap,
}

fn section_constraint(s: &Section) -> Constraint {
    match s {
        Section::Label(_) | Section::CurrentProvider { .. } | Section::Gap => Constraint::Length(1),
        Section::Message(_) | Section::ProviderList { .. } | Section::CalendarList { .. } => {
            Constraint::Min(0)
        }
    }
}

fn render_section(frame: &mut ratatui::Frame, section: Section, area: Rect) {
    match section {
        Section::Label(text) => {
            frame.render_widget(
                Line::from(Span::styled(
                    text,
                    Style::new().add_modifier(Modifier::BOLD),
                )),
                area,
            );
        }
        Section::Message(text) => {
            frame.render_widget(Line::from(Span::styled(text, THEME.muted)), area);
        }
        Section::CurrentProvider { name } => {
            frame.render_widget(
                Line::from(vec![
                    Span::raw("Current: "),
                    Span::styled(name, THEME.status.active),
                ]),
                area,
            );
        }
        Section::ProviderList { items, state } => {
            frame.render_stateful_widget(SelectList::new(items, &THEME), area, state);
        }
        Section::CalendarList { entries, mut state } => {
            frame.render_stateful_widget(MultiSelect::new(entries, &THEME), area, &mut state);
        }
        Section::Gap => {}
    }
}

fn render_sections(frame: &mut ratatui::Frame, sections: Vec<Section>, area: Rect) {
    let constraints: Vec<Constraint> = sections.iter().map(section_constraint).collect();
    let areas = Layout::vertical(constraints).split(area);
    for (section, area) in sections.into_iter().zip(areas.iter()) {
        render_section(frame, section, *area);
    }
}

pub fn draw(frame: &mut ratatui::Frame, app: &mut App) {
    let content = CenteredDialog::new("Configure", &THEME)
        .wide()
        .render(frame);

    let [tabs_area, _gap, content_area, _, hints_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(content);

    draw_tabs(frame, app, tabs_area);

    match app.tab {
        Tab::Stt => draw_provider_content(frame, &mut app.stt, content_area),
        Tab::Llm => draw_provider_content(frame, &mut app.llm, content_area),
        Tab::Calendar => draw_calendar_content(frame, app, content_area),
        Tab::Language => draw_language_content(frame, &mut app.language, content_area),
    }

    let cal_has_items = app.cal_permission == Some(CalendarPermissionState::Authorized)
        && !app.calendars.is_empty();
    let hints = match app.tab {
        Tab::Stt | Tab::Llm => KeyHints::new(&THEME).hints(vec![
            ("\u{2190}\u{2192}", "tab"),
            ("\u{2191}\u{2193}", "navigate"),
            ("Enter", "confirm"),
            ("Esc", "quit"),
        ]),
        Tab::Calendar if cal_has_items => KeyHints::new(&THEME).hints(vec![
            ("\u{2190}\u{2192}", "tab"),
            ("\u{2191}\u{2193}", "navigate"),
            ("Space", "toggle"),
            ("Enter", "save"),
            ("Esc", "quit"),
        ]),
        Tab::Calendar => {
            KeyHints::new(&THEME).hints(vec![("\u{2190}\u{2192}", "tab"), ("Esc", "quit")])
        }
        Tab::Language => KeyHints::new(&THEME).hints(vec![
            ("\u{2190}\u{2192}", "tab"),
            ("Tab", "section"),
            ("\u{2191}\u{2193}", "navigate"),
            ("Space", "toggle"),
            ("Enter", "confirm"),
            ("Esc", "quit"),
        ]),
    };
    frame.render_widget(hints, hints_area);
}

fn draw_tabs(frame: &mut ratatui::Frame, app: &App, area: ratatui::layout::Rect) {
    let mut spans: Vec<Span> = Vec::new();
    for (i, tab) in Tab::ALL.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw("  "));
        }
        if *tab == app.tab {
            spans.push(Span::styled(format!(" {} ", tab.title()), THEME.tab_active));
        } else {
            spans.push(Span::styled(tab.title(), THEME.muted));
        }
    }
    frame.render_widget(Line::from(spans), area);
}

fn draw_provider_content(frame: &mut ratatui::Frame, pt: &mut ProviderTab, area: Rect) {
    if pt.providers.is_empty() {
        render_sections(
            frame,
            vec![
                Section::Label("Provider"),
                Section::Gap,
                Section::Message("No providers configured. Run `char connect` first."),
            ],
            area,
        );
        return;
    }

    let ProviderTab {
        providers,
        current,
        list_state,
    } = pt;

    let items: Vec<ListItem> = providers
        .iter()
        .map(|p| {
            let marker = if current.as_deref() == Some(p.as_str()) {
                "\u{2713} "
            } else {
                "  "
            };
            ListItem::new(Line::from(vec![
                Span::styled(marker, THEME.status.active),
                Span::raw(p.as_str()),
            ]))
        })
        .collect();

    let mut sections: Vec<Section> = vec![Section::Label("Provider")];
    if let Some(cur) = current {
        sections.push(Section::CurrentProvider { name: cur.as_str() });
    }
    sections.push(Section::Gap);
    sections.push(Section::ProviderList {
        items,
        state: list_state,
    });
    render_sections(frame, sections, area);
}

fn draw_calendar_content(frame: &mut ratatui::Frame, app: &mut App, area: Rect) {
    let authorized = app.cal_permission == Some(CalendarPermissionState::Authorized);

    if !authorized {
        render_sections(
            frame,
            vec![
                Section::Label("Apple Calendar"),
                Section::Gap,
                Section::Message(
                    "Permission not granted. Run `char connect` to set up calendar access.",
                ),
            ],
            area,
        );
        return;
    }

    if app.calendars.is_empty() {
        render_sections(
            frame,
            vec![
                Section::Label("Apple Calendar"),
                Section::Gap,
                Section::Message("No calendars found."),
            ],
            area,
        );
        return;
    }

    let mut current_source: Option<&str> = None;
    let mut entries: Vec<MultiSelectEntry> = Vec::new();

    for cal in &app.calendars {
        let source = cal.source.as_str();
        if current_source != Some(source) {
            if current_source.is_some() {
                entries.push(MultiSelectEntry::Group(Line::from("")));
            }
            current_source = Some(source);
            entries.push(MultiSelectEntry::Group(Line::from(Span::styled(
                source,
                THEME.muted,
            ))));
        }
        let color_dot = parse_hex_color(&cal.color);
        let label = Line::from(vec![
            Span::styled("\u{25CF} ", Style::new().fg(color_dot)),
            Span::raw(cal.name.as_str()),
        ]);
        entries.push(MultiSelectEntry::Item {
            checked: cal.enabled,
            label,
        });
    }

    let state = MultiSelectState::new(app.cal_cursor);
    render_sections(
        frame,
        vec![
            Section::Label("Apple Calendar"),
            Section::Gap,
            Section::CalendarList { entries, state },
        ],
        area,
    );
}

fn draw_language_content(frame: &mut ratatui::Frame, lt: &mut LanguageTab, area: Rect) {
    let ai_focused = matches!(lt.focus, LanguageFocus::AiLanguage);
    let spoken_focused = matches!(lt.focus, LanguageFocus::SpokenLanguages);

    let [ai_area, _gap, spoken_area] = Layout::vertical([
        Constraint::Ratio(1, 2),
        Constraint::Length(1),
        Constraint::Ratio(1, 2),
    ])
    .areas(area);

    // AI Language section
    {
        let ai_label_style = if ai_focused {
            Style::new().add_modifier(Modifier::BOLD)
        } else {
            THEME.muted
        };

        let items: Vec<ListItem> = lt
            .languages
            .iter()
            .map(|(code, name)| {
                let marker = if lt.ai_language.as_deref() == Some(code.as_str()) {
                    "\u{2713} "
                } else {
                    "  "
                };
                ListItem::new(Line::from(vec![
                    Span::styled(marker, THEME.status.active),
                    Span::raw(format!("{name} ({code})")),
                ]))
            })
            .collect();

        let [label_area, list_area] =
            Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).areas(ai_area);
        frame.render_widget(
            Line::from(Span::styled("AI Language", ai_label_style)),
            label_area,
        );
        frame.render_stateful_widget(
            SelectList::new(items, &THEME),
            list_area,
            &mut lt.ai_list_state,
        );
    }

    // Spoken Languages section
    {
        let spoken_label_style = if spoken_focused {
            Style::new().add_modifier(Modifier::BOLD)
        } else {
            THEME.muted
        };

        let entries: Vec<MultiSelectEntry> = lt
            .languages
            .iter()
            .map(|(code, name)| MultiSelectEntry::Item {
                checked: lt.spoken_languages.contains(code),
                label: Line::from(format!("{name} ({code})")),
            })
            .collect();

        let mut state = MultiSelectState::new(lt.spoken_cursor);

        let [label_area, list_area] =
            Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).areas(spoken_area);
        frame.render_widget(
            Line::from(Span::styled("Spoken Languages", spoken_label_style)),
            label_area,
        );
        frame.render_stateful_widget(MultiSelect::new(entries, &THEME), list_area, &mut state);
    }
}

fn parse_hex_color(hex: &str) -> Color {
    let hex = hex.trim_start_matches('#');
    if hex.len() >= 6 {
        if let (Ok(r), Ok(g), Ok(b)) = (
            u8::from_str_radix(&hex[0..2], 16),
            u8::from_str_radix(&hex[2..4], 16),
            u8::from_str_radix(&hex[4..6], 16),
        ) {
            return Color::Rgb(r, g, b);
        }
    }
    Color::White
}
