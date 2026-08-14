//! Scientific-computing engine selection and execution context.

use std::path::Path;

use crate::julia::worker::JuliaWorkerManager;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SciEngine {
    /// Use the existing Rust scientific backend.
    Rust,
    /// Use the Julia worker and surface Julia errors directly.
    Julia,
    /// Run Julia first and fall back to Rust if Julia is unavailable or fails.
    JuliaWithRustFallback,
}

#[derive(Clone, Copy)]
pub struct JuliaSciContext<'a> {
    pub app_data_dir: &'a Path,
    pub worker: &'a JuliaWorkerManager,
}

#[derive(Clone, Copy)]
pub struct SciContext<'a> {
    pub engine: SciEngine,
    pub julia: Option<JuliaSciContext<'a>>,
}

impl<'a> SciContext<'a> {
    pub const fn rust() -> Self {
        Self {
            engine: SciEngine::Rust,
            julia: None,
        }
    }

    pub const fn with_engine(engine: SciEngine) -> Self {
        Self {
            engine,
            julia: None,
        }
    }

    pub fn with_julia(
        app_data_dir: &'a Path,
        worker: &'a JuliaWorkerManager,
        engine: SciEngine,
    ) -> Self {
        Self {
            engine,
            julia: Some(JuliaSciContext {
                app_data_dir,
                worker,
            }),
        }
    }
}

impl Default for SciContext<'_> {
    fn default() -> Self {
        Self::rust()
    }
}
