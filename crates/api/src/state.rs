use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::Duration;
use tokio::sync::Semaphore;

/// Resource limits for CPU-heavy endpoints.
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    /// Maximum number of vectorize/optimize jobs running at once.
    pub max_concurrency: usize,
    /// Maximum number of requests waiting for a job slot. Requests beyond
    /// `max_concurrency + max_queue` are rejected with `503` before their
    /// upload is read, so a burst cannot pile up buffered bodies.
    pub max_queue: usize,
    /// How long a request may wait for a job slot before it gets `503`.
    pub queue_timeout: Duration,
    /// How long reading a request body may take before `408`. The body is
    /// read while holding a queue place, so this bounds slow uploads.
    pub upload_timeout: Duration,
    /// Maximum width or height of an uploaded image, checked before decoding.
    pub max_dimension: u32,
    /// Server-side spline input budget in megapixels; larger images are
    /// downscaled before tracing. Clients may request a lower budget, never a
    /// higher one. `0` disables the server cap (then clients decide).
    pub spline_max_megapixels: f64,
}

impl Default for Limits {
    fn default() -> Self {
        let max_concurrency = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4);
        Self {
            max_concurrency,
            max_queue: max_concurrency.saturating_mul(4),
            queue_timeout: Duration::from_secs(30),
            upload_timeout: Duration::from_secs(30),
            max_dimension: covecto_core::DEFAULT_MAX_DIMENSION,
            spline_max_megapixels: covecto_core::DEFAULT_SPLINE_MAX_MEGAPIXELS,
        }
    }
}

impl Limits {
    /// Defaults overridden by `COVECTO_MAX_CONCURRENCY`, `COVECTO_MAX_QUEUE`,
    /// `COVECTO_QUEUE_TIMEOUT_SECS`, `COVECTO_MAX_DIMENSION` and
    /// `COVECTO_SPLINE_MAX_MP`.
    pub fn from_env() -> Self {
        let mut limits = Self::default();
        if let Some(n) = env_parse::<usize>("COVECTO_MAX_CONCURRENCY").filter(|&n| n > 0) {
            limits.max_concurrency = n.min(MAX_CONCURRENCY);
            limits.max_queue = limits.max_concurrency.saturating_mul(4);
        }
        // 0 is valid: no waiting, only `max_concurrency` requests at a time.
        if let Some(n) = env_parse::<usize>("COVECTO_MAX_QUEUE") {
            limits.max_queue = n;
        }
        if let Some(secs) = env_parse::<u64>("COVECTO_QUEUE_TIMEOUT_SECS").filter(|&s| s > 0) {
            limits.queue_timeout = Duration::from_secs(secs);
        }
        if let Some(secs) = env_parse::<u64>("COVECTO_UPLOAD_TIMEOUT_SECS").filter(|&s| s > 0) {
            limits.upload_timeout = Duration::from_secs(secs);
        }
        if let Some(n) = env_parse::<u32>("COVECTO_MAX_DIMENSION").filter(|&n| n > 0) {
            limits.max_dimension = n;
        }
        // Invalid or negative values keep the safe default; only an explicit
        // 0 turns the server cap off.
        if let Some(mp) =
            env_parse::<f64>("COVECTO_SPLINE_MAX_MP").filter(|mp| mp.is_finite() && *mp >= 0.0)
        {
            limits.spline_max_megapixels = mp;
        }
        limits
    }

    /// Effective spline budget for a request: the client's value, capped by
    /// the server's. A client cannot disable or raise the server cap.
    pub fn spline_budget(&self, requested: Option<f64>) -> f64 {
        let requested = requested.filter(|mp| mp.is_finite() && *mp > 0.0);
        if self.spline_max_megapixels > 0.0 {
            requested.map_or(self.spline_max_megapixels, |mp| {
                mp.min(self.spline_max_megapixels)
            })
        } else {
            requested.unwrap_or(0.0)
        }
    }
}

/// Upper bound for `max_concurrency` (well below `Semaphore::MAX_PERMITS`).
const MAX_CONCURRENCY: usize = 1 << 16;

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
    /// Requests admitted (waiting for a slot or running).
    admitted: AtomicUsize,
}

/// Why a job could not run.
#[derive(Debug)]
pub enum JobError {
    /// The queue is full; the request was rejected before reading its body.
    Busy,
    /// No job slot became free within `queue_timeout`.
    Timeout,
    /// The job panicked or was cancelled.
    Failed(tokio::task::JoinError),
}

/// A place in the job queue. Held from admission until the job finishes,
/// including after the client disconnects, so the queue bound counts real
/// work. Dropping it frees the place.
pub struct Ticket {
    inner: Arc<AppStateInner>,
}

impl Drop for Ticket {
    fn drop(&mut self) {
        self.inner.admitted.fetch_sub(1, Ordering::AcqRel);
    }
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
            jobs: Arc::new(Semaphore::new(
                limits.max_concurrency.clamp(1, MAX_CONCURRENCY),
            )),
            admitted: AtomicUsize::new(0),
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

    /// Take a place in the job queue, or `Err(JobError::Busy)` when
    /// `max_concurrency + max_queue` requests are already admitted. Call this
    /// before reading the request body.
    pub fn admit(&self) -> Result<Ticket, JobError> {
        let limits = &self.inner.limits;
        let capacity = limits
            .max_concurrency
            .clamp(1, MAX_CONCURRENCY)
            .saturating_add(limits.max_queue);
        // Increment only when below capacity, so concurrent rejections never
        // make the count read high and turn away a request that would fit.
        let admitted = &self.inner.admitted;
        let mut n = admitted.load(Ordering::Acquire);
        loop {
            if n >= capacity {
                return Err(JobError::Busy);
            }
            match admitted.compare_exchange_weak(n, n + 1, Ordering::AcqRel, Ordering::Acquire) {
                Ok(_) => break,
                Err(current) => n = current,
            }
        }
        Ok(Ticket {
            inner: self.inner.clone(),
        })
    }

    /// Run CPU-bound `work` on the blocking thread pool, at most
    /// `max_concurrency` at a time, so async runtime workers stay free
    /// (health checks, uploads in flight) while jobs run. Waits at most
    /// `queue_timeout` for a slot.
    ///
    /// A panic inside `work` is contained and returned as `Err`, so the
    /// handler can still answer with a 500 instead of dropping the connection.
    pub(crate) async fn run_job<T, F>(&self, ticket: Ticket, work: F) -> Result<T, JobError>
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static,
    {
        let acquire = self.inner.jobs.clone().acquire_owned();
        let permit = tokio::time::timeout(self.inner.limits.queue_timeout, acquire)
            .await
            .map_err(|_| JobError::Timeout)?
            .expect("job semaphore is never closed");
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            let _ticket = ticket;
            work()
        })
        .await
        .map_err(JobError::Failed)
    }

    /// Requests currently admitted (waiting or running).
    pub fn admitted(&self) -> usize {
        self.inner.admitted.load(Ordering::Acquire)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    fn state(max_concurrency: usize) -> AppState {
        create_app_state_with_limits(Limits {
            max_concurrency,
            max_dimension: 64,
            spline_max_megapixels: 2.0,
            ..Limits::default()
        })
    }

    #[test]
    fn spline_budget_client_can_only_lower() {
        let limits = Limits {
            spline_max_megapixels: 2.0,
            ..Limits::default()
        };
        assert_eq!(limits.spline_budget(None), 2.0);
        assert_eq!(limits.spline_budget(Some(0.5)), 0.5);
        assert_eq!(limits.spline_budget(Some(50.0)), 2.0);
        assert_eq!(limits.spline_budget(Some(0.0)), 2.0); // cannot disable
        assert_eq!(limits.spline_budget(Some(f64::NAN)), 2.0);
        let open = Limits {
            spline_max_megapixels: 0.0,
            ..limits
        };
        assert_eq!(open.spline_budget(None), 0.0);
        assert_eq!(open.spline_budget(Some(3.0)), 3.0);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn job_does_not_block_runtime() {
        // On a single-threaded runtime, a blocking job run inline would stall
        // every other task. With run_job, the runtime keeps serving.
        let st = state(1);
        let job = tokio::spawn({
            let st = st.clone();
            async move {
                let ticket = st.admit().unwrap();
                st.run_job(ticket, || std::thread::sleep(Duration::from_millis(400)))
                    .await
                    .unwrap()
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
                    let ticket = st.admit().unwrap();
                    st.run_job(ticket, move || {
                        let now = running.fetch_add(1, Ordering::SeqCst) + 1;
                        peak.fetch_max(now, Ordering::SeqCst);
                        std::thread::sleep(Duration::from_millis(50));
                        running.fetch_sub(1, Ordering::SeqCst);
                    })
                    .await
                    .unwrap()
                })
            })
            .collect();
        for t in tasks {
            t.await.unwrap();
        }
        assert_eq!(peak.load(Ordering::SeqCst), 2);
        assert_eq!(st.admitted(), 0);
    }

    #[tokio::test]
    async fn panicking_job_is_contained_and_releases_its_slot() {
        let st = state(1);
        let res = st
            .run_job(st.admit().unwrap(), || -> u8 { panic!("boom") })
            .await;
        assert!(matches!(res, Err(JobError::Failed(e)) if e.is_panic()));
        // The permit and the queue place were released: the next job runs.
        assert_eq!(st.admitted(), 0);
        assert_eq!(st.run_job(st.admit().unwrap(), || 7).await.unwrap(), 7);
    }

    #[test]
    fn queue_admission_is_bounded() {
        // #60: 1 running + 2 waiting allowed; the 4th request is Busy.
        let st = create_app_state_with_limits(Limits {
            max_concurrency: 1,
            max_queue: 2,
            ..Limits::default()
        });
        let tickets: Vec<_> = (0..3).map(|_| st.admit().unwrap()).collect();
        assert!(matches!(st.admit(), Err(JobError::Busy)));
        assert_eq!(st.admitted(), 3, "a rejected admit must not leak a place");
        drop(tickets);
        assert_eq!(st.admitted(), 0);
        assert!(st.admit().is_ok());
    }

    #[test]
    fn huge_limits_do_not_overflow() {
        let st = create_app_state_with_limits(Limits {
            max_concurrency: usize::MAX,
            max_queue: usize::MAX,
            ..Limits::default()
        });
        let t = st
            .admit()
            .expect("capacity saturates instead of overflowing");
        assert_eq!(st.admitted(), 1);
        drop(t);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn waiting_times_out() {
        let st = create_app_state_with_limits(Limits {
            max_concurrency: 1,
            max_queue: 4,
            queue_timeout: Duration::from_millis(100),
            ..Limits::default()
        });
        let busy = tokio::spawn({
            let st = st.clone();
            async move {
                st.run_job(st.admit().unwrap(), || {
                    std::thread::sleep(Duration::from_millis(600))
                })
                .await
                .unwrap()
            }
        });
        tokio::time::sleep(Duration::from_millis(50)).await;
        let t = Instant::now();
        let res = st.run_job(st.admit().unwrap(), || ()).await;
        assert!(matches!(res, Err(JobError::Timeout)));
        assert!(
            t.elapsed() < Duration::from_millis(400),
            "{:?}",
            t.elapsed()
        );
        busy.await.unwrap();
        assert_eq!(st.admitted(), 0);
    }
}
