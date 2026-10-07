use serde::Deserialize;
use std::time::{Duration, Instant};
use yss_sci::panel::{difference_gmm, fisher_cointegration, fisher_unit_root};
use yss_sci_contract::{execution::*, panel::*};

#[derive(Clone, Deserialize)]
struct Data {
    response: Vec<f64>,
    predictors: Vec<Vec<f64>>,
    entity: Vec<f64>,
    time: Vec<f64>,
}
impl Data {
    fn input(&self) -> PanelData<'_> {
        PanelData {
            response: &self.response,
            predictors: &self.predictors,
            entity: &self.entity,
            time: &self.time,
        }
    }
}
fn fixture() -> serde_json::Value {
    serde_json::from_str(include_str!("fixtures/panel_category_reference.json")).unwrap()
}
fn control() -> ScientificExecutionControl {
    ScientificExecutionControl {
        cancellation: ScientificCancellationToken::new(),
        deadline: Instant::now() + Duration::from_secs(30),
    }
}
fn close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= 1e-7 * (1.0 + expected.abs()),
        "{actual} != {expected}"
    );
}

#[test]
fn panel_fisher_tests_match_statsmodels_reference_distributions() {
    let fixture = fixture();
    let data: Data = serde_json::from_value(fixture["nonstationary"].clone()).unwrap();
    for case in fixture["nonstationary"]["cases"].as_array().unwrap() {
        let width = case["predictors"].as_u64().unwrap() as usize;
        let input = PanelData {
            predictors: &data.predictors[..width],
            ..data.input()
        };
        let options = PanelTestOptions {
            lags: case["lags"].as_u64().unwrap() as usize,
            deterministic: serde_json::from_value(case["regression"].clone()).unwrap(),
        };
        let result = if case["method"] == "unit_root" {
            fisher_unit_root(input, options, &control())
        } else {
            fisher_cointegration(input, options, &control())
        }
        .unwrap();
        assert_eq!(result.entity_tests.len(), 3);
        assert_eq!(result.degrees_of_freedom, 6);
        close(
            result.statistic.unwrap(),
            case["statistic"].as_f64().unwrap(),
        );
        close(result.p_value, case["p_value"].as_f64().unwrap());
        for (actual, expected) in result
            .entity_tests
            .iter()
            .zip(case["entity_tests"].as_array().unwrap())
        {
            close(actual.statistic, expected["statistic"].as_f64().unwrap());
            close(actual.p_value, expected["p_value"].as_f64().unwrap());
            assert_eq!(
                actual.observations,
                expected["observations"].as_u64().unwrap() as usize
            );
        }
        serde_json::to_string(&result).unwrap();
    }
    // A zero individual tail must remain a successful, serializable infinite-limit test.
    let response = (0..160)
        .map(|i| {
            if i % 2 == 0 {
                1.0 + (i as f64).sin() * 0.01
            } else {
                -1.0 + (i as f64).sin() * 0.01
            }
        })
        .collect::<Vec<_>>();
    let entity = (0..160).map(|i| (i / 80) as f64).collect::<Vec<_>>();
    let time = (0..160).map(|i| (i % 80) as f64).collect::<Vec<_>>();
    let result = fisher_unit_root(
        PanelData {
            response: &response,
            predictors: &[],
            entity: &entity,
            time: &time,
        },
        PanelTestOptions {
            lags: 0,
            deterministic: PanelDeterministic::Constant,
        },
        &control(),
    )
    .unwrap();
    assert_eq!(result.statistic, None);
    assert_eq!(result.p_value, 0.0);
    assert!(serde_json::to_value(result).unwrap()["statistic"].is_null());
}

#[test]
fn panel_difference_gmm_matches_independent_entity_moment_fit_and_source_rows() {
    let fixture = fixture();
    let reference = &fixture["dynamic"];
    let mut data: Data = serde_json::from_value(reference.clone()).unwrap();
    // The computation must sort keys while preserving the original row mapping.
    data.response.reverse();
    data.entity.reverse();
    data.time.reverse();
    data.predictors
        .iter_mut()
        .for_each(|column| column.reverse());
    for robust in [false, true] {
        let result = difference_gmm(
            data.input(),
            DynamicPanelOptions {
                max_instrument_lag: 3,
                robust,
            },
            &control(),
        )
        .unwrap();
        assert_eq!(result.instruments, 3);
        assert_eq!(result.observations, 120);
        let covariance = if robust {
            "covariance"
        } else {
            "nonrobust_covariance"
        };
        for (j, beta) in result.coefficients.iter().enumerate() {
            close(*beta, reference["coefficients"][j].as_f64().unwrap());
            for (k, actual) in result.inference.covariance[j].iter().enumerate() {
                close(*actual, reference[covariance][j][k].as_f64().unwrap());
            }
            if robust {
                close(
                    result.inference.p_values[j],
                    reference["p_values"][j].as_f64().unwrap(),
                );
            }
        }
        for i in 0..result.observations {
            close(result.fitted[i], reference["fitted"][i].as_f64().unwrap());
            close(
                result.residuals[i],
                reference["residuals"][i].as_f64().unwrap(),
            );
            assert_eq!(
                result.source_rows[i],
                data.response.len() - 1 - (i / 5 * 7 + i % 5 + 2)
            );
        }
    }
}

#[test]
fn panel_lagged_analyses_reject_invalid_samples_and_honor_execution_control() {
    let fixture = fixture();
    let mut data: Data = serde_json::from_value(fixture["dynamic"].clone()).unwrap();
    let options = DynamicPanelOptions {
        max_instrument_lag: 3,
        robust: true,
    };
    let rejected =
        |data: &Data| assert!(difference_gmm(data.input(), options, &control()).is_err());
    data.time[1] = data.time[0];
    rejected(&data);
    data.time[1] = 1.5;
    rejected(&data);
    data.time[1] = 9.0;
    rejected(&data);
    data.time[1] = 1.0;
    data.response[0] = f64::NAN;
    rejected(&data);
    data = serde_json::from_value(fixture["dynamic"].clone()).unwrap();
    assert!(
        difference_gmm(
            data.input(),
            DynamicPanelOptions {
                max_instrument_lag: usize::MAX,
                robust: true
            },
            &control()
        )
        .is_err()
    );
    let test_options = PanelTestOptions {
        lags: 1,
        deterministic: PanelDeterministic::Constant,
    };
    let cancelled = control();
    cancelled.cancellation.cancel();
    assert_eq!(
        difference_gmm(data.input(), options, &cancelled).unwrap_err(),
        ScientificComputationError::Cancelled
    );
    assert_eq!(
        fisher_cointegration(data.input(), test_options, &cancelled).unwrap_err(),
        ScientificComputationError::Cancelled
    );
    let mut expired = control();
    expired.deadline = Instant::now() - Duration::from_secs(1);
    assert_eq!(
        fisher_unit_root(
            PanelData {
                predictors: &[],
                ..data.input()
            },
            test_options,
            &expired
        )
        .unwrap_err(),
        ScientificComputationError::DeadlineExceeded
    );
    data.predictors[0].fill(1.0);
    rejected(&data);
    assert!(fisher_cointegration(data.input(), test_options, &control()).is_err());
    data.response.truncate(3);
    rejected(&data);
}

#[test]
fn scale_limits_panel_uses_sample_dependent_lags_and_dynamic_width() {
    let mut state = 83u64;
    let mut draw = || {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        (state >> 32) as f64 / u32::MAX as f64 - 0.5
    };
    let n = 50 * 40;
    let data = Data {
        response: (0..n).map(|_| draw()).collect(),
        predictors: (0..17).map(|_| (0..n).map(|_| draw()).collect()).collect(),
        entity: (0..n).map(|i| (i / 40) as f64).collect(),
        time: (0..n).map(|i| (i % 40) as f64).collect(),
    };
    assert!(
        difference_gmm(
            data.input(),
            DynamicPanelOptions {
                max_instrument_lag: 33,
                robust: true
            },
            &control()
        )
        .is_ok()
    );
    let data = Data {
        response: (0..360).map(|_| draw()).collect(),
        predictors: vec![],
        entity: (0..360).map(|i| (i / 120) as f64).collect(),
        time: (0..360).map(|i| (i % 120) as f64).collect(),
    };
    let result = fisher_unit_root(
        data.input(),
        PanelTestOptions {
            lags: 21,
            deterministic: PanelDeterministic::Constant,
        },
        &control(),
    )
    .unwrap();
    assert_eq!(result.entity_tests.len(), 3);
    assert!(result.p_value.is_finite());
    assert!(
        fisher_unit_root(
            data.input(),
            PanelTestOptions {
                lags: usize::MAX,
                deterministic: PanelDeterministic::Constant,
            },
            &control()
        )
        .is_err()
    );
}
