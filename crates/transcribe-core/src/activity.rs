//! Live server-activity registry for the `/dashboard` + `/api/sessions` views.
//!
//! The STT server is a single process that serves **one** live WS transcription
//! session at a time (`hypr_ws_utils::ConnectionManager` cancels any prior one),
//! so a process-global registry is the pragmatic home for this state — it avoids
//! threading shared mutable state through the engine-generic streaming builder.
//! The streaming loop reports lifecycle (`begin`/`progress`/`end`); the axum
//! `/api/sessions` handler reads a `snapshot()`.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, OnceLock};

const RECENT_CAP: usize = 20;
const SAMPLE_CAP: usize = 240; // ~throughput history for the graph

pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[derive(Clone, serde::Serialize)]
pub struct LiveSession {
    pub id: String,
    pub model: String,
    pub started_at_ms: u64,
    pub last_activity_ms: u64,
    /// Furthest audio position (seconds) the engine has produced output for.
    pub audio_secs_processed: f64,
}

#[derive(Clone, serde::Serialize)]
pub struct DoneSession {
    pub id: String,
    pub model: String,
    pub started_at_ms: u64,
    pub ended_at_ms: u64,
    pub audio_secs_processed: f64,
    /// "completed" | "error" | "stalled" | "cancelled" | "closed"
    pub outcome: String,
}

/// A (timestamp_ms, audio_secs_processed) point for the throughput graph.
#[derive(Clone, serde::Serialize)]
pub struct Sample {
    pub t_ms: u64,
    pub audio_secs: f64,
}

#[derive(Clone, serde::Serialize)]
pub struct Snapshot {
    pub now_ms: u64,
    pub current: Option<LiveSession>,
    pub samples: Vec<Sample>,
    pub recent: Vec<DoneSession>,
}

#[derive(Default)]
struct Inner {
    current: Option<LiveSession>,
    samples: VecDeque<Sample>,
    recent: VecDeque<DoneSession>,
}

#[derive(Clone, Default)]
pub struct SessionRegistry(Arc<Mutex<Inner>>);

impl SessionRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// A new session started — becomes the current one, resets the graph.
    pub fn begin(&self, id: impl Into<String>, model: impl Into<String>) {
        let now = now_ms();
        let mut g = self.lock();
        g.current = Some(LiveSession {
            id: id.into(),
            model: model.into(),
            started_at_ms: now,
            last_activity_ms: now,
            audio_secs_processed: 0.0,
        });
        g.samples.clear();
        g.samples.push_back(Sample {
            t_ms: now,
            audio_secs: 0.0,
        });
    }

    /// Report forward progress: the engine has produced output up to
    /// `audio_secs_processed`. Refreshes last-activity and appends a graph point.
    pub fn progress(&self, audio_secs_processed: f64) {
        let now = now_ms();
        let mut g = self.lock();
        if let Some(cur) = g.current.as_mut() {
            cur.last_activity_ms = now;
            if audio_secs_processed > cur.audio_secs_processed {
                cur.audio_secs_processed = audio_secs_processed;
            }
            let audio = cur.audio_secs_processed;
            g.samples.push_back(Sample { t_ms: now, audio_secs: audio });
            while g.samples.len() > SAMPLE_CAP {
                g.samples.pop_front();
            }
        }
    }

    /// The session ended; record it in recent history with an outcome.
    pub fn end(&self, outcome: impl Into<String>) {
        let now = now_ms();
        let mut g = self.lock();
        if let Some(cur) = g.current.take() {
            g.recent.push_front(DoneSession {
                id: cur.id,
                model: cur.model,
                started_at_ms: cur.started_at_ms,
                ended_at_ms: now,
                audio_secs_processed: cur.audio_secs_processed,
                outcome: outcome.into(),
            });
            while g.recent.len() > RECENT_CAP {
                g.recent.pop_back();
            }
        }
    }

    pub fn snapshot(&self) -> Snapshot {
        let g = self.lock();
        Snapshot {
            now_ms: now_ms(),
            current: g.current.clone(),
            samples: g.samples.iter().cloned().collect(),
            recent: g.recent.iter().cloned().collect(),
        }
    }
}

static REGISTRY: OnceLock<SessionRegistry> = OnceLock::new();

/// Process-global activity registry (see module docs for why global is fine).
pub fn registry() -> &'static SessionRegistry {
    REGISTRY.get_or_init(SessionRegistry::new)
}

/// RAII guard: `begin`s a session now and `end`s it when dropped, so the session
/// is recorded as finished no matter which of the streaming loop's many break
/// paths fires.
pub struct ActivityGuard;

impl Drop for ActivityGuard {
    fn drop(&mut self) {
        registry().end("ended");
    }
}

pub fn begin_guarded(id: impl Into<String>, model: impl Into<String>) -> ActivityGuard {
    registry().begin(id, model);
    ActivityGuard
}
