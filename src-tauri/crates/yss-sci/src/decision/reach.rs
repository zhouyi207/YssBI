//! Exact total unduplicated reach search with packed observation sets.
use super::data::*;
use yss_sci_contract::decision::market::TurfResult;

pub fn turf(columns: &[Vec<f64>], size: usize, control: &Control) -> Result<TurfResult> {
    let (n, p) = dimensions(columns, &vec![false; columns.len()], control)?;
    if size == 0 || size > p {
        return Err(parameter());
    }
    let words = n.div_ceil(64);
    let mut sets = Vec::with_capacity(p);
    let mut frequencies = Vec::with_capacity(p);
    for column in columns {
        control.check()?;
        let mut set = vec![0u64; words];
        let mut frequency = 0usize;
        for (i, &x) in column.iter().enumerate() {
            if i.is_multiple_of(1024) {
                control.check()?;
            }
            if x != 0. && x != 1. {
                return Err(parameter());
            }
            if x == 1. {
                set[i / 64] |= 1u64 << (i % 64);
                frequency += 1;
            }
        }
        sets.push(set);
        frequencies.push(frequency);
    }
    let mut choice = (0..size).collect::<Vec<_>>();
    let mut best = choice.clone();
    let mut union = vec![0u64; words];
    let mut best_reach = 0;
    let mut evaluated = 0u64;
    let mut ties = 0u64;
    loop {
        control.check()?;
        union.fill(0);
        for &j in &choice {
            for (i, (out, &set)) in union.iter_mut().zip(&sets[j]).enumerate() {
                if i.is_multiple_of(1024) {
                    control.check()?;
                }
                *out |= set;
            }
        }
        let reach = union.iter().map(|v| v.count_ones() as usize).sum();
        evaluated = evaluated.checked_add(1).ok_or_else(parameter)?;
        if evaluated == 1 || reach > best_reach {
            best_reach = reach;
            best.clone_from(&choice);
            ties = 1;
        } else if reach == best_reach {
            ties = ties.checked_add(1).ok_or_else(parameter)?;
        }
        // Lexicographic enumeration gives deterministic tie selection without retaining all subsets.
        let Some(index) = (0..size).rev().find(|&j| choice[j] < p - size + j) else {
            break;
        };
        choice[index] += 1;
        for j in index + 1..size {
            choice[j] = choice[j - 1] + 1;
        }
    }
    control.check()?;
    let exposures = best
        .iter()
        .try_fold(0usize, |s, &j| s.checked_add(frequencies[j]))
        .ok_or_else(parameter)?;
    Ok(TurfResult {
        observations: n,
        criteria: p,
        combination_size: size,
        selected_criteria: best.into_iter().map(|j| j + 1).collect(),
        reach_count: best_reach,
        reach_percent: 100. * best_reach as f64 / n as f64,
        total_exposures: exposures,
        exposures_per_reached: (best_reach > 0).then(|| exposures as f64 / best_reach as f64),
        combinations_evaluated: evaluated,
        equally_optimal_combinations: ties,
        exact: true,
    })
}
