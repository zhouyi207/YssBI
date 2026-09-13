//! Runtime entry points for regression computations and report encoding.
pub mod report;
pub mod types;
use crate::error::computation_failed;
use yss_sci_contract::regression::OlsOptions;
use yss_sci_contract::regression::fit::{InstrumentalVariableKind, RegressionFit, RegressionKind};
use yss_sci_contract::{SciError, SciOperationCode, StatisticalObservationMetadata};

pub fn fit_regression(
    kind: RegressionKind,
    response: Vec<f64>,
    predictors: Vec<Vec<f64>>,
    weights: Option<Vec<f64>>,
    metadata: StatisticalObservationMetadata,
) -> Result<RegressionFit, SciError> {
    yss_sci::regression::fit::fit_regression(kind, response, predictors, weights, metadata)
}

pub fn fit_ols(
    response: Vec<f64>,
    predictors: &[Vec<f64>],
    config: OlsOptions,
    metadata: StatisticalObservationMetadata,
) -> Result<RegressionFit, SciError> {
    yss_sci::regression::fit::fit_ols(response, predictors, config, metadata)
}

pub fn fit_instrumental_variables(
    kind: InstrumentalVariableKind,
    response: Vec<f64>,
    exogenous: Vec<f64>,
    endogenous: Vec<f64>,
    instruments: Vec<f64>,
) -> Result<serde_json::Value, SciError> {
    let fit = yss_sci::regression::fit::fit_instrumental_variables(
        kind,
        response,
        exogenous,
        endogenous,
        instruments,
    )?;
    serde_json::to_value(fit)
        .map_err(|_| computation_failed(SciOperationCode::InstrumentalVariables))
}

pub fn fit_panel(
    response: Vec<f64>,
    predictors: Vec<Vec<f64>>,
    entity: Vec<f64>,
    time: Vec<f64>,
    treatment: Option<Vec<f64>>,
) -> Result<serde_json::Value, SciError> {
    let fit = yss_sci::regression::fit::fit_panel(response, predictors, entity, time, treatment)?;
    serde_json::to_value(fit).map_err(|_| computation_failed(SciOperationCode::Panel))
}

#[cfg(test)]
mod tests {
    use super::report::regression_report;
    use super::*;
    use crate::time_series::augmented_dickey_fuller;
    use yss_sci_contract::regression::fit::RegressionStatistics;
    use yss_sci_contract::{MissingValuePolicy, StatisticalObservationMetadata};
    use yss_sci_contract::{SciError, SciInputViolation, SciOperationCode};

    fn regression_metadata(observations: usize) -> StatisticalObservationMetadata {
        StatisticalObservationMetadata {
            original_observation_count: observations,
            used_observation_count: observations,
            dropped_null_count: 0,
            dropped_nan_count: 0,
            missing_value_policy: MissingValuePolicy::Listwise,
        }
    }

    #[test]
    fn regression_reports_expose_hypothesis_inputs() {
        let response = vec![1.0, 2.1, 2.9, 4.2, 5.1, 5.9];
        let predictor = vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0];
        let fit = fit_regression(
            RegressionKind::Ols,
            response.clone(),
            vec![predictor],
            None,
            regression_metadata(response.len()),
        )
        .unwrap();

        let report = regression_report(&fit).expect("regression report must serialize");

        assert_eq!(
            report["coefficients"]
                .as_array()
                .unwrap()
                .iter()
                .map(|coefficient| coefficient["coef"].as_f64().unwrap())
                .collect::<Vec<_>>(),
            fit.coefficients
        );
        assert_eq!(report["cov_beta"].as_array().map(Vec::len), Some(2));
        assert!(
            report["cov_beta"]
                .as_array()
                .unwrap()
                .iter()
                .all(|row| row.as_array().map(Vec::len) == Some(2))
        );
    }

    #[test]
    fn binary_regression_reports_preserve_likelihood_statistics() {
        let response = vec![0.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 1.0];
        let predictor = vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0];

        for (kind, expected_link) in [
            (RegressionKind::Logit, "logit"),
            (RegressionKind::Probit, "probit"),
        ] {
            let fit = fit_regression(
                kind,
                response.clone(),
                vec![predictor.clone()],
                None,
                regression_metadata(response.len()),
            )
            .unwrap();
            let report = regression_report(&fit).expect("binary report must serialize");
            let basic = report["model_basic_info"]
                .as_object()
                .expect("binary report must keep the canonical model info object");

            for field in [
                "num_observation",
                "pseudo_r2",
                "adjusted_pseudo_r2",
                "lr_chi2",
                "prob_lr_chi2",
                "df_model",
                "df_residual",
            ] {
                assert!(
                    basic[field].as_f64().is_some_and(f64::is_finite),
                    "canonical model info field {field} must be finite"
                );
            }
            for field in ["model_type", "method", "covariance_type"] {
                assert!(basic[field].as_str().is_some(), "missing {field}");
            }
            assert!(
                report["diagnostic_info"]["cond_no"]
                    .as_f64()
                    .is_some_and(f64::is_finite),
                "binary report must expose a real condition number"
            );
            assert_ne!(basic["pseudo_r2"], serde_json::json!(0.0));
            assert_ne!(basic["lr_chi2"], serde_json::json!(0.0));
            assert_ne!(basic["prob_lr_chi2"], serde_json::json!(1.0));
            assert!(!basic.contains_key("f_statistic"));
            assert!(!basic.contains_key("wald_chi2"));
            assert!(!basic.contains_key("r_squared"));

            for field in ["log_likelihood", "lr_chi2", "prob_lr_chi2", "aic", "bic"] {
                assert!(basic[field].as_f64().is_some(), "missing {field}");
            }

            let statistics = report["model_statistics"]
                .as_object()
                .expect("binary report must expose structured model statistics");
            assert_eq!(statistics["kind"], "binary");
            assert_eq!(statistics["link"], expected_link);
            assert_eq!(statistics["pseudoR2"], basic["pseudo_r2"]);
            assert_eq!(statistics["logLikelihood"], basic["log_likelihood"]);
            assert_eq!(statistics["lrChi2"], basic["lr_chi2"]);
            assert_eq!(statistics["lrPValue"], basic["prob_lr_chi2"]);
            assert_eq!(statistics["aic"], basic["aic"]);
            assert_eq!(statistics["bic"], basic["bic"]);
            assert!(statistics["iterations"].as_u64().unwrap() > 0);
            assert_eq!(statistics["converged"], true);
            let decoded: RegressionStatistics =
                serde_json::from_value(report["model_statistics"].clone()).unwrap();
            assert_eq!(decoded, fit.statistics);
        }
    }

    #[test]
    fn prais_regression_report_preserves_autocorrelation_statistics() {
        let response = vec![1.0, 1.8, 2.7, 3.9, 5.4, 6.8, 8.5, 10.1];
        let predictor = vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0];
        let fit = fit_regression(
            RegressionKind::Prais,
            response.clone(),
            vec![predictor],
            None,
            regression_metadata(response.len()),
        )
        .unwrap();

        let report = regression_report(&fit).expect("Prais report must serialize");
        let prais = report["diagnostic_info"]["prais_info"]
            .as_object()
            .expect("Prais report must expose structured autocorrelation statistics");

        assert!(prais["rho"].as_f64().is_some());
        assert!(prais["dw_original"].as_f64().is_some());
        assert!(prais["dw_transformed"].as_f64().is_some());
        assert!(prais["iterations"].as_u64().unwrap() > 0);
    }

    #[test]
    fn augmented_dickey_fuller_rejects_unknown_regression() {
        let series = [1.0, 1.4, 1.1, 1.8, 1.5, 2.2, 1.9, 2.6, 2.3, 3.0, 2.7, 3.4];

        let error = augmented_dickey_fuller(&series, 1, "unexpected").unwrap_err();

        assert_eq!(
            error,
            SciError::InvalidInput {
                operation: SciOperationCode::Adf,
                violation: SciInputViolation::ParameterOutOfRange,
            }
        );
    }
}
