//! Rank, sign, and sequence tests with explicit small-sample and tie handling.
use super::{Error, Violation, checkpoint, failed, finite, invalid, parameter};
use statrs::distribution::{ChiSquared, ContinuousCDF, Normal};
use yss_sci_contract::execution::ScientificExecutionControl;
use yss_sci_contract::hypothesis::{Alternative, ClassicalTestResult, RankHypothesisTest as Input};

pub fn run(
    input: Input,
    control: &ScientificExecutionControl,
) -> Result<ClassicalTestResult, Error> {
    control.check()?;
    let result = match input {
        Input::WilcoxonOneSample {
            values,
            null_median,
            alternative,
        } => {
            if !null_median.is_finite() {
                return Err(parameter());
            }
            validate(&values, control)?;
            let differences = values
                .into_iter()
                .enumerate()
                .map(|(i, v)| {
                    checkpoint(control, i)?;
                    finite(v - null_median)
                })
                .collect::<Result<Vec<_>, Error>>()?;
            wilcoxon(differences, alternative, "wilcoxon.one_sample", control)
        }
        Input::WilcoxonPaired {
            before,
            after,
            alternative,
        } => {
            if before.len() != after.len() {
                return Err(invalid(Violation::ShapeMismatch));
            }
            validate(&before, control)?;
            validate(&after, control)?;
            wilcoxon(
                before
                    .iter()
                    .zip(after)
                    .enumerate()
                    .map(|(i, (a, b))| {
                        checkpoint(control, i)?;
                        finite(a - b)
                    })
                    .collect::<Result<_, Error>>()?,
                alternative,
                "wilcoxon.paired",
                control,
            )
        }
        Input::MannWhitney {
            first,
            second,
            alternative,
        } => mann_whitney(first, second, alternative, control),
        Input::KruskalWallis { groups } => kruskal_wallis(groups, control),
        Input::Friedman { conditions } => friedman(conditions, control),
        Input::CochranQ { conditions } => cochran_q(conditions, control),
        Input::Runs { values } => runs(values, control),
        Input::MoodMedian { groups } => mood_median(groups, control),
        Input::MannKendall {
            values,
            alternative,
        } => mann_kendall(values, alternative, control),
    };
    control.check()?;
    result
}

fn wilcoxon(
    differences: Vec<f64>,
    alternative: Alternative,
    method: &str,
    control: &ScientificExecutionControl,
) -> Result<ClassicalTestResult, Error> {
    let mut nonzero = Vec::new();
    for (i, value) in differences.into_iter().enumerate() {
        checkpoint(control, i)?;
        if value != 0.0 {
            nonzero.push(value);
        }
    }
    let n = nonzero.len();
    if n < 2 {
        return Err(invalid(Violation::EmptyInput));
    }
    let abs = nonzero
        .iter()
        .enumerate()
        .map(|(i, v)| {
            checkpoint(control, i)?;
            Ok(v.abs())
        })
        .collect::<Result<Vec<_>, Error>>()?;
    let (ranks, _) = ranks(&abs, control)?;
    let mut observed = 0.0;
    let mut mean = 0.0;
    let mut variance = 0.0;
    for (i, (difference, rank)) in nonzero.iter().zip(&ranks).enumerate() {
        checkpoint(control, i)?;
        if *difference > 0.0 {
            observed += rank;
        }
        mean += rank;
        variance += rank * rank;
    }
    let mean = mean / 2.0;
    let variance = variance / 4.0;
    if variance <= 0.0 {
        return Err(invalid(Violation::DataOutOfRange));
    }
    let exact = if n <= 20 {
        Some(wilcoxon_exact(&ranks, observed, alternative, control)?)
    } else {
        None
    };
    let z = (observed - mean) / variance.sqrt();
    let p = exact.unwrap_or(normal_p(z, alternative, control)?);
    let mut result = base(
        method,
        "paired differences have zero median",
        "signed_rank",
        z,
        vec![],
        p,
        vec![n],
    );
    result.alternative = alternative_name(alternative).into();
    result.details.insert("positive_rank_sum".into(), observed);
    result.details.insert(
        "exact_p_value_used".into(),
        if exact.is_some() { 1.0 } else { 0.0 },
    );
    Ok(result)
}

fn wilcoxon_exact(
    ranks: &[f64],
    observed: f64,
    alternative: Alternative,
    control: &ScientificExecutionControl,
) -> Result<f64, Error> {
    let observed2 = (observed * 2.0).round() as u64;
    let ranks2 = ranks
        .iter()
        .map(|r| (r * 2.0).round() as u64)
        .collect::<Vec<_>>();
    let total: u64 = ranks2.iter().sum();
    let center = total as f64 / 2.0;
    let mut extreme = 0u64;
    let combinations = 1u64 << ranks.len();
    for mask in 0..combinations {
        checkpoint(control, mask as usize)?;
        let sum = ranks2
            .iter()
            .enumerate()
            .filter(|(i, _)| mask & (1 << i) != 0)
            .map(|(_, r)| *r)
            .sum::<u64>();
        let keep = match alternative {
            Alternative::Less => sum <= observed2,
            Alternative::Greater => sum >= observed2,
            Alternative::TwoSided => {
                (sum as f64 - center).abs() >= (observed2 as f64 - center).abs() - 1e-12
            }
        };
        extreme += u64::from(keep);
    }
    Ok(extreme as f64 / combinations as f64)
}

fn mann_whitney(
    first: Vec<f64>,
    second: Vec<f64>,
    alternative: Alternative,
    control: &ScientificExecutionControl,
) -> Result<ClassicalTestResult, Error> {
    validate(&first, control)?;
    validate(&second, control)?;
    let n1 = first.len();
    let n2 = second.len();
    let combined = first
        .iter()
        .map(|v| (*v, 0usize))
        .chain(second.iter().map(|v| (*v, 1usize)))
        .enumerate()
        .map(|(i, v)| {
            checkpoint(control, i)?;
            Ok(v)
        })
        .collect::<Result<Vec<_>, Error>>()?;
    let values = combined
        .iter()
        .enumerate()
        .map(|(i, x)| {
            checkpoint(control, i)?;
            Ok(x.0)
        })
        .collect::<Result<Vec<_>, Error>>()?;
    let (rank, ties) = ranks(&values, control)?;
    let mut rank1 = 0.0;
    for (i, (_, group)) in combined.iter().enumerate() {
        checkpoint(control, i)?;
        if *group == 0 {
            rank1 += rank[i];
        }
    }
    let first_n = n1 as f64;
    let second_n = n2 as f64;
    let pair_count = first_n * second_n;
    let u1 = rank1 - first_n * (first_n + 1.0) / 2.0;
    let total = n1 + n2;
    let total_n = total as f64;
    let tie_term = rank_tie_sum(&ties, control)?;
    let variance = pair_count / 12.0 * (total_n + 1.0 - tie_term / (total_n * (total_n - 1.0)));
    if variance <= 0.0 {
        return Err(invalid(Violation::DataOutOfRange));
    }
    let z = (u1 - pair_count / 2.0) / variance.sqrt();
    let mut result = base(
        "mann_whitney",
        "the two populations have the same distribution",
        "U",
        u1,
        vec![],
        normal_p(z, alternative, control)?,
        vec![n1, n2],
    );
    result.alternative = alternative_name(alternative).into();
    result
        .details
        .insert("u_complement".into(), pair_count - u1);
    Ok(result)
}

fn kruskal_wallis(
    groups: Vec<Vec<f64>>,
    control: &ScientificExecutionControl,
) -> Result<ClassicalTestResult, Error> {
    if groups.len() < 2
        || groups.iter().enumerate().try_fold(false, |empty, (i, g)| {
            checkpoint(control, i)?;
            Ok::<_, Error>(empty || g.is_empty())
        })?
    {
        return Err(invalid(Violation::EmptyInput));
    }
    groups.iter().enumerate().try_for_each(|(i, g)| {
        checkpoint(control, i)?;
        validate(g, control)
    })?;
    let sizes = groups
        .iter()
        .enumerate()
        .map(|(i, g)| {
            checkpoint(control, i)?;
            Ok(g.len())
        })
        .collect::<Result<Vec<_>, Error>>()?;
    let vals = groups
        .iter()
        .flatten()
        .enumerate()
        .map(|(i, v)| {
            checkpoint(control, i)?;
            Ok(*v)
        })
        .collect::<Result<Vec<_>, Error>>()?;
    let (rank, ties) = ranks(&vals, control)?;
    let n = vals.len();
    let mut offset = 0;
    let mut sum = 0.0;
    for (i, size) in sizes.iter().enumerate() {
        checkpoint(control, i)?;
        let rs =
            rank[offset..offset + size]
                .iter()
                .enumerate()
                .try_fold(0.0, |sum, (j, value)| {
                    checkpoint(control, j)?;
                    Ok::<_, Error>(sum + value)
                })?;
        sum += rs * rs / *size as f64;
        offset += size;
    }
    let raw = 12.0 / (n as f64 * (n + 1) as f64) * sum - 3.0 * (n + 1) as f64;
    let total_n = n as f64;
    let corr = 1.0 - rank_tie_sum(&ties, control)? / (total_n * (total_n * total_n - 1.0));
    if corr <= 0.0 {
        return Err(invalid(Violation::DataOutOfRange));
    }
    let statistic = raw / corr;
    let df = (groups.len() - 1) as f64;
    control.check()?;
    let p = ChiSquared::new(df).map_err(|_| failed())?.sf(statistic);
    control.check()?;
    Ok(base(
        "kruskal_wallis",
        "all groups have the same distribution",
        "H",
        statistic,
        vec![df],
        p,
        sizes,
    ))
}

fn friedman(
    conditions: Vec<Vec<f64>>,
    control: &ScientificExecutionControl,
) -> Result<ClassicalTestResult, Error> {
    if conditions.len() < 3
        || conditions
            .iter()
            .enumerate()
            .try_fold(false, |empty, (i, g)| {
                checkpoint(control, i)?;
                Ok::<_, Error>(empty || g.is_empty())
            })?
    {
        return Err(invalid(Violation::EmptyInput));
    }
    conditions.iter().enumerate().try_for_each(|(i, g)| {
        checkpoint(control, i)?;
        validate(g, control)
    })?;
    let k = conditions.len();
    let n = conditions[0].len();
    if conditions
        .iter()
        .enumerate()
        .try_fold(false, |invalid, (i, g)| {
            checkpoint(control, i)?;
            Ok::<_, Error>(invalid || g.len() != n)
        })?
    {
        return Err(invalid(Violation::ShapeMismatch));
    }
    if n < 2 {
        return Err(invalid(Violation::EmptyInput));
    }
    let mut sums = vec![0.0; k];
    let mut tie_total = 0.0;
    for row in 0..n {
        checkpoint(control, row)?;
        let values = conditions
            .iter()
            .enumerate()
            .map(|(i, g)| {
                checkpoint(control, i)?;
                Ok(g[row])
            })
            .collect::<Result<Vec<_>, Error>>()?;
        let (r, t) = ranks(&values, control)?;
        for j in 0..k {
            checkpoint(control, j)?;
            sums[j] += r[j];
        }
        tie_total += rank_tie_sum(&t, control)?;
    }
    let rows = n as f64;
    let groups = k as f64;
    let raw = 12.0 / (rows * groups * (groups + 1.0))
        * sums
            .iter()
            .map(|r| r * r)
            .enumerate()
            .try_fold(0.0, |sum, (i, v)| {
                checkpoint(control, i)?;
                Ok::<_, Error>(sum + v)
            })?
        - 3.0 * rows * (groups + 1.0);
    let correction = 1.0 - tie_total / (rows * groups * (groups * groups - 1.0));
    if correction <= 0.0 {
        return Err(invalid(Violation::DataOutOfRange));
    }
    let statistic = raw / correction;
    let df = (k - 1) as f64;
    control.check()?;
    let p = ChiSquared::new(df).map_err(|_| failed())?.sf(statistic);
    control.check()?;
    Ok(base(
        "friedman",
        "repeated conditions have equal location",
        "Q",
        statistic,
        vec![df],
        p,
        vec![n; k],
    ))
}

fn cochran_q(
    conditions: Vec<Vec<f64>>,
    control: &ScientificExecutionControl,
) -> Result<ClassicalTestResult, Error> {
    if conditions.len() < 3
        || conditions
            .iter()
            .enumerate()
            .try_fold(false, |empty, (i, g)| {
                checkpoint(control, i)?;
                Ok::<_, Error>(empty || g.is_empty())
            })?
    {
        return Err(invalid(Violation::EmptyInput));
    }
    let n = conditions[0].len();
    let k = conditions.len();
    if n < 2 {
        return Err(invalid(Violation::EmptyInput));
    }
    let mut col = Vec::with_capacity(k);
    let mut rows = vec![0.0; n];
    let mut total = 0.0;
    for (i, group) in conditions.iter().enumerate() {
        checkpoint(control, i)?;
        if group.len() != n {
            return Err(invalid(Violation::ShapeMismatch));
        }
        let mut sum = 0.0;
        for (j, value) in group.iter().enumerate() {
            checkpoint(control, j)?;
            if !value.is_finite() {
                return Err(invalid(Violation::NonFiniteInput));
            }
            if *value != 0.0 && *value != 1.0 {
                return Err(invalid(Violation::DataOutOfRange));
            }
            sum += value;
            rows[j] += value;
        }
        total += sum;
        col.push(sum);
    }
    let denom = k as f64 * total
        - rows
            .iter()
            .map(|x| x * x)
            .enumerate()
            .try_fold(0.0, |sum, (i, v)| {
                checkpoint(control, i)?;
                Ok::<_, Error>(sum + v)
            })?;
    if denom <= 0.0 {
        return Err(invalid(Violation::DataOutOfRange));
    }
    let statistic = (k - 1) as f64
        * (k as f64
            * col
                .iter()
                .map(|x| x * x)
                .enumerate()
                .try_fold(0.0, |sum, (i, v)| {
                    checkpoint(control, i)?;
                    Ok::<_, Error>(sum + v)
                })?
            - total * total)
        / denom;
    let df = (k - 1) as f64;
    control.check()?;
    let p = ChiSquared::new(df).map_err(|_| failed())?.sf(statistic);
    control.check()?;
    Ok(base(
        "cochran_q",
        "all paired condition probabilities are equal",
        "Q",
        statistic,
        vec![df],
        p,
        vec![n; k],
    ))
}

fn runs(
    values: Vec<f64>,
    control: &ScientificExecutionControl,
) -> Result<ClassicalTestResult, Error> {
    if values.len() < 2 {
        return Err(invalid(Violation::EmptyInput));
    }
    let mut n1 = 0;
    let mut observed = 1;
    for (i, value) in values.iter().enumerate() {
        checkpoint(control, i)?;
        if !value.is_finite() {
            return Err(invalid(Violation::NonFiniteInput));
        }
        if *value != 0.0 && *value != 1.0 {
            return Err(invalid(Violation::DataOutOfRange));
        }
        n1 += usize::from(*value == 1.0);
        if i > 0 && values[i - 1] != *value {
            observed += 1;
        }
    }
    let n0 = values.len() - n1;
    let n = values.len();
    if n0 == 0 || n1 == 0 {
        return Err(invalid(Violation::DataOutOfRange));
    }
    let mean = 1.0 + 2.0 * n0 as f64 * n1 as f64 / n as f64;
    let total_n = n as f64;
    let variance = 2.0 * n0 as f64 * n1 as f64 * (2.0 * n0 as f64 * n1 as f64 - n as f64)
        / (total_n * total_n * (total_n - 1.0));
    if variance <= 0.0 {
        return Err(invalid(Violation::DataOutOfRange));
    }
    let z = (observed as f64 - mean) / variance.sqrt();
    Ok(base(
        "runs",
        "sequence order is random",
        "runs_z",
        z,
        vec![],
        normal_p(z, Alternative::TwoSided, control)?,
        vec![n],
    ))
}

fn mood_median(
    groups: Vec<Vec<f64>>,
    control: &ScientificExecutionControl,
) -> Result<ClassicalTestResult, Error> {
    if groups.len() < 2
        || groups.iter().enumerate().try_fold(false, |empty, (i, g)| {
            checkpoint(control, i)?;
            Ok::<_, Error>(empty || g.is_empty())
        })?
    {
        return Err(invalid(Violation::EmptyInput));
    }
    groups.iter().enumerate().try_for_each(|(i, g)| {
        checkpoint(control, i)?;
        validate(g, control)
    })?;
    let mut all = groups
        .iter()
        .flatten()
        .enumerate()
        .map(|(i, v)| {
            checkpoint(control, i)?;
            Ok(*v)
        })
        .collect::<Result<Vec<_>, Error>>()?;
    control.check()?;
    all.sort_by(f64::total_cmp);
    control.check()?;
    let median = if all.len() % 2 == 0 {
        (all[all.len() / 2 - 1] + all[all.len() / 2]) / 2.0
    } else {
        all[all.len() / 2]
    };
    drop(all);
    let mut table = vec![0.0; groups.len() * 2];
    for (i, g) in groups.iter().enumerate() {
        checkpoint(control, i)?;
        for (j, x) in g.iter().enumerate() {
            checkpoint(control, j)?;
            if *x > median {
                table[i] += 1.0
            } else if *x < median {
                table[groups.len() + i] += 1.0
            }
        }
    }
    let pearson = super::categorical::pearson_table(table, 2, groups.len(), control)?;
    let sizes = groups
        .iter()
        .enumerate()
        .map(|(i, g)| {
            checkpoint(control, i)?;
            Ok(g.len())
        })
        .collect::<Result<_, Error>>()?;
    let mut result = base(
        "mood_median",
        "group medians are equal",
        "chi_squared",
        pearson.statistic.ok_or_else(failed)?,
        pearson.degrees_of_freedom,
        pearson.p_value,
        sizes,
    );
    result.details.insert("pooled_median".into(), median);
    Ok(result)
}

fn mann_kendall(
    values: Vec<f64>,
    alternative: Alternative,
    control: &ScientificExecutionControl,
) -> Result<ClassicalTestResult, Error> {
    validate(&values, control)?;
    let n = values.len();
    if n < 3 {
        return Err(invalid(Violation::EmptyInput));
    }
    let mut ordered = values
        .iter()
        .enumerate()
        .map(|(i, v)| {
            checkpoint(control, i)?;
            Ok(*v)
        })
        .collect::<Result<Vec<_>, Error>>()?;
    control.check()?;
    ordered.sort_by(f64::total_cmp);
    control.check()?;
    let mut unique = 0;
    let mut i = 0;
    let mut tie = 0.0;
    while i < ordered.len() {
        checkpoint(control, i)?;
        let start = i;
        let value = ordered[i];
        i += 1;
        while i < ordered.len() && ordered[i] == value {
            checkpoint(control, i)?;
            i += 1;
        }
        let count = (i - start) as f64;
        tie += count * (count - 1.0) * (2.0 * count + 5.0);
        ordered[unique] = value;
        unique += 1;
    }
    ordered.truncate(unique);
    let mut tree = vec![0i64; ordered.len() + 1];
    let mut s = 0i64;
    for (i, value) in values.iter().enumerate() {
        checkpoint(control, i)?;
        let rank = ordered
            .binary_search_by(|candidate| candidate.partial_cmp(value).expect("finite observation"))
            .expect("compressed value")
            + 1;
        let less = fenwick_sum(&tree, rank - 1);
        let less_or_equal = fenwick_sum(&tree, rank);
        s += less - (i as i64 - less_or_equal);
        let mut index = rank;
        while index < tree.len() {
            tree[index] += 1;
            index += index & index.wrapping_neg();
        }
    }
    let total_n = n as f64;
    let variance = (total_n * (total_n - 1.0) * (2.0 * total_n + 5.0) - tie) / 18.0;
    if variance < 0.0 {
        return Err(failed());
    }
    let corrected = if s > 0 {
        (s - 1) as f64
    } else if s < 0 {
        (s + 1) as f64
    } else {
        0.0
    };
    let z = if s == 0 {
        0.0
    } else {
        corrected / variance.sqrt()
    };
    let p = normal_p(z, alternative, control)?;
    let mut result = base(
        "mann_kendall",
        "observations have no monotonic trend",
        "S_corrected_z",
        z,
        vec![],
        p,
        vec![n],
    );
    result.alternative = alternative_name(alternative).into();
    result.details.insert("s_statistic".into(), s as f64);
    result.details.insert(
        "kendall_tau".into(),
        s as f64 / (total_n * (total_n - 1.0) / 2.0),
    );
    Ok(result)
}

fn fenwick_sum(tree: &[i64], mut index: usize) -> i64 {
    let mut sum = 0;
    while index > 0 {
        sum += tree[index];
        index &= index - 1;
    }
    sum
}

fn ranks(
    values: &[f64],
    control: &ScientificExecutionControl,
) -> Result<(Vec<f64>, Vec<usize>), Error> {
    validate(values, control)?;
    let mut order = (0..values.len())
        .map(|i| {
            checkpoint(control, i)?;
            Ok(i)
        })
        .collect::<Result<Vec<_>, Error>>()?;
    control.check()?;
    order.sort_by(|a, b| values[*a].total_cmp(&values[*b]));
    control.check()?;
    let mut output = vec![0.0; values.len()];
    let mut ties = Vec::new();
    let mut i = 0;
    while i < order.len() {
        checkpoint(control, i)?;
        let mut j = i + 1;
        while j < order.len() && values[order[i]] == values[order[j]] {
            checkpoint(control, j)?;
            j += 1;
        }
        let rank = (i + 1 + j) as f64 / 2.0;
        for (offset, index) in order[i..j].iter().enumerate() {
            checkpoint(control, offset)?;
            output[*index] = rank;
        }
        if j - i > 1 {
            ties.push(j - i);
        }
        i = j;
    }
    Ok((output, ties))
}

fn rank_tie_sum(ties: &[usize], control: &ScientificExecutionControl) -> Result<f64, Error> {
    ties.iter().enumerate().try_fold(0.0, |sum, (i, count)| {
        checkpoint(control, i)?;
        let count = *count as f64;
        Ok(sum + count * (count * count - 1.0))
    })
}

fn validate(values: &[f64], control: &ScientificExecutionControl) -> Result<(), Error> {
    control.check()?;
    if values.is_empty() {
        return Err(invalid(Violation::EmptyInput));
    }
    for (i, value) in values.iter().enumerate() {
        checkpoint(control, i)?;
        if !value.is_finite() {
            return Err(invalid(Violation::NonFiniteInput));
        }
    }
    Ok(())
}

fn normal_p(z: f64, a: Alternative, control: &ScientificExecutionControl) -> Result<f64, Error> {
    finite(z)?;
    let n = Normal::new(0.0, 1.0).expect("standard normal");
    control.check()?;
    let p = match a {
        Alternative::TwoSided => 2.0 * n.sf(z.abs()),
        Alternative::Greater => n.sf(z),
        Alternative::Less => n.cdf(z),
    }
    .clamp(0.0, 1.0);
    control.check()?;
    Ok(p)
}
fn alternative_name(a: Alternative) -> &'static str {
    match a {
        Alternative::TwoSided => "two-sided",
        Alternative::Greater => "greater",
        Alternative::Less => "less",
    }
}
fn base(
    method: &str,
    null: &str,
    name: &str,
    stat: f64,
    df: Vec<f64>,
    p: f64,
    n: Vec<usize>,
) -> ClassicalTestResult {
    ClassicalTestResult {
        method: method.into(),
        null_hypothesis: null.into(),
        alternative: "two-sided".into(),
        statistic_name: name.into(),
        statistic: Some(stat),
        degrees_of_freedom: df,
        p_value: p.clamp(0.0, 1.0),
        estimate: None,
        standard_error: None,
        sample_sizes: n,
        details: Default::default(),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};
    use yss_sci_contract::execution::{ScientificCancellationToken, ScientificComputationError};

    #[test]
    fn exact_enumeration_observes_control_without_a_run_boundary() {
        let cancellation = ScientificCancellationToken::new();
        let mut control = ScientificExecutionControl {
            cancellation: cancellation.clone(),
            deadline: Instant::now() + Duration::from_secs(10),
        };
        assert_eq!(
            wilcoxon_exact(&[1.0, 2.0], 3.0, Alternative::TwoSided, &control).unwrap(),
            0.5
        );
        cancellation.cancel();
        assert!(matches!(
            wilcoxon_exact(&[1.0; 20], 20.0, Alternative::TwoSided, &control),
            Err(ScientificComputationError::Cancelled)
        ));
        control.cancellation = ScientificCancellationToken::new();
        control.deadline = Instant::now();
        assert!(matches!(
            wilcoxon_exact(&[1.0; 20], 20.0, Alternative::TwoSided, &control),
            Err(ScientificComputationError::DeadlineExceeded)
        ));
    }
}
