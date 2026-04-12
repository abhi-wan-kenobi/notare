use std::cell::{Cell, UnsafeCell};
use std::ffi::{CStr, CString};

use crate::error::{Error, Result};
use crate::ffi_utils::{RESPONSE_BUF_SIZE, parse_buf};
use crate::model::{InferenceGuard, Model};

use super::request::PreparedRequest;
use super::{CompleteOptions, CompletionResult, Message};

type TokenCallback = unsafe extern "C" fn(*const std::ffi::c_char, u32, *mut std::ffi::c_void);

struct CallbackState<'a, F: FnMut(&str) -> bool> {
    on_token: UnsafeCell<&'a mut F>,
    model: &'a Model,
    stopped: Cell<bool>,
    in_callback: Cell<bool>,
}

unsafe extern "C" fn token_trampoline<F: FnMut(&str) -> bool>(
    token: *const std::ffi::c_char,
    _token_id: u32,
    user_data: *mut std::ffi::c_void,
) {
    if token.is_null() || user_data.is_null() {
        return;
    }

    let state = unsafe { &*(user_data as *const CallbackState<F>) };
    if state.stopped.get() || state.in_callback.get() {
        return;
    }
    state.in_callback.set(true);

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let chunk = unsafe { CStr::from_ptr(token) }.to_string_lossy();
        let on_token = unsafe { &mut *state.on_token.get() };
        if !on_token(&chunk) {
            state.stopped.set(true);
            state.model.stop();
        }
    }));

    state.in_callback.set(false);
    if result.is_err() {
        state.stopped.set(true);
        state.model.stop();
    }
}

pub(super) fn complete_error(rc: i32) -> Error {
    Error::Inference(format!("cactus_complete failed ({rc})"))
}

pub fn complete(
    model: &Model,
    messages: &[Message],
    options: &CompleteOptions,
) -> Result<CompletionResult> {
    model.ensure_llm_usable()?;
    let mut context = model.llm_context(messages.to_vec());
    context.complete(options)
}

pub(super) fn assistant_message_from_result(result: &CompletionResult) -> Message {
    let mut message = Message::assistant(result.text.clone());
    if !result.function_calls.is_empty() {
        message = message.with_tool_calls(result.function_calls.clone());
    }
    message
}

impl Model {
    pub fn llm_context(&self, initial_messages: Vec<Message>) -> super::LlmContext<'_> {
        super::LlmContext::new(self, initial_messages)
    }

    fn call_complete(
        &self,
        guard: &InferenceGuard<'_>,
        messages_c: &CString,
        options_c: &CString,
        callback: Option<TokenCallback>,
        user_data: *mut std::ffi::c_void,
    ) -> (i32, Vec<u8>) {
        let mut buf = vec![0u8; RESPONSE_BUF_SIZE];

        let rc = unsafe {
            cactus_sys::cactus_complete(
                guard.raw_handle(),
                messages_c.as_ptr(),
                buf.as_mut_ptr().cast::<std::ffi::c_char>(),
                buf.len(),
                options_c.as_ptr(),
                std::ptr::null(),
                callback,
                user_data,
                std::ptr::null(),
                0,
            )
        };

        (rc, buf)
    }

    pub(super) fn complete_prepared(
        &self,
        guard: &InferenceGuard<'_>,
        request: &PreparedRequest,
    ) -> Result<CompletionResult> {
        self.ensure_llm_usable()?;
        let (rc, buf) = self.call_complete(
            guard,
            &request.messages_c,
            &request.options_c,
            None,
            std::ptr::null_mut(),
        );

        if rc < 0 {
            self.poison_llm();
            return Err(complete_error(rc));
        }

        parse_buf(&buf)
            .map_err(|error| {
                self.poison_llm();
                error
            })
            .map_err(Into::into)
    }

    pub(super) fn complete_prepared_streaming_with_guard<F>(
        &self,
        guard: &InferenceGuard<'_>,
        request: &PreparedRequest,
        mut on_token: F,
    ) -> Result<CompletionResult>
    where
        F: FnMut(&str) -> bool,
    {
        self.ensure_llm_usable()?;
        let state = CallbackState {
            on_token: UnsafeCell::new(&mut on_token),
            model: self,
            stopped: Cell::new(false),
            in_callback: Cell::new(false),
        };

        let (rc, buf) = self.call_complete(
            guard,
            &request.messages_c,
            &request.options_c,
            Some(token_trampoline::<F>),
            &state as *const CallbackState<F> as *mut std::ffi::c_void,
        );

        if rc < 0 && !state.stopped.get() {
            self.poison_llm();
            return Err(complete_error(rc));
        }

        parse_buf(&buf)
            .map_err(|error| {
                self.poison_llm();
                error
            })
            .map_err(Into::into)
    }
}
