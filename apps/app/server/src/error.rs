#[derive(Debug, thiserror::Error)]
pub enum Error {}

use serde::{ser::Serializer, Serialize};

impl Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.to_string().as_ref())
    }
}

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()).into_response()
    }
}
