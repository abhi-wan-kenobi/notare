use axum::{extract::Path, http::StatusCode, Extension, Json};

#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct Params {
    id: String,
}

pub async fn handler(
    Extension(db): Extension<hypr_db_user::UserDatabase>,
    Path(params): Path<Params>,
) -> Result<Json<hypr_db_user::Session>, StatusCode> {
    let session = db
        .get_session(hypr_db_user::GetSessionFilter::Id(params.id))
        .await
        .map_err(|e| {
            tracing::error!("Error getting session: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(session))
}
