pub struct Agent<'a, R: tauri::Runtime, M: tauri::Manager<R>> {
    #[allow(dead_code)]
    manager: &'a M,
    _runtime: std::marker::PhantomData<fn() -> R>,
}

impl<R: tauri::Runtime, M: tauri::Manager<R>> Agent<'_, R, M> {
    pub fn ping(&self, payload: hypr_agent_core::PingRequest) -> hypr_agent_core::PingResponse {
        hypr_agent_core::ping(payload)
    }
}

pub trait AgentPluginExt<R: tauri::Runtime> {
    fn agent(&self) -> Agent<'_, R, Self>
    where
        Self: tauri::Manager<R> + Sized;
}

impl<R: tauri::Runtime, T: tauri::Manager<R>> AgentPluginExt<R> for T {
    fn agent(&self) -> Agent<'_, R, Self>
    where
        Self: Sized,
    {
        Agent {
            manager: self,
            _runtime: std::marker::PhantomData,
        }
    }
}
