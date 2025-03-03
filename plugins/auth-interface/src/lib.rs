#[derive(Debug, serde::Serialize, serde::Deserialize, specta::Type, schemars::JsonSchema)]
pub struct RequestParams {
    #[serde(rename = "c")]
    pub code: String,
    #[serde(rename = "f")]
    pub fingerprint: String,
    #[serde(rename = "p")]
    pub port: u16,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, specta::Type, schemars::JsonSchema)]
pub struct ResponseParams {
    #[serde(rename = "ui")]
    pub user_id: String,
    #[serde(rename = "ai")]
    pub account_id: String,
    #[serde(rename = "st")]
    pub server_token: String,
    #[serde(rename = "dt")]
    pub database_token: String,
}
