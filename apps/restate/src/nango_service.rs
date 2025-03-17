use restate_sdk::prelude::*;

#[restate_sdk::service]
pub(crate) trait NangoService {
    async fn handle_webhook(
        request: Json<hypr_nango::NangoConnectWebhook>,
    ) -> Result<bool, HandlerError>;
}

pub struct NangoServiceImpl;

impl NangoService for NangoServiceImpl {
    async fn handle_webhook(
        &self,
        _ctx: Context<'_>,
        _request: Json<hypr_nango::NangoConnectWebhook>,
    ) -> Result<bool, HandlerError> {
        Ok(true)
    }
}
