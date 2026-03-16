use futures_util::StreamExt;
use rig::agent::{Agent, MultiTurnStreamItem};
use rig::client::CompletionClient;
use rig::message::Message;
use rig::providers::{anthropic, openrouter};
use rig::streaming::{StreamedAssistantContent, StreamingChat};

use crate::error::{CliError, CliResult};
use crate::llm::{LlmProvider, ResolvedLlmConfig};

#[derive(Clone)]
pub enum Backend {
    Anthropic(Agent<anthropic::completion::CompletionModel>),
    Openrouter(Agent<openrouter::CompletionModel>),
}

impl Backend {
    pub fn new(config: ResolvedLlmConfig, system_message: Option<String>) -> CliResult<Self> {
        match config.provider {
            LlmProvider::Anthropic => {
                let mut client = anthropic::Client::builder().api_key(config.api_key.as_str());
                if !config.base_url.is_empty() {
                    client = client.base_url(&config.base_url);
                }
                let client = client.build().map_err(|e| {
                    CliError::operation_failed("build anthropic client", e.to_string())
                })?;
                let mut agent = client.agent(config.model);
                if let Some(system_message) = system_message.as_deref() {
                    agent = agent.preamble(system_message);
                }
                Ok(Self::Anthropic(agent.build()))
            }
            LlmProvider::Openrouter => {
                let mut client = openrouter::Client::builder().api_key(config.api_key.as_str());
                if !config.base_url.is_empty() {
                    client = client.base_url(&config.base_url);
                }
                let client = client.build().map_err(|e| {
                    CliError::operation_failed("build openrouter client", e.to_string())
                })?;
                let mut agent = client.agent(config.model);
                if let Some(system_message) = system_message.as_deref() {
                    agent = agent.preamble(system_message);
                }
                Ok(Self::Openrouter(agent.build()))
            }
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
        match self {
            Self::Anthropic(agent) => {
                let mut stream = agent
                    .stream_chat(prompt, history)
                    .multi_turn(max_turns)
                    .await;
                let mut accumulated = String::new();
                let mut final_response = None;
                while let Some(item) = stream.next().await {
                    match item {
                        Ok(MultiTurnStreamItem::StreamAssistantItem(
                            StreamedAssistantContent::Text(text),
                        )) => {
                            accumulated.push_str(&text.text);
                            on_chunk(&text.text)?;
                        }
                        Ok(MultiTurnStreamItem::FinalResponse(response)) => {
                            final_response = Some(response);
                        }
                        Ok(_) => {}
                        Err(error) => {
                            return Err(CliError::operation_failed(
                                "chat stream",
                                error.to_string(),
                            ));
                        }
                    }
                }
                if let Some(response) = final_response
                    && !response.response().is_empty()
                {
                    let final_text = response.response();
                    if accumulated.is_empty() {
                        on_chunk(final_text)?;
                    } else if final_text.starts_with(&accumulated)
                        && final_text.len() > accumulated.len()
                    {
                        on_chunk(&final_text[accumulated.len()..])?;
                    }
                    return Ok(Some(final_text.to_string()));
                }
                Ok((!accumulated.is_empty()).then_some(accumulated))
            }
            Self::Openrouter(agent) => {
                let mut stream = agent
                    .stream_chat(prompt, history)
                    .multi_turn(max_turns)
                    .await;
                let mut accumulated = String::new();
                let mut final_response = None;
                while let Some(item) = stream.next().await {
                    match item {
                        Ok(MultiTurnStreamItem::StreamAssistantItem(
                            StreamedAssistantContent::Text(text),
                        )) => {
                            accumulated.push_str(&text.text);
                            on_chunk(&text.text)?;
                        }
                        Ok(MultiTurnStreamItem::FinalResponse(response)) => {
                            final_response = Some(response);
                        }
                        Ok(_) => {}
                        Err(error) => {
                            return Err(CliError::operation_failed(
                                "chat stream",
                                error.to_string(),
                            ));
                        }
                    }
                }
                if let Some(response) = final_response
                    && !response.response().is_empty()
                {
                    let final_text = response.response();
                    if accumulated.is_empty() {
                        on_chunk(final_text)?;
                    } else if final_text.starts_with(&accumulated)
                        && final_text.len() > accumulated.len()
                    {
                        on_chunk(&final_text[accumulated.len()..])?;
                    }
                    return Ok(Some(final_text.to_string()));
                }
                Ok((!accumulated.is_empty()).then_some(accumulated))
            }
        }
    }
}
