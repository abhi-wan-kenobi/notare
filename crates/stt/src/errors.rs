#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("clova error")]
    Clova(#[from] hypr_clova::Error),
    #[error("clova error {0}")]
    ClovaError(String),
    #[error("deepgram error")]
    Deepgram(#[from] deepgram::DeepgramError),
}
