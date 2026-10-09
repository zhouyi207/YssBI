use statrs::distribution::{ChiSquared, ContinuousCDF};

pub(crate) fn chi_squared_sf(df: f64, statistic: f64) -> f64 {
    if df <= 0.0 || !df.is_finite() || !statistic.is_finite() {
        return f64::NAN;
    }
    ChiSquared::new(df)
        .map(|dist| dist.sf(statistic))
        .unwrap_or(f64::NAN)
}

pub(crate) fn normal_two_sided_p(z_value: f64) -> f64 {
    if !z_value.is_finite() {
        return f64::NAN;
    }
    crate::distribution::normal_two_sided_p(z_value)
}
