use std::{
    net::{Ipv4Addr, SocketAddr},
    path::PathBuf,
};

use axum::{Router, error_handling::HandleError};
use ractor::{Actor, ActorName, ActorProcessingErr, ActorRef, RpcReplyPort};
use reqwest::StatusCode;
use tower_http::cors::{self, CorsLayer};

use super::{ServerInfo, ServerStatus};
#[cfg(feature = "parakeet-onnx")]
use hypr_parakeet_onnx_model::ParakeetOnnxModel;
#[cfg(feature = "whisper-cpp")]
use hypr_whisper_local_model::WhisperModel;

pub enum InternalSTTMessage {
    GetHealth(RpcReplyPort<ServerInfo>),
    ServerError(String),
}

/// The models the in-process `/v1/listen` server can run. One internal
/// server exists at a time; each variant maps to a `TranscribeService<E>`
/// over the matching engine.
#[derive(Clone)]
pub enum InternalModel {
    #[cfg(feature = "whisper-cpp")]
    Whisper(WhisperModel),
    #[cfg(feature = "parakeet-onnx")]
    ParakeetOnnx(ParakeetOnnxModel),
}

impl InternalModel {
    fn local_model(&self) -> crate::LocalModel {
        match self {
            #[cfg(feature = "whisper-cpp")]
            InternalModel::Whisper(model) => crate::LocalModel::Whisper(model.clone()),
            #[cfg(feature = "parakeet-onnx")]
            InternalModel::ParakeetOnnx(model) => crate::LocalModel::ParakeetOnnx(model.clone()),
        }
    }

    /// Path handed to the engine's `TranscribeService`: the model *file*
    /// for whisper.cpp, the model *directory* for Parakeet ONNX.
    fn model_path(&self, model_cache_dir: &std::path::Path) -> PathBuf {
        match self {
            #[cfg(feature = "whisper-cpp")]
            InternalModel::Whisper(model) => model_cache_dir.join(model.file_name()),
            #[cfg(feature = "parakeet-onnx")]
            InternalModel::ParakeetOnnx(model) => model_cache_dir.join(model.model_dir()),
        }
    }
}

#[derive(Clone)]
pub struct InternalSTTArgs {
    pub model_type: InternalModel,
    pub model_cache_dir: PathBuf,
}

pub struct InternalSTTState {
    base_url: String,
    model: crate::LocalModel,
    shutdown: tokio::sync::watch::Sender<()>,
    server_task: tokio::task::JoinHandle<()>,
}

pub struct InternalSTTActor;

impl InternalSTTActor {
    pub fn name() -> ActorName {
        "internal_stt".into()
    }
}

#[ractor::async_trait]
impl Actor for InternalSTTActor {
    type Msg = InternalSTTMessage;
    type State = InternalSTTState;
    type Arguments = InternalSTTArgs;

    async fn pre_start(
        &self,
        myself: ActorRef<Self::Msg>,
        args: Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        let InternalSTTArgs {
            model_type,
            model_cache_dir,
        } = args;

        let model_path = model_type.model_path(&model_cache_dir);
        let on_error = move |err: String| async move {
            let _ = myself.send_message(InternalSTTMessage::ServerError(err.clone()));
            (StatusCode::INTERNAL_SERVER_ERROR, err)
        };

        // Each arm yields the same `Router` type, so the engines can differ.
        let router = match &model_type {
            #[cfg(feature = "whisper-cpp")]
            InternalModel::Whisper(_) => {
                let service = HandleError::new(
                    hypr_transcribe_whisper_local::TranscribeService::builder()
                        .model_path(model_path)
                        .build(),
                    on_error,
                );
                Router::new().route_service("/v1/listen", service)
            }
            #[cfg(feature = "parakeet-onnx")]
            InternalModel::ParakeetOnnx(_) => {
                let service = HandleError::new(
                    hypr_transcribe_core::TranscribeService::<
                        hypr_parakeet_onnx::LoadedParakeet,
                    >::builder()
                    .model_path(model_path)
                    .build(),
                    on_error,
                );
                Router::new().route_service("/v1/listen", service)
            }
        };

        let router = router.layer(
            CorsLayer::new()
                .allow_origin(cors::Any)
                .allow_methods(cors::Any)
                .allow_headers(cors::Any),
        );

        let listener =
            tokio::net::TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0))).await?;

        let server_addr = listener.local_addr()?;
        let base_url = format!("http://{}/v1", server_addr);

        let (shutdown_tx, mut shutdown_rx) = tokio::sync::watch::channel(());

        let server_task = tokio::spawn(async move {
            axum::serve(listener, router)
                .with_graceful_shutdown(async move {
                    shutdown_rx.changed().await.ok();
                })
                .await
                .unwrap();
        });

        Ok(InternalSTTState {
            base_url,
            model: model_type.local_model(),
            shutdown: shutdown_tx,
            server_task,
        })
    }

    async fn post_stop(
        &self,
        _myself: ActorRef<Self::Msg>,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        let _ = state.shutdown.send(());
        state.server_task.abort();
        Ok(())
    }

    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        message: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match message {
            InternalSTTMessage::ServerError(e) => Err(e.into()),
            InternalSTTMessage::GetHealth(reply_port) => {
                let info = ServerInfo {
                    url: Some(state.base_url.clone()),
                    status: ServerStatus::Ready,
                    model: Some(state.model.clone()),
                };

                if let Err(e) = reply_port.send(info) {
                    return Err(e.into());
                }

                Ok(())
            }
        }
    }
}
