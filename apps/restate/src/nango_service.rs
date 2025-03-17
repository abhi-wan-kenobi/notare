use restate_sdk::prelude::*;

#[restate_sdk::service]
pub(crate) trait NangoService {
    async fn handle_webhook(
        request: Json<hypr_nango::NangoConnectWebhook>,
    ) -> Result<bool, HandlerError>;
}

pub struct NangoServiceImpl;

impl NangoService for NangoServiceImpl {
    // https://docs.nango.dev/guides/api-authorization/authorize-in-your-app-default-ui
    async fn handle_webhook(
        &self,
        _ctx: Context<'_>,
        _request: Json<hypr_nango::NangoConnectWebhook>,
    ) -> Result<bool, HandlerError> {
        let _admin_db = {
            let base_db = {
                hypr_db_core::DatabaseBuilder::default()
                    .remote("TODO_DB_URL", "TODO_DB_TOKEN")
                    .build()
                    .await?
            };

            hypr_db_admin::AdminDatabase::from(base_db)
        };

        // admin_db.upsert_integration(integration)
        Ok(true)
    }
}
