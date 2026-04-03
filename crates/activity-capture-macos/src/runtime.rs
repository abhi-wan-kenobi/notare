#![cfg(target_os = "macos")]

use std::{
    pin::Pin,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    task::{Context, Poll},
    thread,
    time::Duration,
};

use futures_core::Stream;
use hypr_activity_capture_interface::{
    CaptureError, CaptureStream, EventCoalescer, Snapshot, Transition, WatchOptions,
};
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};
use tokio_stream::wrappers::UnboundedReceiverStream;

use crate::platform::MacosCapture;

pub(crate) fn spawn_watch_stream(
    capture: MacosCapture,
    options: WatchOptions,
) -> Result<CaptureStream, CaptureError> {
    spawn_watch_stream_with(move || capture.capture_snapshot(), options)
}

fn spawn_watch_stream_with<F>(
    poll_snapshot: F,
    options: WatchOptions,
) -> Result<CaptureStream, CaptureError>
where
    F: FnMut() -> Result<Option<Snapshot>, CaptureError> + Send + 'static,
{
    let (transition_tx, transition_rx) = unbounded_channel();
    let stop = Arc::new(StopSignal::default());
    let thread_stop = Arc::clone(&stop);

    let handle = thread::Builder::new()
        .name("activity-capture-macos".to_string())
        .spawn(move || watch_loop(poll_snapshot, options, thread_stop, transition_tx))
        .map_err(|error| CaptureError::platform(error.to_string()))?;

    Ok(Box::pin(WatchStream {
        inner: UnboundedReceiverStream::new(transition_rx),
        stop,
        handle: Some(handle),
    }))
}

fn watch_loop<F>(
    mut poll_snapshot: F,
    options: WatchOptions,
    stop: Arc<StopSignal>,
    transition_tx: UnboundedSender<Result<Transition, CaptureError>>,
) where
    F: FnMut() -> Result<Option<Snapshot>, CaptureError>,
{
    let mut state = WatchState::new(options);
    let mut first_iteration = true;

    loop {
        if !first_iteration && stop.wait_timeout(options.poll_interval) {
            break;
        }
        first_iteration = false;

        if stop.is_set() {
            break;
        }

        match poll_snapshot() {
            Ok(snapshot) => {
                let Some(transition) = state.push(snapshot) else {
                    continue;
                };

                if transition_tx.send(Ok(transition)).is_err() {
                    break;
                }
            }
            Err(error) => {
                let _ = transition_tx.send(Err(error));
                break;
            }
        }
    }
}

#[derive(Default)]
struct StopSignal {
    stopped: AtomicBool,
    mutex: Mutex<()>,
    condvar: Condvar,
}

impl StopSignal {
    fn stop(&self) {
        self.stopped.store(true, Ordering::SeqCst);
        self.condvar.notify_all();
    }

    fn is_set(&self) -> bool {
        self.stopped.load(Ordering::SeqCst)
    }

    fn wait_timeout(&self, duration: Duration) -> bool {
        if self.is_set() {
            return true;
        }

        let guard = self.mutex.lock().unwrap_or_else(|error| error.into_inner());
        let result = self
            .condvar
            .wait_timeout_while(guard, duration, |_| !self.is_set());
        match result {
            Ok((_, _)) => self.is_set(),
            Err(error) => {
                let _ = error.into_inner();
                self.is_set()
            }
        }
    }
}

struct WatchState {
    coalescer: EventCoalescer,
    first_transition_suppressed: bool,
}

impl WatchState {
    fn new(options: WatchOptions) -> Self {
        Self {
            coalescer: EventCoalescer::default(),
            first_transition_suppressed: !options.emit_initial,
        }
    }

    fn push(&mut self, snapshot: Option<Snapshot>) -> Option<Transition> {
        let transition = self.coalescer.push(snapshot)?;
        if self.first_transition_suppressed
            && transition.previous.is_none()
            && transition.current.is_some()
        {
            self.first_transition_suppressed = false;
            return None;
        }

        self.first_transition_suppressed = false;
        Some(transition)
    }
}

struct WatchStream {
    inner: UnboundedReceiverStream<Result<Transition, CaptureError>>,
    stop: Arc<StopSignal>,
    handle: Option<thread::JoinHandle<()>>,
}

impl Stream for WatchStream {
    type Item = Result<Transition, CaptureError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.inner).poll_next(cx)
    }
}

impl Drop for WatchStream {
    fn drop(&mut self) {
        self.stop.stop();
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
        time::{Duration, Instant, SystemTime},
    };

    use hypr_activity_capture_interface::{
        CaptureError, ContentLevel, Snapshot, SnapshotSource, TextAnchorConfidence, TextAnchorKind,
        WatchOptions,
    };
    use tokio_stream::StreamExt;

    use super::{WatchState, spawn_watch_stream_with};

    fn snapshot(title: &str) -> Snapshot {
        Snapshot {
            captured_at: SystemTime::UNIX_EPOCH + Duration::from_secs(10),
            pid: 42,
            app_name: "Codex".to_string(),
            bundle_id: Some("com.openai.codex".to_string()),
            window_title: Some(title.to_string()),
            url: None,
            visible_text: Some("hello".to_string()),
            text_anchor_kind: Some(TextAnchorKind::FocusedEdit),
            text_anchor_identity: Some("codex:editor".to_string()),
            text_anchor_text: Some("hello".to_string()),
            text_anchor_prefix: None,
            text_anchor_suffix: None,
            text_anchor_selected_text: None,
            text_anchor_confidence: Some(TextAnchorConfidence::High),
            content_level: ContentLevel::Full,
            source: SnapshotSource::Accessibility,
        }
    }

    fn watch_options(emit_initial: bool) -> WatchOptions {
        WatchOptions {
            poll_interval: Duration::from_secs(30),
            emit_initial,
        }
    }

    #[test]
    fn emits_initial_transition_when_requested() {
        let mut state = WatchState::new(watch_options(true));

        let transition = state.push(Some(snapshot("Notes"))).unwrap();

        assert!(transition.previous.is_none());
        assert_eq!(
            transition.current.unwrap().snapshot.window_title.as_deref(),
            Some("Notes")
        );
    }

    #[test]
    fn suppresses_initial_transition_when_requested() {
        let mut state = WatchState::new(watch_options(false));

        assert!(state.push(Some(snapshot("Notes"))).is_none());

        let transition = state.push(Some(snapshot("Docs"))).unwrap();
        assert_eq!(
            transition.current.unwrap().snapshot.window_title.as_deref(),
            Some("Docs")
        );
    }

    #[test]
    fn emits_switch_transition_when_snapshot_changes() {
        let mut state = WatchState::new(watch_options(true));
        let _ = state.push(Some(snapshot("Notes")));

        let transition = state.push(Some(snapshot("Docs"))).unwrap();

        assert_eq!(
            transition
                .previous
                .as_ref()
                .and_then(|event| event.snapshot.window_title.as_deref()),
            Some("Notes")
        );
        assert_eq!(
            transition
                .current
                .as_ref()
                .and_then(|event| event.snapshot.window_title.as_deref()),
            Some("Docs")
        );
    }

    #[test]
    fn suppresses_transitions_for_stable_fingerprints() {
        let mut state = WatchState::new(watch_options(true));
        let _ = state.push(Some(snapshot("Notes")));

        let mut same = snapshot("Notes");
        same.captured_at += Duration::from_secs(5);

        assert!(state.push(Some(same)).is_none());
    }

    #[test]
    fn dropping_stream_stops_thread_promptly() {
        let stream = spawn_watch_stream_with(
            || Ok(Some(snapshot("Notes"))),
            WatchOptions {
                poll_interval: Duration::from_secs(60),
                emit_initial: true,
            },
        )
        .unwrap();

        let start = Instant::now();
        drop(stream);

        assert!(start.elapsed() < Duration::from_secs(1));
    }

    #[test]
    fn polling_stream_emits_transitions() {
        let steps = Arc::new(AtomicUsize::new(0));
        let steps_ref = Arc::clone(&steps);
        let mut stream = spawn_watch_stream_with(
            move || {
                let current = steps_ref.fetch_add(1, Ordering::SeqCst);
                match current {
                    0 => Ok(Some(snapshot("Notes"))),
                    1 => Ok(Some(snapshot("Docs"))),
                    _ => Err(CaptureError::temporarily_unavailable("done")),
                }
            },
            WatchOptions {
                poll_interval: Duration::from_millis(1),
                emit_initial: true,
            },
        )
        .unwrap();

        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        let transitions = runtime.block_on(async {
            let first = stream.next().await.unwrap().unwrap();
            let second = stream.next().await.unwrap().unwrap();
            let third = stream.next().await.unwrap().unwrap_err();
            (first, second, third)
        });

        assert!(transitions.0.previous.is_none());
        assert_eq!(
            transitions
                .1
                .previous
                .as_ref()
                .and_then(|event| event.snapshot.window_title.as_deref()),
            Some("Notes")
        );
        assert_eq!(
            transitions.2.kind,
            CaptureError::temporarily_unavailable("done").kind
        );
    }
}
