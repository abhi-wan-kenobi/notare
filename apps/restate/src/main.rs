use restate_sdk::prelude::*;

mod checkout_service;
use checkout_service::*;

mod nango_service;
use nango_service::*;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    HttpServer::new(
        Endpoint::builder()
            .bind(checkout_service::CheckoutServiceImpl.serve())
            .bind(nango_service::NangoServiceImpl.serve())
            .build(),
    )
    .listen_and_serve("0.0.0.0:9080".parse().unwrap())
    .await;
}
