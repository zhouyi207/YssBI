//! A fitted model and bounded, model-owned memoization of its selected analyses.

use std::sync::{Arc, Mutex};
use yss_sci_contract::hypothesis::{HypothesisTestInput, HypothesisTestOutput};
use yss_sci_contract::regression::summary::LinearSummaryOptions;
use yss_sci_contract::scientific::{
    AcfPacfRequest, AcfPacfResult, LinearRegressionResult, ScientificComputationError,
    ScientificExecutionControl,
};
use yss_sci_contract::serial_tests::{SerialTestsInput, SerialTestsOutput};

use crate::{KernelControl, KernelError};

#[derive(Debug, Default)]
struct AnalysisCache {
    acf: Option<(usize, Arc<AcfPacfResult>)>,
    serial: Option<((usize, bool), Arc<SerialTestsOutput>)>,
    hypothesis: Option<(String, Arc<HypothesisTestOutput>)>,
}

#[derive(Debug)]
pub struct LinearSummary {
    pub options: LinearSummaryOptions,
    pub acf: Option<Arc<AcfPacfResult>>,
    pub serial: Option<Arc<SerialTestsOutput>>,
    pub hypothesis: Option<Arc<HypothesisTestOutput>>,
}

#[derive(Debug)]
pub struct LinearRegressionValue {
    pub model: Arc<LinearRegressionResult>,
    pub summary: Option<Arc<LinearSummary>>,
    cache: Arc<Mutex<AnalysisCache>>,
}

impl PartialEq for LinearRegressionValue {
    fn eq(&self, other: &Self) -> bool {
        self.model == other.model
            && self.summary.as_ref().map(|value| &value.options)
                == other.summary.as_ref().map(|value| &value.options)
    }
}

impl std::ops::Deref for LinearRegressionValue {
    type Target = LinearRegressionResult;
    fn deref(&self) -> &Self::Target {
        &self.model
    }
}

impl From<LinearRegressionResult> for LinearRegressionValue {
    fn from(model: LinearRegressionResult) -> Self {
        Self {
            model: Arc::new(model),
            summary: None,
            cache: Arc::default(),
        }
    }
}

impl LinearRegressionValue {
    pub fn summarize(
        &self,
        options: LinearSummaryOptions,
        control: &KernelControl,
    ) -> Result<Self, KernelError> {
        control.check()?;
        if (options.acf_pacf && !(1..=40).contains(&options.acf_max_lag))
            || (options.serial_tests && !(1..=40).contains(&options.serial_lags))
            || (options.hypothesis_test
                && (options.hypothesis.trim().is_empty() || options.hypothesis.len() > 4096))
        {
            return Err(KernelError::InvalidParameter);
        }
        // Analysis working arrays stay bounded by the same invocation budget as fitting.
        if options.acf_pacf || options.serial_tests || options.hypothesis_test {
            control.check_bytes(
                self.residuals
                    .len()
                    .checked_mul(self.design.len() + 1)
                    .and_then(|n| n.checked_mul(64)),
            )?;
        }
        let mut summary = LinearSummary {
            options,
            acf: None,
            serial: None,
            hypothesis: None,
        };
        if summary.options.acf_pacf {
            let lag = summary.options.acf_max_lag;
            let cached = self
                .cache
                .lock()
                .map_err(|_| KernelError::Failed)?
                .acf
                .clone();
            let value = if let Some((_, value)) = cached.filter(|(key, _)| *key == lag) {
                value
            } else {
                let value = Arc::new(
                    yss_sci_runtime::acf_pacf(
                        AcfPacfRequest {
                            values: self.residuals.clone(),
                            max_lag: lag,
                        },
                        &ScientificExecutionControl::from_shared(
                            control.cancellation.clone(),
                            control.deadline,
                        ),
                    )
                    .map_err(|error| match error {
                        ScientificComputationError::Cancelled => KernelError::Cancelled,
                        ScientificComputationError::DeadlineExceeded => {
                            KernelError::DeadlineExceeded
                        }
                        _ => KernelError::ScientificFailure,
                    })?,
                );
                control.check()?;
                self.cache.lock().map_err(|_| KernelError::Failed)?.acf =
                    Some((lag, value.clone()));
                value
            };
            summary.acf = Some(value);
        }
        control.check()?;
        if summary.options.serial_tests {
            let key = (summary.options.serial_lags, summary.options.bg_nomiss0);
            let cached = self
                .cache
                .lock()
                .map_err(|_| KernelError::Failed)?
                .serial
                .clone();
            let value = if let Some((_, value)) = cached.filter(|(current, _)| *current == key) {
                value
            } else {
                let value = Arc::new(
                    yss_sci_runtime::time_series::serial_tests::compute_serial_tests(
                        SerialTestsInput {
                            residuals: self.residuals.clone(),
                            exog: Some(
                                (0..self.residuals.len())
                                    .map(|row| {
                                        self.design.iter().map(|column| column[row]).collect()
                                    })
                                    .collect(),
                            ),
                            lags: key.0,
                            bg_nomiss0: key.1,
                        },
                    )
                    .map_err(|_| KernelError::ScientificFailure)?,
                );
                control.check()?;
                self.cache.lock().map_err(|_| KernelError::Failed)?.serial =
                    Some((key, value.clone()));
                value
            };
            summary.serial = Some(value);
        }
        control.check()?;
        if summary.options.hypothesis_test {
            let key = &summary.options.hypothesis;
            let cached = self
                .cache
                .lock()
                .map_err(|_| KernelError::Failed)?
                .hypothesis
                .clone();
            let value = if let Some((_, value)) = cached.filter(|(current, _)| current == key) {
                value
            } else {
                let value = Arc::new(
                    yss_sci_runtime::hypothesis::run_hypothesis_test(HypothesisTestInput {
                        betas: self.coefficients.clone(),
                        cov_beta: self.report.cov_beta.clone(),
                        df_residual: self.report.model_basic_info.df_residual,
                        param_names: self
                            .report
                            .coefficients
                            .iter()
                            .map(|value| value.variable.clone())
                            .collect(),
                        hypothesis: key.clone(),
                    })
                    .map_err(|_| KernelError::InvalidParameter)?,
                );
                control.check()?;
                self.cache
                    .lock()
                    .map_err(|_| KernelError::Failed)?
                    .hypothesis = Some((key.clone(), value.clone()));
                value
            };
            summary.hypothesis = Some(value);
        }
        control.check()?;
        Ok(Self {
            model: self.model.clone(),
            summary: Some(Arc::new(summary)),
            cache: self.cache.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;
    use std::time::{Duration, Instant};
    use yss_sci_contract::scientific::{LinearRegressionMethod, LinearRegressionRequest};

    #[test]
    fn selected_analyses_reuse_only_matching_model_and_parameters() {
        let control = KernelControl::new(
            Arc::new(AtomicBool::new(false)),
            Instant::now() + Duration::from_secs(30),
        );
        let model: LinearRegressionValue = yss_sci_runtime::linear_regression(
            LinearRegressionRequest {
                response: (0..48)
                    .map(|i| 2.0 + i as f64 * 0.4 + (i % 7) as f64 * 0.05)
                    .collect(),
                predictors: vec![(0..48).map(|i| i as f64).collect()],
                options: Default::default(),
                method: LinearRegressionMethod::Ols,
            },
            &ScientificExecutionControl::from_shared(
                control.cancellation.clone(),
                control.deadline,
            ),
        )
        .unwrap()
        .into();
        let unselected = LinearSummaryOptions {
            hypothesis: "invalid hypothesis".into(),
            ..Default::default()
        };
        let first = model.summarize(unselected, &control).unwrap();
        assert!(first.summary.as_ref().unwrap().hypothesis.is_none());
        let mut options = LinearSummaryOptions {
            acf_pacf: true,
            acf_max_lag: 3,
            ..Default::default()
        };
        let first = model.summarize(options.clone(), &control).unwrap();
        options.serial_tests = true;
        let added = model.summarize(options.clone(), &control).unwrap();
        assert!(Arc::ptr_eq(&first.model, &added.model));
        let first_acf = first.summary.as_ref().unwrap().acf.as_ref().unwrap();
        assert!(Arc::ptr_eq(
            first_acf,
            added.summary.as_ref().unwrap().acf.as_ref().unwrap()
        ));
        assert!(added.summary.as_ref().unwrap().serial.is_some());
        options.acf_max_lag = 4;
        let changed = model.summarize(options.clone(), &control).unwrap();
        assert!(!Arc::ptr_eq(
            first_acf,
            changed.summary.as_ref().unwrap().acf.as_ref().unwrap()
        ));
        assert_eq!(first.summary.as_ref().unwrap().options.acf_max_lag, 3);
        let replacement: LinearRegressionValue = (*model.model).clone().into();
        let replaced = replacement.summarize(options, &control).unwrap();
        assert!(!Arc::ptr_eq(
            changed.summary.as_ref().unwrap().acf.as_ref().unwrap(),
            replaced.summary.as_ref().unwrap().acf.as_ref().unwrap()
        ));
        assert!(matches!(
            model.summarize(
                LinearSummaryOptions {
                    hypothesis_test: true,
                    hypothesis: "invalid hypothesis".into(),
                    ..Default::default()
                },
                &control
            ),
            Err(KernelError::InvalidParameter)
        ));
    }
}
