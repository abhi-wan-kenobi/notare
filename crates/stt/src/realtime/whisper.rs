use bytes::Bytes;
use futures_util::Stream;
use std::error::Error;

use hypr_whisper::cloud::WhisperClient;

use super::{RealtimeSpeechToText, StreamResponse};

impl<S, E> RealtimeSpeechToText<S, E> for WhisperClient {
    async fn transcribe(
        &mut self,
        _audio: S,
    ) -> Result<
        Box<dyn Stream<Item = Result<StreamResponse, crate::Error>> + Send + Unpin>,
        crate::Error,
    >
    where
        S: Stream<Item = Result<Bytes, E>> + Send + Unpin + 'static,
        E: Error + Send + Sync + 'static,
    {
        todo!()
    }
}
