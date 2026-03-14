use std::time::{Duration, Instant};

use colored::Colorize;
use futures_util::StreamExt;
use owhisper_client::FinalizeHandle;
use owhisper_interface::stream::StreamResponse;

use super::audio::{ChannelKind, DisplayMode};

pub fn fmt_ts(secs: f64) -> String {
    let m = (secs / 60.0) as u32;
    let s = secs % 60.0;
    format!("{:02}:{:02}", m, s as u32)
}

pub struct Segment {
    time: f64,
    text: String,
}

pub struct Transcript {
    segments: Vec<Segment>,
    partial: String,
    t0: Instant,
    kind: ChannelKind,
}

impl Transcript {
    pub fn new(t0: Instant, kind: ChannelKind) -> Self {
        Self {
            segments: Vec::new(),
            partial: String::new(),
            t0,
            kind,
        }
    }

    fn elapsed(&self) -> f64 {
        self.t0.elapsed().as_secs_f64()
    }

    pub fn set_partial(&mut self, text: &str) {
        self.partial = text.to_string();
        self.render();
    }

    pub fn confirm(&mut self, text: &str) {
        self.segments.push(Segment {
            time: self.elapsed(),
            text: text.to_string(),
        });
        self.partial.clear();
        self.trim();
        self.render();
    }

    fn trim(&mut self) {
        const OVERHEAD: usize = 70;
        let max_chars = crossterm::terminal::size()
            .map(|(cols, _)| (cols as usize).saturating_sub(OVERHEAD))
            .unwrap_or(120);

        let partial_len = if self.partial.is_empty() {
            0
        } else {
            self.partial.len() + 1
        };
        let total_len: usize = self
            .segments
            .iter()
            .map(|s| s.text.len() + 1)
            .sum::<usize>()
            + partial_len;
        if total_len > max_chars {
            let drain_count = self.segments.len() * 2 / 3;
            if drain_count > 0 {
                self.segments.drain(..drain_count);
            }
        }
    }

    fn render(&self) {
        let confirmed: String = self
            .segments
            .iter()
            .map(|s| s.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");

        if confirmed.is_empty() && self.partial.is_empty() {
            return;
        }

        let to = self.elapsed();
        let from = self.segments.first().map(|s| fmt_ts(s.time));
        let prefix = format!("[{} / {}]", from.as_deref().unwrap_or("--:--"), fmt_ts(to)).dimmed();

        let colored_confirmed = match self.kind {
            ChannelKind::Mic => confirmed.truecolor(255, 190, 190).bold(),
            ChannelKind::Speaker => confirmed.truecolor(190, 200, 255).bold(),
        };

        let colored_partial = if self.partial.is_empty() {
            None
        } else {
            Some(match self.kind {
                ChannelKind::Mic => self.partial.truecolor(128, 95, 95),
                ChannelKind::Speaker => self.partial.truecolor(95, 100, 128),
            })
        };

        if let Some(partial) = colored_partial {
            eprintln!("{} {} {}", prefix, colored_confirmed, partial);
        } else {
            eprintln!("{} {}", prefix, colored_confirmed);
        }
    }
}

pub async fn process_stream<S, H>(
    response_stream: S,
    handle: H,
    timeout_secs: u64,
    mode: DisplayMode,
) where
    S: futures_util::Stream<Item = Result<StreamResponse, owhisper_client::hypr_ws_client::Error>>,
    H: FinalizeHandle,
{
    futures_util::pin_mut!(response_stream);

    let t0 = Instant::now();
    let mut channels: Vec<(Transcript, Option<String>)> = match &mode {
        DisplayMode::Single(kind) => vec![(Transcript::new(t0, *kind), None)],
        DisplayMode::Dual => vec![
            (Transcript::new(t0, ChannelKind::Mic), None),
            (Transcript::new(t0, ChannelKind::Speaker), None),
        ],
    };

    let read_loop = async {
        while let Some(result) = response_stream.next().await {
            match result {
                Ok(StreamResponse::TranscriptResponse {
                    is_final,
                    channel,
                    channel_index,
                    ..
                }) => {
                    let text = channel
                        .alternatives
                        .first()
                        .map(|a| a.transcript.as_str())
                        .unwrap_or("");

                    let ch = match &mode {
                        DisplayMode::Single(_) => 0,
                        DisplayMode::Dual => {
                            channel_index.first().copied().unwrap_or(0).clamp(0, 1) as usize
                        }
                    };

                    let (transcript, last_confirmed) = &mut channels[ch];
                    if is_final {
                        if last_confirmed.as_deref() == Some(text) {
                            continue;
                        }
                        *last_confirmed = Some(text.to_string());
                        transcript.confirm(text);
                    } else {
                        transcript.set_partial(text);
                    }
                }
                Ok(StreamResponse::TerminalResponse { .. }) => break,
                Ok(StreamResponse::ErrorResponse { error_message, .. }) => {
                    eprintln!("\nerror: {}", error_message);
                    break;
                }
                Ok(_) => {}
                Err(e) => {
                    eprintln!("\nws error: {:?}", e);
                    break;
                }
            }
        }
    };

    let _ = tokio::time::timeout(Duration::from_secs(timeout_secs), read_loop).await;
    handle.finalize().await;
    eprintln!();
}
