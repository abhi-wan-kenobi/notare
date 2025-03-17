use restate_sdk::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CheckoutRequest {
    pub(crate) user_id: String,
    pub(crate) tickets: HashSet<String>,
}

#[restate_sdk::service]
pub(crate) trait CheckoutService {
    async fn handle(request: Json<CheckoutRequest>) -> Result<bool, HandlerError>;
    async fn on_stripe_webhook() -> Result<bool, HandlerError>;
}

pub struct CheckoutServiceImpl;

impl CheckoutService for CheckoutServiceImpl {
    async fn handle(
        &self,
        ctx: Context<'_>,
        Json(CheckoutRequest {
            user_id: _,
            tickets: _,
        }): Json<CheckoutRequest>,
    ) -> Result<bool, HandlerError> {
        let loops = hypr_loops::LoopClientBuilder::default()
            .api_key("TODO")
            .build();

        ctx.run(|| send_checkout_event(&loops)).await?;
        Ok(true)
    }

    // // https://docs.restate.dev/guides/durable-webhooks/
    async fn on_stripe_webhook(&self, _ctx: Context<'_>) -> Result<bool, HandlerError> {
        Ok(true)
    }
}

async fn send_checkout_event(
    loops: &hypr_loops::LoopClient,
) -> Result<Json<hypr_loops::Response>, HandlerError> {
    let res = loops
        .send_event(hypr_loops::Event {
            name: "checkout".to_string(),
            properties: None,
        })
        .await?;

    Ok(Json(res))
}
