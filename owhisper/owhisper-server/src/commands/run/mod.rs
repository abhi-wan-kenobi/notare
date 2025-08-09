mod guard;
mod state;
mod ui;

use guard::*;
use state::*;
use ui::*;

use std::sync::{Arc, Mutex};

use futures_util::StreamExt;
use hypr_audio::AsyncSource;

use crate::{misc::shutdown_signal, Server};

#[derive(clap::Parser)]
pub struct RunArgs {
    pub model: String,
    #[arg(short, long)]
    pub config: Option<String>,
    #[arg(short, long)]
    pub device: Option<String>,
}

pub async fn handle_run(args: RunArgs) -> anyhow::Result<()> {
    let config = owhisper_config::Config::new(args.config)?;
    if !config.models.iter().any(|m| m.id() == args.model) {
        return Err(anyhow::anyhow!(
            "'{}' not found in '{:?}'",
            args.model,
            owhisper_config::global_config_path()
        ));
    }

    let api_key = config.general.as_ref().and_then(|g| g.api_key.clone());
    let server = Server::new(config, None);

    // Build router first to get the port
    let router = server.build_router().await?;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    let port = addr.port();

    let server_handle = tokio::spawn(async move {
        let handle = axum::serve(listener, router.into_make_service())
            .with_graceful_shutdown(shutdown_signal());
        let _ = handle.await;
    });

    let available_devices = hypr_audio::AudioInput::list_mic_devices();
    let initial_device = args
        .device
        .clone()
        .or_else(|| available_devices.first().cloned())
        .ok_or_else(|| anyhow::anyhow!("No audio devices found"))?;

    let mut audio_input = hypr_audio::AudioInput::from_mic(Some(initial_device.clone()))?;
    let device_name = audio_input.device_name().to_string();

    let client = owhisper_client::ListenClient::builder()
        .api_base(&format!("ws://127.0.0.1:{}", port))
        .api_key(api_key.as_deref().unwrap_or(""))
        .params(owhisper_interface::ListenParams {
            model: Some(args.model.clone()),
            languages: vec![hypr_language::ISO639::En.into()],
            redemption_time_ms: 500,
            ..Default::default()
        })
        .build_single();

    let amplitude_data = Arc::new(Mutex::new(AmplitudeData::new()));
    let amplitude_clone = amplitude_data.clone();

    let mut agc = hypr_agc::Agc::default();

    let mic_stream = audio_input
        .stream()
        .resample(16000)
        .chunks(512)
        .map(move |chunk| {
            let samples: Vec<f32> = {
                let mut samples: Vec<f32> = chunk.to_vec();
                agc.process(&mut samples);
                samples
            };

            if let Ok(mut data) = amplitude_clone.lock() {
                let rms = calculate_rms(&samples);
                data.update(rms);
            }

            hypr_audio_utils::f32_to_i16_bytes(samples)
        });

    let response_stream = client.from_realtime_audio(mic_stream).await?;
    futures_util::pin_mut!(response_stream);

    let result = run_tui(
        response_stream,
        device_name,
        available_devices,
        amplitude_data,
    )
    .await;

    server_handle.abort();
    result
}
