mod common;

use std::time::{Duration, Instant};

use common::{
    COMMON_OPTIONS, CommonArgs, LocalServer, build_messages, build_request_body,
    build_response_format, parse_common_args, validate_remaining,
};

struct Args {
    common: CommonArgs,
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

        let common = parse_common_args(&mut args)?;

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

        validate_remaining(args)?;
        if iterations == 0 {
            return Err("--iterations must be greater than 0".into());
        }

        Ok(Self {
            common,
            warmup,
            iterations,
            pause_before_ms,
            pause_between_ms,
            output,
        })
    }
}

fn print_usage() {
    eprintln!(
        "\
Usage:
  cargo run -p llm-cactus --example benchmark -- --model <PATH> [options]

Options:
{COMMON_OPTIONS}
  --warmup <N>              Warmup requests after the cold run (default: 1)
  --iterations <N>          Measured warm requests (default: 5)
  --pause-before-ms <N>     Sleep before the cold run (default: 0)
  --pause-between-ms <N>    Sleep between requests (default: 0)
  --output <FORMAT>         text or json (default: text)
  -h, --help                Show this help
"
    );
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

struct RequestContext {
    client: reqwest::Client,
    url: String,
    body: serde_json::Value,
}

impl RequestContext {
    async fn run(&self) -> Result<RequestMeasurement, String> {
        let started = Instant::now();
        let response = self
            .client
            .post(&self.url)
            .json(&self.body)
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
}

async fn maybe_sleep(ms: u64) {
    if ms > 0 {
        tokio::time::sleep(Duration::from_millis(ms)).await;
    }
}

async fn run() -> Result<(), String> {
    let args = Args::parse()?;

    let started = Instant::now();
    let server =
        LocalServer::spawn(args.common.model.clone(), args.common.model_name.clone()).await;
    let server_started = started.elapsed();
    let pid = std::process::id();
    let messages = build_messages(&args.common)?;
    let response_format = build_response_format(&args.common)?;

    let ctx = RequestContext {
        client: reqwest::Client::new(),
        url: format!("http://{}/v1/chat/completions", server.addr),
        body: build_request_body(&messages, args.common.temperature, &response_format),
    };

    if args.pause_before_ms > 0 {
        eprintln!("benchmark pid: {pid}");
        eprintln!("pausing {} ms before the cold run", args.pause_before_ms);
    }

    maybe_sleep(args.pause_before_ms).await;

    let cold = ctx.run().await?;

    let mut warmups = Vec::with_capacity(args.warmup);
    for _ in 0..args.warmup {
        maybe_sleep(args.pause_between_ms).await;
        warmups.push(ctx.run().await?);
    }

    let mut measured = Vec::with_capacity(args.iterations);
    for _ in 0..args.iterations {
        maybe_sleep(args.pause_between_ms).await;
        measured.push(ctx.run().await?);
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
            println!("model: {}", args.common.model.display());
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
                    "model": args.common.model,
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
