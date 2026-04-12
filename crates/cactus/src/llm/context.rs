use crate::error::Result;
use crate::model::{InferenceGuard, Model};

use super::complete::assistant_message_from_result;
use super::request::serialize_complete_request;
use super::{CompleteOptions, CompletionResult, Message};

/// An exclusive cache-backed LLM session bound to a single model.
///
/// While a context exists, no other inference can use the same model.
/// The session's message history is the Rust-side source of truth for rebuilding
/// native KV cache state after a reset. Dropping the session resets the native
/// cache so it cannot keep growing across unrelated uses of the same model.
pub struct LlmContext<'a> {
    model: &'a Model,
    guard: InferenceGuard<'a>,
    messages: Vec<Message>,
}

impl<'a> LlmContext<'a> {
    pub(super) fn new(model: &'a Model, initial_messages: Vec<Message>) -> Self {
        let guard = model.lock_inference();
        Self {
            model,
            guard,
            messages: initial_messages,
        }
    }

    pub fn messages(&self) -> &[Message] {
        &self.messages
    }

    pub fn push(&mut self, message: Message) {
        self.messages.push(message);
    }

    pub fn reset_cache(&mut self) {
        self.model.reset_with_guard(&self.guard);
    }

    pub fn clear(&mut self) {
        self.reset_cache();
        self.messages.clear();
    }

    pub fn complete(&mut self, options: &CompleteOptions) -> Result<CompletionResult> {
        let request = serialize_complete_request(&self.messages, options)?;
        match self.model.complete_prepared(&self.guard, &request) {
            Ok(result) => {
                self.messages.push(assistant_message_from_result(&result));
                Ok(result)
            }
            Err(error) => {
                self.reset_cache();
                Err(error)
            }
        }
    }

    pub fn complete_streaming<F>(
        &mut self,
        options: &CompleteOptions,
        on_token: F,
    ) -> Result<CompletionResult>
    where
        F: FnMut(&str) -> bool,
    {
        let request = serialize_complete_request(&self.messages, options)?;
        match self
            .model
            .complete_prepared_streaming_with_guard(&self.guard, &request, on_token)
        {
            Ok(result) => {
                self.messages.push(assistant_message_from_result(&result));
                Ok(result)
            }
            Err(error) => {
                self.reset_cache();
                Err(error)
            }
        }
    }
}

impl Drop for LlmContext<'_> {
    fn drop(&mut self) {
        self.model.reset_with_guard(&self.guard);
    }
}
