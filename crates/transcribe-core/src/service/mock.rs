//! Test-only engine used to exercise the generic service plumbing without a
//! real model on disk.

use std::path::Path;

use crate::engine::{EngineError, EngineSegment, SttEngine, SttEngineSession};

pub(crate) struct MockEngine;

#[derive(Debug, thiserror::Error)]
#[error("mock engine load error")]
pub(crate) struct MockLoadError;

impl hypr_model_manager::ModelLoader for MockEngine {
    type Error = MockLoadError;

    fn load(_path: &Path) -> Result<Self, Self::Error> {
        Ok(MockEngine)
    }
}

pub(crate) struct MockSession;

impl SttEngineSession for MockSession {
    fn transcribe(&mut self, samples: &[f32]) -> Result<Vec<EngineSegment>, EngineError> {
        Ok(vec![EngineSegment {
            text: "mock".to_string(),
            start: 0.0,
            end: samples.len() as f64 / crate::TARGET_SAMPLE_RATE as f64,
            confidence: 1.0,
            language: None,
        }])
    }
}

impl SttEngine for MockEngine {
    type Session = MockSession;

    fn session(
        &self,
        _languages: Vec<hypr_language::Language>,
    ) -> Result<Self::Session, EngineError> {
        Ok(MockSession)
    }

    fn arch() -> &'static str {
        "mock-engine"
    }
}
