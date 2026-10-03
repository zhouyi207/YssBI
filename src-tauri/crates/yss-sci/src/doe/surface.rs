//! Full quadratic OLS in range-coded factors, followed by canonical geometry.
use super::*;
use crate::regression::models::common::{failed, ols};
use yss_sci_contract::regression::models::RegressionCoefficient;
use yss_sci_linalg::{Mat, SymmetricEigen};

pub fn response_surface_parameter_count(factors: usize) -> Result<usize> {
    if factors == 0 {
        return Err(parameter());
    }
    factors
        .checked_add(1)
        .and_then(|a| factors.checked_add(2).and_then(|b| a.checked_mul(b)))
        .map(|n| n / 2)
        .ok_or_else(parameter)
}

pub fn response_surface(
    y: &[f64],
    factors: &[Vec<f64>],
    control: &Control,
) -> Result<ResponseSurfaceResult> {
    validate(y, factors, control)?;
    let p = factors.len();
    let k = response_surface_parameter_count(p)?;
    if y.len() <= k {
        return Err(parameter());
    }
    let mut centers = Vec::with_capacity(p);
    let mut half_ranges = Vec::with_capacity(p);
    let mut basis = Vec::with_capacity(k - 1);
    let mut names = vec!["intercept".to_string()];
    for (j, x) in factors.iter().enumerate() {
        control.check()?;
        let low = x.iter().copied().fold(f64::INFINITY, f64::min);
        let high = x.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        if low == high {
            return Err(parameter());
        }
        let scale = low.abs().max(high.abs());
        let center = (low / scale + high / scale) / 2.;
        let half = (high / scale - low / scale) / 2.;
        if half <= 0. {
            return Err(parameter());
        }
        centers.push(finite(center * scale)?);
        half_ranges.push(finite(half * scale)?);
        basis.push(
            x.iter()
                .map(|v| (v / scale - center) / half)
                .collect::<Vec<_>>(),
        );
        names.push(format!("z{}", j + 1));
    }
    for j in 0..p {
        control.check()?;
        basis.push(basis[j].iter().map(|z| z * z).collect());
        names.push(format!("z{}^2", j + 1));
    }
    for j in 0..p {
        for l in j + 1..p {
            control.check()?;
            basis.push(basis[j].iter().zip(&basis[l]).map(|(a, b)| a * b).collect());
            names.push(format!("z{}:z{}", j + 1, l + 1));
        }
    }
    let mut model = ols(y, &basis, true, control)?;
    model.method = "response_surface_full_quadratic".into();
    for (coefficient, name) in model.coefficients.iter_mut().zip(names) {
        coefficient.term = name;
    }
    let geometry = geometry(&model.coefficients, centers, half_ranges, y.len(), control)?;
    control.check()?;
    Ok(ResponseSurfaceResult { model, geometry })
}

fn geometry(
    coefficients: &[RegressionCoefficient],
    centers: Vec<f64>,
    half_ranges: Vec<f64>,
    observations: usize,
    control: &Control,
) -> Result<SurfaceGeometry> {
    let p = centers.len();
    let beta: Vec<_> = coefficients.iter().map(|c| c.estimate).collect();
    let mut h = Mat::zeros(p, p);
    for j in 0..p {
        h[(j, j)] = finite(2. * beta[1 + p + j])?;
    }
    let mut index = 1 + 2 * p;
    for j in 0..p {
        for l in j + 1..p {
            h[(j, l)] = beta[index];
            h[(l, j)] = beta[index];
            index += 1;
        }
    }
    control.check()?;
    let eigen = SymmetricEigen::factor(h.as_ref()).map_err(|_| failed())?;
    control.check()?;
    let values: Vec<_> = eigen
        .values()
        .iter()
        .map(|&v| finite(v))
        .collect::<Result<_>>()?;
    // OLS cross-products sum over observations. Include that roundoff scale so
    // a linear response does not acquire a spurious far-away stationary point.
    let scale = beta.iter().map(|v| v.abs()).fold(0., f64::max);
    let tolerance = 128. * f64::EPSILON * observations as f64 * scale;
    let degenerate = values.iter().any(|v| v.abs() <= tolerance);
    let classification = if degenerate {
        "degenerate"
    } else if values.iter().all(|&v| v > 0.) {
        "minimum"
    } else if values.iter().all(|&v| v < 0.) {
        "maximum"
    } else {
        "saddle"
    };
    let stationary_point = if degenerate {
        None
    } else {
        let vectors = eigen.vectors();
        let mut coded = vec![0.; p];
        for j in 0..p {
            control.check()?;
            let projection = (0..p).map(|l| vectors[(l, j)] * beta[l + 1]).sum::<f64>();
            for l in 0..p {
                coded[l] -= vectors[(l, j)] * projection / values[j];
            }
        }
        let factors = coded
            .iter()
            .enumerate()
            .map(|(j, z)| finite(centers[j] + half_ranges[j] * z))
            .collect::<Result<_>>()?;
        let predicted_response = finite(
            beta[0]
                + 0.5
                    * coded
                        .iter()
                        .zip(&beta[1..=p])
                        .map(|(z, b)| z * b)
                        .sum::<f64>(),
        )?;
        Some(StationaryPoint {
            inside_factor_ranges: coded.iter().all(|z| z.abs() <= 1. + 64. * f64::EPSILON),
            coded_factors: coded,
            factors,
            predicted_response,
        })
    };
    Ok(SurfaceGeometry {
        centers,
        half_ranges,
        hessian_eigenvalues: values,
        classification,
        stationary_point,
    })
}
