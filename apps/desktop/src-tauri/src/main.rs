// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|arg| arg == "--mcp-stdio") {
        let mut port = 8787;
        let mut token = String::new();
        let mut i = 1;
        while i < args.len() {
            if args[i] == "--port" && i + 1 < args.len() {
                port = args[i + 1].parse().unwrap_or(8787);
                i += 1;
            } else if args[i] == "--token" && i + 1 < args.len() {
                token = args[i + 1].clone();
                i += 1;
            }
            i += 1;
        }
        if token.is_empty() {
            token = std::env::var("NOTARE_MCP_TOKEN").unwrap_or_default();
        }
        let rt = tokio::runtime::Runtime::new().expect("failed to start tokio runtime");
        if let Err(e) = rt.block_on(tauri_plugin_mcp_server::stdio_bridge::run_stdio_bridge(
            port, token,
        )) {
            eprintln!("stdio bridge error: {e}");
        }
        return;
    }
    hyprnote_desktop_lib::main()
}
