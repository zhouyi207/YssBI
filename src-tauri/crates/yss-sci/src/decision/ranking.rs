use super::{data::*, weights};

pub fn calculate(
    columns: &[Vec<f64>],
    options: RankingOptions,
    control: &Control,
) -> Result<RankingResult> {
    let (n, p) = dimensions(columns, &options.costs, control)?;
    if options.method == RankingMethod::GreyRelational
        && !(options.grey_resolution.is_finite()
            && 0. < options.grey_resolution
            && options.grey_resolution <= 1.)
    {
        return Err(parameter());
    }
    if matches!(
        options.method,
        RankingMethod::Efficacy | RankingMethod::GreyRelational
    ) && options.normalization != Normalization::MinMax
    {
        return Err(parameter());
    }
    let weighting = weights::calculate(
        columns,
        &options.costs,
        options.weighting,
        &options.weights,
        control,
    )?;
    let z = normalize(columns, options.normalization, control)?;
    let best = z
        .iter()
        .enumerate()
        .map(|(j, x)| {
            x.iter()
                .copied()
                .reduce(if options.costs[j] { f64::min } else { f64::max })
                .unwrap()
        })
        .collect::<Vec<_>>();
    let worst = z
        .iter()
        .enumerate()
        .map(|(j, x)| {
            x.iter()
                .copied()
                .reduce(if options.costs[j] { f64::max } else { f64::min })
                .unwrap()
        })
        .collect::<Vec<_>>();
    let ranks = if options.method == RankingMethod::Wrsr {
        columns
            .iter()
            .map(|x| crate::association::ranks(x, control).map(|r| r.values))
            .collect::<Result<Vec<_>>>()?
    } else {
        vec![]
    };
    let max_delta = if options.method == RankingMethod::GreyRelational {
        (0..p)
            .map(|j| (best[j] - worst[j]).abs())
            .fold(0., f64::max)
    } else {
        0.
    };
    let mut rows = Vec::with_capacity(n);
    for i in 0..n {
        if i.is_multiple_of(1024) {
            control.check()?;
        }
        let mut score = 0.;
        let mut distance_best = 0.;
        let mut distance_worst = 0.;
        for j in 0..p {
            let w = weighting.weights[j];
            match options.method {
                RankingMethod::Topsis => {
                    distance_best = f64::hypot(distance_best, w * (z[j][i] - best[j]));
                    distance_worst = f64::hypot(distance_worst, w * (z[j][i] - worst[j]));
                }
                RankingMethod::GreyRelational => {
                    score += w * if max_delta == 0. {
                        1.
                    } else {
                        options.grey_resolution * max_delta
                            / ((z[j][i] - best[j]).abs() + options.grey_resolution * max_delta)
                    };
                }
                RankingMethod::Wrsr => {
                    let rank = if options.costs[j] {
                        n as f64 + 1. - ranks[j][i]
                    } else {
                        ranks[j][i]
                    };
                    score += w * rank / n as f64;
                }
                RankingMethod::Composite | RankingMethod::Efficacy => {
                    let oriented = if options.costs[j] {
                        if options.normalization == Normalization::MinMax {
                            if best[j] == worst[j] {
                                0.
                            } else {
                                1. - z[j][i]
                            }
                        } else {
                            -z[j][i]
                        }
                    } else {
                        z[j][i]
                    };
                    score += w * oriented;
                }
            }
        }
        if options.method == RankingMethod::Topsis {
            let total = distance_best + distance_worst;
            if !total.is_finite() || total <= 0. {
                return Err(parameter());
            }
            score = distance_worst / total;
        }
        if options.method == RankingMethod::Efficacy {
            score = 60. + 40. * score;
        }
        rows.push(ScoreRow {
            observation: i + 1,
            score: finite(score)?,
            rank: 0.,
            distance_best: finite(distance_best)?,
            distance_worst: finite(distance_worst)?,
        });
    }
    let scores = rows.iter().map(|r| r.score).collect::<Vec<_>>();
    let ranked = crate::association::ranks(&scores, control)?.values;
    for (row, rank) in rows.iter_mut().zip(ranked) {
        row.rank = n as f64 + 1. - rank;
    }
    Ok(RankingResult {
        summary: RankingSummary {
            observations: n,
            criteria: p,
            method: options.method,
            normalization: options.normalization,
            cost_criteria: options
                .costs
                .iter()
                .enumerate()
                .filter_map(|(j, &cost)| cost.then_some(j + 1))
                .collect(),
            weighting,
        },
        rows,
    })
}
