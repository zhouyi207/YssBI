//! Serial-correlation test application API and Rust backend orchestration.
//!
//! Covers Durbin-Watson, Ljung-Box Q, and optional Breusch-Godfrey LM tests.

use yss_sci_contract::serial_tests::{
    DurbinWatsonResult, SerialTestWithLag, SerialTestsInput, SerialTestsOutput,
};

use yss_sci_contract::{SciError, SciInputViolation, SciOperationCode};

pub fn compute_serial_tests(input: SerialTestsInput) -> Result<SerialTestsOutput, SciError> {
    let lags = normalized_lags(&input)?;
    Ok(compute(
        &input.residuals,
        input.exog.as_deref(),
        lags,
        input.bg_nomiss0,
    ))
}

fn normalized_lags(input: &SerialTestsInput) -> Result<usize, SciError> {
    let n = input.residuals.len();
    if n < 4 {
        return Err(SciError::InvalidInput {
            operation: SciOperationCode::SerialTests,
            violation: SciInputViolation::EmptyInput,
        });
    }
    Ok(input.lags.min(n / 2 - 1).clamp(1, 40))
}

use yss_sci::ts::serial_correlation::{breusch_godfrey, durbin_watson, ljung_box_q};

fn compute(
    residuals: &[f64],
    exog: Option<&[Vec<f64>]>,
    lags: usize,
    bg_nomiss0: bool,
) -> SerialTestsOutput {
    let dw = DurbinWatsonResult {
        d: durbin_watson(residuals),
    };

    let bg = exog
        .and_then(|exog| breusch_godfrey(residuals, exog, lags, bg_nomiss0))
        .map(|(stat, p_value)| SerialTestWithLag {
            stat,
            p_value,
            lags,
        });

    let q = ljung_box_q(residuals, lags).map(|(stat, p_value)| SerialTestWithLag {
        stat,
        p_value,
        lags,
    });

    SerialTestsOutput { bg, q, dw }
}
