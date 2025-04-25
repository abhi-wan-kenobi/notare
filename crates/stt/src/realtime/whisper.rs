use bytes::Bytes;
use futures_util::{Stream, StreamExt};
use std::error::Error;

use hypr_whisper::cloud::WhisperClient;

use super::{RealtimeSpeechToText, StreamResponse, StreamResponseWord};

impl<S, E> RealtimeSpeechToText<S, E> for WhisperClient {
    async fn transcribe(
        &mut self,
        audio: S,
    ) -> Result<
        Box<dyn Stream<Item = Result<StreamResponse, crate::Error>> + Send + Unpin>,
        crate::Error,
    >
    where
        S: Stream<Item = Result<Bytes, E>> + Send + Unpin + 'static,
        E: Error + Send + Sync + 'static,
    {
        let audio_stream = Box::pin(audio.filter_map(|chunk| async { chunk.ok() }));
        let s1 = self.from_audio(audio_stream).await.unwrap();
        let s2 = s1.map(|output| {
            Ok(StreamResponse {
                words: vec![StreamResponseWord {
                    text: output.text,
                    start: 0,
                    end: 0,
                }],
            })
        });

        Ok(Box::from(Box::pin(s2)))
    }
}
