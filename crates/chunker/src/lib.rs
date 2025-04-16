mod fixed;
mod vad;

pub use fixed::*;
pub use vad::*;

use kalosm_sound::AsyncSource;
use std::time::Duration;

pub trait ChunkerExt: AsyncSource + Sized {
    fn fixed_chunks(self, chunk_duration: Duration) -> FixedChunkStream<Self>
    where
        Self: Unpin,
    {
        FixedChunkStream::new(self, chunk_duration)
    }

    fn vad_chunks(self) -> VoiceActivityRechunker<Self>
    where
        Self: Unpin,
    {
        VoiceActivityRechunker::new(self)
    }
}

impl<T: AsyncSource> ChunkerExt for T {}
