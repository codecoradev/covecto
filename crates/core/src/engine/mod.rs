//! The two vectorization engines.
//!
//! Crate-private: callers use the crate's interface (`vectorize`,
//! `vectorize_to`, `render_to`, `select_engine`), which picks the engine,
//! runs the optimizer and reports metadata consistently.

pub(crate) mod pixel_exact;
pub(crate) mod spline;

/// What an engine reports after streaming its SVG to a writer.
#[derive(Debug, Clone)]
pub(crate) struct EngineOutput {
    pub byte_count: usize,
    pub path_count: usize,
}
