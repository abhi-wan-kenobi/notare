#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    TonicTransportError(#[from] tonic::transport::Error),
    #[error(transparent)]
    TonicErrorStatus(#[from] tonic::Status),
    #[error(transparent)]
    SerdeJsonError(#[from] serde_json::Error),
}
