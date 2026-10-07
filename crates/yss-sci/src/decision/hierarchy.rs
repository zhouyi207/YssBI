//! Single-level reciprocal and fuzzy-complementary judgment matrices.
use super::data::*;
use crate::regression::models::common::failed;
use yss_sci_contract::decision::hierarchy::*;
use yss_sci_linalg::{Eigen, Mat};

pub fn ahp(columns: &[Vec<f64>], random_index: f64, control: &Control) -> Result<AhpResult> {
    let n = square(columns, control)?;
    if !random_index.is_finite() || random_index < 0. {
        return Err(parameter());
    }
    for (i, col_i) in columns.iter().enumerate() {
        control.check()?;
        for (j, &b) in col_i.iter().enumerate() {
            let a = columns[j][i];
            if a <= 0. || b <= 0. || (a.ln() + b.ln()).abs() > 1e-8 {
                return Err(parameter());
            }
        }
    }
    let scale = columns.iter().flatten().copied().fold(0., f64::max);
    let matrix = Mat::from_fn(n, n, |i, j| columns[j][i] / scale);
    let eigen = Eigen::factor(matrix.as_ref()).map_err(|_| failed())?;
    control.check()?;
    let index = (0..n)
        .max_by(|&i, &j| eigen.values()[i].re.total_cmp(&eigen.values()[j].re))
        .unwrap();
    let eigenvalue = eigen.values()[index];
    if eigenvalue.im.abs() > 1e-8 * eigenvalue.re.abs() || eigenvalue.re <= 0. {
        return Err(parameter());
    }
    let lambda = finite(eigenvalue.re * scale)?;
    if lambda < n as f64 * (1. - 1e-8) {
        return Err(parameter());
    }
    let vectors = eigen.vectors();
    let sign = vectors[(0, index)].re.signum();
    let raw = (0..n)
        .map(|i| {
            let v = vectors[(i, index)];
            if v.im.abs() > 1e-8 || v.re * sign <= 0. {
                Err(parameter())
            } else {
                Ok(v.re * sign)
            }
        })
        .collect::<Result<Vec<_>>>()?;
    let weights = normalized_weights(&raw)?;
    let ci = if n > 1 {
        ((lambda - n as f64) / (n - 1) as f64).max(0.)
    } else {
        0.
    };
    // Tummala & Ling (1998), also used by PyMCDM. Larger matrices still produce weights and CI.
    const RI: [f64; 15] = [
        0., 0., 0.5799, 0.8921, 1.1159, 1.2358, 1.3322, 1.3952, 1.4537, 1.4882, 1.5117, 1.5356,
        1.5571, 1.5714, 1.5831,
    ];
    let (ri, source) = if random_index > 0. {
        (Some(random_index), "supplied")
    } else {
        (
            RI.get(n - 1).copied().filter(|v| *v > 0.),
            "tummala_ling_1998",
        )
    };
    let cr = ri.map(|ri| finite(ci / ri)).transpose()?;
    Ok(AhpResult {
        criteria: n,
        weights,
        principal_eigenvalue: lambda,
        consistency_index: ci,
        random_index: ri,
        random_index_source: source,
        consistency_ratio: cr,
        consistency_satisfied: cr.map(|v| v <= 0.1),
    })
}

pub fn fuzzy_ahp(columns: &[Vec<f64>], control: &Control) -> Result<FuzzyAhpResult> {
    let n = square(columns, control)?;
    for (i, col_i) in columns.iter().enumerate() {
        control.check()?;
        for (j, &b) in col_i.iter().enumerate() {
            if !(0.0..=1.0).contains(&columns[j][i]) || (columns[j][i] + b - 1.).abs() > 1e-8 {
                return Err(parameter());
            }
        }
    }
    let nf = n as f64;
    let weights = if n == 1 {
        vec![1.]
    } else {
        normalized_weights(
            &(0..n)
                .map(|i| {
                    (columns.iter().map(|x| x[i]).sum::<f64>() + nf / 2. - 1.) / (nf * (nf - 1.))
                })
                .collect::<Vec<_>>(),
        )?
    };
    let mut compatibility = 0.;
    for i in 0..n {
        control.check()?;
        for j in 0..n {
            compatibility +=
                (columns[j][i] - weights[i] / (weights[i] + weights[j])).abs() / nf / nf;
        }
    }
    Ok(FuzzyAhpResult {
        criteria: n,
        weights,
        compatibility_index: finite(compatibility)?,
        consistency_satisfied: compatibility <= 0.1,
    })
}
