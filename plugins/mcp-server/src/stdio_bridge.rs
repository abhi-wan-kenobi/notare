use std::io::{self, BufRead, Write};

pub fn extract_request_id(line: &str) -> Option<serde_json::Value> {
    serde_json::from_str::<serde_json::Value>(line)
        .ok()
        .and_then(|v| v.get("id").cloned())
}

pub fn format_json_rpc_error(id: Option<serde_json::Value>, message: &str) -> String {
    let id_val = id.unwrap_or(serde_json::Value::Null);
    let err_obj = serde_json::json!({
        "jsonrpc": "2.0",
        "id": id_val,
        "error": {
            "code": -32603,
            "message": message,
        }
    });
    serde_json::to_string(&err_obj).unwrap_or_else(|_| {
        format!(r#"{{"jsonrpc":"2.0","id":null,"error":{{"code":-32603,"message":"{message}"}}}}"#)
    })
}

pub fn parse_sse_events(body: &str) -> Vec<String> {
    let mut messages = Vec::new();
    let mut current_data = Vec::new();

    for line in body.lines() {
        let trimmed = line.trim_end();
        if trimmed.is_empty() {
            // End of an SSE event
            if !current_data.is_empty() {
                let joined = current_data.join("\n");
                let compact = match serde_json::from_str::<serde_json::Value>(&joined) {
                    Ok(val) => serde_json::to_string(&val).unwrap_or(joined),
                    Err(_) => joined.replace('\n', " "),
                };
                if !compact.trim().is_empty() {
                    messages.push(compact);
                }
                current_data.clear();
            }
        } else if let Some(rest) = trimmed.strip_prefix("data:") {
            let data = rest.strip_prefix(' ').unwrap_or(rest);
            current_data.push(data.to_string());
        }
        // Lines starting with event:, id:, retry:, or comments (:) are ignored
    }

    if !current_data.is_empty() {
        let joined = current_data.join("\n");
        let compact = match serde_json::from_str::<serde_json::Value>(&joined) {
            Ok(val) => serde_json::to_string(&val).unwrap_or(joined),
            Err(_) => joined.replace('\n', " "),
        };
        if !compact.trim().is_empty() {
            messages.push(compact);
        }
    }

    messages
}

pub fn parse_http_response(content_type: Option<&str>, body: &str) -> Vec<String> {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }

    let is_sse = content_type
        .map(|ct| ct.contains("text/event-stream"))
        .unwrap_or_else(|| {
            trimmed.starts_with("data:")
                || trimmed.starts_with("event:")
                || trimmed.starts_with(':')
        });

    if is_sse {
        parse_sse_events(body)
    } else {
        // Plain JSON response (possibly multi-line or pretty-printed)
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(trimmed) {
            vec![serde_json::to_string(&val).unwrap_or_else(|_| trimmed.to_string())]
        } else {
            trimmed
                .lines()
                .map(|l| l.trim())
                .filter(|l| !l.is_empty())
                .map(|l| l.to_string())
                .collect()
        }
    }
}

pub async fn run_stdio_bridge(port: u16, token: String) -> Result<(), Box<dyn std::error::Error>> {
    if token.trim().is_empty() {
        eprintln!("Error: bearer token cannot be empty");
        return Err("bearer token cannot be empty".into());
    }

    let url = format!("http://127.0.0.1:{port}/");
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()?;

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

        let req_id = extract_request_id(trimmed);

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
                let content_type = response
                    .headers()
                    .get(reqwest::header::CONTENT_TYPE)
                    .and_then(|v| v.to_str().ok())
                    .map(|s| s.to_string());

                let text = response.text().await.unwrap_or_default();

                if !status.is_success() {
                    eprintln!("Bridge error HTTP {status}: {text}");
                    let msgs = parse_http_response(content_type.as_deref(), &text);
                    if !msgs.is_empty() {
                        for msg in msgs {
                            if writeln!(stdout, "{msg}").is_err() || stdout.flush().is_err() {
                                return Ok(());
                            }
                        }
                    } else if req_id.is_some() {
                        let err_msg = format!("HTTP {status}: {text}");
                        let err_rpc = format_json_rpc_error(req_id, &err_msg);
                        if writeln!(stdout, "{err_rpc}").is_err() || stdout.flush().is_err() {
                            return Ok(());
                        }
                    }
                } else {
                    let messages = parse_http_response(content_type.as_deref(), &text);
                    for msg in messages {
                        if writeln!(stdout, "{msg}").is_err() || stdout.flush().is_err() {
                            return Ok(());
                        }
                    }
                }
            }
            Err(err) => {
                eprintln!("Failed to send to Notare MCP HTTP server: {err}");
                if req_id.is_some() {
                    let err_msg = format!("Transport error: {err}");
                    let err_rpc = format_json_rpc_error(req_id, &err_msg);
                    if writeln!(stdout, "{err_rpc}").is_err() || stdout.flush().is_err() {
                        return Ok(());
                    }
                }
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plain_json_framing_compact() {
        let input = r#"{"jsonrpc":"2.0","id":1,"result":"ok"}"#;
        let framed = parse_http_response(Some("application/json"), input);
        assert_eq!(framed.len(), 1);
        assert!(!framed[0].contains('\n'));
        let parsed: serde_json::Value = serde_json::from_str(&framed[0]).unwrap();
        assert_eq!(parsed["id"], 1);
        assert_eq!(parsed["result"], "ok");
    }

    #[test]
    fn test_pretty_json_framing_compacts_to_single_line() {
        let input = r#"{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "tools": [
      {
        "name": "list_meetings"
      }
    ]
  }
}"#;
        let framed = parse_http_response(Some("application/json"), input);
        assert_eq!(framed.len(), 1);
        assert!(
            !framed[0].contains('\n'),
            "framed message must be on a single line"
        );
        let parsed: serde_json::Value = serde_json::from_str(&framed[0]).unwrap();
        assert_eq!(parsed["id"], 1);
        assert_eq!(parsed["result"]["tools"][0]["name"], "list_meetings");
    }

    #[test]
    fn test_sse_single_event_framing() {
        let sse = "event: message\ndata: {\"jsonrpc\":\"2.0\",\"id\":1,\"result\":\"ok\"}\n\n";
        let framed = parse_http_response(Some("text/event-stream"), sse);
        assert_eq!(framed.len(), 1);
        // Key order depends on serde_json's `preserve_order` feature, which
        // differs between the Linux and macOS dependency graphs.
        let parsed: serde_json::Value = serde_json::from_str(&framed[0]).unwrap();
        assert_eq!(
            parsed,
            serde_json::json!({"jsonrpc":"2.0","id":1,"result":"ok"})
        );
        assert!(!framed[0].contains('\n'));
    }

    #[test]
    fn test_sse_multiline_data_framing() {
        let sse = "event: message\ndata: {\ndata:   \"jsonrpc\": \"2.0\",\ndata:   \"id\": 2,\ndata:   \"result\": 42\ndata: }\n\n";
        let framed = parse_http_response(Some("text/event-stream"), sse);
        assert_eq!(framed.len(), 1);
        assert!(
            !framed[0].contains('\n'),
            "multiline SSE data must be joined into a single line JSON"
        );
        let parsed: serde_json::Value = serde_json::from_str(&framed[0]).unwrap();
        assert_eq!(parsed["id"], 2);
        assert_eq!(parsed["result"], 42);
    }

    #[test]
    fn test_sse_multiple_events_framing() {
        let sse = "data: {\"jsonrpc\":\"2.0\",\"method\":\"notifications/message\",\"params\":{\"text\":\"hello\"}}\n\ndata: {\"jsonrpc\":\"2.0\",\"id\":10,\"result\":true}\n\n";
        let framed = parse_http_response(Some("text/event-stream"), sse);
        assert_eq!(framed.len(), 2);
        assert!(!framed[0].contains('\n'));
        assert!(!framed[1].contains('\n'));
        let parsed1: serde_json::Value = serde_json::from_str(&framed[0]).unwrap();
        let parsed2: serde_json::Value = serde_json::from_str(&framed[1]).unwrap();
        assert_eq!(parsed1["method"], "notifications/message");
        assert_eq!(parsed2["id"], 10);
    }

    #[test]
    fn test_sse_comments_and_ids_ignored() {
        let sse = ": keepalive ping\nid: 123\nevent: message\ndata: {\"jsonrpc\":\"2.0\",\"id\":1,\"result\":null}\n\n: another comment\n";
        let framed = parse_http_response(Some("text/event-stream"), sse);
        assert_eq!(framed.len(), 1);
        let parsed: serde_json::Value = serde_json::from_str(&framed[0]).unwrap();
        assert_eq!(parsed["id"], 1);
    }

    #[test]
    fn test_extract_request_id() {
        assert_eq!(
            extract_request_id(r#"{"jsonrpc":"2.0","id":42,"method":"test"}"#),
            Some(serde_json::json!(42))
        );
        assert_eq!(
            extract_request_id(r#"{"jsonrpc":"2.0","id":"abc-123","method":"test"}"#),
            Some(serde_json::json!("abc-123"))
        );
        assert_eq!(
            extract_request_id(r#"{"jsonrpc":"2.0","method":"notify"}"#),
            None
        );
    }

    #[test]
    fn test_format_json_rpc_error() {
        let err_json = format_json_rpc_error(Some(serde_json::json!(5)), "Connection refused");
        let parsed: serde_json::Value = serde_json::from_str(&err_json).unwrap();
        assert_eq!(parsed["id"], 5);
        assert_eq!(parsed["error"]["code"], -32603);
        assert_eq!(parsed["error"]["message"], "Connection refused");
    }
}
