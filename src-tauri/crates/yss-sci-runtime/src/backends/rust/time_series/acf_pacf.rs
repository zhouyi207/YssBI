use yss_sci::ts::acf_pacf::{acf, pacf};

use crate::api::time_series::acf_pacf::AcfPacfOutput;
use yss_sci_contract::{SciError, SciInputViolation, SciOperationCode};

pub fn compute_at_lag(values: &[f64], max_lag: usize) -> Result<AcfPacfOutput, SciError> {
    let n = values.len();
    if n < 4 {
        return Err(SciError::InvalidInput {
            operation: SciOperationCode::AcfPacf,
            violation: SciInputViolation::EmptyInput,
        });
    }
    let max_lag = max_lag.min(n - 1);
    Ok(AcfPacfOutput {
        acf: acf(values, max_lag),
        pacf: pacf(values, max_lag),
        n,
    })
}
