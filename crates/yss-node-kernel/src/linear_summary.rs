//! A fitted model and bounded, model-owned memoization of its selected analyses.

use std::sync::{Arc, Mutex};
use yss_sci_contract::diagnostics::serial_correlation::{SerialTestsInput, SerialTestsOutput};
use yss_sci_contract::execution::{ScientificComputationError, ScientificExecutionControl};
use yss_sci_contract::hypothesis::{HypothesisTestInput, HypothesisTestOutput};
use yss_sci_contract::regression::linear::LinearRegressionResult;
use yss_sci_contract::regression::summary::LinearSummaryOptions;
use yss_sci_contract::time_series::acf_pacf::{AcfPacfRequest, AcfPacfResult};

use crate::{KernelControl, KernelError};

#[derive(Debug, Default)]
struct AnalysisCache {
    acf: Option<(usize, Arc<AcfPacfResult>)>,
    serial: Option<((usize, bool), Arc<SerialTestsOutput>)>,
    hypothesis: Option<(String, Arc<HypothesisTestOutput>)>,
    diagnostics: Option<Arc<LinearDiagnostics>>,
    leverage: Option<Arc<Vec<f64>>>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct LinearDiagnosticEntry {
    pub name: String,
    pub value: Option<serde_json::Value>,
    pub unavailable_reason: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct LinearDiagnostics {
    pub tests: Vec<LinearDiagnosticEntry>,
    pub leverage_density: Vec<yss_sci_contract::visualization::PlotPoint>,
    pub leverage_unavailable_reason: Option<String>,
}

#[derive(Debug)]
pub struct LinearSummary {
    pub options: LinearSummaryOptions,
    pub acf: Option<Arc<AcfPacfResult>>,
    pub serial: Option<Arc<SerialTestsOutput>>,
    pub hypothesis: Option<Arc<HypothesisTestOutput>>,
    pub diagnostics: Option<Arc<LinearDiagnostics>>,
    pub leverage: Option<Arc<Vec<f64>>>,
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
            diagnostics: None,
            leverage: None,
        };
        if summary.options.diagnostics || summary.options.residual_plot {
            use yss_sci_contract::diagnostics::residual::{
                ResidualDiagnostic as Test, ResidualDiagnosticResult,
            };
            let n = self.residuals.len();
            let k = self.design.len();
            let expanded = if summary.options.diagnostics {
                k.checked_mul(k + 4).ok_or(KernelError::BudgetExceeded)?
            } else {
                k + 1
            };
            control.check_bytes(n.checked_mul(expanded).and_then(|v| v.checked_mul(64)))?;
            let cached = self
                .cache
                .lock()
                .map_err(|_| KernelError::Failed)?
                .leverage
                .clone();
            let (leverage, leverage_error) = if let Some(values) = cached {
                (Some(values), None)
            } else {
                match yss_sci_runtime::diagnostics::residual::diagnose(self, Test::Leverage) {
                    Ok(ResidualDiagnosticResult::Leverage(values)) => {
                        let values = Arc::new(values);
                        self.cache.lock().map_err(|_| KernelError::Failed)?.leverage =
                            Some(values.clone());
                        (Some(values), None)
                    }
                    Err(reason) => (None, Some(reason)),
                    _ => return Err(KernelError::ScientificFailure),
                }
            };
            control.check()?;
            summary.leverage = leverage.clone();
            if summary.options.diagnostics {
                let cached = self
                    .cache
                    .lock()
                    .map_err(|_| KernelError::Failed)?
                    .diagnostics
                    .clone();
                let diagnostics = if let Some(value) = cached {
                    value
                } else {
                    let mut tests = Vec::new();
                    for (name, test) in [
                        (
                            "Breusch–Pagan (fitted values, classical)",
                            Test::BreuschPagan {
                                rhs: false,
                                koenker: false,
                            },
                        ),
                        ("White", Test::White),
                        ("Information matrix", Test::InformationMatrix),
                        ("RESET (fitted powers)", Test::Reset { rhs: false }),
                        ("Variance inflation factors", Test::Vif),
                    ] {
                        control.check()?;
                        let result = yss_sci_runtime::diagnostics::residual::diagnose(self, test)
                            .and_then(|v| {
                                let mut value =
                                    serde_json::to_value(v).map_err(|e| e.to_string())?;
                                if matches!(test, Test::Vif)
                                    && let Some(rows) = value["result"].as_array_mut()
                                {
                                    for (row, coefficient) in
                                        rows.iter_mut().zip(&self.report.coefficients)
                                    {
                                        row["variable"] = coefficient.variable.clone().into();
                                    }
                                }
                                Ok(value)
                            });
                        tests.push(match result {
                            Ok(value) => LinearDiagnosticEntry {
                                name: name.into(),
                                value: Some(value),
                                unavailable_reason: None,
                            },
                            Err(reason) => LinearDiagnosticEntry {
                                name: name.into(),
                                value: None,
                                unavailable_reason: Some(reason),
                            },
                        });
                    }
                    let normality =
                        yss_sci_runtime::diagnostics::residual::normality(&self.residuals)
                            .and_then(|v| serde_json::to_value(v).map_err(|e| e.to_string()));
                    tests.push(match normality {
                        Ok(value) => LinearDiagnosticEntry {
                            name: "Residual normality".into(),
                            value: Some(value),
                            unavailable_reason: None,
                        },
                        Err(reason) => LinearDiagnosticEntry {
                            name: "Residual normality".into(),
                            value: None,
                            unavailable_reason: Some(reason),
                        },
                    });
                    let (density, reason) = if let Some(values) = &leverage {
                        match yss_sci_runtime::visualization::kde(
                            values,
                            128,
                            &ScientificExecutionControl::from_shared(
                                control.cancellation.clone(),
                                control.deadline,
                            ),
                        ) {
                            Ok(plot) => (plot.data, None),
                            Err(error) => {
                                control.check()?;
                                (Vec::new(), Some(error.to_string()))
                            }
                        }
                    } else {
                        (Vec::new(), leverage_error)
                    };
                    control.check()?;
                    let value = Arc::new(LinearDiagnostics {
                        tests,
                        leverage_density: density,
                        leverage_unavailable_reason: reason,
                    });
                    self.cache
                        .lock()
                        .map_err(|_| KernelError::Failed)?
                        .diagnostics = Some(value.clone());
                    value
                };
                summary.diagnostics = Some(diagnostics);
            }
        }
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
                    yss_sci_runtime::time_series::acf_pacf(
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
                    yss_sci_runtime::diagnostics::serial_correlation::compute_serial_tests(
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
    use yss_sci_contract::regression::linear::{LinearRegressionMethod, LinearRegressionRequest};

    #[test]
    fn selected_analyses_reuse_only_matching_model_and_parameters() {
        let control = KernelControl::new(
            Arc::new(AtomicBool::new(false)),
            Instant::now() + Duration::from_secs(30),
        );
        let model: LinearRegressionValue = yss_sci_runtime::regression::linear::linear_regression(
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
