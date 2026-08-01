use std::pin::Pin;

use serde::de::DeserializeOwned;

use futures_util::{
    SinkExt, Stream, StreamExt,
    future::{FutureExt, pending},
};
pub use tokio_tungstenite::tungstenite::{ClientRequestBuilder, Utf8Bytes, protocol::Message};

pub use crate::retry::{
    WebSocketConnectPolicy, WebSocketReconnectPolicy, WebSocketRetryCallback, WebSocketRetryEvent,
};

/// Output-stream type shared by the single-connection and reconnecting paths
/// (the two branches produce different concrete streams, so they unify behind
/// a boxed `dyn Stream`).
type BoxedOutputStream<O> = Pin<Box<dyn Stream<Item = Result<O, crate::Error>> + Send>>;

/// A mid-stream transport failure we can recover from by reconnecting: a
/// dropped/reset socket or a stalled path surfaced as a transport-level error.
/// Deliberately narrow — a remote *Close* frame (`RemoteClosed`), a payload
/// `ParseError`, or an auth failure is a deliberate server decision, not a
/// blip, so reconnecting would just loop.
fn is_reconnectable_transport_error(error: &crate::Error) -> bool {
    matches!(error, crate::Error::Connection(_))
}

const TRAILING_MESSAGE_GRACE: std::time::Duration = std::time::Duration::from_secs(5);

#[derive(Debug)]
enum ControlCommand {
    Finalize(Option<Message>),
}

struct OutputDropGuard(Option<tokio::sync::oneshot::Sender<()>>);

impl Drop for OutputDropGuard {
    fn drop(&mut self) {
        if let Some(cancel_tx) = self.0.take() {
            let _ = cancel_tx.send(());
        }
    }
}

#[derive(Clone)]
struct KeepAliveConfig {
    interval: std::time::Duration,
    message: Message,
}

#[derive(Clone)]
pub struct WebSocketHandle {
    control_tx: tokio::sync::mpsc::UnboundedSender<ControlCommand>,
}

impl WebSocketHandle {
    pub async fn finalize_with_text(&self, text: Utf8Bytes) {
        let _ = self
            .control_tx
            .send(ControlCommand::Finalize(Some(Message::Text(text))));
    }

    /// Forward an already-built finalize command to the live connection. Used
    /// by the reconnecting supervisor to relay the caller's finalize to the
    /// *current* inner connection (which is swapped on each reconnect).
    fn forward_finalize(&self, message: Option<Message>) {
        let _ = self.control_tx.send(ControlCommand::Finalize(message));
    }
}

pub trait WebSocketIO: Send + 'static {
    type Data: Send;
    type Input: Send;
    type Output: DeserializeOwned;

    fn to_input(data: Self::Data) -> Self::Input;
    fn to_message(input: Self::Input) -> Message;
    fn from_message(msg: Message) -> Result<Option<Self::Output>, crate::Error>;
}

#[derive(Clone)]
pub struct WebSocketClient {
    request: ClientRequestBuilder,
    keep_alive: Option<KeepAliveConfig>,
    connect_policy: WebSocketConnectPolicy,
    on_retry: Option<WebSocketRetryCallback>,
    reconnect: Option<WebSocketReconnectPolicy>,
}

impl WebSocketClient {
    pub fn new(request: ClientRequestBuilder) -> Self {
        Self {
            request,
            keep_alive: None,
            connect_policy: WebSocketConnectPolicy::default(),
            on_retry: None,
            reconnect: None,
        }
    }

    pub fn with_keep_alive_message(
        mut self,
        interval: std::time::Duration,
        message: Message,
    ) -> Self {
        self.keep_alive = Some(KeepAliveConfig { interval, message });
        self
    }

    pub fn with_connect_policy(mut self, policy: WebSocketConnectPolicy) -> Self {
        self.connect_policy = policy;
        self
    }

    pub fn on_retry(mut self, callback: WebSocketRetryCallback) -> Self {
        self.on_retry = Some(callback);
        self
    }

    /// Enable mid-stream reconnection: after an established session's transport
    /// drops (and only then — never during finalize/user-stop, and never before
    /// the first connect), transparently reconnect and resume streaming new
    /// audio on a fresh server session, up to `policy.max_cycles` times.
    pub fn with_reconnect(mut self, policy: WebSocketReconnectPolicy) -> Self {
        self.reconnect = Some(policy);
        self
    }

    pub async fn from_audio<T: WebSocketIO, S: Stream<Item = T::Data> + Send + Unpin + 'static>(
        &self,
        initial_message: Option<Message>,
        audio_stream: S,
    ) -> Result<(BoxedOutputStream<T::Output>, WebSocketHandle), crate::Error>
    where
        T::Data: Send + 'static,
        T::Output: Send + 'static,
    {
        if let Some(policy) = self.reconnect.clone() {
            self.from_audio_reconnecting::<T, S>(initial_message, audio_stream, policy)
                .await
        } else {
            self.from_audio_single::<T, S>(initial_message, audio_stream)
                .await
        }
    }

    async fn from_audio_single<
        T: WebSocketIO,
        S: Stream<Item = T::Data> + Send + Unpin + 'static,
    >(
        &self,
        initial_message: Option<Message>,
        mut audio_stream: S,
    ) -> Result<(BoxedOutputStream<T::Output>, WebSocketHandle), crate::Error>
    where
        T::Output: Send + 'static,
    {
        let keep_alive_config = self.keep_alive.clone();
        let ws_stream = crate::retry::connect_with_retry(
            self.request.clone(),
            &self.connect_policy,
            self.on_retry.as_ref(),
        )
        .await?;

        let (mut ws_sender, mut ws_receiver) = ws_stream.split();

        let (control_tx, mut control_rx) = tokio::sync::mpsc::unbounded_channel();
        let (error_tx, mut error_rx) = tokio::sync::mpsc::unbounded_channel::<crate::Error>();
        let (cancel_tx, mut cancel_rx) = tokio::sync::oneshot::channel();
        let handle = WebSocketHandle { control_tx };

        let _send_task = tokio::spawn(async move {
            #[derive(Debug)]
            enum SendLoopExit {
                Finalize,
                InputEnded,
                Error,
                Cancelled,
            }

            if let Some(msg) = initial_message
                && let Err(e) = ws_sender.send(msg).await
            {
                tracing::error!("ws_initial_message_failed: {:?}", e);
                let _ = error_tx.send(e.into());
                return;
            }

            let mut last_outbound_at = tokio::time::Instant::now();
            let mut audio_closed = false;
            let mut control_closed = false;
            let mut input_end_deadline: Option<tokio::time::Instant> = None;
            let mut waited_for_input_end = false;

            let exit_reason = loop {
                if audio_closed && control_closed {
                    break SendLoopExit::InputEnded;
                }

                let mut keep_alive_fut = if !audio_closed {
                    if let Some(cfg) = keep_alive_config.as_ref() {
                        tokio::time::sleep_until(last_outbound_at + cfg.interval).boxed()
                    } else {
                        pending().boxed()
                    }
                } else {
                    pending().boxed()
                };
                let mut input_end_fut = if let Some(deadline) = input_end_deadline {
                    tokio::time::sleep_until(deadline).boxed()
                } else {
                    pending().boxed()
                };

                tokio::select! {
                    biased;

                    _ = &mut cancel_rx => break SendLoopExit::Cancelled,
                    _ = keep_alive_fut.as_mut() => {
                        if let Some(cfg) = keep_alive_config.as_ref() {
                            if let Err(e) = ws_sender.send(cfg.message.clone()).await {
                                tracing::error!("ws_keepalive_failed: {:?}", e);
                                let _ = error_tx.send(e.into());
                                break SendLoopExit::Error;
                            }
                            last_outbound_at = tokio::time::Instant::now();
                        }
                    }
                    maybe_data = audio_stream.next(), if !audio_closed => {
                        match maybe_data {
                            Some(data) => {
                                let input = T::to_input(data);
                                let msg = T::to_message(input);

                                if let Err(e) = ws_sender.send(msg).await {
                                    tracing::error!("ws_send_failed: {:?}", e);
                                    let _ = error_tx.send(e.into());
                                    break SendLoopExit::Error;
                                }
                                last_outbound_at = tokio::time::Instant::now();
                            }
                            None => {
                                audio_closed = true;
                                input_end_deadline = Some(tokio::time::Instant::now() + TRAILING_MESSAGE_GRACE);
                            }
                        }
                    }
                    _ = input_end_fut.as_mut(), if input_end_deadline.is_some() => {
                        waited_for_input_end = true;
                        break SendLoopExit::InputEnded;
                    }
                    command = control_rx.recv(), if !control_closed => {
                        match command {
                            Some(ControlCommand::Finalize(maybe_msg)) => {
                                if let Some(msg) = maybe_msg
                                    && let Err(e) = ws_sender.send(msg).await {
                                        tracing::error!("ws_finalize_failed: {:?}", e);
                                        let _ = error_tx.send(e.into());
                                    }
                                break SendLoopExit::Finalize;
                            }
                            None => {
                                control_closed = true;
                            }
                        }
                    }
                    else => break SendLoopExit::InputEnded,
                }
            };

            // D3 instrumentation (2026-07-31): name the send-task exit in the
            // log - "audio channel closed" on the session side only says the
            // task died, not why.
            tracing::info!(?exit_reason, "ws_send_task_exit");

            if matches!(exit_reason, SendLoopExit::Finalize)
                || (matches!(exit_reason, SendLoopExit::InputEnded) && !waited_for_input_end)
            {
                tokio::select! {
                    _ = tokio::time::sleep(TRAILING_MESSAGE_GRACE) => {}
                    _ = &mut cancel_rx => {}
                }
            }

            let _ = ws_sender.close().await;
        });

        let output_stream = async_stream::stream! {
            let _drop_guard = OutputDropGuard(Some(cancel_tx));

            loop {
                tokio::select! {
                    biased;

                    Some(msg_result) = ws_receiver.next() => {
                        match msg_result {
                            Ok(msg) => {
                                match msg {
                                    Message::Text(_) | Message::Binary(_) => {
                                        match T::from_message(msg) {
                                            Ok(Some(output)) => {
                                                yield Ok(output);
                                            }
                                            Ok(None) => {}
                                            Err(error) => {
                                                yield Err(error);
                                                break;
                                            }
                                        }
                                    }
                                    Message::Ping(_) | Message::Pong(_) | Message::Frame(_) => continue,
                                    Message::Close(frame) => {
                                        if let Ok(error) = error_rx.try_recv() {
                                            yield Err(error);
                                            break;
                                        }

                                        if let Some(frame) = frame
                                            && frame.code != tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode::Normal
                                        {
                                            yield Err(crate::Error::remote_closed(
                                                Some(u16::from(frame.code)),
                                                frame.reason.to_string(),
                                            ));
                                        }

                                        break;
                                    }
                                }
                            }
                            Err(e) => {
                                tracing::error!("ws_receiver_failed: {:?}", e);
                                yield Err(e.into());
                                break;
                            }
                        }
                    }
                    Some(error) = error_rx.recv() => {
                        yield Err(error);
                        break;
                    }
                    else => {
                        if let Ok(error) = error_rx.try_recv() {
                            yield Err(error);
                        }
                        break;
                    }
                }
            }
        };

        Ok((Box::pin(output_stream), handle))
    }

    /// Reconnecting wrapper around `from_audio_single`. It owns the caller's
    /// audio stream and forwards it into a fresh single-connection inner client
    /// for each connection cycle. The inner connection's send/keepalive/finalize
    /// semantics are unchanged (this layer never touches the socket directly);
    /// this supervisor only watches the inner output for a mid-stream transport
    /// error and, when the session is still live (audio open, not finalizing),
    /// swaps in a new inner connection and resumes.
    ///
    /// Loss semantics: a reconnect abandons the dead connection's forwarding
    /// channel, so any audio already handed to it — the in-flight tail of the
    /// current utterance — is dropped. VAD chunks are independent server-side,
    /// so only that one partial utterance is lost; streaming resumes cleanly on
    /// the next chunk. Logged at `warn`.
    async fn from_audio_reconnecting<
        T: WebSocketIO,
        S: Stream<Item = T::Data> + Send + Unpin + 'static,
    >(
        &self,
        initial_message: Option<Message>,
        mut audio_stream: S,
        policy: WebSocketReconnectPolicy,
    ) -> Result<(BoxedOutputStream<T::Output>, WebSocketHandle), crate::Error>
    where
        T::Data: Send + 'static,
        T::Output: Send + 'static,
    {
        const AUDIO_FWD_BUFFER: usize = 32;

        // Single-connection clients (reconnect disabled to avoid recursion):
        // the first connect keeps the caller's policy; reconnects use the
        // dedicated mid-stream policy.
        let mut initial_client = self.clone();
        initial_client.reconnect = None;
        let mut reconnect_client = initial_client.clone();
        reconnect_client.connect_policy = policy.connect.clone();

        // Establish the first connection eagerly so a connect failure surfaces
        // to the caller at call time, exactly like the non-reconnecting path.
        let (first_tx, first_rx) = tokio::sync::mpsc::channel::<T::Data>(AUDIO_FWD_BUFFER);
        let (mut inner_out, mut inner_handle) = initial_client
            .from_audio_single::<T, _>(
                initial_message.clone(),
                tokio_stream::wrappers::ReceiverStream::new(first_rx),
            )
            .await?;

        let (out_tx, mut out_rx) =
            tokio::sync::mpsc::unbounded_channel::<Result<T::Output, crate::Error>>();
        let (ctrl_tx, mut ctrl_rx) = tokio::sync::mpsc::unbounded_channel::<ControlCommand>();
        let (cancel_tx, mut cancel_rx) = tokio::sync::oneshot::channel::<()>();
        let handle = WebSocketHandle {
            control_tx: ctrl_tx,
        };

        tokio::spawn(async move {
            let mut audio_tx: Option<tokio::sync::mpsc::Sender<T::Data>> = Some(first_tx);
            // `audio_open`: the caller's audio stream hasn't ended. Once it
            // ends we drop the forwarding sender (inner sees EOF -> normal
            // finalize-grace close) and never reconnect again.
            let mut audio_open = true;
            // `audio_paused`: the current inner send half is gone (its channel
            // closed) but the inner output error that drives the reconnect
            // hasn't been observed yet — stop pumping audio so we don't spin.
            let mut audio_paused = false;
            let mut finalizing = false;
            // Once the caller's handle is dropped, `ctrl_rx.recv()` resolves to
            // `None` on every poll; stop selecting on it so a dropped handle
            // can't busy-loop and starve output draining.
            let mut ctrl_open = true;
            let mut cycles_used = 0usize;
            let mut reconnect_count = 0u32;

            loop {
                tokio::select! {
                    biased;

                    _ = &mut cancel_rx => break,

                    maybe_ctrl = ctrl_rx.recv(), if ctrl_open => {
                        match maybe_ctrl {
                            Some(ControlCommand::Finalize(message)) => {
                                finalizing = true;
                                inner_handle.forward_finalize(message);
                            }
                            None => ctrl_open = false,
                        }
                    }

                    item = inner_out.next() => {
                        match item {
                            Some(Ok(output)) => {
                                if out_tx.send(Ok(output)).is_err() {
                                    break;
                                }
                            }
                            Some(Err(error)) => {
                                let should_reconnect = audio_open
                                    && !finalizing
                                    && cycles_used < policy.max_cycles
                                    && is_reconnectable_transport_error(&error);

                                if !should_reconnect {
                                    let _ = out_tx.send(Err(error));
                                    break;
                                }

                                cycles_used += 1;
                                reconnect_count += 1;
                                tracing::warn!(
                                    attempt = cycles_used,
                                    max_cycles = policy.max_cycles,
                                    cause = ?error,
                                    "ws_reconnect: mid-stream transport drop, reconnecting"
                                );

                                let (tx, rx) =
                                    tokio::sync::mpsc::channel::<T::Data>(AUDIO_FWD_BUFFER);
                                match reconnect_client
                                    .from_audio_single::<T, _>(
                                        initial_message.clone(),
                                        tokio_stream::wrappers::ReceiverStream::new(rx),
                                    )
                                    .await
                                {
                                    Ok((new_out, new_handle)) => {
                                        inner_out = new_out;
                                        inner_handle = new_handle;
                                        audio_tx = Some(tx);
                                        audio_paused = false;
                                        tracing::warn!(
                                            reconnect_count,
                                            "ws_reconnect_resumed: fresh server session; \
                                             in-flight audio tail (a partial utterance) was lost"
                                        );
                                    }
                                    Err(connect_error) => {
                                        tracing::error!(
                                            attempt = cycles_used,
                                            error = ?connect_error,
                                            "ws_reconnect_failed: surfacing terminal error"
                                        );
                                        let _ = out_tx.send(Err(connect_error));
                                        break;
                                    }
                                }
                            }
                            None => break,
                        }
                    }

                    maybe_audio = audio_stream.next(), if audio_open && !audio_paused => {
                        match maybe_audio {
                            Some(data) => {
                                if let Some(tx) = audio_tx.as_ref()
                                    && tx.send(data).await.is_err()
                                {
                                    // Inner send half gone; let the inner output
                                    // error arm drive the reconnect.
                                    audio_paused = true;
                                }
                            }
                            None => {
                                audio_open = false;
                                audio_tx = None;
                            }
                        }
                    }
                }
            }
        });

        let output = async_stream::stream! {
            let _drop_guard = OutputDropGuard(Some(cancel_tx));
            while let Some(item) = out_rx.recv().await {
                yield item;
            }
        };

        Ok((Box::pin(output), handle))
    }
}
