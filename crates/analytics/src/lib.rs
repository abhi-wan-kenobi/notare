use std::collections::HashMap;

mod error;

pub use error::*;

#[derive(Clone)]
pub struct DeviceFingerprint(pub String);

#[derive(Clone)]
pub struct AuthenticatedUserId(pub String);

#[derive(Clone)]
pub struct AnalyticsClient;

#[derive(Default)]
pub struct AnalyticsClientBuilder {
    posthog_key: Option<String>,
    posthog_personal_key: Option<String>,
}

impl AnalyticsClientBuilder {
    pub fn with_posthog(mut self, key: impl Into<String>) -> Self {
        self.posthog_key = Some(key.into());
        self
    }

    pub fn with_local_evaluation(mut self, personal_api_key: impl Into<String>) -> Self {
        self.posthog_personal_key = Some(personal_api_key.into());
        self
    }

    pub fn build(self) -> AnalyticsClient {
        // Notare is telemetry-free: never construct a PostHog backend, no matter
        // what keys the build carries. Every send/flag path in this crate
        // no-ops (local tracing only).
        let _ = (self.posthog_key, self.posthog_personal_key);
        AnalyticsClient
    }
}

impl AnalyticsClient {
    pub async fn event(
        &self,
        distinct_id: impl Into<String>,
        payload: AnalyticsPayload,
    ) -> Result<(), Error> {
        let _ = distinct_id.into();
        tracing::info!("event: {:?}", payload);
        Ok(())
    }

    pub async fn set_properties(
        &self,
        distinct_id: impl Into<String>,
        payload: PropertiesPayload,
    ) -> Result<(), Error> {
        let _ = distinct_id.into();
        tracing::info!("set_properties: {:?}", payload);
        Ok(())
    }

    pub async fn is_feature_enabled(
        &self,
        flag_key: &str,
        distinct_id: &str,
    ) -> Result<bool, Error> {
        tracing::info!("is_feature_enabled: {} (no client)", flag_key);
        let _ = distinct_id;
        Ok(false)
    }

    pub async fn get_feature_flag(
        &self,
        flag_key: &str,
        distinct_id: &str,
        person_properties: Option<HashMap<String, serde_json::Value>>,
        group_properties: Option<HashMap<String, HashMap<String, serde_json::Value>>>,
    ) -> Result<Option<FlagValue>, Error> {
        tracing::info!("get_feature_flag: {} (no client)", flag_key);
        let _ = (distinct_id, person_properties, group_properties);
        Ok(None)
    }

    pub async fn get_feature_flag_payload(
        &self,
        flag_key: &str,
        distinct_id: &str,
    ) -> Result<Option<serde_json::Value>, Error> {
        tracing::info!("get_feature_flag_payload: {} (no client)", flag_key);
        let _ = distinct_id;
        Ok(None)
    }

    pub async fn identify(
        &self,
        user_id: impl Into<String>,
        anon_distinct_id: impl Into<String>,
        payload: PropertiesPayload,
    ) -> Result<(), Error> {
        let user_id = user_id.into();
        let anon_distinct_id = anon_distinct_id.into();
        tracing::info!(
            "identify: user_id={}, anon_distinct_id={}, payload={:?}",
            user_id,
            anon_distinct_id,
            payload
        );
        Ok(())
    }
}

pub trait ToAnalyticsPayload {
    fn to_analytics_payload(&self) -> AnalyticsPayload;

    fn to_analytics_properties(&self) -> Option<PropertiesPayload> {
        None
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct AnalyticsPayload {
    pub event: String,
    #[serde(flatten)]
    pub props: HashMap<String, serde_json::Value>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct PropertiesPayload {
    #[serde(default)]
    pub set: HashMap<String, serde_json::Value>,
    #[serde(default)]
    pub set_once: HashMap<String, serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
}

#[derive(Default)]
pub struct PropertiesPayloadBuilder {
    set: HashMap<String, serde_json::Value>,
    set_once: HashMap<String, serde_json::Value>,
}

impl PropertiesPayload {
    pub fn builder() -> PropertiesPayloadBuilder {
        PropertiesPayloadBuilder::default()
    }
}

impl PropertiesPayloadBuilder {
    pub fn set(mut self, key: impl Into<String>, value: impl Into<serde_json::Value>) -> Self {
        self.set.insert(key.into(), value.into());
        self
    }

    pub fn set_once(mut self, key: impl Into<String>, value: impl Into<serde_json::Value>) -> Self {
        self.set_once.insert(key.into(), value.into());
        self
    }

    pub fn build(self) -> PropertiesPayload {
        PropertiesPayload {
            set: self.set,
            set_once: self.set_once,
            email: None,
            user_id: None,
        }
    }
}

#[derive(Clone)]
pub struct AnalyticsPayloadBuilder {
    event: Option<String>,
    props: HashMap<String, serde_json::Value>,
}

impl AnalyticsPayload {
    pub fn builder(event: impl Into<String>) -> AnalyticsPayloadBuilder {
        AnalyticsPayloadBuilder {
            event: Some(event.into()),
            props: HashMap::new(),
        }
    }
}

impl AnalyticsPayloadBuilder {
    pub fn with(mut self, key: impl Into<String>, value: impl Into<serde_json::Value>) -> Self {
        self.props.insert(key.into(), value.into());
        self
    }

    pub fn build(self) -> AnalyticsPayload {
        if self.event.is_none() {
            panic!("'Event' is not specified");
        }

        AnalyticsPayload {
            event: self.event.unwrap(),
            props: self.props,
        }
    }
}

// `FlagValue` was previously re-exported from `posthog_rs`. Drop the dep and
// keep the public name on the crate so downstream callers keep compiling.
pub type FlagValue = ();

#[cfg(test)]
mod tests {
    use super::*;

    #[ignore]
    #[tokio::test]
    async fn test_analytics() {
        let client = AnalyticsClientBuilder::default().build();
        let payload = AnalyticsPayload::builder("test_event")
            .with("key1", "value1")
            .with("key2", 2)
            .build();

        client.event("machine_id_123", payload).await.unwrap();
    }
}
