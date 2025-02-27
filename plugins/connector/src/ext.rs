use std::future::Future;

#[derive(Debug, serde::Deserialize, serde::Serialize, specta::Type)]
pub enum ConnectionType {
    #[serde(rename = "auto-llm")]
    #[specta(rename = "auto-llm")]
    AutoLLM,
    #[serde(rename = "auto-stt")]
    #[specta(rename = "auto-stt")]
    AutoSTT,
}

pub trait ConnectorPluginExt<R: tauri::Runtime> {
    fn is_online(&self) -> impl Future<Output = bool>;
    fn get_api_base(&self, t: ConnectionType) -> impl Future<Output = Option<String>>;
}

impl<R: tauri::Runtime, T: tauri::Manager<R>> ConnectorPluginExt<R> for T {
    async fn is_online(&self) -> bool {
        let target = "8.8.8.8".to_string();
        let interval = std::time::Duration::from_secs(1);
        let options = pinger::PingOptions::new(target, interval, None);

        if let Ok(stream) = pinger::ping(options) {
            if let Some(message) = stream.into_iter().next() {
                match message {
                    pinger::PingResult::Pong(_, _) => return true,
                    _ => return false,
                }
            }
        }

        false
    }

    async fn get_api_base(&self, t: ConnectionType) -> Option<String> {
        match t {
            ConnectionType::AutoLLM => {
                let local_llm_state = self.state::<tauri_plugin_local_llm::SharedState>();
                let local_llm_state = local_llm_state.lock().await;

                match local_llm_state.api_base.clone() {
                    Some(api_base) => Some(api_base),
                    None => {
                        if cfg!(debug_assertions) {
                            Some("http://localhost:1234".to_string())
                        } else {
                            Some("https://app.hyprnote.com".to_string())
                        }
                    }
                }
            }
            ConnectionType::AutoSTT => {
                let local_stt_state = self.state::<tauri_plugin_local_stt::SharedState>();
                let local_stt_state = local_stt_state.lock().await;

                match local_stt_state.api_base.clone() {
                    Some(api_base) => Some(api_base),
                    None => {
                        if cfg!(debug_assertions) {
                            Some("http://localhost:1234".to_string())
                        } else {
                            Some("https://app.hyprnote.com".to_string())
                        }
                    }
                }
            }
        }
    }
}
