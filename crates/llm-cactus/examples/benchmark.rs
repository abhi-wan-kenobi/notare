use std::net::{Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

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
    warmup: usize,
    iterations: usize,
    pause_before_ms: u64,
    pause_between_ms: u64,
    output: OutputFormat,
}

#[derive(Clone, Copy)]
enum OutputFormat {
    Text,
    Json,
}

impl OutputFormat {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "text" => Ok(Self::Text),
            "json" => Ok(Self::Json),
            _ => Err(format!(
                "invalid --output '{value}', expected one of: text, json"
            )),
        }
    }
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
        let warmup = args
            .opt_value_from_str("--warmup")
            .map_err(|error| error.to_string())?
            .unwrap_or(1);
        let iterations = args
            .opt_value_from_str("--iterations")
            .map_err(|error| error.to_string())?
            .unwrap_or(5);
        let pause_before_ms = args
            .opt_value_from_str("--pause-before-ms")
            .map_err(|error| error.to_string())?
            .unwrap_or(0);
        let pause_between_ms = args
            .opt_value_from_str("--pause-between-ms")
            .map_err(|error| error.to_string())?
            .unwrap_or(0);
        let output = args
            .opt_value_from_str::<_, String>("--output")
            .map_err(|error| error.to_string())?
            .map(|value| OutputFormat::parse(&value))
            .transpose()?
            .unwrap_or(OutputFormat::Text);
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
        if iterations == 0 {
            return Err("--iterations must be greater than 0".into());
        }

        Ok(Self {
            model,
            prompt,
            images,
            image_detail,
            system,
            model_name,
            temperature,
            warmup,
            iterations,
            pause_before_ms,
            pause_between_ms,
            output,
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
  cargo run -p llm-cactus --example benchmark -- --model <PATH> [options]

Options:
  --model <PATH>            Model path
  --prompt <TEXT>           User text prompt
  --image <PATH>            User image path (repeatable)
  --image-detail <VALUE>    Image detail: auto, low, high
  --system <TEXT>           System prompt
  --model-name <NAME>       Request model name (default: cactus)
  --temperature <FLOAT>     Sampling temperature
  --warmup <N>              Warmup requests after the cold run (default: 1)
  --iterations <N>          Measured warm requests (default: 5)
  --pause-before-ms <N>     Sleep before the cold run (default: 0)
  --pause-between-ms <N>    Sleep between requests (default: 0)
  --output <FORMAT>         text or json (default: text)
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

struct RequestMeasurement {
    duration: Duration,
    response_bytes: usize,
}

struct Summary {
    min_ms: f64,
    max_ms: f64,
    mean_ms: f64,
    p50_ms: f64,
    p95_ms: f64,
}

impl Summary {
    fn from_durations(durations: &[Duration]) -> Self {
        let mut millis = durations
            .iter()
            .map(|duration| duration.as_secs_f64() * 1000.0)
            .collect::<Vec<_>>();
        millis.sort_by(f64::total_cmp);

        let min_ms = *millis.first().unwrap_or(&0.0);
        let max_ms = *millis.last().unwrap_or(&0.0);
        let mean_ms = if millis.is_empty() {
            0.0
        } else {
            millis.iter().sum::<f64>() / millis.len() as f64
        };

        Self {
            min_ms,
            max_ms,
            mean_ms,
            p50_ms: percentile(&millis, 0.50),
            p95_ms: percentile(&millis, 0.95),
        }
    }
}

fn percentile(sorted_millis: &[f64], percentile: f64) -> f64 {
    if sorted_millis.is_empty() {
        return 0.0;
    }

    let rank = ((sorted_millis.len() as f64) * percentile).ceil() as usize;
    let index = rank.saturating_sub(1).min(sorted_millis.len() - 1);
    sorted_millis[index]
}

async fn run_request(
    client: &reqwest::Client,
    url: &str,
    model_name: &str,
    messages: &[serde_json::Value],
    temperature: Option<f32>,
) -> Result<RequestMeasurement, String> {
    let started = Instant::now();
    let response = client
        .post(url)
        .json(&serde_json::json!({
            "model": model_name,
            "stream": false,
            "temperature": temperature,
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

    serde_json::from_str::<serde_json::Value>(&body)
        .map_err(|error| format!("invalid JSON response: {error}"))?;

    Ok(RequestMeasurement {
        duration: started.elapsed(),
        response_bytes: body.len(),
    })
}

async fn maybe_sleep(ms: u64) {
    if ms > 0 {
        tokio::time::sleep(Duration::from_millis(ms)).await;
    }
}

async fn run() -> Result<(), String> {
    let args = Args::parse()?;
    if !args.model.exists() {
        return Err(format!("model not found: {}", args.model.display()));
    }

    let started = Instant::now();
    let server = LocalServer::spawn(args.model.clone(), args.model_name.clone()).await;
    let server_started = started.elapsed();
    let pid = std::process::id();
    let client = reqwest::Client::new();
    let url = format!("http://{}/v1/chat/completions", server.addr);
    let messages = build_messages(&args)?;

    if args.pause_before_ms > 0 {
        eprintln!("benchmark pid: {pid}");
        eprintln!("pausing {} ms before the cold run", args.pause_before_ms);
    }

    maybe_sleep(args.pause_before_ms).await;

    let cold = run_request(&client, &url, &args.model_name, &messages, args.temperature).await?;

    let mut warmups = Vec::with_capacity(args.warmup);
    for _ in 0..args.warmup {
        maybe_sleep(args.pause_between_ms).await;
        warmups
            .push(run_request(&client, &url, &args.model_name, &messages, args.temperature).await?);
    }

    let mut measured = Vec::with_capacity(args.iterations);
    for _ in 0..args.iterations {
        maybe_sleep(args.pause_between_ms).await;
        measured
            .push(run_request(&client, &url, &args.model_name, &messages, args.temperature).await?);
    }

    let durations = measured
        .iter()
        .map(|measurement| measurement.duration)
        .collect::<Vec<_>>();
    let summary = Summary::from_durations(&durations);

    match args.output {
        OutputFormat::Text => {
            println!("pid: {pid}");
            println!("server_addr: {}", server.addr);
            println!("model: {}", args.model.display());
            println!(
                "server_start_ms: {:.2}",
                server_started.as_secs_f64() * 1000.0
            );
            println!("cold_run_ms: {:.2}", cold.duration.as_secs_f64() * 1000.0);
            println!("cold_run_response_bytes: {}", cold.response_bytes);
            println!("warmup_runs: {}", warmups.len());
            println!("measured_runs: {}", measured.len());
            println!(
                "measured_ms: {}",
                durations
                    .iter()
                    .map(|duration| format!("{:.2}", duration.as_secs_f64() * 1000.0))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            println!("min_ms: {:.2}", summary.min_ms);
            println!("p50_ms: {:.2}", summary.p50_ms);
            println!("mean_ms: {:.2}", summary.mean_ms);
            println!("p95_ms: {:.2}", summary.p95_ms);
            println!("max_ms: {:.2}", summary.max_ms);
        }
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "pid": pid,
                    "server_addr": server.addr.to_string(),
                    "model": args.model,
                    "server_start_ms": server_started.as_secs_f64() * 1000.0,
                    "cold_run_ms": cold.duration.as_secs_f64() * 1000.0,
                    "cold_run_response_bytes": cold.response_bytes,
                    "warmup_runs": warmups.len(),
                    "measured_runs": measured.len(),
                    "measured_ms": durations
                        .iter()
                        .map(|duration| duration.as_secs_f64() * 1000.0)
                        .collect::<Vec<_>>(),
                    "summary": {
                        "min_ms": summary.min_ms,
                        "p50_ms": summary.p50_ms,
                        "mean_ms": summary.mean_ms,
                        "p95_ms": summary.p95_ms,
                        "max_ms": summary.max_ms,
                    },
                }))
                .map_err(|error| format!("failed to format JSON output: {error}"))?
            );
        }
    }

    drop(server);

    Ok(())
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}
