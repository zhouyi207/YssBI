use super::*;
use yss_sci_contract::spatial::*;

/// Validate the numerical matrix and return its maximum absolute row sum.
pub fn validate(w: &[Vec<f64>], control: &Control) -> Result<f64> {
    control.check()?;
    if w.len() < 2 {
        return Err(parameter());
    }
    let mut maximum = 0.0_f64;
    for (i, row) in w.iter().enumerate() {
        control.check()?;
        if row.len() != w.len() {
            return Err(invalid(Violation::ShapeMismatch));
        }
        if row.iter().any(|v| !v.is_finite()) {
            return Err(invalid(Violation::NonFiniteInput));
        }
        if row[i] != 0.0 || row.iter().any(|v| *v < 0.0) {
            return Err(parameter());
        }
        maximum = maximum.max(finite(row.iter().sum())?);
    }
    if maximum <= 0.0 {
        return Err(parameter());
    }
    Ok(maximum)
}

pub fn construct(
    x: &[f64],
    y: &[f64],
    options: WeightsOptions,
    control: &Control,
) -> Result<SpatialWeights> {
    crate::regression::models::common::validate(x, &[y.to_vec()], control)?;
    let n = x.len();
    if n < 2
        || (options.rule == WeightRule::Knn && (options.neighbors == 0 || options.neighbors >= n))
        || !options.radius.is_finite()
        || options.radius <= 0.0
        || !options.power.is_finite()
        || options.power <= 0.0
    {
        return Err(parameter());
    }
    let mut w = vec![vec![0.0; n]; n];
    for i in 0..n {
        control.check()?;
        let mut distances = Vec::with_capacity(n - 1);
        for j in 0..n {
            if i == j {
                continue;
            }
            let d = finite((x[i] - x[j]).hypot(y[i] - y[j]))?;
            distances.push((j, d));
        }
        if options.rule == WeightRule::Knn {
            // Break distance ties by the declared weights index, reproducibly.
            distances.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)));
            for &(j, _) in distances.iter().take(options.neighbors) {
                w[i][j] = 1.0;
            }
        } else {
            for (j, d) in distances {
                if d <= options.radius {
                    w[i][j] = if options.rule == WeightRule::DistanceBand {
                        1.0
                    } else {
                        if d == 0.0 {
                            return Err(parameter());
                        }
                        let v = finite(d.powf(-options.power))?;
                        if v <= 0.0 {
                            return Err(parameter());
                        }
                        v
                    };
                }
            }
        }
    }
    if options.symmetrize {
        for i in 0..n {
            control.check()?;
            let (before, remaining) = w.split_at_mut(i);
            let row = &mut remaining[0];
            for (j, other) in before.iter_mut().enumerate() {
                let v = row[j].max(other[i]);
                row[j] = v;
                other[i] = v;
            }
        }
    }
    let mut islands = vec![];
    for (i, row) in w.iter_mut().enumerate() {
        let sum = finite(row.iter().sum())?;
        if sum == 0.0 {
            islands.push(i);
        } else if options.row_standardize {
            for v in row {
                *v /= sum;
            }
        }
    }
    validate(&w, control)?;
    Ok(SpatialWeights {
        units: (0..n).collect(),
        matrix: w,
        options,
        islands,
    })
}
