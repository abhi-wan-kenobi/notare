use hypr_llm_types::{ImageDetail, MessageContent, MessagePart};

const TEXT_TEMPERATURE: f32 = 0.1;

#[derive(serde::Deserialize)]
pub(super) struct JsonSchemaConfig {
    pub schema: Option<serde_json::Value>,
}

#[derive(serde::Deserialize)]
#[serde(tag = "type")]
pub(super) enum ResponseFormat {
    #[serde(rename = "text")]
    Text,
    #[serde(rename = "json_object")]
    JsonObject,
    #[serde(rename = "json_schema")]
    JsonSchema { json_schema: JsonSchemaConfig },
}

impl ResponseFormat {
    pub(super) fn system_instruction(&self) -> Option<String> {
        match self {
            Self::Text => None,
            Self::JsonObject => Some(
                "Respond with valid JSON only. No explanation. No code fences. The response must be directly parsable."
                    .to_string(),
            ),
            Self::JsonSchema { .. } => Some(
                "Respond with valid JSON only, complying with the provided JSON schema. No explanation. No code fences. The response must be directly parsable."
                    .to_string(),
            ),
        }
    }

    pub(super) fn json_schema(&self) -> Option<serde_json::Value> {
        match self {
            Self::JsonSchema { json_schema } => json_schema.schema.clone(),
            _ => None,
        }
    }
}

#[derive(serde::Deserialize)]
pub(super) struct ChatCompletionRequest {
    #[serde(default, rename = "model")]
    pub(super) _ignored_model: Option<String>,
    pub(super) messages: Vec<ChatMessage>,
    #[serde(default)]
    pub(super) stream: Option<bool>,
    #[serde(default)]
    max_tokens: Option<u32>,
    #[serde(default)]
    max_completion_tokens: Option<u32>,
    #[serde(default)]
    pub(super) response_format: Option<ResponseFormat>,
}

#[derive(serde::Deserialize)]
pub(super) struct ChatMessage {
    role: String,
    #[serde(default)]
    content: Option<ChatMessageContent>,
}

#[derive(serde::Deserialize)]
#[serde(untagged)]
enum ChatMessageContent {
    Text(String),
    Parts(Vec<ChatMessagePart>),
}

#[derive(serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ChatMessagePart {
    Text { text: String },
    ImageUrl { image_url: ChatImageUrl },
}

#[derive(serde::Deserialize)]
struct ChatImageUrl {
    url: String,
    #[serde(default)]
    detail: Option<ImageDetail>,
}

pub(super) fn convert_messages(
    messages: &[ChatMessage],
) -> Result<Vec<hypr_llm_types::Message>, String> {
    messages.iter().map(convert_message).collect()
}

fn convert_message(message: &ChatMessage) -> Result<hypr_llm_types::Message, String> {
    let content = match &message.content {
        Some(ChatMessageContent::Text(text)) => MessageContent::Text(text.clone()),
        Some(ChatMessageContent::Parts(parts)) => {
            let mut converted = Vec::with_capacity(parts.len());
            for part in parts {
                match part {
                    ChatMessagePart::Text { text } => {
                        converted.push(MessagePart::text(text.clone()))
                    }
                    ChatMessagePart::ImageUrl { image_url } => {
                        if message.role != "user" {
                            return Err(
                                "image_url parts are only supported for user messages".into()
                            );
                        }
                        converted.push(MessagePart::image_url_with_detail(
                            image_url.url.clone(),
                            image_url.detail.clone().unwrap_or(ImageDetail::Auto),
                        ));
                    }
                }
            }
            MessageContent::Parts(converted)
        }
        None => MessageContent::default(),
    };

    Ok(hypr_llm_types::Message {
        role: message.role.clone(),
        content,
        name: None,
        audio: None,
        tool_calls: None,
    })
}

pub(super) fn build_options(request: &ChatCompletionRequest) -> hypr_cactus::CompleteOptions {
    hypr_cactus::CompleteOptions {
        temperature: Some(TEXT_TEMPERATURE),
        max_tokens: request.max_completion_tokens.or(request.max_tokens),
        ..Default::default()
    }
}

pub(super) fn apply_response_format(
    response_format: Option<&ResponseFormat>,
    messages: &mut Vec<hypr_llm_types::Message>,
    options: &mut hypr_cactus::CompleteOptions,
) {
    let format = match response_format {
        Some(format) => format,
        None => return,
    };

    let instruction = match format.system_instruction() {
        Some(instruction) => instruction,
        None => return,
    };

    if let Some(schema) = format.json_schema() {
        options.json_schema = Some(schema);
    }

    if let Some(system_msg) = messages.iter_mut().find(|m| m.role == "system") {
        match &mut system_msg.content {
            MessageContent::Text(text) => {
                text.push_str("\n\n");
                text.push_str(&instruction);
            }
            _ => {
                messages.insert(0, hypr_llm_types::Message::system(instruction));
            }
        }
    } else {
        messages.insert(0, hypr_llm_types::Message::system(instruction));
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;
    use crate::{ModelManager, service::CompleteService};
    use axum::{body::Body, body::to_bytes, http::Request, http::StatusCode};
    use tower::Service;

    struct TestImagePath {
        path: PathBuf,
    }

    impl TestImagePath {
        fn create_file() -> Self {
            let path = unique_temp_path("png");
            std::fs::write(&path, b"image").unwrap();
            Self { path }
        }

        fn create_dir() -> Self {
            let path = unique_temp_path("dir");
            std::fs::create_dir(&path).unwrap();
            Self { path }
        }

        fn url(&self) -> String {
            file_url(&self.path)
        }
    }

    impl Drop for TestImagePath {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.path);
            let _ = std::fs::remove_dir(&self.path);
        }
    }

    fn unique_temp_path(suffix: &str) -> PathBuf {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "llm-cactus-test-{}-{timestamp}-{suffix}",
            std::process::id()
        ))
    }

    fn make_request(content: serde_json::Value, stream: bool) -> Request<Body> {
        Request::builder()
            .method("POST")
            .uri("/v1/chat/completions")
            .body(Body::from(
                serde_json::json!({
                    "stream": stream,
                    "messages": [content]
                })
                .to_string(),
            ))
            .unwrap()
    }

    fn file_url(path: &std::path::Path) -> String {
        format!("file://{}", path.to_string_lossy())
    }

    fn image_url_content(url: &str) -> serde_json::Value {
        serde_json::json!({
            "role": "user",
            "content": [{"type": "image_url", "image_url": { "url": url }}]
        })
    }

    async fn assert_bad_request(content: serde_json::Value, stream: bool, needle: &str) {
        let manager = ModelManager::<hypr_cactus::Model>::builder().build();
        let mut service = CompleteService::new(manager);
        let response = service.call(make_request(content, stream)).await.unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let text = String::from_utf8(body.to_vec()).unwrap();
        assert!(text.contains(needle), "{text}");
    }

    async fn assert_bad_request_both_modes(url: &str, needle: &str) {
        for stream in [false, true] {
            assert_bad_request(image_url_content(url), stream, needle).await;
        }
    }

    fn snapshot_messages(messages: &[hypr_llm_types::Message]) -> String {
        serde_json::to_string_pretty(messages).unwrap()
    }

    #[test]
    fn converts_text_only_messages() {
        let request = vec![ChatMessage {
            role: "user".into(),
            content: Some(ChatMessageContent::Text("hello".into())),
        }];

        let messages = convert_messages(&request).unwrap();

        assert_eq!(messages, vec![hypr_llm_types::Message::user("hello")]);
    }

    #[test]
    fn converts_openai_style_image_parts() {
        let request = vec![ChatMessage {
            role: "user".into(),
            content: Some(ChatMessageContent::Parts(vec![
                ChatMessagePart::Text {
                    text: "describe ".into(),
                },
                ChatMessagePart::ImageUrl {
                    image_url: ChatImageUrl {
                        url: "file:///tmp/test.png".into(),
                        detail: Some(ImageDetail::High),
                    },
                },
            ])),
        }];

        let messages = convert_messages(&request).unwrap();

        assert_eq!(
            messages,
            vec![hypr_llm_types::Message::user(vec![
                MessagePart::text("describe "),
                MessagePart::image_url_with_detail("file:///tmp/test.png", ImageDetail::High),
            ])]
        );
    }

    #[test]
    fn rejects_image_parts_for_non_user_messages() {
        let request = vec![ChatMessage {
            role: "assistant".into(),
            content: Some(ChatMessageContent::Parts(vec![ChatMessagePart::ImageUrl {
                image_url: ChatImageUrl {
                    url: "file:///tmp/test.png".into(),
                    detail: None,
                },
            }])),
        }];

        let error = convert_messages(&request).unwrap_err();

        assert!(error.contains("only supported for user messages"));
    }

    #[test]
    fn builds_hardcoded_generation_options() {
        let request = ChatCompletionRequest {
            _ignored_model: None,
            messages: vec![ChatMessage {
                role: "user".into(),
                content: Some(ChatMessageContent::Text("hello".into())),
            }],
            stream: None,
            max_tokens: Some(123),
            max_completion_tokens: None,
            response_format: None,
        };

        let options = build_options(&request);

        assert_eq!(options.temperature, Some(TEXT_TEMPERATURE));
        assert_eq!(options.max_tokens, Some(123));
    }

    #[test]
    fn apply_response_format_none_is_noop() {
        let mut messages = vec![hypr_llm_types::Message::system("You are helpful.")];
        let mut options = hypr_cactus::CompleteOptions::default();

        apply_response_format(None, &mut messages, &mut options);

        assert_eq!(messages.len(), 1);
        assert!(options.json_schema.is_none());
    }

    #[test]
    fn apply_response_format_text_is_noop() {
        let mut messages = vec![hypr_llm_types::Message::system("You are helpful.")];
        let mut options = hypr_cactus::CompleteOptions::default();

        apply_response_format(Some(&ResponseFormat::Text), &mut messages, &mut options);

        assert_eq!(messages.len(), 1);
        assert!(options.json_schema.is_none());
    }

    #[test]
    fn apply_response_format_json_object_injects_into_system() {
        let mut messages = vec![
            hypr_llm_types::Message::system("You are helpful."),
            hypr_llm_types::Message::user("hello"),
        ];
        let mut options = hypr_cactus::CompleteOptions::default();

        apply_response_format(
            Some(&ResponseFormat::JsonObject),
            &mut messages,
            &mut options,
        );

        assert_eq!(messages.len(), 2);
        match &messages[0].content {
            MessageContent::Text(text) => {
                assert!(text.contains("Respond with valid JSON only."));
            }
            _ => panic!("expected text content"),
        }
        assert!(options.json_schema.is_none());
    }

    #[test]
    fn apply_response_format_json_schema_injects_and_sets_option() {
        let schema =
            serde_json::json!({"type": "object", "properties": {"name": {"type": "string"}}});
        let mut messages = vec![
            hypr_llm_types::Message::system("You are helpful."),
            hypr_llm_types::Message::user("hello"),
        ];
        let mut options = hypr_cactus::CompleteOptions::default();
        let fmt = ResponseFormat::JsonSchema {
            json_schema: JsonSchemaConfig {
                schema: Some(schema.clone()),
            },
        };

        apply_response_format(Some(&fmt), &mut messages, &mut options);

        insta::assert_snapshot!(
            snapshot_messages(&messages),
            @r#"
        [
          {
            "role": "system",
            "content": "You are helpful.\n\nRespond with valid JSON only, complying with the provided JSON schema. No explanation. No code fences. The response must be directly parsable."
          },
          {
            "role": "user",
            "content": "hello"
          }
        ]
        "#
        );
        assert_eq!(options.json_schema, Some(schema));
    }

    #[test]
    fn apply_response_format_inserts_system_when_missing() {
        let mut messages = vec![hypr_llm_types::Message::user("hello")];
        let mut options = hypr_cactus::CompleteOptions::default();

        apply_response_format(
            Some(&ResponseFormat::JsonObject),
            &mut messages,
            &mut options,
        );

        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, "system");
    }

    #[test]
    fn apply_response_format_json_schema_inserts_new_system_message_snapshot() {
        let mut messages = vec![hypr_llm_types::Message::user("hello")];
        let mut options = hypr_cactus::CompleteOptions::default();
        let schema = serde_json::json!({
            "type": "object",
            "required": ["name"],
            "properties": {
                "name": {"type": "string"}
            }
        });
        let fmt = ResponseFormat::JsonSchema {
            json_schema: JsonSchemaConfig {
                schema: Some(schema.clone()),
            },
        };

        apply_response_format(Some(&fmt), &mut messages, &mut options);

        insta::assert_snapshot!(
            snapshot_messages(&messages),
            @r#"
        [
          {
            "role": "system",
            "content": "Respond with valid JSON only, complying with the provided JSON schema. No explanation. No code fences. The response must be directly parsable."
          },
          {
            "role": "user",
            "content": "hello"
          }
        ]
        "#
        );
        assert_eq!(options.json_schema, Some(schema));
    }

    #[test]
    fn deserializes_response_format_variants() {
        let text: ResponseFormat = serde_json::from_str(r#"{"type":"text"}"#).unwrap();
        assert!(matches!(text, ResponseFormat::Text));

        let json_obj: ResponseFormat = serde_json::from_str(r#"{"type":"json_object"}"#).unwrap();
        assert!(matches!(json_obj, ResponseFormat::JsonObject));

        let json_schema: ResponseFormat = serde_json::from_str(
            r#"{"type":"json_schema","json_schema":{"schema":{"type":"object"}}}"#,
        )
        .unwrap();
        assert!(matches!(json_schema, ResponseFormat::JsonSchema { .. }));
    }

    #[test]
    fn deserializes_request_without_response_format() {
        let req: ChatCompletionRequest =
            serde_json::from_str(r#"{"messages":[{"role":"user","content":"hi"}]}"#).unwrap();
        assert!(req.response_format.is_none());
    }

    #[test]
    fn deserializes_request_with_response_format() {
        let req: ChatCompletionRequest = serde_json::from_str(
            r#"{"messages":[{"role":"user","content":"hi"}],"response_format":{"type":"json_object"}}"#,
        )
        .unwrap();
        assert!(matches!(
            req.response_format,
            Some(ResponseFormat::JsonObject)
        ));
    }

    #[tokio::test]
    async fn returns_bad_request_for_invalid_non_user_image_part() {
        assert_bad_request(
            serde_json::json!({
                "role": "assistant",
                "content": [{"type": "image_url", "image_url": { "url": "file:///tmp/test.png" }}]
            }),
            false,
            "only supported for user messages",
        )
        .await;
    }

    #[tokio::test]
    async fn returns_bad_request_for_non_file_image_urls() {
        assert_bad_request_both_modes("https://example.com/test.png", "local file:// URL").await;
    }

    #[tokio::test]
    async fn returns_bad_request_for_malformed_file_urls() {
        assert_bad_request_both_modes(
            "file://remote-host/tmp/test.png",
            "must resolve to a local path",
        )
        .await;
    }

    #[tokio::test]
    async fn returns_bad_request_for_missing_local_image_files() {
        let url = file_url(&unique_temp_path("missing.png"));
        assert_bad_request_both_modes(&url, "image file is not accessible").await;
    }

    #[tokio::test]
    async fn returns_bad_request_for_non_file_targets() {
        let dir = TestImagePath::create_dir();
        assert_bad_request_both_modes(&dir.url(), "must point to a file").await;
    }

    #[test]
    fn accepts_existing_local_file_urls() {
        let image = TestImagePath::create_file();
        let messages = convert_messages(&[ChatMessage {
            role: "user".into(),
            content: Some(ChatMessageContent::Parts(vec![ChatMessagePart::ImageUrl {
                image_url: ChatImageUrl {
                    url: image.url(),
                    detail: Some(ImageDetail::Low),
                },
            }])),
        }])
        .unwrap();

        hypr_cactus::validate_messages(&messages).unwrap();
    }
}
