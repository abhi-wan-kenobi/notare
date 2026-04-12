use std::net::{Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};

use axum::http::StatusCode;
use hypr_llm_types::ImageDetail;
use llm_cactus::{CompleteService, ModelManagerBuilder};
use url::Url;

pub const COMMON_OPTIONS: &str = "\
  --model <PATH>            Model path
  --prompt <TEXT>           User text prompt
  --image <PATH>            User image path (repeatable)
  --image-detail <VALUE>    Image detail: auto, low, high
  --response-format <TYPE>  json-object or json-schema
  --schema-file <PATH>      JSON schema file for json-schema response format
  --system <TEXT>           System prompt
  --model-name <NAME>       Configured model label (default: cactus)
  --temperature <FLOAT>     Sampling temperature";

pub struct CommonArgs {
    pub model: PathBuf,
    pub prompt: Option<String>,
    pub images: Vec<PathBuf>,
    pub image_detail: Option<ImageDetail>,
    pub response_format: Option<ResponseFormatArg>,
    pub schema_file: Option<PathBuf>,
    pub system: Option<String>,
    pub model_name: String,
    pub temperature: Option<f32>,
}

#[derive(Clone, Copy)]
pub enum ResponseFormatArg {
    JsonObject,
    JsonSchema,
}

impl ResponseFormatArg {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "json_object" | "json-object" => Ok(Self::JsonObject),
            "json_schema" | "json-schema" => Ok(Self::JsonSchema),
            _ => Err(format!(
                "invalid --response-format '{value}', expected one of: json-object, json-schema"
            )),
        }
    }
}

pub fn parse_image_detail(value: &str) -> Result<ImageDetail, String> {
    match value {
        "auto" => Ok(ImageDetail::Auto),
        "low" => Ok(ImageDetail::Low),
        "high" => Ok(ImageDetail::High),
        _ => Err(format!(
            "invalid --image-detail '{value}', expected one of: auto, low, high"
        )),
    }
}

pub fn parse_common_args(args: &mut pico_args::Arguments) -> Result<CommonArgs, String> {
    let model: PathBuf = args
        .value_from_str("--model")
        .map_err(|_| "--model <PATH> is required".to_string())?;
    let prompt = args
        .opt_value_from_str("--prompt")
        .map_err(|error| error.to_string())?;
    let response_format = args
        .opt_value_from_str::<_, String>("--response-format")
        .map_err(|error| error.to_string())?
        .map(|value| ResponseFormatArg::parse(&value))
        .transpose()?;
    let schema_file = args
        .opt_value_from_str("--schema-file")
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

    if !model.exists() {
        return Err(format!("model not found: {}", model.display()));
    }
    if prompt.is_none() && images.is_empty() {
        return Err("at least one of --prompt <TEXT> or --image <PATH> is required".into());
    }
    if matches!(response_format, Some(ResponseFormatArg::JsonSchema)) && schema_file.is_none() {
        return Err("--schema-file <PATH> is required for --response-format json-schema".into());
    }
    if schema_file.is_some() && !matches!(response_format, Some(ResponseFormatArg::JsonSchema)) {
        return Err("--schema-file can only be used with --response-format json-schema".into());
    }

    Ok(CommonArgs {
        model,
        prompt,
        images,
        image_detail,
        response_format,
        schema_file,
        system,
        model_name,
        temperature,
    })
}

pub fn validate_remaining(args: pico_args::Arguments) -> Result<(), String> {
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
    Ok(())
}

pub struct LocalServer {
    pub addr: SocketAddr,
    shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,
}

impl LocalServer {
    pub async fn spawn(model_path: PathBuf, model_name: String) -> Self {
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

        Self {
            addr,
            shutdown_tx: Some(shutdown_tx),
        }
    }
}

impl Drop for LocalServer {
    fn drop(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
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

pub fn build_user_content(args: &CommonArgs) -> Result<serde_json::Value, String> {
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
        let mut image_url_value = serde_json::json!({
            "url": image_url(image)?,
        });
        if let Some(detail) = &args.image_detail {
            image_url_value["detail"] =
                serde_json::to_value(detail).expect("image detail serializes");
        }
        parts.push(serde_json::json!({
            "type": "image_url",
            "image_url": image_url_value,
        }));
    }

    Ok(serde_json::Value::Array(parts))
}

pub fn build_messages(args: &CommonArgs) -> Result<Vec<serde_json::Value>, String> {
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

pub fn build_response_format(args: &CommonArgs) -> Result<Option<serde_json::Value>, String> {
    match args.response_format {
        None => Ok(None),
        Some(ResponseFormatArg::JsonObject) => {
            Ok(Some(serde_json::json!({ "type": "json_object" })))
        }
        Some(ResponseFormatArg::JsonSchema) => {
            let schema_path = args
                .schema_file
                .as_ref()
                .expect("validated schema file for json-schema format");
            let schema_text = std::fs::read_to_string(schema_path).map_err(|error| {
                format!(
                    "failed to read schema file {}: {error}",
                    schema_path.display()
                )
            })?;
            let schema =
                serde_json::from_str::<serde_json::Value>(&schema_text).map_err(|error| {
                    format!("invalid JSON schema in {}: {error}", schema_path.display())
                })?;
            Ok(Some(serde_json::json!({
                "type": "json_schema",
                "json_schema": { "schema": schema },
            })))
        }
    }
}

pub fn build_request_body(
    messages: &[serde_json::Value],
    temperature: Option<f32>,
    response_format: &Option<serde_json::Value>,
) -> serde_json::Value {
    serde_json::json!({
        "stream": false,
        "temperature": temperature,
        "messages": messages,
        "response_format": response_format,
    })
}
