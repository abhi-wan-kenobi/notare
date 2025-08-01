use std::sync::Arc;

use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::{self, Next},
    response::Response,
};
use axum_extra::{headers::authorization::Bearer, headers::Authorization, TypedHeader};

#[derive(clap::Args)]
pub struct ServeArgs {
    #[arg(short, long)]
    pub config_path: String,
    #[arg(short, long)]
    pub port: u16,
}

#[derive(Clone)]
struct AppState {
    api_key: Option<String>,
}

pub async fn handle_serve(args: ServeArgs) -> anyhow::Result<()> {
    let config = owhisper_config::Config::new(&args.config_path);

    let api_key = config.general.as_ref().and_then(|g| g.api_key.clone());
    let app_state = Arc::new(AppState { api_key });

    let aws_service = hypr_transcribe_aws::TranscribeService::new(
        hypr_transcribe_aws::TranscribeConfig::default(),
    )
    .await
    .unwrap();

    let whisper_cpp_service = hypr_transcribe_whisper_local::WhisperStreamingService::builder()
        .model_path(config.serve.unwrap().whisper_cpp.unwrap().model_path.into())
        .build();

    let stt_router = axum::Router::new()
        .route_service("/aws", aws_service)
        .route_service("/whisper-cpp", whisper_cpp_service);

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", args.port))
        .await
        .unwrap();

    let app = axum::Router::new()
        .route("/health", axum::routing::get(health))
        .merge(stt_router)
        .layer(middleware::from_fn_with_state(
            app_state.clone(),
            auth_middleware,
        ))
        .into_make_service();

    axum::serve(listener, app).await.unwrap();
    Ok(())
}

async fn health() -> &'static str {
    "OK"
}

async fn auth_middleware(
    State(state): State<Arc<AppState>>,
    auth_header: Option<TypedHeader<Authorization<Bearer>>>,
    req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    if state.api_key.is_none() || req.uri().path() == "/health" {
        return Ok(next.run(req).await);
    }

    let expected_token = state
        .api_key
        .as_ref()
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;

    match auth_header {
        None => Err(StatusCode::UNAUTHORIZED),
        Some(TypedHeader(Authorization(bearer))) => {
            if bearer.token() == expected_token {
                Ok(next.run(req).await)
            } else {
                Err(StatusCode::UNAUTHORIZED)
            }
        }
    }
}
