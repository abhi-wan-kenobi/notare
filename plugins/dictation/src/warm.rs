//! Lane B2 "warm-mic": optionally keep a microphone capture stream open while
//! dictation is idle, so a hotkey press skips the OS mic-open latency (the
//! dominant press->capture cost - see `session.rs`'s cold-open note).
//!
//! **OFF by default** and privacy-sensitive: a held-open mic keeps the OS
//! "microphone in use" indicator lit the whole time. Nothing here runs unless
//! `dictation_enabled && dictation_warm_mic` is on (the frontend gates the
//! `set_warm_mic` command; when off, no [`WarmHandle`] exists and
//! [`WarmMicState::try_handoff`] returns `None` synchronously, so the session
//! path is byte-identical to the pre-B2 cold open).
//!
//! ## The holder state machine ([`run_warm_holder`])
//!
//! A single background task owns the open [`CaptureStream`] and runs one of two
//! sinks:
//!
//! - **Drain** (idle): every captured frame is read and *discarded*. Draining
//!   continuously is the whole point - it keeps the OS/cpal buffer empty, so no
//!   idle audio is ever retained and the buffer can't back up.
//! - **Forward** (a session is live): frames are forwarded to the session's
//!   [`CaptureStream`] over an mpsc channel.
//!
//! ## No-bleed handoff (acceptance criterion)
//!
//! On a session start with a warm stream available, [`WarmMicState::try_handoff`]
//! sends [`WarmCmd::HandOff`]. Before switching the sink to `Forward`, the holder
//! runs [`drain_ready`], which polls the stream with `now_or_never` until it is
//! `Pending` - discarding *every* frame already buffered at that instant. Only
//! after the buffer is empty is the forward channel created.
//!
//! Precise no-bleed guarantee: all audio buffered up to the handoff instant is
//! discarded. The audio callback runs on another thread and needs no waker to
//! push, so at most ONE in-flight callback frame - the tens of milliseconds
//! immediately preceding the hotkey press - can land between the final drain
//! and the switch and reach the session. That residue is the user's own
//! pre-press instant (breath/keypress, which VAD discards), NOT retained idle
//! audio: anything older was eaten by the continuous drain. Making even that
//! frame impossible would need producer-side capture timestamps through the
//! audio crate - out of proportion to a one-frame pre-roll.
//!
//! ## Lifecycle
//!
//! - **Session end** (the session drops its forwarded `CaptureStream`): the next
//!   forward send fails and the holder returns to `Drain` (still warm), unless a
//!   disable arrived mid-session, in which case it closes fully.
//! - **Disabled while idle**: [`WarmMicState::disable`] drops the handle, the
//!   holder observes the command channel close and returns - dropping the
//!   `CaptureStream`, which releases the device (`CaptureStreamInner::Drop`).
//! - **App exit**: the plugin's `on_event(RunEvent::Exit)` calls
//!   [`WarmMicState::shutdown`], same drop-the-handle path, so no zombie
//!   "mic in use" survives a quit.
//! - **Dead stream / device switch** (the D3-adjacent WASAPI case where the
//!   default device changes and the stream ends/errs): while draining, the
//!   holder attempts exactly **one** immediate re-open; if that fails it dies
//!   and stays closed until the next enable-tick (the frontend re-fires
//!   `set_warm_mic(true)` on an interval) recreates it. A handoff that finds the
//!   warm stream dead replies `None`, so the session start transparently falls
//!   back to the cold open path and never fails.

use std::sync::{Arc, Mutex};

use futures_util::{FutureExt, StreamExt};
use tokio::sync::{mpsc, oneshot};
use tokio_stream::wrappers::ReceiverStream;

use hypr_audio::{AudioProvider, CaptureFrame, CaptureStream, Error as AudioError};

/// A blocking mic-open closure (cpal cold-open). Always invoked inside
/// `spawn_blocking` because the underlying `open_mic_capture` blocks.
pub(crate) type Opener = Arc<dyn Fn() -> Result<CaptureStream, AudioError> + Send + Sync + 'static>;

/// Build the production opener from the managed audio provider.
pub(crate) fn make_opener(
    audio: Arc<dyn AudioProvider>,
    sample_rate: u32,
    chunk_size: usize,
) -> Opener {
    Arc::new(move || audio.open_mic_capture(None, sample_rate, chunk_size))
}

enum WarmCmd {
    /// A session start requests the live stream. The holder replies with a
    /// post-handoff `CaptureStream` when a live warm stream is available, or
    /// `None` when the warm stream is dead (caller falls back to cold open).
    HandOff(oneshot::Sender<Option<CaptureStream>>),
}

struct WarmHandle {
    cmd_tx: mpsc::Sender<WarmCmd>,
}

/// Managed plugin state holding the (optional) warm-mic holder.
#[derive(Default)]
pub struct WarmMicState {
    inner: Mutex<WarmSlot>,
}

/// Holder handle + the monotonic IPC gate, under ONE lock so a stale
/// out-of-order `set_warm_mic` can neither pass the gate nor apply its
/// enable/disable after a newer call did (check and apply are atomic).
#[derive(Default)]
struct WarmSlot {
    handle: Option<WarmHandle>,
    last_seq: u32,
}

impl WarmMicState {
    /// Accept a command only if its sequence number is not older than the
    /// newest seen (equal is allowed: retries of the same intent). Stale
    /// out-of-order IPCs are dropped so a late `enable` can never override a
    /// newer `disable`.
    /// Enable warm-mic: spawn a holder if one is not already running. Idempotent
    /// - a healthy holder is left untouched, and a dead one (its task exited) is
    /// replaced. Called on the enable tick from the frontend.
    /// Apply an enable with its IPC sequence number. The gate and the apply
    /// share the slot's lock, so a stale out-of-order call can never override
    /// a newer one. Returns whether the call was accepted.
    pub fn enable(&self, opener: Opener, seq: u32) -> bool {
        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if seq < guard.last_seq {
            return false; // stale: a newer set_warm_mic already applied
        }
        guard.last_seq = seq;
        if let Some(handle) = guard.handle.as_ref() {
            if !handle.cmd_tx.is_closed() {
                // A holder is already live; don't restart it (would drop the
                // open device and re-pay the cold-open we are trying to avoid).
                return true;
            }
        }
        let (cmd_tx, cmd_rx) = mpsc::channel(4);
        tauri::async_runtime::spawn(run_warm_holder(opener, cmd_rx));
        guard.handle = Some(WarmHandle { cmd_tx });
        tracing::debug!("warm mic holder started");
        true
    }

    /// Disable warm-mic: drop the handle. The holder observes its command
    /// channel close and closes the device (immediately if idle, or after the
    /// current session ends if one is live).
    ///
    /// Known bounded edge: disable during a live session orphans the old
    /// holder (it keeps feeding that session, then closes); a re-enable
    /// before the session ends spawns a second holder, so two capture
    /// streams coexist until the session finishes. Shared-mode capture makes
    /// this harmless, and it self-resolves - not worth a liveness registry.
    pub fn disable(&self, seq: u32) -> bool {
        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if seq < guard.last_seq {
            return false;
        }
        guard.last_seq = seq;
        if guard.handle.take().is_some() {
            tracing::debug!("warm mic holder disabled");
        }
        true
    }

    /// App-exit teardown (wired to `RunEvent::Exit`): unconditional - exit
    /// outranks any in-flight IPC ordering.
    pub fn shutdown(&self) {
        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        guard.last_seq = u32::MAX;
        if guard.handle.take().is_some() {
            tracing::debug!("warm mic holder disabled");
        }
    }

    /// Try to take over the warm stream for a starting session. Returns `None`
    /// synchronously when warm-mic is off (no holder) - the guard that keeps the
    /// OFF path byte-identical to the cold open - and also `None` when the warm
    /// stream is dead, so the caller cold-opens instead.
    pub async fn try_handoff(&self) -> Option<CaptureStream> {
        let cmd_tx = {
            let guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            guard.handle.as_ref().map(|handle| handle.cmd_tx.clone())
        }?;

        let (reply_tx, reply_rx) = oneshot::channel();
        if cmd_tx.send(WarmCmd::HandOff(reply_tx)).await.is_err() {
            // Holder task already gone (dead stream, no retry left).
            return None;
        }
        reply_rx.await.ok().flatten()
    }
}

/// The forwarding sink: `None` = draining (discard frames, keep the OS buffer
/// empty), `Some(tx)` = a session is live and frames go to its `CaptureStream`.
type Forward = Option<mpsc::Sender<Result<CaptureFrame, AudioError>>>;

/// Resolves when a live session drops its receiver (its `CaptureStream` was
/// dropped, i.e. the session ended). Pends forever while draining, so as a
/// `select!` branch it only ever fires in the forwarding state. This detects
/// session-end independently of frame flow, so a session that ends while the
/// mic is silently stalled (the D3 WASAPI case) still releases the device.
async fn forward_closed(forward: &Forward) {
    match forward {
        Some(tx) => tx.closed().await,
        None => std::future::pending::<()>().await,
    }
}

enum DrainOutcome {
    /// The stream is caught up (nothing more buffered) and healthy.
    CaughtUp,
    /// The stream ended or errored while draining (dead device).
    Dead,
}

/// Discard every frame currently buffered in `stream`, returning once it would
/// block (caught up) or has died. This is the no-bleed guarantee: the caller
/// switches to forwarding only after this returns `CaughtUp`, so the first
/// forwarded frame is captured strictly after the drain instant.
fn drain_ready(stream: &mut CaptureStream) -> DrainOutcome {
    loop {
        match stream.next().now_or_never() {
            Some(Some(Ok(_frame))) => {} // buffered idle frame: discard
            Some(Some(Err(_))) | Some(None) => return DrainOutcome::Dead,
            None => return DrainOutcome::CaughtUp, // no frame ready: caught up
        }
    }
}

/// Open (or re-open) the mic on the blocking pool.
async fn open_blocking(opener: &Opener) -> Option<CaptureStream> {
    let opener = opener.clone();
    match tokio::task::spawn_blocking(move || opener()).await {
        Ok(Ok(stream)) => Some(stream),
        Ok(Err(error)) => {
            tracing::warn!(%error, "warm mic open failed");
            None
        }
        Err(error) => {
            tracing::warn!(%error, "warm mic open task panicked");
            None
        }
    }
}

/// A stream mutation deferred out of the `select!` so it never conflicts with
/// the `stream.next()` borrow held by the poll branch. Processed at the top of
/// the loop, where no select future is alive.
enum Pending {
    /// Drain the buffer and hand the live stream to a starting session.
    HandOff(oneshot::Sender<Option<CaptureStream>>),
    /// Re-open the device after it died while idle.
    Reopen,
}

async fn run_warm_holder(opener: Opener, mut cmd_rx: mpsc::Receiver<WarmCmd>) {
    let Some(mut stream) = open_blocking(&opener).await else {
        // Initial open failed: die immediately. `enable`'s next tick will retry.
        return;
    };

    let mut forward: Forward = None;
    // "Bounded: one immediate retry, then stay closed" - a single lifetime
    // reconnect so a permanently-gone device can't spin a hot re-open loop; the
    // enable-tick recreates the holder for later recovery.
    let mut retry_used = false;
    let mut close_after_session = false;
    let mut pending: Option<Pending> = None;
    // Once the command channel closes (disable/shutdown) it is perpetually
    // `Ready(None)`; with a `biased` select that would starve the frame branch
    // and spin. Latch it so the closed branch is skipped from then on.
    let mut cmd_closed = false;

    loop {
        // Deferred stream mutations run here, outside `select!`, so they hold the
        // only borrow of `stream`.
        match pending.take() {
            Some(Pending::HandOff(reply)) => {
                // A session is already being fed: a second handoff must NOT
                // clobber its forward channel (that would end the live session
                // mid-flight). The racer cold-opens instead.
                if forward.is_some() {
                    let _ = reply.send(None);
                    continue;
                }
                // The requester gave up (dropped its reply half, e.g. a
                // cancelled start): don't stand up a forward channel for
                // nobody - it would trip forward_closed and kill the holder.
                if reply.is_closed() {
                    continue;
                }
                match drain_ready(&mut stream) {
                    DrainOutcome::CaughtUp => {
                        let (frame_tx, frame_rx) = mpsc::channel(32);
                        forward = Some(frame_tx);
                        let handoff = CaptureStream::new(ReceiverStream::new(frame_rx));
                        let _ = reply.send(Some(handoff));
                    }
                    DrainOutcome::Dead => {
                        // Warm stream died: fall the session back to cold.
                        let _ = reply.send(None);
                        return;
                    }
                }
                continue;
            }
            Some(Pending::Reopen) => {
                if retry_used {
                    return; // one retry spent: stay closed
                }
                retry_used = true;
                match open_blocking(&opener).await {
                    Some(reopened) => {
                        stream = reopened;
                        continue;
                    }
                    None => return,
                }
            }
            None => {}
        }

        tokio::select! {
            // Commands win ties so a handoff is never starved by a busy mic.
            biased;

            cmd = cmd_rx.recv(), if !cmd_closed => match cmd {
                // Defer the drain+handoff so it runs with exclusive stream access.
                Some(WarmCmd::HandOff(reply)) => {
                    // One handoff at a time: a second request while one is
                    // pending (or a session live) is answered None so its
                    // caller cold-opens, instead of overwriting the pending
                    // slot / the live session's channel.
                    if pending.is_some() || forward.is_some() {
                        let _ = reply.send(None);
                    } else {
                        pending = Some(Pending::HandOff(reply));
                    }
                }
                // Command channel closed => disabled/shutdown.
                None => {
                    cmd_closed = true;
                    if forward.is_none() {
                        return; // idle: close now
                    }
                    // A session is live: keep feeding it, close on its end.
                    close_after_session = true;
                }
            },

            // Session end, detected independently of frame flow (see
            // `forward_closed`): pends forever while draining, fires when a live
            // session drops its receiver.
            () = forward_closed(&forward) => {
                if close_after_session {
                    return; // disabled mid-session: now safe to release the device
                }
                forward = None; // return to draining, still warm
            }

            frame = stream.next() => match frame {
                Some(Ok(frame)) => {
                    if let Some(frame_tx) = &forward {
                        if frame_tx.send(Ok(frame)).await.is_err() {
                            // The session dropped its stream: it ended.
                            if close_after_session {
                                return;
                            }
                            forward = None;
                        }
                    }
                    // Draining: discard, keeping the OS buffer empty.
                }
                // Dead stream: device invalidated / default device changed.
                Some(Err(_)) | None => {
                    if let Some(frame_tx) = &forward {
                        // Surface the end to the live session so it finalizes,
                        // then die (recreated on the next enable-tick).
                        let _ = frame_tx.send(Err(AudioError::MicStreamEnded)).await;
                        return;
                    }
                    // Defer the re-open so it runs with exclusive stream access.
                    pending = Some(Pending::Reopen);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    fn frame(marker: f32) -> CaptureFrame {
        CaptureFrame {
            raw_mic: Arc::from([marker]),
            raw_speaker: Arc::from([0.0_f32]),
            aec_mic: None,
        }
    }

    fn marker(frame: &CaptureFrame) -> f32 {
        frame.raw_mic[0]
    }

    /// A `CaptureStream` fed from a channel the test controls, plus its sender.
    fn test_stream() -> (
        mpsc::Sender<Result<CaptureFrame, AudioError>>,
        CaptureStream,
    ) {
        let (tx, rx) = mpsc::channel(64);
        (tx, CaptureStream::new(ReceiverStream::new(rx)))
    }

    /// An opener that hands out the pre-built streams in order, then fails
    /// (models a device that can/can't be re-opened).
    fn queue_opener(streams: Vec<CaptureStream>) -> Opener {
        let queue = Arc::new(Mutex::new(
            streams
                .into_iter()
                .collect::<std::collections::VecDeque<_>>(),
        ));
        Arc::new(move || {
            queue
                .lock()
                .unwrap()
                .pop_front()
                .ok_or(AudioError::MicOpenFailed)
        })
    }

    async fn settle() {
        tokio::time::sleep(Duration::from_millis(25)).await;
    }

    /// Join a holder task without ever blocking the test forever. A holder that
    /// fails to terminate is the liveness leak that orphans a CPU-spinning test
    /// process (see the 21h runaway that motivated this): bound the wait so it
    /// surfaces as a red test in seconds instead of hanging the binary. On
    /// timeout the handle is dropped (detached); the per-test runtime aborts the
    /// task when the test returns, so nothing escapes the process.
    async fn join_holder(holder: tokio::task::JoinHandle<()>) {
        if tokio::time::timeout(Duration::from_secs(5), holder)
            .await
            .is_err()
        {
            panic!("warm holder did not terminate within 5s - liveness leak");
        }
    }

    #[tokio::test]
    async fn drains_idle_audio_and_handoff_yields_only_post_handoff_frames() {
        let (frame_tx, stream) = test_stream();
        let (cmd_tx, cmd_rx) = mpsc::channel(4);
        let holder = tokio::spawn(run_warm_holder(queue_opener(vec![stream]), cmd_rx));

        // Idle audio captured before the hotkey (marker 0.0) - must never reach
        // the session.
        for _ in 0..5 {
            frame_tx.send(Ok(frame(0.0))).await.unwrap();
        }
        settle().await;

        let (reply_tx, reply_rx) = oneshot::channel();
        cmd_tx.send(WarmCmd::HandOff(reply_tx)).await.unwrap();
        let mut session = reply_rx.await.unwrap().expect("warm stream available");

        // Post-hotkey audio (marker 1.0).
        for _ in 0..3 {
            frame_tx.send(Ok(frame(1.0))).await.unwrap();
        }

        for _ in 0..3 {
            let f = session.next().await.unwrap().unwrap();
            assert_eq!(marker(&f), 1.0, "session received buffered idle audio");
        }

        drop(frame_tx);
        drop(cmd_tx);
        join_holder(holder).await;
    }

    /// A second handoff while a session is live must NOT clobber the live
    /// session's stream - the racer is told None and cold-opens.
    #[tokio::test]
    async fn second_handoff_while_live_is_rejected_and_session_survives() {
        let (frame_tx, stream) = test_stream();
        let (cmd_tx, cmd_rx) = mpsc::channel(4);
        let holder = tokio::spawn(run_warm_holder(queue_opener(vec![stream]), cmd_rx));
        settle().await;

        let (reply_tx, reply_rx) = oneshot::channel();
        cmd_tx.send(WarmCmd::HandOff(reply_tx)).await.unwrap();
        let mut session = reply_rx.await.unwrap().expect("first handoff succeeds");

        let (reply2_tx, reply2_rx) = oneshot::channel();
        cmd_tx.send(WarmCmd::HandOff(reply2_tx)).await.unwrap();
        assert!(
            reply2_rx.await.unwrap().is_none(),
            "second handoff must be refused while a session is live"
        );

        // The first session still receives frames - its channel survived.
        frame_tx.send(Ok(frame(1.0))).await.unwrap();
        let f = session.next().await.unwrap().unwrap();
        assert_eq!(marker(&f), 1.0);

        drop(frame_tx);
        drop(cmd_tx);
        join_holder(holder).await;
    }

    /// The IPC sequence gate: a stale enable arriving after a newer disable
    /// must be dropped (check and apply share the slot lock).
    #[tokio::test]
    async fn stale_enable_after_newer_disable_is_rejected() {
        let state = WarmMicState::default();
        let opener = queue_opener(vec![]);

        assert!(state.disable(5), "newer disable applies");
        assert!(
            !state.enable(opener.clone(), 4),
            "stale enable must be rejected after a newer disable"
        );
        {
            let guard = state.inner.lock().unwrap();
            assert!(guard.handle.is_none(), "no holder may exist");
        }
        // Equal-or-newer is accepted (retry semantics).
        assert!(state.enable(opener, 5));
    }

    #[tokio::test]
    async fn disable_while_idle_closes_the_device() {
        let (frame_tx, stream) = test_stream();
        let (cmd_tx, cmd_rx) = mpsc::channel(4);
        let holder = tokio::spawn(run_warm_holder(queue_opener(vec![stream]), cmd_rx));
        settle().await;

        // Disable = drop the command channel.
        drop(cmd_tx);
        join_holder(holder).await; // task exits => underlying CaptureStream dropped

        // The stream (ReceiverStream over `rx`) is gone, so its sender is closed.
        assert!(
            frame_tx.is_closed(),
            "warm device was not released on disable"
        );
    }

    #[tokio::test]
    async fn session_end_returns_to_draining() {
        let (frame_tx, stream) = test_stream();
        let (cmd_tx, cmd_rx) = mpsc::channel(4);
        let holder = tokio::spawn(run_warm_holder(queue_opener(vec![stream]), cmd_rx));

        // First session.
        let (reply_tx, reply_rx) = oneshot::channel();
        cmd_tx.send(WarmCmd::HandOff(reply_tx)).await.unwrap();
        let session = reply_rx.await.unwrap().expect("first handoff");
        drop(session); // session ends
        settle().await;

        // Frames sent while idle after the session must be drained, not leak.
        frame_tx.send(Ok(frame(0.0))).await.unwrap();
        settle().await;

        // The holder is still alive and warm: a second handoff succeeds.
        let (reply_tx, reply_rx) = oneshot::channel();
        cmd_tx.send(WarmCmd::HandOff(reply_tx)).await.unwrap();
        let mut session2 = reply_rx
            .await
            .unwrap()
            .expect("holder returned to draining and can hand off again");
        frame_tx.send(Ok(frame(2.0))).await.unwrap();
        let f = session2.next().await.unwrap().unwrap();
        assert_eq!(marker(&f), 2.0);

        drop(frame_tx);
        drop(cmd_tx);
        join_holder(holder).await;
    }

    #[tokio::test]
    async fn handoff_with_dead_stream_falls_back_to_cold() {
        // Single stream, no re-open available.
        let (frame_tx, stream) = test_stream();
        let (cmd_tx, cmd_rx) = mpsc::channel(4);
        let holder = tokio::spawn(run_warm_holder(queue_opener(vec![stream]), cmd_rx));
        settle().await;

        // Kill the warm stream (device gone); the one retry also fails.
        drop(frame_tx);
        settle().await;

        // Either the holder already died (send error) or it replies None - both
        // tell the session to cold-open.
        let (reply_tx, reply_rx) = oneshot::channel();
        match cmd_tx.send(WarmCmd::HandOff(reply_tx)).await {
            Err(_) => {}
            Ok(()) => assert!(
                reply_rx.await.unwrap().is_none(),
                "a dead warm stream must reply None so the session cold-opens"
            ),
        }
        join_holder(holder).await;
    }

    #[tokio::test]
    async fn one_immediate_retry_reopens_after_device_death() {
        let (frame_tx1, stream1) = test_stream();
        let (frame_tx2, stream2) = test_stream();
        let (cmd_tx, cmd_rx) = mpsc::channel(4);
        let holder = tokio::spawn(run_warm_holder(
            queue_opener(vec![stream1, stream2]),
            cmd_rx,
        ));
        settle().await;

        // Kill the first stream; the holder should re-open (stream2).
        drop(frame_tx1);
        settle().await;

        let (reply_tx, reply_rx) = oneshot::channel();
        cmd_tx.send(WarmCmd::HandOff(reply_tx)).await.unwrap();
        let mut session = reply_rx
            .await
            .unwrap()
            .expect("holder re-opened after one retry");
        frame_tx2.send(Ok(frame(7.0))).await.unwrap();
        let f = session.next().await.unwrap().unwrap();
        assert_eq!(marker(&f), 7.0);

        drop(frame_tx2);
        drop(cmd_tx);
        join_holder(holder).await;
    }

    #[tokio::test]
    async fn initial_open_failure_dies_without_a_handoff() {
        // Empty queue: the very first open fails.
        let (cmd_tx, cmd_rx) = mpsc::channel(4);
        let holder = tokio::spawn(run_warm_holder(queue_opener(vec![]), cmd_rx));
        join_holder(holder).await; // exits immediately

        // The command channel is closed, so a handoff attempt errors out.
        let (reply_tx, _reply_rx) = oneshot::channel();
        assert!(cmd_tx.send(WarmCmd::HandOff(reply_tx)).await.is_err());
    }

    #[tokio::test]
    async fn disable_during_a_session_closes_after_it_ends() {
        let (frame_tx, stream) = test_stream();
        let (cmd_tx, cmd_rx) = mpsc::channel(4);
        let holder = tokio::spawn(run_warm_holder(queue_opener(vec![stream]), cmd_rx));

        let (reply_tx, reply_rx) = oneshot::channel();
        cmd_tx.send(WarmCmd::HandOff(reply_tx)).await.unwrap();
        let session = reply_rx.await.unwrap().expect("handoff");

        // Disable mid-session: the holder must keep the session alive, not close.
        drop(cmd_tx);
        settle().await;
        assert!(
            !frame_tx.is_closed(),
            "disabling mid-session must not yank the device from the live session"
        );

        // End the session: now the holder closes fully.
        drop(session);
        join_holder(holder).await;
        assert!(
            frame_tx.is_closed(),
            "warm device must be released once the session ends after a mid-session disable"
        );
    }
}
