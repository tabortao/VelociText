use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;

/// 进度追踪器 - 用于报告转录进度
pub struct ProgressTracker {
    percent: Arc<AtomicU32>,
    cancelled: Arc<AtomicBool>,
}

impl ProgressTracker {
    pub fn new() -> Self {
        Self {
            percent: Arc::new(AtomicU32::new(0)),
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn progress(&self) -> u32 {
        self.percent.load(Ordering::SeqCst)
    }

    pub fn set_progress(&self, percent: u32) {
        self.percent.store(percent.min(100), Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }
}

impl Default for ProgressTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for ProgressTracker {
    fn clone(&self) -> Self {
        Self {
            percent: Arc::clone(&self.percent),
            cancelled: Arc::clone(&self.cancelled),
        }
    }
}