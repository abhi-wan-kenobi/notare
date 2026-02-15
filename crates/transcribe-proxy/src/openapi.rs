use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    paths(
        crate::routes::start::handler,
        crate::routes::status::handler,
    ),
    components(schemas(
        crate::routes::start::StartRequest,
        crate::routes::start::StartResponse,
        crate::routes::status::SttStatusResponse,
    )),
    tags((name = "stt", description = "Speech-to-text transcription proxy"))
)]
pub struct ApiDoc;

pub fn openapi() -> utoipa::openapi::OpenApi {
    ApiDoc::openapi()
}
