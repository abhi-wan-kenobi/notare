use std::collections::HashMap;
use std::sync::{atomic::AtomicBool, Arc, RwLock};

#[derive(Default)]
pub struct TaskState {
    active_tasks: Arc<RwLock<HashMap<String, Arc<AtomicBool>>>>,
}

impl TaskState {
    pub fn register_task(&self, id: String, cancel_flag: Arc<AtomicBool>) {
        self.active_tasks.write().unwrap().insert(id, cancel_flag);
    }

    pub fn cancel_task(&self, id: &str) -> bool {
        if let Some(cancel_flag) = self.active_tasks.read().unwrap().get(id) {
            cancel_flag.store(true, std::sync::atomic::Ordering::Relaxed);
            true
        } else {
            false
        }
    }

    pub fn remove_task(&self, id: &str) {
        self.active_tasks.write().unwrap().remove(id);
    }
}
