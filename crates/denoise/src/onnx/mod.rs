mod buffer;
mod context;
mod denoiser;
mod error;
pub mod model;
mod stream;

pub use denoiser::Denoiser;
pub use error::*;
pub use stream::StreamDenoiser;

// cargo test -p denoise --features onnx
//
// Set UPDATE_SNAPSHOTS=1 to regenerate baseline snapshots.
#[cfg(test)]
mod tests;
