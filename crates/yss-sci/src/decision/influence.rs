//! Direct/total influences and reachability-based system levels.
use super::data::*;
use crate::regression::models::common::failed;
use yss_sci_contract::decision::influence::*;
use yss_sci_linalg::{Eigen, Mat, MatrixExt, Solve};

pub fn dematel(
    columns: &[Vec<f64>],
    normalization: InfluenceNormalization,
    attenuation: f64,
    control: &Control,
) -> Result<DematelResult> {
    let n = square(columns, control)?;
    if !attenuation.is_finite()
        || attenuation <= 0.
        || attenuation > 1.
        || columns.iter().flatten().any(|v| *v < 0.)
        || (0..n).any(|i| columns[i][i] != 0.)
    {
        return Err(parameter());
    }
    let scale = columns.iter().flatten().copied().fold(0., f64::max);
    let divisor = if scale > 0. {
        (0..n)
            .map(|i| {
                let row = columns.iter().map(|x| x[i] / scale).sum::<f64>();
                let col = columns[i].iter().map(|v| v / scale).sum::<f64>();
                row.max(col)
            })
            .fold(0., f64::max)
    } else {
        1.
    };
    let direct = Mat::from_fn(n, n, |i, j| {
        attenuation
            * match normalization {
                InfluenceNormalization::None => columns[j][i],
                InfluenceNormalization::MaxSum if scale > 0. => columns[j][i] / scale / divisor,
                InfluenceNormalization::MaxSum => 0.,
            }
    });
    control.check()?;
    let eigen = Eigen::factor(direct.as_ref()).map_err(|_| failed())?;
    control.check()?;
    let radius = finite(
        eigen
            .values()
            .iter()
            .map(|v| v.re.hypot(v.im))
            .fold(0., f64::max),
    )?;
    // An algebraic inverse outside the convergent geometric series is not total influence.
    if radius >= 1. - 64. * n as f64 * f64::EPSILON {
        return Err(parameter());
    }
    let identity = Mat::identity(n, n);
    let total = (&identity - &direct)
        .checked_lu()
        .map_err(|_| failed())?
        .solve(&direct);
    control.check()?;
    let largest = (0..n)
        .flat_map(|i| (0..n).map(move |j| (i, j)))
        .map(|(i, j)| total[(i, j)].abs())
        .fold(0., f64::max);
    let tolerance = 64. * n as f64 * f64::EPSILON * largest.max(1.);
    let mut outgoing = vec![0.; n];
    let mut incoming = vec![0.; n];
    let mut matrix = Vec::with_capacity(n.checked_mul(n).ok_or_else(parameter)?);
    for i in 0..n {
        control.check()?;
        for j in 0..n {
            let value = finite(total[(i, j)])?;
            if value < -tolerance {
                return Err(failed());
            }
            let value = value.max(0.);
            outgoing[i] += value;
            incoming[j] += value;
            matrix.push(InfluenceCell {
                source: i + 1,
                target: j + 1,
                direct: direct[(i, j)],
                total: value,
            });
        }
    }
    let prominence = outgoing
        .iter()
        .zip(&incoming)
        .map(|(a, b)| finite(a + b))
        .collect::<Result<Vec<_>>>()?;
    let weights = if prominence.iter().all(|v| *v == 0.) {
        None
    } else {
        Some(normalized_weights(&prominence)?)
    };
    let rows = (0..n)
        .map(|i| InfluenceRow {
            criterion: i + 1,
            outgoing: outgoing[i],
            incoming: incoming[i],
            prominence: prominence[i],
            net_cause: outgoing[i] - incoming[i],
            weight: weights.as_ref().map(|w| w[i]),
        })
        .collect();
    Ok(DematelResult {
        summary: DematelSummary {
            criteria: n,
            normalization,
            attenuation,
            spectral_radius: radius,
        },
        rows,
        matrix,
    })
}

pub fn ism(columns: &[Vec<f64>], control: &Control) -> Result<IsmResult> {
    let n = square(columns, control)?;
    let mut reach = vec![vec![0u64; n.div_ceil(64)]; n];
    for i in 0..n {
        control.check()?;
        for j in 0..n {
            let value = columns[j][i];
            if value != 0. && value != 1. {
                return Err(parameter());
            }
            if value == 1. || i == j {
                reach[i][j / 64] |= 1 << (j % 64);
            }
        }
    }
    for k in 0..n {
        control.check()?;
        let targets = reach[k].clone();
        for row in &mut reach {
            if row[k / 64] & (1 << (k % 64)) != 0 {
                for (j, (word, mask)) in row.iter_mut().zip(&targets).enumerate() {
                    if j.is_multiple_of(1024) {
                        control.check()?;
                    }
                    *word |= mask;
                }
            }
        }
    }
    let connected = |i: usize, j: usize| reach[i][j / 64] & (1 << (j % 64)) != 0;
    let mut assigned = vec![false; n];
    let mut components = Vec::<Vec<usize>>::new();
    for i in 0..n {
        control.check()?;
        if assigned[i] {
            continue;
        }
        let component = (i..n)
            .filter(|&j| !assigned[j] && connected(i, j) && connected(j, i))
            .collect::<Vec<_>>();
        for &j in &component {
            assigned[j] = true;
        }
        components.push(component);
    }
    let count = components.len();
    let mut remaining = vec![true; count];
    let mut outgoing = (0..count)
        .map(|i| {
            (0..count)
                .filter(|&j| i != j && connected(components[i][0], components[j][0]))
                .count()
        })
        .collect::<Vec<_>>();
    let mut levels = vec![];
    let mut row_levels = vec![0; n];
    while remaining.iter().any(|v| *v) {
        control.check()?;
        let sinks = (0..count)
            .filter(|&i| remaining[i] && outgoing[i] == 0)
            .collect::<Vec<_>>();
        if sinks.is_empty() {
            return Err(failed());
        }
        let mut level = vec![];
        for &i in &sinks {
            remaining[i] = false;
            for &j in &components[i] {
                row_levels[j] = levels.len() + 1;
                level.push(j + 1);
            }
        }
        for i in 0..count {
            if remaining[i] {
                for &j in &sinks {
                    if connected(components[i][0], components[j][0]) {
                        outgoing[i] -= 1;
                    }
                }
            }
        }
        level.sort_unstable();
        levels.push(level);
    }
    let mut rows = Vec::with_capacity(n);
    let mut matrix = Vec::with_capacity(n.checked_mul(n).ok_or_else(parameter)?);
    for (i, &level) in row_levels.iter().enumerate() {
        control.check()?;
        let mut driving = 0;
        let mut dependence = 0;
        for j in 0..n {
            let reachable = connected(i, j);
            driving += usize::from(reachable);
            dependence += usize::from(connected(j, i));
            matrix.push(ReachabilityCell {
                source: i + 1,
                target: j + 1,
                reachable,
            });
        }
        rows.push(IsmRow {
            criterion: i + 1,
            level,
            driving_power: driving,
            dependence,
        });
    }
    Ok(IsmResult {
        summary: IsmSummary {
            criteria: n,
            levels,
            strongly_connected_components: components
                .into_iter()
                .map(|v| v.into_iter().map(|j| j + 1).collect())
                .collect(),
        },
        rows,
        matrix,
    })
}
