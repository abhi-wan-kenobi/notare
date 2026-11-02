use clap::Parser;

#[derive(Parser, Debug)]
#[command(
    name = "notare-mcp",
    about = "Stdio to HTTP bridge for Notare MCP server"
)]
struct Args {
    #[arg(short, long, env = "NOTARE_MCP_PORT", default_value_t = 8787)]
    port: u16,

    #[arg(short, long, env = "NOTARE_MCP_TOKEN")]
    token: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    tauri_plugin_mcp_server::stdio_bridge::run_stdio_bridge(args.port, args.token).await
}
