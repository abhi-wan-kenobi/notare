mod common;

use common::{
    COMMON_OPTIONS, CommonArgs, LocalServer, build_messages, build_request_body,
    build_response_format, parse_common_args, validate_remaining,
};

fn print_usage() {
    eprintln!(
        "\
Usage:
  cargo run -p llm-cactus --example complete -- --model <PATH> [options]

Options:
{COMMON_OPTIONS}
  -h, --help                Show this help
"
    );
}

fn parse_args() -> Result<CommonArgs, String> {
    let mut args = pico_args::Arguments::from_env();

    if args.contains(["-h", "--help"]) {
        print_usage();
        std::process::exit(0);
    }

    let common = parse_common_args(&mut args)?;
    validate_remaining(args)?;
    Ok(common)
}

async fn run() -> Result<(), String> {
    let args = parse_args()?;

    let server = LocalServer::spawn(args.model.clone(), args.model_name.clone()).await;
    let client = reqwest::Client::new();
    let url = format!("http://{}/v1/chat/completions", server.addr);
    let messages = build_messages(&args)?;
    let response_format = build_response_format(&args)?;
    let body = build_request_body(&messages, args.temperature, &response_format);

    let response = client
        .post(url)
        .json(&body)
        .send()
        .await
        .map_err(|error| format!("request failed: {error}"))?;

    let status = response.status();
    let text = response
        .text()
        .await
        .map_err(|error| format!("failed to read response body: {error}"))?;

    if !status.is_success() {
        return Err(format!("request failed: HTTP {status}\n{text}"));
    }

    let json: serde_json::Value =
        serde_json::from_str(&text).map_err(|error| format!("invalid JSON response: {error}"))?;
    println!(
        "{}",
        serde_json::to_string_pretty(&json)
            .map_err(|error| format!("failed to format response: {error}"))?
    );

    Ok(())
}

/// Text only:
/// cargo run -p llm-cactus --example complete -- --model ~/Library/Application\ Support/hyprnote/models/cactus/qwen2.5-3b-instruct-q4km --prompt "Write a haiku about note taking"
///
/// JSON schema:
/// cargo run -p llm-cactus --example complete -- --model ~/Library/Application\ Support/hyprnote/models/cactus/qwen2.5-3b-instruct-q4km --prompt "Return a person object" --response-format json-schema --schema-file /tmp/person.schema.json
///
/// Text + image:
/// cargo run -p llm-cactus --example complete -- --model ~/Library/Application\ Support/hyprnote/models/cactus/qwen2.5-3b-instruct-q4km --prompt "Describe this image" --image /tmp/example.png --image-detail high
#[tokio::main(flavor = "current_thread")]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}
