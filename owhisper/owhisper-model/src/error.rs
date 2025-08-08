#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("not supported")]
    NotSupported,
}
