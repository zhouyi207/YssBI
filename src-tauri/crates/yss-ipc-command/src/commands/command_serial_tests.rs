//! Serial-correlation test commands.

use crate::error::CommandError;
use crate::schema::statistics::{
    DurbinWatsonResultDto, SerialTestWithLagDto, SerialTestsRequestDto, SerialTestsResponseDto,
};
use yss_application::statistics::{
    SerialTestsApplicationError, SerialTestsRequest,
    compute_serial_tests as compute_serial_tests_application,
};

#[tauri::command]
pub fn compute_serial_tests(
    req: SerialTestsRequestDto,
) -> Result<SerialTestsResponseDto, CommandError> {
    let result = compute_serial_tests_application(SerialTestsRequest {
        residuals: req.residuals,
        lags: req.lags,
        exog: req.exog,
        bg_nomiss0: req.bg_nomiss0,
    })
    .map_err(sci_command_error)?;
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

fn sci_command_error(error: SerialTestsApplicationError) -> CommandError {
    CommandError::expected(error.command_code())
}
