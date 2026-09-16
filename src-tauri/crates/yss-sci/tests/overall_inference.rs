use yss_sci::regression::linear_model::{OLS, OlsFitError, WLS, WLSConfig};
use yss_sci_contract::regression::{OlsCovariance, OlsOptions};
use yss_sci_linalg::{Col, Mat};

#[test]
fn named_covariance_rejects_unknown_or_incomplete_configuration() {
    use yss_sci::regression::covariance::compute_cov_beta;
    let x = Mat::from_fn(3, 1, |_, _| 1.0);
    let inverse = Mat::from_fn(1, 1, |_, _| 1.0 / 3.0);
    let residuals = Col::from_fn(3, |i| i as f64 - 1.0);
    for name in ["HC4", "nonrobuts", "cluster", "fixed scale"] {
        assert!(compute_cov_beta(&x, &inverse, &residuals, 2, name, None).is_err());
    }
    assert!(compute_cov_beta(&x, &inverse, &residuals, 2, "nonrobust", None).is_ok());
}

#[test]
fn singular_robust_covariance_is_an_inference_error() {
    let exog = Mat::from_fn(5, 2, |i, j| if j == 0 { 1.0 } else { i as f64 });
    let ols = OLS {
        endog: Col::zeros(5),
        exog: exog.clone(),
        config: OlsOptions {
            constant: true,
            covariance: OlsCovariance::Hc1,
        },
    };
    assert!(matches!(ols.fit(), Err(OlsFitError::Inference(_))));
    let wls = WLS {
        endog: Col::zeros(5),
        exog,
        weights: Col::from_fn(5, |_| 1.0),
        config: WLSConfig {
            constant: true,
            covariance: OlsCovariance::Hc1,
        },
    };
    assert!(wls.fit().unwrap_err().contains("Wald test covariance"));
}

#[test]
fn robust_wls_joint_test_matches_single_slope_t_squared() {
    let wls = WLS {
        endog: Col::from_fn(5, |i| [1., 2., 1., 4., 3.][i]),
        exog: Mat::from_fn(5, 2, |i, j| if j == 0 { 1.0 } else { i as f64 }),
        weights: Col::from_fn(5, |i| (1 << i) as f64),
        config: WLSConfig {
            constant: true,
            covariance: OlsCovariance::Hc1,
        },
    }
    .fit()
    .unwrap();
    assert!((wls.fvalue - wls.tvalues[1].powi(2)).abs() < 1e-10);
    assert!((wls.f_p_value - wls.pvalues[1]).abs() < 1e-10);
    assert!((wls.fvalue - 3.750197).abs() < 1e-6);
    assert!((wls.f_p_value - 0.148213).abs() < 1e-6);
}
