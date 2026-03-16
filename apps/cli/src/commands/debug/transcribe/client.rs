use std::sync::Arc;

use futures_util::StreamExt;
use owhisper_client::{ListenClient, RealtimeSttAdapter};
use owhisper_interface::stream::StreamResponse;
use tokio::sync::mpsc;

use crate::error::{CliError, CliResult};

use super::TranscribeCtx;
use super::audio::*;
use super::raw;
use super::rich;

pub fn default_listen_params() -> owhisper_interface::ListenParams {
    owhisper_interface::ListenParams {
        sample_rate: DEFAULT_SAMPLE_RATE,
        languages: vec![hypr_language::ISO639::En.into()],
        ..Default::default()
    }
}

fn build_client_builder<A: RealtimeSttAdapter>(
    api_base: impl Into<String>,
    api_key: Option<String>,
    params: owhisper_interface::ListenParams,
) -> owhisper_client::ListenClientBuilder<A> {
    let mut builder = ListenClient::builder()
        .adapter::<A>()
        .api_base(api_base.into())
        .params(params);

    if let Some(api_key) = api_key {
        builder = builder.api_key(api_key);
    }

    builder
}

pub(super) async fn run_for_source<A: RealtimeSttAdapter>(
    audio: Arc<dyn AudioProvider>,
    source: AudioSource,
    api_base: impl Into<String>,
    api_key: Option<String>,
    params: owhisper_interface::ListenParams,
    ctx: &TranscribeCtx,
) -> CliResult<()> {
    let builder = build_client_builder::<A>(api_base, api_key, params);

    if source.is_dual() {
        let client = builder.build_dual().await;
        let audio_stream = create_dual_audio_stream(&audio, &source, DEFAULT_SAMPLE_RATE)?;
        let (response_stream, handle) =
            client
                .from_realtime_audio(audio_stream)
                .await
                .map_err(|e| {
                    CliError::operation_failed("connect realtime transcription", e.to_string())
                })?;
        run_screen(response_stream, handle, DEFAULT_TIMEOUT_SECS, ctx, None).await
    } else {
        let kind = match source {
            AudioSource::Input => ChannelKind::Mic,
            AudioSource::Output => ChannelKind::Speaker,
            AudioSource::Mock => ChannelKind::Mic,
            _ => unreachable!(),
        };
        let client = builder.build_single().await;
        let audio_stream = create_single_audio_stream(&audio, &source, DEFAULT_SAMPLE_RATE)?;
        let (response_stream, handle) =
            client
                .from_realtime_audio(audio_stream)
                .await
                .map_err(|e| {
                    CliError::operation_failed("connect realtime transcription", e.to_string())
                })?;
        run_screen(
            response_stream,
            handle,
            DEFAULT_TIMEOUT_SECS,
            ctx,
            Some(kind),
        )
        .await
    }
}

async fn run_screen<S, H>(
    response_stream: S,
    handle: H,
    timeout_secs: u64,
    ctx: &TranscribeCtx,
    single_kind: Option<ChannelKind>,
) -> CliResult<()>
where
    S: futures_util::Stream<Item = Result<StreamResponse, owhisper_client::hypr_ws_client::Error>>
        + Send
        + 'static,
    H: owhisper_client::FinalizeHandle + Send + 'static,
{
    let tracing = Arc::clone(&ctx.tracing);

    match ctx.mode {
        super::TranscribeMode::Raw => {
            let display_mode = match single_kind {
                Some(kind) => DisplayMode::Single(kind),
                None => DisplayMode::Dual,
            };

            let (tx, rx) = mpsc::unbounded_channel();
            let task = spawn_stream_forwarder(
                response_stream,
                handle,
                timeout_secs,
                {
                    let display_mode_is_dual = matches!(display_mode, DisplayMode::Dual);
                    move |response| raw::RawEvent::StreamResponse {
                        response,
                        display_mode: if display_mode_is_dual {
                            DisplayMode::Dual
                        } else {
                            DisplayMode::Single(single_kind.unwrap_or(ChannelKind::Mic))
                        },
                    }
                },
                || raw::RawEvent::StreamEnded,
                tx,
            );

            let screen = raw::RawTranscribeScreen::new(tracing);
            run_tui(screen, rx, task).await
        }
        super::TranscribeMode::Rich => {
            let (tx, rx) = mpsc::unbounded_channel();
            let task = spawn_stream_forwarder(
                response_stream,
                handle,
                timeout_secs,
                rich::RichEvent::StreamResponse,
                || rich::RichEvent::StreamEnded,
                tx,
            );

            let screen = rich::RichTranscribeScreen::new(tracing);
            run_tui(screen, rx, task).await
        }
    }
}

fn spawn_stream_forwarder<S, H, E, F, G>(
    response_stream: S,
    handle: H,
    timeout_secs: u64,
    map_response: F,
    make_ended: G,
    tx: mpsc::UnboundedSender<E>,
) -> tokio::task::JoinHandle<()>
where
    S: futures_util::Stream<Item = Result<StreamResponse, owhisper_client::hypr_ws_client::Error>>
        + Send
        + 'static,
    H: owhisper_client::FinalizeHandle + Send + 'static,
    E: Send + 'static,
    F: Fn(StreamResponse) -> E + Send + Sync + 'static,
    G: FnOnce() -> E + Send + 'static,
{
    tokio::spawn(async move {
        futures_util::pin_mut!(response_stream);
        let read_loop = async {
            while let Some(result) = response_stream.next().await {
                match result {
                    Ok(response) => {
                        let done = matches!(
                            &response,
                            StreamResponse::TerminalResponse { .. }
                                | StreamResponse::ErrorResponse { .. }
                        );
                        let _ = tx.send(map_response(response));
                        if done {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        };
        let _ = tokio::time::timeout(std::time::Duration::from_secs(timeout_secs), read_loop).await;
        let _ = tx.send(make_ended());
        handle.finalize().await;
    })
}

async fn run_tui<S: hypr_cli_tui::Screen>(
    screen: S,
    rx: mpsc::UnboundedReceiver<S::ExternalEvent>,
    task: tokio::task::JoinHandle<()>,
) -> CliResult<()> {
    hypr_cli_tui::run_screen(screen, Some(rx))
        .await
        .map_err(|e| CliError::operation_failed("run transcribe screen", e.to_string()))?;
    task.abort();
    Ok(())
}
