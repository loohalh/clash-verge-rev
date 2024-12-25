use crate::utils::help::format_bytes_speed;
use parking_lot::RwLock;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct SpeedRate {
    pub up_text: Arc<RwLock<Option<String>>>,
    pub down_text: Arc<RwLock<Option<String>>>,
}

impl SpeedRate {
    pub fn new() -> Self {
        Self {
            up_text: Arc::new(RwLock::new(None)),
            down_text: Arc::new(RwLock::new(None)),
        }
    }

    pub fn update_traffic(&self, up: u64, down: u64) {
        *self.up_text.write() = Some(format_bytes_speed(up));
        *self.down_text.write() = Some(format_bytes_speed(down));
    }
}

#[derive(Debug, Clone)]
pub struct Traffic {
    pub up: u64,
    pub down: u64,
}
