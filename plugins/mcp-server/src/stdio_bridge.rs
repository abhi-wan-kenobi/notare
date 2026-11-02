use std::io::{self, BufRead, Write};

pub async fn run_stdio_bridge(port: u16, token: String) -> Result<(), Box<dyn std::error::Error>> {
    let url = format!("http://127.0.0.1:{port}/");
    let client = reqwest::Client::new();

    let stdin = io::stdin();
    let mut stdout = io::stdout();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let res = client
            .post(&url)
            .header("Authorization", format!("Bearer {token}"))
            .header("Content-Type", "application/json")
            .header("Accept", "application/json, text/event-stream")
            .body(trimmed.to_string())
            .send()
            .await;

        match res {
            Ok(response) => {
                let status = response.status();
                let text = response.text().await.unwrap_or_default();
                if !status.is_success() {
                    eprintln!("Bridge error HTTP {status}: {text}");
                }
                // Handle both SSE (event: ... data: ...) and plain JSON-RPC responses
                for resp_line in text.lines() {
                    if let Some(data) = resp_line.strip_prefix("data: ") {
                        let _ = writeln!(stdout, "{data}");
                    } else if !resp_line.starts_with("event:")
                        && !resp_line.starts_with("id:")
                        && !resp_line.is_empty()
                    {
                        let _ = writeln!(stdout, "{resp_line}");
                    }
                }
                let _ = stdout.flush();
            }
            Err(err) => {
                eprintln!("Failed to send to Notare MCP HTTP server: {err}");
            }
        }
    }

    Ok(())
}
