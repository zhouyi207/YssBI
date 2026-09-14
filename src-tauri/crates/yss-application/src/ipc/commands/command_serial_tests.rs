//! Serial-correlation test commands.

use crate::ipc::error::CommandError;
use crate::ipc::schema::statistics::{
    DurbinWatsonResultDto, SerialTestWithLagDto, SerialTestsRequestDto, SerialTestsResponseDto,
};
use yss_sci_contract::serial_tests::SerialTestsInput;
use yss_sci_runtime::time_series::serial_tests::compute_serial_tests as compute_serial_tests_runtime;

#[tauri::command]
pub fn compute_serial_tests(
    req: SerialTestsRequestDto,
) -> Result<SerialTestsResponseDto, CommandError> {
    let result = compute_serial_tests_runtime(SerialTestsInput {
        residuals: req.residuals,
        lags: req.lags,
        exog: req.exog,
        bg_nomiss0: req.bg_nomiss0,
    })
    .map_err(|error| CommandError::expected(error.code()))?;
    Ok(SerialTestsResponseDto {
        bg: result.bg.map(|value| SerialTestWithLagDto {
            stat: value.stat,
            p_value: value.p_value,
            lags: value.lags,
        }),
        q: result.q.map(|value| SerialTestWithLagDto {
            stat: value.stat,
            p_value: value.p_value,
            lags: value.lags,
        }),
        dw: DurbinWatsonResultDto { d: result.dw.d },
    })
}
