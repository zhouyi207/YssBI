use yss_sci::regression::linear_model::{GLS, GLSConfig, WLS, WLSConfig};
use yss_sci_linalg::{Col, Mat};

fn near(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 1e-8, "{actual} != {expected}");
}

#[test]
fn weighted_statistics_use_the_transformed_intercept() {
    let weights = Col::from_fn(5, |i| (1 << i) as f64);
    for constant in [true, false] {
        let exog = Mat::from_fn(5, if constant { 2 } else { 1 }, |i, j| {
            if constant && j == 0 { 1.0 } else { i as f64 }
        });
        for offset in [0.0, 100.0] {
            let endog = Col::from_fn(5, |i| [1., 2., 1., 4., 3.][i] + offset);
            let wls = WLS {
                endog: endog.clone(),
                exog: exog.clone(),
                weights: weights.clone(),
                config: WLSConfig {
                    constant,
                    cov_type: String::new(),
                    cov_params: None,
                },
            }
            .fit()
            .unwrap();
            let gls = GLS {
                endog: endog.clone(),
                exog: exog.clone(),
                sigma: Mat::from_fn(5, 5, |i, j| if i == j { 1.0 / weights[i] } else { 0.0 }),
                config: GLSConfig { constant },
            }
            .fit()
            .unwrap();
            near(wls.ss_total, gls.ss_total);
            near(wls.r2, gls.r2);
            near(wls.fvalue, gls.fvalue);
            if constant {
                // Exact weighted least-squares values derived using rational arithmetic.
                near(wls.ss_total, 914.0 / 31.0);
                near(wls.r2, 4913.0 / 15081.0);
                near(wls.fvalue, 14739.0 / 10168.0);
            } else {
                near(
                    wls.ss_total,
                    (0..5).map(|i| weights[i] * endog[i].powi(2)).sum(),
                );
            }
        }
    }
}

#[test]
fn correlated_gls_centers_against_the_whitened_constant() {
    // Sigma = L L', with L[0,0]=2, L[1,0]=1 and other diagonal entries 1.
    let sigma = Mat::from_fn(5, 5, |i, j| match (i, j) {
        (0, 0) => 4.0,
        (1, 1) | (0, 1) | (1, 0) => 2.0,
        _ if i == j => 1.0,
        _ => 0.0,
    });
    // L^-1 * 1 = [.5,.5,1,1,1]; L^-1 * y = [.5,1.5,1,4,3].
    let c = [0.5, 0.5, 1.0, 1.0, 1.0];
    let z = [0.5, 1.5, 1.0, 4.0, 3.0];
    let mean: f64 = 9.0 / 3.5;
    let expected = (0..5).map(|i| (z[i] - mean * c[i]).powi(2)).sum();
    for offset in [0.0, 100.0] {
        let result = GLS {
            endog: Col::from_fn(5, |i| [1., 2., 1., 4., 3.][i] + offset),
            exog: Mat::from_fn(5, 2, |i, j| if j == 0 { 1.0 } else { i as f64 }),
            sigma: sigma.clone(),
            config: GLSConfig { constant: true },
        }
        .fit()
        .unwrap();
        near(result.ss_total, expected);
    }
}
