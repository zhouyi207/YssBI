use yss_sci::ts::serial_correlation::{breusch_godfrey, durbin_watson, ljung_box_q};

use crate::sci::api::time_series::serial_tests::{
    DurbinWatsonResult, SerialTestWithLag, SerialTestsOutput,
};

pub fn compute(
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
