use ndarray::Array2;
use yss_sci::ts::var::{VAR, VARConfig};
use yss_sci::ts::vec::{VECConfig, VecTrendSpec, vec_estimate};

fn cointegrated_sample() -> Array2<f64> {
    let mut state = 17_u64;
    let mut noise = || {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        ((state >> 32) as f64 / u32::MAX as f64) - 0.5
    };
    let mut sample = Array2::zeros((160, 2));
    let mut level = 0.0;
    let mut spread = 0.0;
    for row in 0..160 {
        level += noise();
        spread = 0.4 * spread + noise();
        sample[(row, 0)] = level;
        sample[(row, 1)] = 0.7 * level + spread;
    }
    sample
}

fn assert_values(actual: impl IntoIterator<Item = f64>, expected: &[f64]) {
    let actual: Vec<f64> = actual.into_iter().collect();
    assert_eq!(actual.len(), expected.len());
    for (actual, &expected) in actual.iter().zip(expected) {
        assert!(
            (actual - expected).abs() < 1e-8 * expected.abs().max(1.0),
            "{actual} != {expected}"
        );
    }
}

#[test]
fn var_full_fit_preserves_coefficients_and_stability_spectrum() {
    let result = VAR {
        y: cointegrated_sample(),
        exog: None,
        config: VARConfig {
            step: 4,
            ..VARConfig::default()
        },
        var_names: None,
        exog_names: None,
        regression_times: None,
    }
    .fit()
    .unwrap();
    assert_values(
        result.coefficients.into_iter().flatten(),
        &[
            0.9850730986309457,
            0.04725331784187457,
            0.016636645662576083,
            -0.13216959661340977,
            -0.17970042689362095,
            0.43244540139398946,
            0.07634480458041656,
            0.32044441825045616,
            -0.09401853771725986,
            -0.13862499978115922,
        ],
    );
    assert_values([result.log_likelihood], &[-62.961109403429724]);
    assert_values(
        result.oirf[4].iter().flatten().copied(),
        &[
            0.2592323636190237,
            -0.04572394194333182,
            0.18213900464111,
            -0.028585877762558366,
        ],
    );
    let mut roots: Vec<_> = result
        .varstable
        .iter()
        .map(|root| (root.re, root.im))
        .collect();
    roots.sort_by(|a, b| a.partial_cmp(b).unwrap());
    assert_values(
        roots.into_iter().flat_map(|(re, im)| [re, im]),
        &[
            -0.0002336376356122259,
            -0.12903735724777535,
            -0.0002336376356122259,
            0.12903735724777535,
            0.35768314444575455,
            0.,
            0.9483016477068706,
            0.,
        ],
    );
}

#[test]
fn vec_fit_preserves_cointegration_and_short_run_estimates() {
    let result = vec_estimate(
        &cointegrated_sample(),
        &VECConfig {
            trend_spec: VecTrendSpec::Constant,
            lags: 2,
            rank: 1,
            mlag: 2,
        },
        None,
        None,
    )
    .unwrap();
    assert_values(
        result.beta.into_iter().flatten(),
        &[1., -1.4570479553381204, -0.12642073895613215],
    );
    assert_values(
        result.coefficients.into_iter().flatten(),
        &[
            0.08528514459386069,
            -0.05143280849517221,
            0.13366687702222804,
            -0.013375004133533644,
            0.5337422849298061,
            -0.07831401691752685,
            0.09472399761602275,
            0.002137153442175969,
        ],
    );
    assert_values([result.log_likelihood], &[-66.54617133774867]);
    let mut roots: Vec<_> = result
        .vecstable
        .iter()
        .map(|root| (root.re, root.im))
        .collect();
    roots.sort_by(|a, b| a.partial_cmp(b).unwrap());
    assert_values(
        roots.into_iter().flat_map(|(re, im)| [re, im]),
        &[
            -7.456253562724755e-5,
            -0.12625970849908247,
            -7.456253562724755e-5,
            0.12625970849908247,
            0.3510373538515096,
            0.,
            1.,
            0.,
        ],
    );
}
