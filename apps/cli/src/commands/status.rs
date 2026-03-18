use std::io::IsTerminal;

use ratatui::layout::Constraint;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Text;
use ratatui::widgets::{Cell, Row, Table};

use crate::config::desktop;
use crate::error::CliResult;
use crate::widgets::InlineBox;

pub fn run() -> CliResult<()> {
    let paths = desktop::resolve_paths();
    let settings = desktop::load_settings(&paths.settings_path);

    eprintln!("settings: {}", paths.settings_path.display());
    eprintln!();

    let Some(settings) = settings else {
        eprintln!("No settings found. Run `char connect` to configure a provider.");
        return Ok(());
    };

    let is_tty = std::io::stdout().is_terminal();

    print_section(
        "STT",
        &settings.current_stt_provider,
        &settings.stt_providers,
        is_tty,
    );
    println!();
    print_section(
        "LLM",
        &settings.current_llm_provider,
        &settings.llm_providers,
        is_tty,
    );

    Ok(())
}

fn print_section(
    label: &str,
    current: &Option<String>,
    providers: &std::collections::HashMap<String, desktop::ProviderConfig>,
    is_tty: bool,
) {
    let current_str = current.as_deref().unwrap_or("(none)");
    eprintln!("{label} provider: {current_str}");

    if providers.is_empty() {
        eprintln!("  No {label} providers configured.");
        return;
    }

    if !is_tty {
        for (name, config) in providers {
            let active = if current.as_deref() == Some(name) {
                "*"
            } else {
                ""
            };
            let url = config.base_url.as_deref().unwrap_or("-");
            let key = if config.api_key.is_some() {
                "yes"
            } else {
                "no"
            };
            println!("{}\t{}\t{}\t{}", active, name, url, key);
        }
        return;
    }

    let dim = Style::default().add_modifier(Modifier::DIM);

    let header = Row::new(["", "Provider", "Base URL", "API Key"]).style(dim);

    let mut names: Vec<&String> = providers.keys().collect();
    names.sort();

    let rows: Vec<Row> = names
        .iter()
        .map(|name| {
            let config = &providers[*name];
            let active = if current.as_deref() == Some(name.as_str()) {
                Cell::from(Text::raw("*")).style(Style::default().fg(Color::Green))
            } else {
                Cell::from("")
            };
            let url = Cell::from(config.base_url.as_deref().unwrap_or("-"));
            let key = if config.api_key.is_some() {
                Cell::from("yes").style(Style::default().fg(Color::Green))
            } else {
                Cell::from("no").style(Style::default().fg(Color::DarkGray))
            };
            Row::new([active, Cell::from(name.as_str()), url, key])
        })
        .collect();

    let content_lines = (rows.len() + 1) as u16;
    let height = InlineBox::viewport_height(content_lines);

    let widths = [
        Constraint::Length(1),
        Constraint::Length(16),
        Constraint::Min(20),
        Constraint::Length(7),
    ];
    let table = Table::new(rows, widths).header(header);

    let _ = hypr_cli_tui::render_inline(height, |frame| {
        let inner = InlineBox::render(frame);
        frame.render_widget(table, inner);
    });
}
