use std::io::Write;

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::terminal;
use futures_util::StreamExt;
use hypr_openrouter::{ChatCompletionRequest, ChatMessage, Client, Role};

use crate::error::{CliError, CliResult};
use crate::runtime::session_context::load_chat_system_message;

pub struct Args {
    pub session: Option<String>,
    pub api_key: Option<String>,
    pub model: Option<String>,
}

pub async fn run(args: Args) -> CliResult<()> {
    let api_key = args
        .api_key
        .ok_or_else(|| CliError::required_argument_with_hint("--api-key", "set CHAR_API_KEY"))?;
    let model = args
        .model
        .unwrap_or_else(|| "anthropic/claude-sonnet-4".to_string());

    let client = Client::new(api_key);
    let mut messages: Vec<ChatMessage> = Vec::new();
    if let Some(session_id) = args.session.as_deref() {
        let session_context = load_chat_system_message(session_id)?;
        messages.push(ChatMessage::new(Role::System, session_context));
    }

    loop {
        eprint!("\x1b[1m> \x1b[0m");
        std::io::stderr().flush().ok();

        let user_input = match read_line() {
            Ok(Some(line)) => line,
            Ok(None) => break,
            Err(e) => return Err(e),
        };

        let trimmed = user_input.trim();
        if trimmed.is_empty() {
            continue;
        }

        messages.push(ChatMessage::new(Role::User, trimmed));

        let req = ChatCompletionRequest {
            model: Some(model.clone()),
            messages: messages.clone(),
            ..Default::default()
        };

        let mut stream = client
            .chat_completion_stream(&req)
            .await
            .map_err(|e| CliError::external_action_failed("chat request", e.to_string()))?;

        let mut full_response = String::new();
        let mut stdout = std::io::stdout();

        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(chunk) => {
                    if let Some(choice) = chunk.choices.first() {
                        if let Some(content) = choice.delta.content.as_deref() {
                            print!("{content}");
                            stdout.flush().ok();
                            full_response.push_str(content);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("\nerror: {e}");
                    break;
                }
            }
        }

        println!();
        messages.push(ChatMessage::new(Role::Assistant, full_response.as_str()));
    }

    Ok(())
}

fn read_line() -> CliResult<Option<String>> {
    terminal::enable_raw_mode()
        .map_err(|e| CliError::operation_failed("terminal", e.to_string()))?;
    let result = read_line_raw();
    let _ = terminal::disable_raw_mode();
    result
}

fn read_line_raw() -> CliResult<Option<String>> {
    let mut line = String::new();
    let mut stderr = std::io::stderr();

    loop {
        let Event::Key(key) =
            event::read().map_err(|e| CliError::operation_failed("read input", e.to_string()))?
        else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }

        if key.modifiers.contains(KeyModifiers::CONTROL) {
            match key.code {
                KeyCode::Char('c') => {
                    eprintln!();
                    return Ok(None);
                }
                KeyCode::Char('d') if line.is_empty() => {
                    eprintln!();
                    return Ok(None);
                }
                _ => {}
            }
        }

        match key.code {
            KeyCode::Enter => {
                eprint!("\r\n");
                stderr.flush().ok();
                return Ok(Some(line));
            }
            KeyCode::Backspace => {
                if !line.is_empty() {
                    line.pop();
                    eprint!("\x08 \x08");
                    stderr.flush().ok();
                }
            }
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                line.push(c);
                eprint!("{c}");
                stderr.flush().ok();
            }
            _ => {}
        }
    }
}
