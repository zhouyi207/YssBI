use std::sync::Arc;
use std::time::{Duration, Instant};

use thiserror::Error;

use crate::execution::{ApplicationState, SessionCaptureError};
use yss_execution::ports::scientific::{
    AcfPacfRequest, AcfPacfResult, BackendExecutionControl, ScientificBackendError,
};
use yss_sci_contract::SciError;
use yss_sci_runtime::api::time_series::serial_tests::{
    SerialTestsInput, compute_serial_tests as compute_serial_tests_api,
};

#[derive(Debug, Error)]
pub enum AcfPacfApplicationError {
    #[error("application session capture failed")]
    SessionCapture(#[from] SessionCaptureError),
    #[error("scientific backend failed")]
    Backend(#[from] ScientificBackendError),
}

pub fn compute_acf_pacf(
    application: &ApplicationState,
    residuals: Vec<f64>,
    max_lag: usize,
) -> Result<AcfPacfResult, AcfPacfApplicationError> {
    let session = application.capture_session()?;
    let cancellation = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let control = BackendExecutionControl::from_shared(
        cancellation,
        Instant::now() + Duration::from_secs(60),
    );
    session
        .execution()
        .scientific_backend()
        .acf_pacf(
            AcfPacfRequest {
                values: residuals,
                max_lag,
            },
            &control,
        )
        .map_err(Into::into)
}

pub struct SerialTestsRequest {
    pub residuals: Vec<f64>,
    pub lags: usize,
    pub exog: Option<Vec<Vec<f64>>>,
    pub bg_nomiss0: bool,
}

pub struct SerialTestWithLag {
    pub stat: f64,
    pub p_value: f64,
    pub lags: usize,
}

pub struct DurbinWatsonResult {
    pub d: f64,
}

pub struct SerialTestsResult {
    pub bg: Option<SerialTestWithLag>,
    pub q: Option<SerialTestWithLag>,
    pub dw: DurbinWatsonResult,
}

pub fn compute_serial_tests(
    input: SerialTestsRequest,
) -> Result<SerialTestsResult, SerialTestsApplicationError> {
    let result = compute_serial_tests_api(SerialTestsInput {
        residuals: input.residuals,
        lags: input.lags,
        exog: input.exog,
        bg_nomiss0: input.bg_nomiss0,
    })
    .map_err(SerialTestsApplicationError)?;
    Ok(SerialTestsResult {
        bg: result.bg.map(|value| SerialTestWithLag {
            stat: value.stat,
            p_value: value.p_value,
            lags: value.lags,
        }),
        q: result.q.map(|value| SerialTestWithLag {
            stat: value.stat,
            p_value: value.p_value,
            lags: value.lags,
        }),
        dw: DurbinWatsonResult { d: result.dw.d },
    })
}

#[derive(Debug)]
pub struct SerialTestsApplicationError(SciError);

impl SerialTestsApplicationError {
    pub fn command_code(&self) -> &'static str {
        self.0.code()
    }
}
