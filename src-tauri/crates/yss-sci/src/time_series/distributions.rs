use statrs::distribution::{ChiSquared, ContinuousCDF, Normal};

pub(crate) fn chi_squared_sf(df: f64, statistic: f64) -> f64 {
    if df <= 0.0 || !df.is_finite() || !statistic.is_finite() {
        return f64::NAN;
    }
    ChiSquared::new(df)
        .map(|dist| 1.0 - dist.cdf(statistic))
        .unwrap_or(f64::NAN)
}

pub(crate) fn normal_cdf(value: f64) -> f64 {
    if !value.is_finite() {
        return f64::NAN;
    }
    Normal::new(0.0, 1.0)
        .map(|dist| dist.cdf(value))
        .unwrap_or(f64::NAN)
}

pub(crate) fn normal_two_sided_p(z_value: f64) -> f64 {
    2.0 * (1.0 - normal_cdf(z_value.abs()))
}
