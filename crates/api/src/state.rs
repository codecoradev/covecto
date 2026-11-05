use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::Semaphore;

/// Resource limits for CPU-heavy endpoints.
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    /// Maximum number of vectorize/optimize jobs running at once.
    /// Further requests wait for a free slot (backpressure, no rejection).
    pub max_concurrency: usize,
    /// Maximum width or height of an uploaded image, checked before decoding.
    pub max_dimension: u32,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_concurrency: std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(4),
            max_dimension: covecto_core::DEFAULT_MAX_DIMENSION,
        }
    }
}

impl Limits {
    /// Defaults overridden by `COVECTO_MAX_CONCURRENCY` / `COVECTO_MAX_DIMENSION`.
    pub fn from_env() -> Self {
        let mut limits = Self::default();
        if let Some(n) = env_parse::<usize>("COVECTO_MAX_CONCURRENCY").filter(|&n| n > 0) {
            limits.max_concurrency = n;
        }
        if let Some(n) = env_parse::<u32>("COVECTO_MAX_DIMENSION").filter(|&n| n > 0) {
            limits.max_dimension = n;
        }
        limits
    }
}

fn env_parse<T: std::str::FromStr>(key: &str) -> Option<T> {
    std::env::var(key).ok()?.trim().parse().ok()
}

#[derive(Clone)]
pub struct AppState {
    inner: Arc<AppStateInner>,
}

#[derive(Debug)]
struct AppStateInner {
    request_count: AtomicU64,
    total_processing_ms: AtomicU64,
    limits: Limits,
    jobs: Arc<Semaphore>,
}

/// State with limits taken from the environment (see [`Limits::from_env`]).
pub fn create_app_state() -> AppState {
    create_app_state_with_limits(Limits::from_env())
}

/// State with explicit limits.
pub fn create_app_state_with_limits(limits: Limits) -> AppState {
    AppState {
        inner: Arc::new(AppStateInner {
            request_count: AtomicU64::new(0),
            total_processing_ms: AtomicU64::new(0),
            jobs: Arc::new(Semaphore::new(limits.max_concurrency.max(1))),
            limits,
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

    pub fn limits(&self) -> Limits {
        self.inner.limits
    }

    /// Run CPU-bound `work` on the blocking thread pool, at most
    /// `max_concurrency` at a time, so async runtime workers stay free
    /// (health checks, uploads in flight) while jobs run.
    pub(crate) async fn run_job<T, F>(&self, work: F) -> T
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static,
    {
        let permit = self
            .inner
            .jobs
            .clone()
            .acquire_owned()
            .await
            .expect("job semaphore is never closed");
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            work()
        })
        .await
        .unwrap_or_else(|e| std::panic::resume_unwind(e.into_panic()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;
    use std::time::{Duration, Instant};

    fn state(max_concurrency: usize) -> AppState {
        create_app_state_with_limits(Limits {
            max_concurrency,
            max_dimension: 64,
        })
    }

    #[tokio::test(flavor = "current_thread")]
    async fn job_does_not_block_runtime() {
        // On a single-threaded runtime, a blocking job run inline would stall
        // every other task. With run_job, the runtime keeps serving.
        let st = state(1);
        let job = tokio::spawn({
            let st = st.clone();
            async move {
                st.run_job(|| std::thread::sleep(Duration::from_millis(400)))
                    .await
            }
        });
        tokio::task::yield_now().await;
        let t = Instant::now();
        tokio::time::sleep(Duration::from_millis(10)).await;
        assert!(
            t.elapsed() < Duration::from_millis(300),
            "runtime was blocked"
        );
        job.await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn concurrency_is_bounded() {
        let st = state(2);
        let running = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let tasks: Vec<_> = (0..6)
            .map(|_| {
                let (st, running, peak) = (st.clone(), running.clone(), peak.clone());
                tokio::spawn(async move {
                    st.run_job(move || {
                        let now = running.fetch_add(1, Ordering::SeqCst) + 1;
                        peak.fetch_max(now, Ordering::SeqCst);
                        std::thread::sleep(Duration::from_millis(50));
                        running.fetch_sub(1, Ordering::SeqCst);
                    })
                    .await
                })
            })
            .collect();
        for t in tasks {
            t.await.unwrap();
        }
        assert_eq!(peak.load(Ordering::SeqCst), 2);
    }
}
