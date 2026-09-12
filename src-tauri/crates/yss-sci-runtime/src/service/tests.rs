use std::time::{Duration, Instant};

use super::{SciRuntimeBackend, map_sci_error};
use yss_sci_contract::scientific::{
    AcfPacfRequest, BackendCancellationToken, BackendExecutionControl, ScientificBackend,
    ScientificBackendError, ScientificInputViolation,
};
use yss_sci_contract::{SciError, SciOperationCode};

fn active_control() -> BackendExecutionControl {
    BackendExecutionControl {
        cancellation: BackendCancellationToken::new(),
        deadline: Instant::now() + Duration::from_secs(5),
    }
}

#[test]
fn adapter_maps_acf_pacf_results_and_rejects_invalid_requests() {
    let backend: &dyn ScientificBackend = &SciRuntimeBackend::new();
    let result = backend
        .acf_pacf(
            AcfPacfRequest {
                values: vec![1.0, 0.0, -1.0, 0.0, 1.0, 0.0],
                max_lag: 2,
            },
            &active_control(),
        )
        .expect("a valid ACF/PACF request must map through the SCI runtime");
    assert_eq!(result.acf.len(), 3);
    assert_eq!(result.pacf.len(), 2);
    assert_eq!(result.n, 6);

    assert_eq!(
        backend.acf_pacf(
            AcfPacfRequest {
                values: vec![1.0, f64::NAN, -1.0, 0.0],
                max_lag: 1,
            },
            &active_control(),
        ),
        Err(ScientificBackendError::InvalidInput {
            violation: ScientificInputViolation::NonFiniteInput,
        })
    );
    assert_eq!(
        backend.acf_pacf(
            AcfPacfRequest {
                values: vec![1.0, 0.0, -1.0, 0.0],
                max_lag: 0,
            },
            &active_control(),
        ),
        Err(ScientificBackendError::InvalidInput {
            violation: ScientificInputViolation::ParameterOutOfRange,
        })
    );
    assert_eq!(
        map_sci_error(SciError::ComputationFailed {
            operation: SciOperationCode::AcfPacf,
        }),
        ScientificBackendError::ComputationFailed
    );
}

#[test]
fn adapter_rejects_cancelled_or_expired_calls_at_admission() {
    let backend: &dyn ScientificBackend = &SciRuntimeBackend::new();
    let request = || AcfPacfRequest {
        values: vec![1.0, 0.0, -1.0, 0.0],
        max_lag: 1,
    };
    let cancelled = BackendCancellationToken::new();
    cancelled.cancel();

    assert_eq!(
        backend.acf_pacf(
            request(),
            &BackendExecutionControl {
                cancellation: cancelled,
                deadline: Instant::now() + Duration::from_secs(5),
            },
        ),
        Err(ScientificBackendError::Cancelled)
    );
    assert_eq!(
        backend.acf_pacf(
            request(),
            &BackendExecutionControl {
                cancellation: BackendCancellationToken::new(),
                deadline: Instant::now(),
            },
        ),
        Err(ScientificBackendError::DeadlineExceeded)
    );
}

#[test]
fn shared_ols_options_reach_the_model_and_typed_report() {
    use yss_sci_contract::regression::{OlsCovariance, OlsOptions};
    use yss_sci_contract::scientific::OlsRequest;
    let backend = SciRuntimeBackend::new();
    let response = vec![1.1, 2.2, 2.8, 4.1, 5.3, 5.7, 7.2, 8.4];
    let predictors = vec![(1..=8).map(f64::from).collect::<Vec<_>>()];
    let options = OlsOptions {
        constant: false,
        covariance: OlsCovariance::Hc3,
    };
    let result = backend
        .ols(
            OlsRequest {
                response: response.clone(),
                predictors: predictors.clone(),
                options: options.clone(),
            },
            &active_control(),
        )
        .unwrap();
    let numerical = yss_sci::regression::linear_model::OLS {
        endog: faer::Col::from_iter(response.iter().copied()),
        exog: faer::Mat::from_fn(response.len(), 1, |row, _| predictors[0][row]),
        config: options,
    }
    .fit()
    .unwrap();
    assert_eq!(
        result.coefficients,
        numerical.betas.iter().copied().collect::<Vec<_>>()
    );
    assert_eq!(
        result.fitted,
        numerical.fitted.iter().copied().collect::<Vec<_>>()
    );
    assert_eq!(
        result.residuals,
        numerical.residuals.iter().copied().collect::<Vec<_>>()
    );
    assert_eq!(result.design, predictors);
    assert_eq!(result.report.model_basic_info.covariance_type, "HC3");
    assert_eq!(
        result.report.model_basic_info.df_residual,
        numerical.df_residual
    );
    assert_eq!(result.report.coefficients.len(), 1);
    assert_eq!(result.report.coefficients[0].variable, "x1");
    let nonrobust = backend
        .ols(
            OlsRequest {
                response,
                predictors,
                options: OlsOptions {
                    constant: false,
                    covariance: OlsCovariance::NonRobust,
                },
            },
            &active_control(),
        )
        .unwrap();
    assert_ne!(
        result.report.coefficients[0].std_err,
        nonrobust.report.coefficients[0].std_err
    );
}
