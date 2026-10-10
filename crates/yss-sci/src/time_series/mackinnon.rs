//! MacKinnon (1994) tau response-surface coefficients, N = 1..6.
//! Numerical tables: statsmodels 0.14.6, statsmodels/tsa/adfvalues.py.
//! https://www.statsmodels.org/stable/generated/statsmodels.tsa.stattools.coint.html
use yss_sci_contract::time_series::forecast::Deterministic;

pub(crate) fn p_value(statistic: f64, deterministic: Deterministic, series: usize) -> f64 {
    use Deterministic::*;
    let (max, min, split, small, large) = match (deterministic, series) {
        (None, 1) => (
            f64::INFINITY,
            -19.04,
            -1.04,
            [0.6344, 1.2378, 0.032496000000000004],
            [0.4797, 0.9355700000000001, -0.06999, 0.033066],
        ),
        (Constant, 1) => (
            2.74,
            -18.83,
            -1.61,
            [2.1659, 1.4412, 0.038269000000000004],
            [1.7339, 0.9320200000000001, -0.12745, -0.010368],
        ),
        (Constant, 2) => (
            0.92,
            -18.86,
            -2.62,
            [2.92, 1.5012, 0.039796],
            [2.1945, 0.64695, -0.29198, -0.042377000000000005],
        ),
        (Constant, 3) => (
            0.55,
            -23.48,
            -3.13,
            [3.4699, 1.4856, 0.03164],
            [2.5893, 0.45168, -0.36529, -0.050074],
        ),
        (Constant, 4) => (
            0.61,
            -28.07,
            -3.47,
            [3.9673, 1.4777, 0.026315],
            [3.0387, 0.45452000000000004, -0.33666, -0.041921],
        ),
        (Constant, 5) => (
            0.79,
            -25.96,
            -3.78,
            [4.5509, 1.5338, 0.029545],
            [3.5049, 0.5209800000000001, -0.29158, -0.033468],
        ),
        (Constant, 6) => (
            1.0,
            -23.27,
            -3.93,
            [5.1399, 1.6036, 0.034445],
            [3.9489, 0.58933, -0.25359, -0.02721],
        ),
        (Trend, 1) => (
            0.7,
            -16.18,
            -2.89,
            [3.2512, 1.6047, 0.049588],
            [2.5261, 0.6165400000000001, -0.37956, -0.060285000000000005],
        ),
        (Trend, 2) => (
            0.63,
            -21.15,
            -3.19,
            [3.6646, 1.5419, 0.036448],
            [2.85, 0.5272, -0.36622, -0.051695000000000005],
        ),
        (Trend, 3) => (
            0.71,
            -25.37,
            -3.5,
            [4.0983, 1.5173, 0.029897999999999997],
            [3.221, 0.5255, -0.32685000000000003, -0.041501],
        ),
        (Trend, 4) => (
            0.93,
            -26.63,
            -3.65,
            [4.5844, 1.5338, 0.028796],
            [3.652, 0.59758, -0.27483, -0.032081],
        ),
        (Trend, 5) => (
            1.19,
            -26.53,
            -3.8,
            [5.0722, 1.5634, 0.029472],
            [4.0712, 0.6642800000000001, -0.23464000000000002, -0.02546],
        ),
        (Trend, 6) => (
            1.42,
            -26.18,
            -4.36,
            [5.53, 1.5914, 0.030392000000000002],
            [4.4735, 0.71757, -0.20681, -0.021196000000000003],
        ),
        _ => unreachable!("validated deterministic terms and series count"),
    };
    if statistic > max {
        return 1.0;
    }
    if statistic < min {
        return 0.0;
    }
    let x = if statistic <= split {
        small[0] + statistic * (small[1] + statistic * small[2])
    } else {
        large[0] + statistic * (large[1] + statistic * (large[2] + statistic * large[3]))
    };
    crate::distribution::normal::cdf(x)
}
