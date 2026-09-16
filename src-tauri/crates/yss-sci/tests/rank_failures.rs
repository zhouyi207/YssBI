use yss_sci::regression::linear_model::{GLS, GLSConfig, OLS, WLS, WLSConfig};
use yss_sci_linalg::{Col, Mat};

#[test]
fn models_reject_failed_rank_zero_rank_and_exhausted_degrees_of_freedom() {
    for exog in [
        Mat::full(3, 1, f64::NAN),
        Mat::zeros(3, 1),
        Mat::identity(3, 3),
    ] {
        let endog = Col::from_fn(3, |i| i as f64);
        assert!(
            OLS {
                endog: endog.clone(),
                exog: exog.clone(),
                config: Default::default(),
            }
            .fit()
            .is_err()
        );
        assert!(
            WLS {
                endog: endog.clone(),
                exog: exog.clone(),
                weights: Col::from_fn(3, |_| 1.0),
                config: WLSConfig {
                    constant: true,
                    cov_type: String::new(),
                    cov_params: None
                },
            }
            .fit()
            .is_err()
        );
        assert!(
            GLS {
                endog,
                exog,
                sigma: Mat::identity(3, 3),
                config: GLSConfig { constant: true },
            }
            .fit()
            .is_err()
        );
    }
}
