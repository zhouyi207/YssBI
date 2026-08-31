use std::future::Future;

use crate::SqlSourceError;

/// Run a SQLx future behind the crate's synchronous import API.
///
/// When a caller is already inside a Tokio runtime, a scoped worker runtime prevents Tokio's
/// nested-`block_on` panic. SQL imports are coarse operations, so one bounded worker thread per
/// call is preferable to leaking a process-global runtime or blocking an async executor thread.
pub(crate) fn run<F, T>(future: F) -> Result<T, SqlSourceError>
where
    F: Future<Output = Result<T, SqlSourceError>> + Send + 'static,
    T: Send + 'static,
{
    if tokio::runtime::Handle::try_current().is_ok() {
        return std::thread::Builder::new()
            .name("yss-sql-source".to_string())
            .spawn(move || run_on_local_runtime(future))
            .map_err(SqlSourceError::RuntimeThread)?
            .join()
            .map_err(|_| SqlSourceError::RuntimePanicked)?;
    }

    run_on_local_runtime(future)
}

fn run_on_local_runtime<F, T>(future: F) -> Result<T, SqlSourceError>
where
    F: Future<Output = Result<T, SqlSourceError>>,
{
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(SqlSourceError::RuntimeInit)?
        .block_on(future)
}
