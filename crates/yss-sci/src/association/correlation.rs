use super::*;
use yss_sci_contract::association::{
    CorrelationOptions, CorrelationResult, MAX_EXACT_RANK_OBSERVATIONS, RankCorrelationOptions,
    RankInference,
};
use yss_sci_linalg::{Mat, MatrixExt, Solve, matrix_rank};

fn result(
    method: &'static str,
    r: f64,
    n: usize,
    controls: usize,
    alternative_kind: Alternative,
    inference: TestInference,
    confidence_interval: Option<ConfidenceInterval>,
) -> CorrelationResult {
    CorrelationResult {
        method,
        coefficient: r,
        observations: n,
        control_variables: controls,
        alternative: alternative(alternative_kind),
        inference,
        confidence_interval,
        concordant_pairs: None,
        discordant_pairs: None,
        tied_pairs_x: None,
        tied_pairs_y: None,
    }
}

pub fn pearson(
    x: &[f64],
    y: &[f64],
    options: CorrelationOptions,
    control: &Control,
) -> Result<CorrelationResult, Error> {
    paired(x, y, 3, control)?;
    level(options.confidence_level)?;
    let r = coefficient(x, y, control)?;
    let output = result(
        "pearson",
        r,
        x.len(),
        0,
        options.alternative,
        student_test(r, x.len() - 2, options.alternative, "student_t")?,
        fisher_interval(r, x.len(), 0, options.confidence_level)?,
    );
    control.check()?;
    Ok(output)
}

pub fn partial(
    x: &[f64],
    y: &[f64],
    controls: &[Vec<f64>],
    options: CorrelationOptions,
    control: &Control,
) -> Result<CorrelationResult, Error> {
    paired(x, y, 3, control)?;
    level(options.confidence_level)?;
    let q = controls.len();
    let n = x.len();
    if q == 0 {
        return Err(invalid(Violation::ShapeMismatch));
    }
    if n <= q + 2 {
        return Err(invalid(Violation::EmptyInput));
    }
    let mut columns = Vec::with_capacity(q);
    for column in controls {
        if column.len() != n {
            return Err(Error::InvalidInput {
                violation: Violation::ShapeMismatch,
            });
        }
        columns.push(unit_vector(column, control)?);
    }
    let x = unit_vector(x, control)?;
    let y = unit_vector(y, control)?;
    let design = Mat::from_fn(n, q, |i, j| columns[j][i]);
    let (rank, _) = matrix_rank(design.as_ref()).map_err(|_| Error::ComputationFailed)?;
    control.check()?;
    if rank != q {
        return Err(invalid(Violation::DataOutOfRange));
    }
    let targets = Mat::from_fn(n, 2, |i, j| if j == 0 { x[i] } else { y[i] });
    let gram = design.transpose() * design.as_ref();
    let rhs = design.transpose() * targets.as_ref();
    let fitted = design.as_ref()
        * gram
            .checked_cholesky()
            .map_err(|_| Error::ComputationFailed)?
            .solve(&rhs)
            .as_ref();
    control.check()?;
    let mut rx = Vec::with_capacity(n);
    let mut ry = Vec::with_capacity(n);
    for i in 0..n {
        checkpoint(control, i)?;
        rx.push(x[i] - fitted[(i, 0)]);
        ry.push(y[i] - fitted[(i, 1)]);
    }
    let tolerance = 8.0 * n.max(q) as f64 * f64::EPSILON;
    if sum(rx.iter().map(|x| x * x)).sqrt() <= tolerance
        || sum(ry.iter().map(|x| x * x)).sqrt() <= tolerance
    {
        return Err(invalid(Violation::DataOutOfRange));
    }
    let r = coefficient(&rx, &ry, control)?;
    let output = result(
        "partial_pearson",
        r,
        n,
        q,
        options.alternative,
        student_test(r, n - q - 2, options.alternative, "student_t")?,
        fisher_interval(r, n, q, options.confidence_level)?,
    );
    control.check()?;
    Ok(output)
}

fn exact_requested(method: RankInference, n: usize) -> Result<bool, Error> {
    match method {
        RankInference::Auto => Ok(n <= MAX_EXACT_RANK_OBSERVATIONS),
        RankInference::Asymptotic => Ok(false),
        RankInference::PermutationExact if n <= MAX_EXACT_RANK_OBSERVATIONS => Ok(true),
        _ => Err(invalid(Violation::ParameterOutOfRange)),
    }
}

fn permutations(
    n: usize,
    observed: i64,
    alternative: Alternative,
    score: impl Fn(&[usize]) -> i64,
    control: &Control,
) -> Result<f64, Error> {
    let mut order = (0..n).collect::<Vec<_>>();
    let (mut total, mut lower, mut upper) = (0usize, 0usize, 0usize);
    loop {
        checkpoint(control, total)?;
        let value = score(&order);
        total += 1;
        lower += usize::from(value <= observed);
        upper += usize::from(value >= observed);
        let Some(pivot) = (0..n - 1).rev().find(|&i| order[i] < order[i + 1]) else {
            break;
        };
        let successor = (pivot + 1..n)
            .rev()
            .find(|&j| order[j] > order[pivot])
            .ok_or(Error::ComputationFailed)?;
        order.swap(pivot, successor);
        order[pivot + 1..].reverse();
    }
    control.check()?;
    bounded(
        match alternative {
            Alternative::TwoSided => 2.0 * lower.min(upper) as f64 / total as f64,
            Alternative::Greater => upper as f64 / total as f64,
            Alternative::Less => lower as f64 / total as f64,
        }
        .min(1.0),
        0.0,
        1.0,
    )
}

pub fn spearman(
    x: &[f64],
    y: &[f64],
    options: RankCorrelationOptions,
    control: &Control,
) -> Result<CorrelationResult, Error> {
    paired(x, y, 2, control)?;
    let rx = ranks(x, control)?;
    let ry = ranks(y, control)?;
    let r = coefficient(&rx.values, &ry.values, control)?;
    let inference = if exact_requested(options.inference, x.len())? {
        let n = x.len() as i64;
        let a = rx
            .values
            .iter()
            .map(|r| (2.0 * r) as i64 - n - 1)
            .collect::<Vec<_>>();
        let b = ry
            .values
            .iter()
            .map(|r| (2.0 * r) as i64 - n - 1)
            .collect::<Vec<_>>();
        let observed = a.iter().zip(&b).map(|(x, y)| x * y).sum();
        let p = permutations(
            x.len(),
            observed,
            options.alternative,
            |order| order.iter().enumerate().map(|(i, &j)| a[i] * b[j]).sum(),
            control,
        )?;
        TestInference {
            statistic_name: "rho",
            statistic: Some(r),
            degrees_of_freedom: vec![],
            p_value: Some(p),
            method: "permutation_exact",
        }
    } else {
        student_test(
            r,
            x.len()
                .checked_sub(2)
                .ok_or_else(|| invalid(Violation::EmptyInput))?,
            options.alternative,
            "student_t_approximation",
        )?
    };
    control.check()?;
    Ok(result(
        "spearman",
        r,
        x.len(),
        0,
        options.alternative,
        inference,
        None,
    ))
}

fn prefix(tree: &[u64], mut i: usize) -> u64 {
    let mut count = 0;
    while i > 0 {
        count += tree[i];
        i &= i - 1;
    }
    count
}
fn add(tree: &mut [u64], mut i: usize) {
    while i < tree.len() {
        tree[i] += 1;
        i += i & i.wrapping_neg();
    }
}
fn kendall_counts(x: &[f64], y: &[f64], control: &Control) -> Result<(u64, u64), Error> {
    let mut order = (0..x.len()).collect::<Vec<_>>();
    order.sort_unstable_by(|&a, &b| x[a].total_cmp(&x[b]).then(y[a].total_cmp(&y[b])));
    let mut unique = y.to_vec();
    unique.sort_unstable_by(f64::total_cmp);
    unique.dedup();
    let mut tree = vec![0; unique.len() + 1];
    let (mut concordant, mut discordant, mut seen, mut first) = (0u64, 0u64, 0u64, 0usize);
    while first < order.len() {
        control.check()?;
        let mut last = first + 1;
        while last < order.len() && x[order[last]] == x[order[first]] {
            checkpoint(control, last)?;
            last += 1;
        }
        for (step, &row) in order[first..last].iter().enumerate() {
            checkpoint(control, step)?;
            let rank = unique
                .binary_search_by(|value| value.total_cmp(&y[row]))
                .map_err(|_| Error::ComputationFailed)?
                + 1;
            concordant += prefix(&tree, rank - 1);
            discordant += seen - prefix(&tree, rank);
        }
        for (step, &row) in order[first..last].iter().enumerate() {
            checkpoint(control, step)?;
            let rank = unique
                .binary_search_by(|value| value.total_cmp(&y[row]))
                .map_err(|_| Error::ComputationFailed)?
                + 1;
            add(&mut tree, rank);
        }
        seen += (last - first) as u64;
        first = last;
    }
    control.check()?;
    Ok((concordant, discordant))
}

pub fn kendall(
    x: &[f64],
    y: &[f64],
    options: RankCorrelationOptions,
    control: &Control,
) -> Result<CorrelationResult, Error> {
    paired(x, y, 2, control)?;
    let rx = ranks(x, control)?;
    let ry = ranks(y, control)?;
    let (c, d) = kendall_counts(&rx.values, &ry.values, control)?;
    let n = x.len();
    let pairs = n as u64 * (n as u64 - 1) / 2;
    let denominator =
        ((pairs - rx.tie_pairs) as f64).sqrt() * ((pairs - ry.tie_pairs) as f64).sqrt();
    if denominator == 0.0 {
        return Err(invalid(Violation::DataOutOfRange));
    }
    let score = c as f64 - d as f64;
    let r = bounded(score / denominator, -1.0, 1.0)?;
    let inference = if exact_requested(options.inference, n)? {
        let p = permutations(
            n,
            c as i64 - d as i64,
            options.alternative,
            |order| {
                let mut score = 0;
                for i in 0..n {
                    for j in 0..i {
                        let a = rx.values[i].total_cmp(&rx.values[j]);
                        let b = ry.values[order[i]].total_cmp(&ry.values[order[j]]);
                        if !a.is_eq() && !b.is_eq() {
                            score += if a == b { 1 } else { -1 };
                        }
                    }
                }
                score
            },
            control,
        )?;
        TestInference {
            statistic_name: "tau_b",
            statistic: Some(r),
            degrees_of_freedom: vec![],
            p_value: Some(p),
            method: "permutation_exact",
        }
    } else {
        if n < 3 {
            return Err(invalid(Violation::EmptyInput));
        }
        let m = (n as f64) * (n - 1) as f64;
        let variance = (m * (2 * n + 5) as f64 - rx.tie_variance - ry.tie_variance) / 18.0
            + 2.0 * rx.tie_pairs as f64 * ry.tie_pairs as f64 / m
            + rx.tie_cubic * ry.tie_cubic / (9.0 * m * (n - 2) as f64);
        if !variance.is_finite() || variance <= 0.0 {
            return Err(Error::ComputationFailed);
        }
        let z = finite(score / variance.sqrt())?;
        TestInference {
            statistic_name: "z",
            statistic: Some(z),
            degrees_of_freedom: vec![],
            p_value: Some(normal_tail(z, options.alternative)?),
            method: "normal_approximation_with_ties",
        }
    };
    let mut output = result(
        "kendall_tau_b",
        r,
        n,
        0,
        options.alternative,
        inference,
        None,
    );
    output.concordant_pairs = Some(c);
    output.discordant_pairs = Some(d);
    output.tied_pairs_x = Some(rx.tie_pairs);
    output.tied_pairs_y = Some(ry.tie_pairs);
    control.check()?;
    Ok(output)
}
