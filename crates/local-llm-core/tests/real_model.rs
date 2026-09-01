//! End-to-end test against the real downloaded `HyprLLM` model (CPU).
//!
//! Ignored by default (needs `hypr-llm.gguf`, ~1GB, downloaded from
//! `GgufLlmModel::HyprLLM::model_url()`). Run with:
//!
//! ```sh
//! HYPR_LLM_GGUF_PATH=/path/to/hypr-llm.gguf \
//!   cargo test -p local-llm-core --features llama --test real_model -- --ignored --nocapture
//! ```
#![cfg(feature = "llama")]

use local_llm_core::LlmServer;

#[tokio::test]
#[ignore]
async fn starts_serves_a_chat_completion_and_a_grammar_constrained_one() {
    let model_path = std::env::var("HYPR_LLM_GGUF_PATH")
        .expect("set HYPR_LLM_GGUF_PATH to a downloaded hypr-llm.gguf");

    let load_started = std::time::Instant::now();
    let server = LlmServer::start_with_model_path("HyprLLM".to_string(), &model_path)
        .await
        .expect("server starts");
    println!(
        "model load + server start: {:.1}s",
        load_started.elapsed().as_secs_f64()
    );
    println!("server url: {}", server.url());

    let client = reqwest::Client::new();

    // Plain chat completion.
    let started = std::time::Instant::now();
    let res = client
        .post(format!("{}/chat/completions", server.url()))
        .json(&serde_json::json!({
            "model": "local",
            "messages": [{"role": "user", "content": "Reply with exactly the word: pong"}],
            "max_tokens": 32,
            "temperature": 0.0,
        }))
        .send()
        .await
        .expect("request succeeds");
    assert!(res.status().is_success(), "status: {}", res.status());
    let body: serde_json::Value = res.json().await.expect("valid JSON response");
    let content = body["choices"][0]["message"]["content"]
        .as_str()
        .expect("choices[0].message.content is a string")
        .to_string();
    println!(
        "plain completion ({:.1}s): {content:?}",
        started.elapsed().as_secs_f64()
    );
    assert!(!content.trim().is_empty());

    // Grammar-constrained (json_schema response_format) completion — this is
    // Requirement 3's guarantee: the output must be valid JSON matching the
    // schema, not merely likely to be.
    let schema = serde_json::json!({
        "type": "object",
        "properties": { "ok": { "type": "boolean" } },
        "required": ["ok"],
        "additionalProperties": false,
    });
    let started = std::time::Instant::now();
    let res = client
        .post(format!("{}/chat/completions", server.url()))
        .json(&serde_json::json!({
            "model": "local",
            "messages": [{
                "role": "user",
                "content": "Reply with the JSON object {\"ok\": true} and nothing else.",
            }],
            "max_tokens": 32,
            "temperature": 0.0,
            "response_format": {
                "type": "json_schema",
                "json_schema": { "name": "probe", "schema": schema, "strict": true },
            },
        }))
        .send()
        .await
        .expect("request succeeds");
    assert!(res.status().is_success(), "status: {}", res.status());
    let body: serde_json::Value = res.json().await.expect("valid JSON response");
    let content = body["choices"][0]["message"]["content"]
        .as_str()
        .expect("choices[0].message.content is a string")
        .to_string();
    println!(
        "grammar-constrained completion ({:.1}s): {content:?}",
        started.elapsed().as_secs_f64()
    );
    let parsed: serde_json::Value =
        serde_json::from_str(content.trim()).expect("grammar-constrained output is valid JSON");
    assert!(parsed.get("ok").and_then(|v| v.as_bool()).is_some());

    server.stop().await;
}
