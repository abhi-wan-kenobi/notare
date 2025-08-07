use bytes::Bytes;
use futures_util::{Stream, StreamExt};

use crate::{misc::shutdown_signal, Server};

#[derive(clap::Parser)]
pub struct RunArgs {
    pub model: String,
    #[arg(short, long)]
    pub config: Option<String>,
}

pub async fn handle_run(args: RunArgs) -> anyhow::Result<()> {
    let config = owhisper_config::Config::new(args.config)?;
    let api_key = config.general.as_ref().and_then(|g| g.api_key.clone());
    let server = Server::new(config, None);

    let port = 1234;
    let addr = format!("127.0.0.1:{}", port);

    // Spawn the server in a background task
    let server_handle =
        tokio::spawn(async move { server.run_with_shutdown(shutdown_signal()).await });

    // Wait a moment for server to be ready
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    let input_devices: Vec<String> = hypr_audio_input::MicInput::list_devices();
    log::info!("Input devices: {:#?}", input_devices);

    // Create mic input stream
    // let audio_stream = create_audio_stream()?;

    // Create client with server address
    let api_base = format!("ws://{}/whisper-cpp", addr);

    let client = owhisper_client::ListenClient::builder()
        .api_base(&api_base)
        .api_key(api_key.as_deref().unwrap_or(""))
        .params(owhisper_interface::ListenParams {
            ..Default::default()
        })
        .build_single();

    println!("Starting audio streaming from microphone...");
    println!("Press Ctrl+C to stop.\n");

    // let response_stream = client.from_realtime_audio(audio_stream).await?;
    // futures_util::pin_mut!(response_stream);

    // while let Some(chunk) = response_stream.next().await {
    //     if !chunk.words.is_empty() {
    //         let text = chunk
    //             .words
    //             .iter()
    //             .map(|w| w.text.as_str())
    //             .collect::<Vec<_>>()
    //             .join(" ");

    //         // Check if this is a final transcript based on metadata
    //         if let Some(meta) = &chunk.meta {
    //             if let Some(is_final) = meta.get("is_final").and_then(|v| v.as_bool()) {
    //                 if is_final {
    //                     println!("\n[FINAL] {}", text);
    //                 } else {
    //                     print!("\r[PARTIAL] {}", text);
    //                     use std::io::Write;
    //                     std::io::stdout().flush()?;
    //                 }
    //             } else {
    //                 println!("{}", text);
    //             }
    //         } else {
    //             println!("{}", text);
    //         }
    //     }
    // }

    println!("\nShutting down...");

    Ok(())
}

// Create an audio stream that properly implements AsyncSource
// fn create_audio_stream() -> anyhow::Result<impl Stream<Item = Bytes> + Send + Unpin + 'static> {
//     use hypr_audio_utils::AudioFormatExt;

//     let mic =
//         MicInput::new(None).map_err(|e| anyhow::anyhow!("Failed to create mic input: {}", e))?;

//     let stream = mic
//         .stream()
//         .to_i16_le_chunks(16 * 1000, 1024)
//         .map(|chunk| Bytes::from(chunk));

//     Ok(stream)
// }
