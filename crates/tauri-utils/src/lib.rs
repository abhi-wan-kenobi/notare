use std::future::Future;

/// Run an async block synchronously from Tauri's synchronous plugin
/// `.setup()` closure.
///
/// Setup runs on a thread that is already inside a Tokio runtime, so calling
/// `block_on` directly (or via `tauri::async_runtime::block_on`) panics with
/// *"Cannot start a runtime from within a runtime"*.
///
/// Inside a multi-thread runtime (the app's), the future runs on that runtime
/// via `block_in_place`. It must not run on a throwaway runtime there: sqlx
/// returns a released pooled connection to its pool on a task it spawns onto
/// the current runtime, and dropping that runtime right after the future
/// returns cancels the task. The connection then closes raw, and with the
/// cr-sqlite engine loaded that `sqlite3_close` panics the sqlx worker
/// ("unable to close due to unfinalized statements"). Anywhere else (no
/// runtime, or a current-thread one, where `block_in_place` panics) a
/// dedicated thread with its own current-thread runtime runs it.
///
/// Two patterns are common:
///
/// | Need | Function | Behaviour |
/// |------|----------|-----------|
/// | Result **must** be ready before setup returns (e.g. DB migrations) | [`block_on`] | Blocks the calling thread until the future completes |
/// | Work can finish in the background (e.g. legacy data import) | [`spawn`] | Fire-and-forget; errors are logged, never block setup |
pub fn block_on<F, T>(future: F) -> T
where
    F: Future<Output = T> + Send,
    T: Send,
{
    if let Ok(handle) = tokio::runtime::Handle::try_current()
        && handle.runtime_flavor() == tokio::runtime::RuntimeFlavor::MultiThread
    {
        return tokio::task::block_in_place(|| handle.block_on(future));
    }

    std::thread::scope(|s| {
        s.spawn(|| {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("failed to create tokio runtime");
            runtime.block_on(future)
        })
        .join()
        .expect("spawned thread panicked")
    })
}

/// Spawn an async block as a fire-and-forget background task.
///
/// Uses `tauri::async_runtime::spawn` under the hood, which schedules the
/// future onto the existing Tauri Tokio runtime — no extra threads or runtimes
/// are created. Errors are logged via `tracing::error!` and do not propagate.
///
/// Use this for non-critical initialization work that should not block app
/// startup (e.g. importing legacy data, warming caches).
pub fn spawn<F, E>(label: &'static str, future: F)
where
    F: Future<Output = Result<(), E>> + Send + 'static,
    E: std::fmt::Display + Send + 'static,
{
    tokio::spawn(async move {
        if let Err(error) = future.await {
            tracing::error!("{label}: {error}");
        }
    });
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    // sqlx hands a released pooled connection back on a task it spawns while
    // the future runs; if that task is cancelled the connection closes raw.
    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn tasks_spawned_inside_outlive_the_call() {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let gate = Arc::new(tokio::sync::Notify::new());
        let task_gate = Arc::clone(&gate);

        super::block_on(async move {
            tokio::spawn(async move {
                task_gate.notified().await;
                let _ = tx.send(());
            });
        });

        gate.notify_one();
        rx.await
            .expect("a task spawned inside block_on was cancelled when it returned");
    }

    #[test]
    fn runs_outside_any_runtime() {
        assert_eq!(super::block_on(async { 7 }), 7);
    }
}
