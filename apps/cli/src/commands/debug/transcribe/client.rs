use std::sync::Arc;

use owhisper_client::{ListenClient, ListenClientDual, RealtimeSttAdapter};

use crate::error::{CliError, CliResult};

use super::audio::*;
use super::display::process_stream;

pub fn default_listen_params() -> owhisper_interface::ListenParams {
    owhisper_interface::ListenParams {
        sample_rate: DEFAULT_SAMPLE_RATE,
        languages: vec![hypr_language::ISO639::En.into()],
        ..Default::default()
    }
}

pub async fn build_single_client<A: RealtimeSttAdapter>(
    api_base: impl Into<String>,
    api_key: Option<String>,
    params: owhisper_interface::ListenParams,
) -> ListenClient<A> {
    let mut builder = ListenClient::builder()
        .adapter::<A>()
        .api_base(api_base.into())
        .params(params);

    if let Some(api_key) = api_key {
        builder = builder.api_key(api_key);
    }

    builder.build_single().await
}

pub async fn build_dual_client<A: RealtimeSttAdapter>(
    api_base: impl Into<String>,
    api_key: Option<String>,
    params: owhisper_interface::ListenParams,
) -> ListenClientDual<A> {
    let mut builder = ListenClient::builder()
        .adapter::<A>()
        .api_base(api_base.into())
        .params(params);

    if let Some(api_key) = api_key {
        builder = builder.api_key(api_key);
    }

    builder.build_dual().await
}

pub async fn run_for_source<A: RealtimeSttAdapter>(
    audio: Arc<dyn AudioProvider>,
    source: AudioSource,
    api_base: impl Into<String>,
    api_key: Option<String>,
    params: owhisper_interface::ListenParams,
) -> CliResult<()> {
    if source.is_dual() {
        let client = build_dual_client::<A>(api_base, api_key, params).await;
        run_dual_client(
            audio,
            source,
            client,
            DEFAULT_SAMPLE_RATE,
            DEFAULT_TIMEOUT_SECS,
        )
        .await?;
    } else {
        let client = build_single_client::<A>(api_base, api_key, params).await;
        run_single_client(
            audio,
            source,
            client,
            DEFAULT_SAMPLE_RATE,
            DEFAULT_TIMEOUT_SECS,
        )
        .await?;
    }
    Ok(())
}

pub async fn run_single_client<A: RealtimeSttAdapter>(
    audio: Arc<dyn AudioProvider>,
    source: AudioSource,
    client: ListenClient<A>,
    sample_rate: u32,
    timeout_secs: u64,
) -> CliResult<()> {
    let kind = match source {
        AudioSource::Input => ChannelKind::Mic,
        AudioSource::Output => ChannelKind::Speaker,
        _ => unreachable!(),
    };

    print_audio_info(&*audio, &source, sample_rate);

    let audio_stream = create_single_audio_stream(&audio, &source, sample_rate)?;
    let (response_stream, handle) = client
        .from_realtime_audio(audio_stream)
        .await
        .map_err(|e| CliError::operation_failed("connect realtime transcription", e.to_string()))?;

    process_stream(
        response_stream,
        handle,
        timeout_secs,
        DisplayMode::Single(kind),
    )
    .await;
    Ok(())
}

pub async fn run_dual_client<A: RealtimeSttAdapter>(
    audio: Arc<dyn AudioProvider>,
    source: AudioSource,
    client: ListenClientDual<A>,
    sample_rate: u32,
    timeout_secs: u64,
) -> CliResult<()> {
    print_dual_audio_info(&*audio, &source, sample_rate);

    let audio_stream = create_dual_audio_stream(&audio, &source, sample_rate)?;
    let (response_stream, handle) = client
        .from_realtime_audio(audio_stream)
        .await
        .map_err(|e| CliError::operation_failed("connect realtime transcription", e.to_string()))?;

    process_stream(response_stream, handle, timeout_secs, DisplayMode::Dual).await;
    Ok(())
}
