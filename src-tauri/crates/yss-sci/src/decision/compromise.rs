//! VIKOR group utility, individual regret and compromise-set conditions.
use super::data::*;
pub fn vikor(
    columns: &[Vec<f64>],
    costs: &[bool],
    weights: &[f64],
    v: f64,
    control: &Control,
) -> Result<VikorResult> {
    let (n, p) = dimensions(columns, costs, control)?;
    if n < 2 || !v.is_finite() || !(0.0..=1.0).contains(&v) {
        return Err(parameter());
    }
    let weights = supplied_weights(weights, p)?;
    let z = utilities(columns, costs, true, control)?;
    let active = z
        .iter()
        .map(|x| x.iter().any(|v| *v != x[0]))
        .collect::<Vec<_>>();
    let mut sums = vec![0.; n];
    let mut regrets = vec![0_f64; n];
    for j in 0..p {
        control.check()?;
        if active[j] {
            for i in 0..n {
                let loss = weights[j] * (1. - z[j][i]);
                sums[i] += loss;
                regrets[i] = regrets[i].max(loss);
            }
        }
    }
    let s_min = sums.iter().copied().fold(f64::INFINITY, f64::min);
    let s_max = sums.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let r_min = regrets.iter().copied().fold(f64::INFINITY, f64::min);
    let r_max = regrets.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let fraction = |x, low, high| {
        if high > low {
            (x - low) / (high - low)
        } else {
            0.
        }
    };
    let q = (0..n)
        .map(|i| {
            v * fraction(sums[i], s_min, s_max) + (1. - v) * fraction(regrets[i], r_min, r_max)
        })
        .collect::<Vec<_>>();
    let ranks = crate::association::ranks(&q, control)?.values;
    let mut order = (0..n).collect::<Vec<_>>();
    order.sort_by(|&a, &b| q[a].total_cmp(&q[b]).then(a.cmp(&b)));
    let first = order[0];
    let threshold = 1. / (n - 1) as f64;
    let advantage = q[order[1]] - q[first] >= threshold;
    let stability = sums[first] == s_min || regrets[first] == r_min;
    let compromise = if !advantage {
        order
            .iter()
            .copied()
            .take_while(|&i| q[i] - q[first] < threshold)
            .collect::<Vec<_>>()
    } else if !stability {
        order[..2].to_vec()
    } else {
        vec![first]
    };
    let rows = (0..n)
        .map(|i| VikorRow {
            observation: i + 1,
            compromise_score: q[i],
            group_utility: sums[i],
            individual_regret: regrets[i],
            rank: ranks[i],
        })
        .collect();
    control.check()?;
    Ok(VikorResult {
        summary: VikorSummary {
            observations: n,
            criteria: p,
            weights,
            majority_weight: v,
            acceptable_advantage: advantage,
            acceptable_stability: stability,
            compromise_alternatives: compromise.into_iter().map(|i| i + 1).collect(),
        },
        rows,
    })
}
