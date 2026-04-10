use std::net::{Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};

use axum::http::StatusCode;
use hypr_llm_types::ImageDetail;
use llm_cactus::{CompleteService, ModelManagerBuilder};
use url::Url;

struct Args {
    model: PathBuf,
    prompt: Option<String>,
    images: Vec<PathBuf>,
    image_detail: Option<ImageDetail>,
    system: Option<String>,
    model_name: String,
    temperature: Option<f32>,
}

impl Args {
    fn parse() -> Result<Self, String> {
        let mut args = pico_args::Arguments::from_env();

        if args.contains(["-h", "--help"]) {
            print_usage();
            std::process::exit(0);
        }

        let model = args
            .value_from_str("--model")
            .map_err(|_| "--model <PATH> is required".to_string())?;
        let prompt = args
            .opt_value_from_str("--prompt")
            .map_err(|error| error.to_string())?;
        let system = args
            .opt_value_from_str("--system")
            .map_err(|error| error.to_string())?;
        let model_name = args
            .opt_value_from_str("--model-name")
            .map_err(|error| error.to_string())?
            .unwrap_or_else(|| "cactus".to_string());
        let temperature = args
            .opt_value_from_str("--temperature")
            .map_err(|error| error.to_string())?;
        let image_detail = args
            .opt_value_from_str::<_, String>("--image-detail")
            .map_err(|error| error.to_string())?
            .map(|value| parse_image_detail(&value))
            .transpose()?;
        let mut images = Vec::new();
        while let Ok(image) = args.value_from_str("--image") {
            images.push(image);
        }

        let remaining = args.finish();
        if !remaining.is_empty() {
            return Err(format!(
                "unexpected arguments: {}",
                remaining
                    .iter()
                    .map(|value| value.to_string_lossy())
                    .collect::<Vec<_>>()
                    .join(" ")
            ));
        }

        if prompt.is_none() && images.is_empty() {
            return Err("at least one of --prompt <TEXT> or --image <PATH> is required".into());
        }

        Ok(Self {
            model,
            prompt,
            images,
            image_detail,
            system,
            model_name,
            temperature,
        })
    }
}

fn parse_image_detail(value: &str) -> Result<ImageDetail, String> {
    match value {
        "auto" => Ok(ImageDetail::Auto),
        "low" => Ok(ImageDetail::Low),
        "high" => Ok(ImageDetail::High),
        _ => Err(format!(
            "invalid --image-detail '{value}', expected one of: auto, low, high"
        )),
    }
}

fn print_usage() {
    eprintln!(
        "\
Usage:
  cargo run -p llm-cactus --example complete -- --model <PATH> [options]

Options:
  --model <PATH>            Model path
  --prompt <TEXT>           User text prompt
  --image <PATH>            User image path (repeatable)
  --image-detail <VALUE>    Image detail: auto, low, high
  --system <TEXT>           System prompt
  --model-name <NAME>       Request model name (default: cactus)
  --temperature <FLOAT>     Sampling temperature
  -h, --help                Show this help
"
    );
}

struct LocalServer {
    addr: SocketAddr,
    shutdown_tx: tokio::sync::oneshot::Sender<()>,
}

impl LocalServer {
    async fn spawn(model_path: PathBuf, model_name: String) -> Self {
        let manager = ModelManagerBuilder::default()
            .register(model_name.clone(), model_path)
            .default_model(model_name)
            .build();

        let app = CompleteService::new(manager)
            .into_router(|err| async move { (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()) });

        let listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0u16))
            .await
            .expect("failed to bind local server");
        let addr = listener.local_addr().expect("failed to read local addr");
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();

        tokio::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(async {
                    let _ = shutdown_rx.await;
                })
                .await
                .expect("local server crashed");
        });

        Self { addr, shutdown_tx }
    }
}

impl Drop for LocalServer {
    fn drop(&mut self) {
        let shutdown_tx =
            std::mem::replace(&mut self.shutdown_tx, tokio::sync::oneshot::channel().0);
        let _ = shutdown_tx.send(());
    }
}

fn image_url(path: &Path) -> Result<String, String> {
    let path = std::fs::canonicalize(path)
        .map_err(|error| format!("failed to resolve image path {}: {error}", path.display()))?;
    let metadata = std::fs::metadata(&path)
        .map_err(|error| format!("failed to read image path {}: {error}", path.display()))?;
    if !metadata.is_file() {
        return Err(format!(
            "image path must point to a file: {}",
            path.display()
        ));
    }

    Url::from_file_path(&path)
        .map(|url| url.to_string())
        .map_err(|_| {
            format!(
                "failed to convert image path to file URL: {}",
                path.display()
            )
        })
}

fn build_user_content(args: &Args) -> Result<serde_json::Value, String> {
    if args.images.is_empty() {
        return Ok(serde_json::Value::String(
            args.prompt.clone().unwrap_or_default(),
        ));
    }

    let mut parts = Vec::new();

    if let Some(prompt) = args.prompt.as_deref().filter(|prompt| !prompt.is_empty()) {
        parts.push(serde_json::json!({
            "type": "text",
            "text": prompt,
        }));
    }

    for image in &args.images {
        let mut image_url = serde_json::json!({
            "url": image_url(image)?,
        });
        if let Some(detail) = &args.image_detail {
            image_url["detail"] = serde_json::to_value(detail).expect("image detail serializes");
        }
        parts.push(serde_json::json!({
            "type": "image_url",
            "image_url": image_url,
        }));
    }

    Ok(serde_json::Value::Array(parts))
}

fn build_messages(args: &Args) -> Result<Vec<serde_json::Value>, String> {
    let mut messages = Vec::new();
    if let Some(system) = &args.system {
        messages.push(serde_json::json!({
            "role": "system",
            "content": system,
        }));
    }
    messages.push(serde_json::json!({
        "role": "user",
        "content": build_user_content(args)?,
    }));
    Ok(messages)
}

async fn run() -> Result<(), String> {
    let args = Args::parse()?;
    if !args.model.exists() {
        return Err(format!("model not found: {}", args.model.display()));
    }

    let server = LocalServer::spawn(args.model.clone(), args.model_name.clone()).await;
    let client = reqwest::Client::new();
    let url = format!("http://{}/v1/chat/completions", server.addr);
    let messages = build_messages(&args)?;

    let response = client
        .post(url)
        .json(&serde_json::json!({
            "model": args.model_name,
            "stream": false,
            "temperature": args.temperature,
            "messages": messages,
        }))
        .send()
        .await
        .map_err(|error| format!("request failed: {error}"))?;

    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|error| format!("failed to read response body: {error}"))?;

    if !status.is_success() {
        return Err(format!("request failed: HTTP {status}\n{body}"));
    }

    let json: serde_json::Value =
        serde_json::from_str(&body).map_err(|error| format!("invalid JSON response: {error}"))?;
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
/// Text + image:
/// cargo run -p llm-cactus --example complete -- --model ~/Library/Application\ Support/hyprnote/models/cactus/qwen2.5-3b-instruct-q4km --prompt "Describe this image" --image /tmp/example.png --image-detail high
#[tokio::main(flavor = "current_thread")]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}
