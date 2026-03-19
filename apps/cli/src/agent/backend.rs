use futures_util::StreamExt;
use rig::agent::{Agent, MultiTurnStreamItem, StreamingError};
use rig::client::CompletionClient;
use rig::message::Message;
use rig::providers::{anthropic, openai, openrouter};
use rig::streaming::{StreamedAssistantContent, StreamingChat};

use crate::error::{CliError, CliResult};
use crate::llm::{LlmProvider, ResolvedLlmConfig};

use super::tools;

macro_rules! build_agent {
    ($client_type:path, $config:expr, $system_message:expr, $provider_name:expr) => {{
        let mut builder = <$client_type>::builder().api_key($config.api_key.as_str());
        if !$config.base_url.is_empty() {
            builder = builder.base_url(&$config.base_url);
        }
        let client = builder.build().map_err(|e| {
            CliError::operation_failed(concat!("build ", $provider_name, " client"), e.to_string())
        })?;
        let mut agent = client.agent($config.model);
        if let Some(msg) = $system_message.as_deref() {
            agent = agent.preamble(msg);
        }
        agent.tool(tools::Char).build()
    }};
}

#[derive(Clone)]
pub enum Backend {
    Anthropic(Agent<anthropic::completion::CompletionModel>),
    Openai(Agent<openai::CompletionModel>),
    Openrouter(Agent<openrouter::CompletionModel>),
}

impl Backend {
    pub fn new(config: ResolvedLlmConfig, system_message: Option<String>) -> CliResult<Self> {
        match config.provider {
            LlmProvider::Anthropic => Ok(Self::Anthropic(build_agent!(
                anthropic::Client,
                config,
                system_message,
                "anthropic"
            ))),
            LlmProvider::Openai => Ok(Self::Openai(build_agent!(
                openai::CompletionsClient,
                config,
                system_message,
                "openai"
            ))),
            LlmProvider::Openrouter => Ok(Self::Openrouter(build_agent!(
                openrouter::Client,
                config,
                system_message,
                "openrouter"
            ))),
        }
    }

    pub async fn stream_text<F>(
        &self,
        prompt: String,
        history: Vec<Message>,
        max_turns: usize,
        mut on_chunk: F,
    ) -> CliResult<Option<String>>
    where
        F: FnMut(&str) -> CliResult<()>,
    {
        macro_rules! do_stream {
            ($agent:expr) => {{
                let stream = $agent
                    .stream_chat(prompt, history)
                    .multi_turn(max_turns)
                    .await;
                process_stream(stream, &mut on_chunk).await
            }};
        }

        match self {
            Self::Anthropic(agent) => do_stream!(agent),
            Self::Openai(agent) => do_stream!(agent),
            Self::Openrouter(agent) => do_stream!(agent),
        }
    }
}

async fn process_stream<S, R, F>(mut stream: S, on_chunk: &mut F) -> CliResult<Option<String>>
where
    S: StreamExt<Item = Result<MultiTurnStreamItem<R>, StreamingError>> + Unpin,
    F: FnMut(&str) -> CliResult<()>,
{
    let mut accumulated = String::new();
    let mut final_response = None;
    while let Some(item) = stream.next().await {
        match item {
            Ok(MultiTurnStreamItem::StreamAssistantItem(StreamedAssistantContent::Text(text))) => {
                accumulated.push_str(&text.text);
                on_chunk(&text.text)?;
            }
            Ok(MultiTurnStreamItem::FinalResponse(response)) => {
                final_response = Some(response);
            }
            Ok(_) => {}
            Err(error) => {
                return Err(CliError::operation_failed("chat stream", error.to_string()));
            }
        }
    }
    if let Some(response) = final_response
        && !response.response().is_empty()
    {
        let final_text = response.response();
        if accumulated.is_empty() {
            on_chunk(final_text)?;
        } else if final_text.starts_with(&accumulated) && final_text.len() > accumulated.len() {
            on_chunk(&final_text[accumulated.len()..])?;
        }
        return Ok(Some(final_text.to_string()));
    }
    Ok((!accumulated.is_empty()).then_some(accumulated))
}
