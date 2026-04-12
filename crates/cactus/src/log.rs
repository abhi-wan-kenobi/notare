use crate::health;

pub fn init() {
    health::init_runtime();
}

pub fn drain_errors() -> Vec<String> {
    health::latest_errors_snapshot()
}
