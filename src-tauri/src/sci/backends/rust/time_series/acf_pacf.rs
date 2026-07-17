use yss_sci::ts::acf_pacf::{acf, pacf};

use crate::sci::api::time_series::acf_pacf::AcfPacfOutput;
use crate::sci::error::SciError;

pub fn compute_at_lag(values: &[f64], max_lag: usize) -> Result<AcfPacfOutput, SciError> {
    let n = values.len();
    if n < 4 {
        return Err(SciError::invalid_input("ACF/PACF: 至少需要 4 个观测值"));
    }
    let max_lag = max_lag.min(n - 1);
    Ok(AcfPacfOutput {
        acf: acf(values, max_lag),
        pacf: pacf(values, max_lag),
        n,
    })
}
