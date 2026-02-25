use ndarray::{Array1, Axis};
use statrs::distribution::{ChiSquared, ContinuousCDF};

use crate::tools::skewness_kurtosis;

// DW ≈ 2 → 无自相关
// DW < 2 → 正自相关
// DW > 2 → 负自相关
// 值域：[0, 4]
// 仅适用于时序性数据
pub fn durbin_waston(resid: &Array1<f64>) -> f64 {
    let diff = resid.diff(1, Axis(0)).pow2().sum();
    let num = resid.pow2().sum();
    return diff / num;
}

// 残差的正态性检验
// 小样本表现更好，统计上更“讲究”
// 统计严谨 / 学术分析
pub fn omnibus(resid: &Array1<f64>) -> (f64, f64) {
    let n = resid.len() as f64;
    let (s, k) = skewness_kurtosis(resid);

    let z1 = s * ((n * (n - 1.0)).sqrt() / (n - 2.0));
    let z2 = (k - 3.0) * ((n - 1.0) / (24.0 * n)).sqrt();

    let omni = z1 * z1 + z2 * z2;
    let chi2 = ChiSquared::new(2.0).unwrap();
    let p_value = 1.0 - chi2.cdf(omni);

    (omni, p_value)
}

// 残差的正态性检验
// 大样本渐近结果；简单、直观、但近似比较粗
// 经济计量 / 金融工程
pub fn jarque_bera(resid: &Array1<f64>) -> (f64, f64) {
    let n = resid.len() as f64;
    let (s, k) = skewness_kurtosis(resid);

    let jb = n / 6.0 * (s * s + (k - 3.0).powi(2) / 4.0);

    let chi2 = ChiSquared::new(2.0).unwrap();
    let p_value = 1.0 - chi2.cdf(jb);

    (jb, p_value)
}
