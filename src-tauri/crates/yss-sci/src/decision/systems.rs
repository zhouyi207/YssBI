//! Coupling of supplied subsystem indices and attribution of criterion obstacles.
use super::data::*;

pub fn coupling(
    columns: &[Vec<f64>],
    weights: &[f64],
    control: &Control,
) -> Result<CouplingResult> {
    let (n, p) = dimensions(columns, &vec![false; columns.len()], control)?;
    if p < 2 || columns.iter().flatten().any(|v| !(0.0..=1.0).contains(v)) {
        return Err(parameter());
    }
    let weights = supplied_weights(weights, p)?;
    let mut rows = Vec::with_capacity(n);
    let mut undefined = 0;
    for i in 0..n {
        if i.is_multiple_of(1024) {
            control.check()?;
        }
        let mean = columns.iter().map(|x| x[i] / p as f64).sum::<f64>();
        let index = columns
            .iter()
            .zip(&weights)
            .map(|(x, w)| x[i] * w)
            .sum::<f64>();
        let coupling = if mean == 0. {
            undefined += 1;
            None
        } else if columns.iter().any(|x| x[i] == 0.) {
            Some(0.)
        } else {
            Some(
                (columns.iter().map(|x| x[i].ln() / p as f64).sum::<f64>() - mean.ln())
                    .exp()
                    .clamp(0., 1.),
            )
        };
        rows.push(CouplingRow {
            observation: i + 1,
            coupling,
            coordination_index: finite(index)?,
            coordination_degree: finite((coupling.unwrap_or(0.) * index).sqrt())?,
        });
    }
    Ok(CouplingResult {
        summary: SystemSummary {
            method: "coupling_coordination",
            observations: n,
            criteria: p,
            weights,
            undefined_rows: undefined,
        },
        rows,
    })
}
pub fn obstacles(
    columns: &[Vec<f64>],
    costs: &[bool],
    weights: &[f64],
    rescale: bool,
    control: &Control,
) -> Result<ObstacleResult> {
    let (n, p) = dimensions(columns, costs, control)?;
    let weights = supplied_weights(weights, p)?;
    let z = utilities(columns, costs, rescale, control)?;
    let mut rows = Vec::with_capacity(n.checked_mul(p).ok_or_else(parameter)?);
    let mut undefined = 0;
    for i in 0..n {
        if i.is_multiple_of(1024) {
            control.check()?;
        }
        let total = z
            .iter()
            .zip(&weights)
            .map(|(x, w)| w * (1. - x[i]))
            .sum::<f64>();
        if total == 0. {
            undefined += 1;
        }
        for j in 0..p {
            let deviation = 1. - z[j][i];
            let weighted = weights[j] * deviation;
            rows.push(ObstacleRow {
                observation: i + 1,
                criterion: j + 1,
                deviation,
                weighted_deviation: weighted,
                obstacle_percent: (total > 0.).then(|| 100. * weighted / total),
            });
        }
    }
    Ok(ObstacleResult {
        summary: SystemSummary {
            method: "obstacle_degree",
            observations: n,
            criteria: p,
            weights,
            undefined_rows: undefined,
        },
        rows,
    })
}
