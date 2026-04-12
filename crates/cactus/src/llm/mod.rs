mod complete;
mod context;
mod request;
mod result;
mod stream;

pub use complete::complete;
pub use context::LlmContext;
pub use hypr_llm_types::{Message, ToolCall};
pub use request::validate_messages;
pub use result::CompletionResult;
pub use stream::{CompletionStream, complete_stream};

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct CompleteOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_k: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequences: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence_threshold: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub force_tools: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_rag_top_k: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_stop_sequences: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub use_vad: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub telemetry_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_handoff: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cloud_timeout_ms: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub handoff_with_images: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enable_thinking_if_supported: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub json_schema: Option<serde_json::Value>,
}
