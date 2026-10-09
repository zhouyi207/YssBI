use std::time::{Duration, Instant};
use yss_sci::{anova::anova, regression::linear::OLS};
use yss_sci_contract::{
    anova::{AnovaOptions, Factor},
    execution::{ScientificCancellationToken, ScientificExecutionControl},
    regression::OlsOptions,
};
use yss_sci_linalg::{Col, Mat};

#[test]
fn factor_and_linear_tests_retain_representable_f_tails() {
    let response = vec![
        1.,
        2.,
        3.,
        1e10 + 1.,
        1e10 + 2.,
        1e10 + 3.,
        2e10 + 1.,
        2e10 + 2.,
        2e10 + 3.,
    ];
    let control = ScientificExecutionControl {
        cancellation: ScientificCancellationToken::new(),
        deadline: Instant::now() + Duration::from_secs(30),
    };
    let factor = Factor {
        values: (0..9).map(|i| i / 3).collect(),
        levels: 3,
    };
    let factor_fit = anova(&response, &[factor], &[], AnovaOptions::default(), &control).unwrap();
    let linear_fit = OLS {
        endog: response.into_iter().collect::<Col<f64>>(),
        exog: Mat::from_fn(9, 3, |row, column| {
            f64::from(column == 0 || row / 3 == column)
        }),
        config: OlsOptions::default(),
    }
    .fit()
    .unwrap();
    assert_eq!((factor_fit.table[0].df, factor_fit.error.df), (2, 6));
    assert_eq!((linear_fit.df_model, linear_fit.df_residual), (2, 6));
    let reports = [
        (
            "anova",
            factor_fit.table[0].f_statistic,
            factor_fit.table[0].p_value,
        ),
        ("linear", linear_fit.fvalue, linear_fit.f_p_value),
    ];
    for (method, statistic, p) in reports {
        assert!(
            (statistic / 3e20 - 1.0).abs() < 1e-4,
            "{method}: {statistic}"
        );
        // F(2, 6) has upper tail (3 / (3 + F))^3.
        let expected = (3.0 / (3.0 + statistic)).powi(3);
        assert!(
            (p / expected - 1.0).abs() < 1e-12,
            "representable F tails were lost: {reports:?}"
        );
    }
}
