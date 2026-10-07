use super::*;
use rand::{SeedableRng, rngs::StdRng, seq::SliceRandom};
use statrs::distribution::{ContinuousCDF, Normal};
use yss_sci_contract::spatial::*;

fn centered(y: &[f64]) -> Result<Vec<f64>> {
    if y.is_empty() || y.iter().all(|v| *v == y[0]) {
        return Err(parameter());
    }
    let mean = y.iter().map(|v| v / y.len() as f64).sum::<f64>();
    let scale = y.iter().map(|v| (v - mean).abs()).fold(0.0, f64::max);
    if !mean.is_finite() || !scale.is_finite() || scale == 0.0 {
        return Err(parameter());
    }
    Ok(y.iter().map(|v| (v - mean) / scale).collect())
}
fn index(z: &[f64], w: &[Vec<f64>], factor: f64, control: &Control) -> Result<f64> {
    let wz = lag(w, z, control)?;
    finite(factor * z.iter().zip(wz).map(|(a, b)| a * b).sum::<f64>())
}
pub(super) fn descriptive(y: &[f64], w: &[Vec<f64>], control: &Control) -> Result<Option<f64>> {
    let z = match centered(y) {
        Ok(z) => z,
        Err(_) => return Ok(None),
    };
    let s0 = w.iter().flatten().sum::<f64>();
    Ok(Some(index(
        &z,
        w,
        y.len() as f64 / s0 / z.iter().map(|v| v * v).sum::<f64>(),
        control,
    )?))
}
pub fn analyze(
    y: &[f64],
    w: &[Vec<f64>],
    options: MoranOptions,
    control: &Control,
) -> Result<MoranResult> {
    crate::regression::models::common::validate(y, &[], control)?;
    weights::validate(w, control)?;
    if y.len() != w.len() {
        return Err(invalid(Violation::ShapeMismatch));
    }
    let n = y.len() as f64;
    let mut z = centered(y)?;
    let ss = z.iter().map(|v| v * v).sum::<f64>();
    let s0 = finite(w.iter().flatten().sum())?;
    let mut s1 = 0.0;
    let mut s2 = 0.0;
    for (i, row) in w.iter().enumerate() {
        control.check()?;
        let mut row_col = 0.0;
        for (value, other) in row.iter().zip(w) {
            let sum = value + other[i];
            s1 += sum.powi(2) / 2.0;
            row_col += sum;
        }
        s2 += row_col * row_col;
    }
    let factor = n / s0 / ss;
    let observed = index(&z, w, factor, control)?;
    let expected = -1.0 / (n - 1.0);
    let variance =
        (n * n * s1 - n * s2 + 3.0 * s0 * s0) / ((n * n - 1.0) * s0 * s0) - expected * expected;
    let random_variance = if y.len() > 3 {
        let kurtosis = n * z.iter().map(|v| v.powi(4)).sum::<f64>() / (ss * ss);
        Some(
            (n * ((n * n - 3.0 * n + 3.0) * s1 - n * s2 + 3.0 * s0 * s0)
                - kurtosis * ((n * n - n) * s1 - 2.0 * n * s2 + 6.0 * s0 * s0))
                / ((n - 1.0) * (n - 2.0) * (n - 3.0) * s0 * s0)
                - expected * expected,
        )
    } else {
        None
    };
    let normal = Normal::new(0.0, 1.0).expect("normal");
    let infer = |v: Option<f64>| {
        let v = v.filter(|v| v.is_finite() && *v > 1e-14);
        let z = v.map(|v| (observed - expected) / v.sqrt());
        (v, z, z.map(|z| (2.0 * normal.sf(z.abs())).clamp(0.0, 1.0)))
    };
    let (normal_variance, normal_z, normal_p_value) = infer(Some(variance));
    let (randomization_variance, randomization_z, randomization_p_value) = infer(random_variance);
    let permutation_p_value = if options.permutations > 0 {
        let mut rng = StdRng::seed_from_u64(options.seed);
        let mut extreme = 0usize;
        for _ in 0..options.permutations {
            control.check()?;
            z.shuffle(&mut rng);
            if (index(&z, w, factor, control)? - expected).abs() + 1e-14
                >= (observed - expected).abs()
            {
                extreme += 1;
            }
        }
        Some((extreme as f64 + 1.0) / (options.permutations as f64 + 1.0))
    } else {
        None
    };
    Ok(MoranResult {
        observations: y.len(),
        statistic: observed,
        expected,
        normal_variance,
        normal_z,
        normal_p_value,
        randomization_variance,
        randomization_z,
        randomization_p_value,
        permutations: options.permutations,
        seed: options.seed,
        permutation_p_value,
    })
}
