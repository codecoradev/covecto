use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Clone)]
pub struct AppState {
    inner: Arc<AppStateInner>,
}

#[derive(Debug)]
struct AppStateInner {
    request_count: AtomicU64,
    total_processing_ms: AtomicU64,
}

pub fn create_app_state() -> AppState {
    AppState {
        inner: Arc::new(AppStateInner {
            request_count: AtomicU64::new(0),
            total_processing_ms: AtomicU64::new(0),
        }),
    }
}

impl AppState {
    pub fn record_request(&self, processing_ms: u64) {
        self.inner.request_count.fetch_add(1, Ordering::Relaxed);
        self.inner
            .total_processing_ms
            .fetch_add(processing_ms, Ordering::Relaxed);
    }

    pub fn request_count(&self) -> u64 {
        self.inner.request_count.load(Ordering::Relaxed)
    }

    pub fn avg_processing_ms(&self) -> f64 {
        let count = self.request_count();
        if count == 0 {
            0.0
        } else {
            self.inner.total_processing_ms.load(Ordering::Relaxed) as f64 / count as f64
        }
    }
}
