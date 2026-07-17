use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use model_downloader::{
    DownloadPart, DownloadStatus, DownloadableModel, Error, ModelDownloadManager,
    ModelDownloaderRuntime, ModelIntegrity, verify_model,
};

// --- test fixtures ---

const PART_A: &[u8] = b"encoder-bytes-encoder-bytes";
const PART_B: &[u8] = b"vocab";

struct TestRuntime {
    temp_dir: Arc<tempfile::TempDir>,
    progress_log: Arc<Mutex<Vec<(String, DownloadStatus)>>>,
}

impl TestRuntime {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            temp_dir: Arc::new(tempfile::TempDir::new().unwrap()),
            progress_log: Arc::new(Mutex::new(Vec::new())),
        })
    }

    fn progress_statuses(&self) -> Vec<DownloadStatus> {
        self.progress_log
            .lock()
            .unwrap()
            .iter()
            .map(|(_, s)| s.clone())
            .collect()
    }
}

impl ModelDownloaderRuntime<MultiTestModel> for TestRuntime {
    fn models_base(&self) -> Result<PathBuf, Error> {
        Ok(self.temp_dir.path().to_path_buf())
    }

    fn emit_progress(&self, model: &MultiTestModel, status: DownloadStatus) {
        self.progress_log
            .lock()
            .unwrap()
            .push((model.download_key(), status));
    }
}

#[derive(Clone)]
struct MultiTestModel {
    key: String,
    parts: Vec<DownloadPart>,
}

impl MultiTestModel {
    fn new(key: &str, base_url: &str, corrupt_second_checksum: bool) -> Self {
        let checksum_b = if corrupt_second_checksum {
            crc32fast::hash(PART_B) ^ 0xdead_beef
        } else {
            crc32fast::hash(PART_B)
        };

        Self {
            key: key.to_string(),
            parts: vec![
                DownloadPart {
                    url: format!("{base_url}/encoder.onnx"),
                    relative_path: "encoder.onnx".to_string(),
                    checksum: Some(crc32fast::hash(PART_A)),
                    expected_size: Some(PART_A.len() as u64),
                },
                DownloadPart {
                    url: format!("{base_url}/vocab.txt"),
                    relative_path: "vocab.txt".to_string(),
                    checksum: Some(checksum_b),
                    expected_size: Some(PART_B.len() as u64),
                },
            ],
        }
    }
}

impl DownloadableModel for MultiTestModel {
    fn download_key(&self) -> String {
        self.key.clone()
    }

    fn download_url(&self) -> Option<String> {
        None
    }

    fn download_parts(&self) -> Option<Vec<DownloadPart>> {
        Some(self.parts.clone())
    }

    fn download_destination(&self, models_base: &Path) -> PathBuf {
        models_base.join(&self.key)
    }

    fn is_downloaded(&self, models_base: &Path) -> Result<bool, Error> {
        let dir = self.download_destination(models_base);
        Ok(self.parts.iter().all(|part| {
            let file = dir.join(&part.relative_path);
            match (part.expected_size, std::fs::metadata(&file)) {
                (Some(expected), Ok(meta)) => meta.is_file() && meta.len() == expected,
                (None, Ok(meta)) => meta.is_file(),
                (_, Err(_)) => false,
            }
        }))
    }

    fn finalize_download(&self, _downloaded_path: &Path, _models_base: &Path) -> Result<(), Error> {
        Ok(())
    }

    fn delete_downloaded(&self, models_base: &Path) -> Result<(), Error> {
        let dir = self.download_destination(models_base);
        if dir.exists() {
            std::fs::remove_dir_all(&dir).map_err(|e| Error::DeleteFailed(e.to_string()))?;
        }
        Ok(())
    }
}

// --- helpers ---

async fn mount_part(server: &MockServer, route: &str, body: &[u8], delay: Option<Duration>) {
    let len = body.len().to_string();

    Mock::given(method("HEAD"))
        .and(path(route))
        .respond_with(ResponseTemplate::new(200).insert_header("content-length", len.as_str()))
        .mount(server)
        .await;

    let mut response = ResponseTemplate::new(200)
        .set_body_bytes(body.to_vec())
        .insert_header("content-length", len.as_str());
    if let Some(delay) = delay {
        response = response.set_delay(delay);
    }

    Mock::given(method("GET"))
        .and(path(route))
        .respond_with(response)
        .mount(server)
        .await;
}

async fn wait_until_done(manager: &ModelDownloadManager<MultiTestModel>, model: &MultiTestModel) {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if !manager.is_downloading(model).await {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("download did not complete within 10s");
}

fn part_files_in(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        if let Ok(entries) = std::fs::read_dir(&current) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else if path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .is_some_and(|s| s.contains(".part-"))
                {
                    out.push(path);
                }
            }
        }
    }
    out
}

// --- tests ---

#[tokio::test]
async fn multi_part_happy_path_downloads_all_parts_and_verifies() {
    let server = MockServer::start().await;
    mount_part(&server, "/encoder.onnx", PART_A, None).await;
    mount_part(&server, "/vocab.txt", PART_B, None).await;

    let runtime = TestRuntime::new();
    let manager = ModelDownloadManager::new(runtime.clone());
    let model = MultiTestModel::new("multi_ok", &server.uri(), false);

    manager.download(&model).await.unwrap();
    wait_until_done(&manager, &model).await;

    assert!(manager.is_downloaded(&model).await.unwrap());

    let dir = runtime.temp_dir.path().join("multi_ok");
    assert_eq!(std::fs::read(dir.join("encoder.onnx")).unwrap(), PART_A);
    assert_eq!(std::fs::read(dir.join("vocab.txt")).unwrap(), PART_B);
    assert!(
        part_files_in(runtime.temp_dir.path()).is_empty(),
        "should not leave .part-* files behind"
    );

    let events = runtime.progress_statuses();
    assert!(
        events.contains(&DownloadStatus::Downloading(0)),
        "should emit Downloading(0): {events:?}"
    );
    assert!(
        events.contains(&DownloadStatus::Completed),
        "should emit Completed: {events:?}"
    );

    // integrity: every part checksummed -> Verified, with per-file stamps
    assert_eq!(
        manager.verify_integrity(&model).await.unwrap(),
        ModelIntegrity::Verified
    );
    assert!(dir.join("encoder.onnx.verified").is_file());
    assert!(dir.join("vocab.txt.verified").is_file());
}

#[tokio::test]
async fn multi_part_checksum_failure_fails_and_cleans_up() {
    let server = MockServer::start().await;
    mount_part(&server, "/encoder.onnx", PART_A, None).await;
    mount_part(&server, "/vocab.txt", PART_B, None).await;

    let runtime = TestRuntime::new();
    let manager = ModelDownloadManager::new(runtime.clone());
    // second part's catalog checksum is wrong -> per-part CRC verify must fail
    let model = MultiTestModel::new("multi_bad_crc", &server.uri(), true);

    manager.download(&model).await.unwrap();
    wait_until_done(&manager, &model).await;

    assert!(!manager.is_downloaded(&model).await.unwrap());
    assert!(
        runtime
            .progress_statuses()
            .iter()
            .any(|s| matches!(s, DownloadStatus::Failed(_))),
        "should emit Failed on per-part checksum mismatch"
    );
    assert!(
        part_files_in(runtime.temp_dir.path()).is_empty(),
        "should not leave .part-* files behind"
    );
    // The first (good) part must not have been promoted into place either.
    let dir = runtime.temp_dir.path().join("multi_bad_crc");
    assert!(!dir.join("encoder.onnx").exists());
    assert!(!dir.join("vocab.txt").exists());
}

#[tokio::test]
async fn multi_part_cancellation_mid_download_cleans_up() {
    let server = MockServer::start().await;
    // First part is slow so we can cancel while it is in flight.
    mount_part(
        &server,
        "/encoder.onnx",
        PART_A,
        Some(Duration::from_millis(500)),
    )
    .await;
    mount_part(&server, "/vocab.txt", PART_B, None).await;

    let runtime = TestRuntime::new();
    let manager = ModelDownloadManager::new(runtime.clone());
    let model = MultiTestModel::new("multi_cancel", &server.uri(), false);

    manager.download(&model).await.unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(manager.is_downloading(&model).await);

    let cancelled = manager.cancel_download(&model).await.unwrap();

    assert!(cancelled);
    assert!(!manager.is_downloading(&model).await);
    assert!(!manager.is_downloaded(&model).await.unwrap());
    assert!(
        runtime
            .progress_statuses()
            .iter()
            .any(|s| matches!(s, DownloadStatus::Failed(_))),
        "should emit Failed on cancellation"
    );
    assert!(
        part_files_in(runtime.temp_dir.path()).is_empty(),
        "should not leave .part-* files behind"
    );
}

#[tokio::test]
async fn multi_part_verify_detects_swapped_content() {
    let server = MockServer::start().await;
    mount_part(&server, "/encoder.onnx", PART_A, None).await;
    mount_part(&server, "/vocab.txt", PART_B, None).await;

    let runtime = TestRuntime::new();
    let manager = ModelDownloadManager::new(runtime.clone());
    let model = MultiTestModel::new("multi_swap", &server.uri(), false);

    manager.download(&model).await.unwrap();
    wait_until_done(&manager, &model).await;
    assert!(manager.is_downloaded(&model).await.unwrap());

    // Corrupt one part in place (same size, different bytes, fresh mtime).
    let dir = runtime.temp_dir.path().join("multi_swap");
    let mut wrong = PART_B.to_vec();
    wrong[0] ^= 0xff;
    std::fs::write(dir.join("vocab.txt"), &wrong).unwrap();
    let file = std::fs::OpenOptions::new()
        .append(true)
        .open(dir.join("vocab.txt"))
        .unwrap();
    file.set_modified(std::time::SystemTime::now() + Duration::from_secs(10))
        .unwrap();

    match verify_model(&model, runtime.temp_dir.path()).unwrap() {
        ModelIntegrity::Corrupt(reason) => {
            assert!(reason.contains("vocab.txt"), "reason: {reason}")
        }
        other => panic!("expected Corrupt, got {other:?}"),
    }
}

#[tokio::test]
async fn multi_part_missing_file_is_not_installed() {
    let runtime = TestRuntime::new();
    let model = MultiTestModel::new("multi_missing", "http://localhost:1", false);

    assert_eq!(
        verify_model(&model, runtime.temp_dir.path()).unwrap(),
        ModelIntegrity::NotInstalled
    );
}
