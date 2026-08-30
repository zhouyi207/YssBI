use yss_execution::ports::relational::{
    RelationalBackend, RelationalError, RelationalExecutionControl, RelationalRequest,
    RelationalResult,
};

#[derive(Default)]
pub struct ProductionRelationalBackend;

impl RelationalBackend for ProductionRelationalBackend {
    fn execute(
        &self,
        _request: RelationalRequest,
        control: &RelationalExecutionControl,
    ) -> Result<RelationalResult, RelationalError> {
        if control.cancellation.is_cancelled() {
            return Err(RelationalError::Cancelled);
        }
        if control.deadline <= std::time::Instant::now() {
            return Err(RelationalError::DeadlineExceeded);
        }
        Err(RelationalError::Unavailable)
    }
}
