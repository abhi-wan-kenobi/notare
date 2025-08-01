use axum::Router;

#[derive(clap::Args)]
pub struct ServeArgs {
    #[arg(short, long)]
    pub port: u16,
}

pub async fn handle_serve(args: ServeArgs) -> anyhow::Result<()> {
    let aws_service = hypr_transcribe_aws::TranscribeService::new(
        hypr_transcribe_aws::TranscribeConfig::default(),
    )
    .await
    .unwrap();

    let stt_router = Router::new().route_service("/aws", aws_service);

    let app = Router::new()
        .route("/health", axum::routing::get(health))
        .merge(stt_router);

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", args.port))
        .await
        .unwrap();

    axum::serve(listener, app).await.unwrap();

    Ok(())
}

async fn health() -> &'static str {
    "OK"
}
