mod common;

use std::{
    net::SocketAddr,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use common::{TEST_TIMEOUT, TestIO, test_client, test_message};
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio_stream::wrappers::ReceiverStream;
use tokio_tungstenite::{accept_async, tungstenite::protocol::Message};
use ws_client::client::{WebSocketConnectPolicy, WebSocketReconnectPolicy};

/// Fast, deterministic reconnect policy for tests (no multi-second waits).
fn fast_reconnect_policy() -> WebSocketReconnectPolicy {
    WebSocketReconnectPolicy {
        max_cycles: 2,
        connect: WebSocketConnectPolicy {
            connect_timeout: Duration::from_millis(200),
            max_attempts: 1,
            retry_delay: Duration::from_millis(50),
        },
    }
}

/// First accepted connection echoes exactly one message then drops the socket
/// ungracefully (no close frame); every later connection is a normal echo loop.
/// `accepts` counts how many connections the client established.
async fn drop_once_then_echo_server(accepts: Arc<AtomicUsize>) -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            let n = accepts.fetch_add(1, Ordering::SeqCst);
            tokio::spawn(async move {
                let ws = accept_async(stream).await.unwrap();
                let (mut tx, mut rx) = ws.split();
                if n == 0 {
                    // Serve one echo, then drop the socket mid-session.
                    if let Some(Ok(msg @ (Message::Text(_) | Message::Binary(_)))) = rx.next().await
                    {
                        let _ = tx.send(msg).await;
                    }
                    // Dropping tx/rx here severs the TCP connection without a
                    // WebSocket close handshake -> a mid-stream transport error.
                } else {
                    while let Some(Ok(msg)) = rx.next().await {
                        match msg {
                            Message::Text(_) | Message::Binary(_) => {
                                if tx.send(msg).await.is_err() {
                                    break;
                                }
                            }
                            Message::Close(_) => break,
                            _ => {}
                        }
                    }
                }
            });
        }
    });

    addr
}

/// Accepts exactly one connection (echo one message, then drop), then stops
/// listening entirely so every reconnect attempt is refused.
async fn drop_then_refuse_server(accepts: Arc<AtomicUsize>) -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        if let Ok((stream, _)) = listener.accept().await {
            accepts.fetch_add(1, Ordering::SeqCst);
            let ws = accept_async(stream).await.unwrap();
            let (mut tx, mut rx) = ws.split();
            if let Some(Ok(msg @ (Message::Text(_) | Message::Binary(_)))) = rx.next().await {
                let _ = tx.send(msg).await;
            }
            // Fall out of scope -> socket dropped AND `listener` dropped, so the
            // port stops accepting: subsequent reconnects get connection-refused.
        }
    });

    addr
}

/// Echo loop that drops the socket ungracefully as soon as it receives its
/// first text frame (used to stand in for "the transport dies right after the
/// client sent its Finalize control message").
async fn drop_on_first_text_server(accepts: Arc<AtomicUsize>) -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            accepts.fetch_add(1, Ordering::SeqCst);
            tokio::spawn(async move {
                let ws = accept_async(stream).await.unwrap();
                let (_tx, mut rx) = ws.split();
                while let Some(Ok(msg)) = rx.next().await {
                    if matches!(msg, Message::Text(_)) {
                        break; // drop the socket
                    }
                }
            });
        }
    });

    addr
}

async fn wait_for_accepts(accepts: &Arc<AtomicUsize>, target: usize) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
    while accepts.load(Ordering::SeqCst) < target {
        if tokio::time::Instant::now() > deadline {
            panic!(
                "server never reached {target} accepts (saw {})",
                accepts.load(Ordering::SeqCst)
            );
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

#[tokio::test]
async fn reconnects_and_resumes_after_mid_stream_drop() {
    let accepts = Arc::new(AtomicUsize::new(0));
    let addr = drop_once_then_echo_server(accepts.clone()).await;

    let client = test_client(addr).with_reconnect(fast_reconnect_policy());

    // A caller-driven audio stream we can feed on demand and keep open.
    let (audio_tx, audio_rx) = tokio::sync::mpsc::channel::<common::TestMessage>(8);
    let (output, _handle) = client
        .from_audio::<TestIO, _>(None, ReceiverStream::new(audio_rx))
        .await
        .unwrap();
    futures_util::pin_mut!(output);

    // First message is echoed by connection #0, which then drops.
    audio_tx.send(test_message("a", 1)).await.unwrap();
    let first = tokio::time::timeout(TEST_TIMEOUT, output.next())
        .await
        .expect("first echo should arrive")
        .expect("stream open")
        .expect("first echo ok");
    assert_eq!(first, test_message("a", 1));

    // The drop triggers a reconnect; wait until connection #1 is established so
    // the next message deterministically lands on the resumed session.
    wait_for_accepts(&accepts, 2).await;

    audio_tx.send(test_message("b", 2)).await.unwrap();
    let second = tokio::time::timeout(Duration::from_secs(2), output.next())
        .await
        .expect("post-reconnect echo should arrive")
        .expect("stream still open after reconnect")
        .expect("post-reconnect echo ok");
    assert_eq!(
        second,
        test_message("b", 2),
        "streaming must resume on the reconnected session"
    );
    assert_eq!(accepts.load(Ordering::SeqCst), 2, "exactly one reconnect");
}

#[tokio::test]
async fn exhausts_reconnects_then_surfaces_terminal_error() {
    let accepts = Arc::new(AtomicUsize::new(0));
    let addr = drop_then_refuse_server(accepts.clone()).await;

    let client = test_client(addr).with_reconnect(fast_reconnect_policy());

    let (audio_tx, audio_rx) = tokio::sync::mpsc::channel::<common::TestMessage>(8);
    let (output, _handle) = client
        .from_audio::<TestIO, _>(None, ReceiverStream::new(audio_rx))
        .await
        .unwrap();
    futures_util::pin_mut!(output);

    audio_tx.send(test_message("a", 1)).await.unwrap();
    let first = tokio::time::timeout(TEST_TIMEOUT, output.next())
        .await
        .expect("first echo should arrive")
        .expect("stream open")
        .expect("first echo ok");
    assert_eq!(first, test_message("a", 1));

    // Server is gone; the reconnect can't connect, so the terminal transport
    // error must surface just as it would with reconnect disabled.
    let terminal = tokio::time::timeout(Duration::from_secs(3), output.next())
        .await
        .expect("terminal error should surface within bounded reconnect attempts")
        .expect("stream should yield a terminal item");
    assert!(
        terminal.is_err(),
        "expected a terminal transport error, got {terminal:?}"
    );
}

#[tokio::test]
async fn dropped_handle_does_not_starve_output() {
    // Regression: with reconnect enabled the supervisor also owns the control
    // channel; a dropped caller handle must not busy-loop that arm and starve
    // output draining. Drop the handle, then confirm echoes still flow.
    let addr = common::echo_server().await;
    let client = test_client(addr).with_reconnect(fast_reconnect_policy());

    let (audio_tx, audio_rx) = tokio::sync::mpsc::channel::<common::TestMessage>(8);
    let (output, handle) = client
        .from_audio::<TestIO, _>(None, ReceiverStream::new(audio_rx))
        .await
        .unwrap();
    futures_util::pin_mut!(output);

    drop(handle);

    audio_tx.send(test_message("still-flowing", 7)).await.unwrap();
    let echo = tokio::time::timeout(Duration::from_secs(2), output.next())
        .await
        .expect("echo should still arrive after the handle is dropped")
        .expect("stream open")
        .expect("echo ok");
    assert_eq!(echo, test_message("still-flowing", 7));
}

#[tokio::test]
async fn finalize_path_never_reconnects() {
    let accepts = Arc::new(AtomicUsize::new(0));
    let addr = drop_on_first_text_server(accepts.clone()).await;

    let client = test_client(addr).with_reconnect(fast_reconnect_policy());

    // Audio stays open (pending) for the whole test, so it is specifically the
    // finalizing state - not an audio EOF - that must suppress the reconnect.
    let (output, handle) = client
        .from_audio::<TestIO, _>(None, futures_util::stream::pending::<common::TestMessage>())
        .await
        .unwrap();
    futures_util::pin_mut!(output);

    // Finalize -> the server drops the socket ungracefully on receiving it.
    handle
        .finalize_with_text(
            serde_json::to_string(&test_message("bye", 0))
                .unwrap()
                .into(),
        )
        .await;

    // The stream terminates (drained here) without ever reconnecting.
    let _ = tokio::time::timeout(Duration::from_secs(3), async {
        while let Some(item) = output.next().await {
            let _ = item;
        }
    })
    .await;

    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(
        accepts.load(Ordering::SeqCst),
        1,
        "a drop during finalize must not trigger a reconnect"
    );
}
