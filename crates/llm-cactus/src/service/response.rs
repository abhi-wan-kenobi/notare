use axum::response::{IntoResponse, Response, sse};
use futures_util::{StreamExt, stream};
use hypr_llm_types::{Response as LlmResponse, StreamingParser};

pub(super) struct ModelError(pub hypr_cactus::Error);

fn status_code_for(error: &hypr_cactus::Error) -> axum::http::StatusCode {
    match error {
        hypr_cactus::Error::InvalidRequest(_) | hypr_cactus::Error::InvalidJsonSchema { .. } => {
            axum::http::StatusCode::BAD_REQUEST
        }
        hypr_cactus::Error::InvalidStructuredOutput { .. }
        | hypr_cactus::Error::JsonSchemaValidation { .. } => {
            axum::http::StatusCode::UNPROCESSABLE_ENTITY
        }
        _ => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
    }
}

impl IntoResponse for ModelError {
    fn into_response(self) -> Response {
        if let Some(body) = structured_model_error_body(&self.0) {
            return (status_code_for(&self.0), axum::Json(body)).into_response();
        }

        (status_code_for(&self.0), self.0.to_string()).into_response()
    }
}

fn model_name(model: &Option<String>) -> &str {
    model.as_deref().unwrap_or("cactus")
}

pub(super) fn build_streaming_response(
    completion_stream: hypr_cactus::CompletionStream,
    model: &Option<String>,
) -> Response {
    let id = format!("chatcmpl-{}", uuid::Uuid::new_v4());
    let created = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let model_name = model_name(model).to_string();

    let id_for_events = id.clone();
    let model_for_events = model_name.clone();

    let data_events = completion_stream.filter_map(move |item| {
        let id = id_for_events.clone();
        let model_name = model_for_events.clone();

        async move {
            let delta = match item {
                LlmResponse::TextDelta(text) => {
                    serde_json::json!({ "content": text, "role": "assistant" })
                }
                LlmResponse::ToolCall { name, arguments } => {
                    serde_json::json!({
                        "tool_calls": [{
                            "index": 0,
                            "id": format!("call_{}", uuid::Uuid::new_v4()),
                            "type": "function",
                            "function": {
                                "name": name,
                                "arguments": serde_json::to_string(&arguments).unwrap_or_default()
                            }
                        }]
                    })
                }
                LlmResponse::Reasoning(_) => return None,
            };

            let chunk = serde_json::json!({
                "id": id,
                "object": "chat.completion.chunk",
                "created": created,
                "model": model_name,
                "choices": [{
                    "index": 0,
                    "delta": delta,
                    "finish_reason": serde_json::Value::Null
                }]
            });

            Some(Ok::<_, std::convert::Infallible>(
                sse::Event::default().data(serde_json::to_string(&chunk).unwrap_or_default()),
            ))
        }
    });

    let stop_chunk = serde_json::json!({
        "id": id,
        "object": "chat.completion.chunk",
        "created": created,
        "model": model_name,
        "choices": [{"index": 0, "delta": {}, "finish_reason": "stop"}]
    });

    let stop_event = stream::once(futures_util::future::ready(
        Ok::<_, std::convert::Infallible>(
            sse::Event::default().data(serde_json::to_string(&stop_chunk).unwrap_or_default()),
        ),
    ));

    let done_event = stream::once(futures_util::future::ready(
        Ok::<_, std::convert::Infallible>(sse::Event::default().data("[DONE]")),
    ));

    let event_stream = data_events.chain(stop_event).chain(done_event);

    sse::Sse::new(event_stream).into_response()
}

pub(super) async fn build_non_streaming_response(
    model: &std::sync::Arc<hypr_cactus::Model>,
    messages: Vec<hypr_llm_types::Message>,
    options: hypr_cactus::CompleteOptions,
    model_label: &Option<String>,
) -> Response {
    let model = std::sync::Arc::clone(model);

    let result = tokio::task::spawn_blocking(move || {
        hypr_cactus::complete(model.as_ref(), &messages, &options)
    })
    .await;

    let completion = match result {
        Ok(Ok(r)) => r,
        Ok(Err(e)) => {
            return ModelError(e).into_response();
        }
        Err(_) => {
            return (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "worker task panicked".to_string(),
            )
                .into_response();
        }
    };

    let mut parser = StreamingParser::new();
    let mut responses = parser.process_chunk(&completion.text);
    if let Some(r) = parser.flush() {
        responses.push(r);
    }

    let mut content = String::new();
    let mut tool_calls: Vec<serde_json::Value> = Vec::new();

    for item in responses {
        match item {
            LlmResponse::TextDelta(text) => content.push_str(&text),
            LlmResponse::ToolCall { name, arguments } => {
                tool_calls.push(serde_json::json!({
                    "id": format!("call_{}", uuid::Uuid::new_v4()),
                    "type": "function",
                    "function": {
                        "name": name,
                        "arguments": serde_json::to_string(&arguments).unwrap_or_default()
                    }
                }));
            }
            LlmResponse::Reasoning(_) => {}
        }
    }

    let id = format!("chatcmpl-{}", uuid::Uuid::new_v4());
    let created = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let mut message = serde_json::json!({ "role": "assistant" });
    if !content.is_empty() {
        message["content"] = serde_json::Value::String(content);
    }
    if !tool_calls.is_empty() {
        message["tool_calls"] = serde_json::Value::Array(tool_calls);
    }

    let response = serde_json::json!({
        "id": id,
        "object": "chat.completion",
        "created": created,
        "model": model_name(model_label),
        "choices": [{
            "index": 0,
            "message": message,
            "finish_reason": "stop"
        }],
        "usage": {
            "prompt_tokens": completion.prefill_tokens,
            "completion_tokens": completion.decode_tokens,
            "total_tokens": completion.total_tokens
        }
    });

    axum::Json(response).into_response()
}

fn structured_model_error_body(error: &hypr_cactus::Error) -> Option<serde_json::Value> {
    match error {
        hypr_cactus::Error::InvalidJsonSchema { message } => Some(structured_error(
            message,
            "invalid_request_error",
            "invalid_json_schema",
            Some(serde_json::json!({
                "schema_error": message,
            })),
        )),
        hypr_cactus::Error::InvalidStructuredOutput {
            message,
            raw_output,
        } => Some(structured_error(
            message,
            "invalid_response_error",
            "invalid_structured_output",
            Some(serde_json::json!({
                "raw_output": raw_output,
            })),
        )),
        hypr_cactus::Error::JsonSchemaValidation {
            message,
            violations,
            raw_output,
        } => Some(structured_error(
            message,
            "invalid_response_error",
            "json_schema_validation_failed",
            Some(serde_json::json!({
                "violations": violations,
                "raw_output": raw_output,
            })),
        )),
        _ => None,
    }
}

fn structured_error(
    message: &str,
    error_type: &str,
    code: &str,
    details: Option<serde_json::Value>,
) -> serde_json::Value {
    let mut error = serde_json::json!({
        "message": message,
        "type": error_type,
        "code": code,
    });

    if let Some(details) = details {
        error["details"] = details;
    }

    serde_json::json!({ "error": error })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_json_schema_maps_to_bad_request_with_structured_body() {
        let error = hypr_cactus::Error::InvalidJsonSchema {
            message: "type must be a string".to_string(),
        };

        assert_eq!(status_code_for(&error), axum::http::StatusCode::BAD_REQUEST);

        let body = structured_model_error_body(&error).expect("structured error body");
        assert_eq!(body["error"]["type"], "invalid_request_error");
        assert_eq!(body["error"]["code"], "invalid_json_schema");
        assert_eq!(
            body["error"]["details"]["schema_error"],
            "type must be a string"
        );
    }

    #[test]
    fn invalid_structured_output_maps_to_unprocessable_entity() {
        let error = hypr_cactus::Error::InvalidStructuredOutput {
            message: "final output is not valid JSON".to_string(),
            raw_output: "hello".to_string(),
        };

        assert_eq!(
            status_code_for(&error),
            axum::http::StatusCode::UNPROCESSABLE_ENTITY
        );

        let body = structured_model_error_body(&error).expect("structured error body");
        assert_eq!(body["error"]["type"], "invalid_response_error");
        assert_eq!(body["error"]["code"], "invalid_structured_output");
        assert_eq!(body["error"]["details"]["raw_output"], "hello");
    }

    #[test]
    fn schema_validation_error_includes_violations() {
        let error = hypr_cactus::Error::JsonSchemaValidation {
            message: "schema mismatch".to_string(),
            violations: vec![hypr_cactus::JsonSchemaViolation {
                message: "\"oops\" is not of type \"integer\"".to_string(),
                keyword: "type".to_string(),
                instance_path: "/answer".to_string(),
                schema_path: "/properties/answer/type".to_string(),
                evaluation_path: "/properties/answer/type".to_string(),
            }],
            raw_output: r#"{"answer":"oops"}"#.to_string(),
        };

        assert_eq!(
            status_code_for(&error),
            axum::http::StatusCode::UNPROCESSABLE_ENTITY
        );

        let body = structured_model_error_body(&error).expect("structured error body");
        assert_eq!(body["error"]["code"], "json_schema_validation_failed");
        assert_eq!(body["error"]["details"]["violations"][0]["keyword"], "type");
        assert_eq!(
            body["error"]["details"]["violations"][0]["instance_path"],
            "/answer"
        );
    }
}
