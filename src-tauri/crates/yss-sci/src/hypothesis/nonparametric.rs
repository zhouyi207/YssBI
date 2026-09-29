//! Rank, sign, and sequence tests with explicit small-sample and tie handling.
use statrs::distribution::{ChiSquared, ContinuousCDF, Normal};
use yss_sci_contract::hypothesis::{Alternative, ClassicalTestResult, RankHypothesisTest as Input};

pub fn run(input: Input) -> Result<ClassicalTestResult, String> {
    match input {
        Input::WilcoxonOneSample {
            values,
            null_median,
            alternative,
        } => {
            if !null_median.is_finite() {
                return Err("Wilcoxon null median must be finite".into());
            }
            let differences = values
                .into_iter()
                .map(|v| v - null_median)
                .collect::<Vec<_>>();
            wilcoxon(differences, alternative, "wilcoxon.one_sample")
        }
        Input::WilcoxonPaired {
            before,
            after,
            alternative,
        } => {
            if before.len() != after.len() {
                return Err("paired Wilcoxon inputs must align".into());
            }
            wilcoxon(
                before.iter().zip(after).map(|(a, b)| a - b).collect(),
                alternative,
                "wilcoxon.paired",
            )
        }
        Input::MannWhitney {
            first,
            second,
            alternative,
        } => mann_whitney(first, second, alternative),
        Input::KruskalWallis { groups } => kruskal_wallis(groups),
        Input::Friedman { conditions } => friedman(conditions),
        Input::CochranQ { conditions } => cochran_q(conditions),
        Input::Runs { values } => runs(values),
        Input::MoodMedian { groups } => mood_median(groups),
        Input::MannKendall {
            values,
            alternative,
        } => mann_kendall(values, alternative),
    }
}

fn wilcoxon(
    differences: Vec<f64>,
    alternative: Alternative,
    method: &str,
) -> Result<ClassicalTestResult, String> {
    validate(&differences)?;
    let nonzero = differences
        .into_iter()
        .filter(|x| *x != 0.0)
        .collect::<Vec<_>>();
    let n = nonzero.len();
    if n < 2 {
        return Err("Wilcoxon signed-rank test requires at least two non-zero differences".into());
    }
    let abs = nonzero.iter().map(|v| v.abs()).collect::<Vec<_>>();
    let (ranks, _) = ranks(&abs)?;
    let observed = nonzero
        .iter()
        .zip(&ranks)
        .filter_map(|(d, r)| (*d > 0.0).then_some(*r))
        .sum::<f64>();
    let mean = ranks.iter().sum::<f64>() / 2.0;
    let variance = ranks.iter().map(|r| r * r).sum::<f64>() / 4.0;
    if variance <= 0.0 {
        return Err("Wilcoxon rank variance is zero".into());
    }
    let exact = if n <= 20 {
        Some(wilcoxon_exact(&ranks, observed, alternative))
    } else {
        None
    };
    let z = (observed - mean) / variance.sqrt();
    let p = exact.unwrap_or(normal_p(z, alternative));
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

fn wilcoxon_exact(ranks: &[f64], observed: f64, alternative: Alternative) -> f64 {
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
    extreme as f64 / combinations as f64
}

fn mann_whitney(
    first: Vec<f64>,
    second: Vec<f64>,
    alternative: Alternative,
) -> Result<ClassicalTestResult, String> {
    validate(&first)?;
    validate(&second)?;
    if first.is_empty() || second.is_empty() {
        return Err("Mann–Whitney requires two non-empty samples".into());
    }
    let n1 = first.len();
    let n2 = second.len();
    let combined = first
        .iter()
        .map(|v| (*v, 0usize))
        .chain(second.iter().map(|v| (*v, 1usize)))
        .collect::<Vec<_>>();
    let values = combined.iter().map(|x| x.0).collect::<Vec<_>>();
    let (rank, ties) = ranks(&values)?;
    let rank1 = combined
        .iter()
        .enumerate()
        .filter_map(|(i, (_, g))| (*g == 0).then_some(rank[i]))
        .sum::<f64>();
    let u1 = rank1 - (n1 * (n1 + 1) / 2) as f64;
    let total = n1 + n2;
    let tie_term = ties.iter().map(|t| (t.pow(3) - t) as f64).sum::<f64>();
    let variance = n1 as f64 * n2 as f64 / 12.0
        * ((total + 1) as f64 - tie_term / (total * (total - 1)) as f64);
    if variance <= 0.0 {
        return Err("Mann–Whitney rank variance is zero".into());
    }
    let z = (u1 - (n1 * n2) as f64 / 2.0) / variance.sqrt();
    let mut result = base(
        "mann_whitney",
        "the two populations have the same distribution",
        "U",
        u1,
        vec![],
        normal_p(z, alternative),
        vec![n1, n2],
    );
    result.alternative = alternative_name(alternative).into();
    result
        .details
        .insert("u_complement".into(), (n1 * n2) as f64 - u1);
    Ok(result)
}

fn kruskal_wallis(groups: Vec<Vec<f64>>) -> Result<ClassicalTestResult, String> {
    if groups.len() < 2 || groups.iter().any(Vec::is_empty) {
        return Err("Kruskal–Wallis requires at least two non-empty groups".into());
    }
    groups.iter().try_for_each(|g| validate(g))?;
    let sizes = groups.iter().map(Vec::len).collect::<Vec<_>>();
    let vals = groups.iter().flatten().copied().collect::<Vec<_>>();
    let (rank, ties) = ranks(&vals)?;
    let n = vals.len();
    let mut offset = 0;
    let mut sum = 0.0;
    for size in &sizes {
        let rs = rank[offset..offset + size].iter().sum::<f64>();
        sum += rs * rs / *size as f64;
        offset += size;
    }
    let raw = 12.0 / (n as f64 * (n + 1) as f64) * sum - 3.0 * (n + 1) as f64;
    let corr =
        1.0 - ties.iter().map(|t| (t.pow(3) - t) as f64).sum::<f64>() / ((n * n * n - n) as f64);
    if corr <= 0.0 {
        return Err("Kruskal–Wallis tie correction is zero".into());
    }
    let statistic = raw / corr;
    let df = (groups.len() - 1) as f64;
    let p = ChiSquared::new(df)
        .map_err(|_| "invalid Kruskal–Wallis chi-square")?
        .sf(statistic);
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

fn friedman(conditions: Vec<Vec<f64>>) -> Result<ClassicalTestResult, String> {
    if conditions.len() < 3 || conditions.iter().any(Vec::is_empty) {
        return Err("Friedman test requires at least three non-empty repeated conditions".into());
    }
    conditions.iter().try_for_each(|g| validate(g))?;
    let k = conditions.len();
    let n = conditions[0].len();
    if n < 2 || conditions.iter().any(|g| g.len() != n) {
        return Err("Friedman conditions must contain at least two aligned subjects".into());
    }
    let mut sums = vec![0.0; k];
    let mut tie_total = 0.0;
    for row in 0..n {
        let values = conditions.iter().map(|g| g[row]).collect::<Vec<_>>();
        let (r, t) = ranks(&values)?;
        for j in 0..k {
            sums[j] += r[j];
        }
        tie_total += t.iter().map(|v| (v.pow(3) - v) as f64).sum::<f64>();
    }
    let raw = 12.0 / (n * k * (k + 1)) as f64 * sums.iter().map(|r| r * r).sum::<f64>()
        - 3.0 * n as f64 * (k + 1) as f64;
    let correction = 1.0 - tie_total / (n as f64 * k as f64 * ((k * k - 1) as f64));
    if correction <= 0.0 {
        return Err("Friedman tie correction is zero".into());
    }
    let statistic = raw / correction;
    let df = (k - 1) as f64;
    let p = ChiSquared::new(df)
        .map_err(|_| "invalid Friedman chi-square")?
        .sf(statistic);
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

fn cochran_q(conditions: Vec<Vec<f64>>) -> Result<ClassicalTestResult, String> {
    if conditions.len() < 3 || conditions.iter().any(Vec::is_empty) {
        return Err("Cochran Q requires at least three non-empty paired conditions".into());
    }
    let n = conditions[0].len();
    let k = conditions.len();
    if n < 2
        || conditions
            .iter()
            .any(|g| g.len() != n || g.iter().any(|x| *x != 0.0 && *x != 1.0))
    {
        return Err("Cochran Q requires aligned binary condition values".into());
    }
    let col = conditions
        .iter()
        .map(|g| g.iter().sum::<f64>())
        .collect::<Vec<_>>();
    let rows = (0..n)
        .map(|i| conditions.iter().map(|g| g[i]).sum::<f64>())
        .collect::<Vec<_>>();
    let total = col.iter().sum::<f64>();
    let denom = k as f64 * total - rows.iter().map(|x| x * x).sum::<f64>();
    if denom <= 0.0 {
        return Err("Cochran Q has no outcome variation".into());
    }
    let statistic = (k - 1) as f64
        * (k as f64 * col.iter().map(|x| x * x).sum::<f64>() - total * total)
        / denom;
    let df = (k - 1) as f64;
    let p = ChiSquared::new(df)
        .map_err(|_| "invalid Cochran Q chi-square")?
        .sf(statistic);
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

fn runs(values: Vec<f64>) -> Result<ClassicalTestResult, String> {
    if values.len() < 2 || values.iter().any(|x| *x != 0.0 && *x != 1.0) {
        return Err(
            "runs test requires at least two binary 0/1 observations in sequence order".into(),
        );
    }
    let n1 = values.iter().filter(|x| **x == 1.0).count();
    let n0 = values.len() - n1;
    let n = values.len();
    if n0 == 0 || n1 == 0 {
        return Err("runs test requires both categories".into());
    }
    let observed = 1 + values.windows(2).filter(|w| w[0] != w[1]).count();
    let mean = 1.0 + 2.0 * n0 as f64 * n1 as f64 / n as f64;
    let variance = 2.0 * n0 as f64 * n1 as f64 * (2.0 * n0 as f64 * n1 as f64 - n as f64)
        / (n * n * (n - 1)) as f64;
    if variance <= 0.0 {
        return Err("runs variance is zero".into());
    }
    let z = (observed as f64 - mean) / variance.sqrt();
    Ok(base(
        "runs",
        "sequence order is random",
        "runs_z",
        z,
        vec![],
        normal_p(z, Alternative::TwoSided),
        vec![n],
    ))
}

fn mood_median(groups: Vec<Vec<f64>>) -> Result<ClassicalTestResult, String> {
    if groups.len() < 2 || groups.iter().any(Vec::is_empty) {
        return Err("Mood median test requires at least two non-empty groups".into());
    }
    groups.iter().try_for_each(|g| validate(g))?;
    let mut all = groups.iter().flatten().copied().collect::<Vec<_>>();
    all.sort_by(f64::total_cmp);
    let median = if all.len() % 2 == 0 {
        (all[all.len() / 2 - 1] + all[all.len() / 2]) / 2.0
    } else {
        all[all.len() / 2]
    };
    let mut above = vec![0.0; groups.len()];
    let mut below = vec![0.0; groups.len()];
    for (i, g) in groups.iter().enumerate() {
        for x in g {
            if *x > median {
                above[i] += 1.0
            } else if *x < median {
                below[i] += 1.0
            }
        }
    }
    let mut table = Vec::with_capacity(groups.len() * 2);
    for i in 0..groups.len() {
        table.push(above[i]);
        table.push(below[i]);
    }
    let mut result = chi_table(
        "mood_median",
        "group medians are equal",
        table,
        2,
        groups.len(),
    )?;
    result.details.insert("pooled_median".into(), median);
    result.sample_sizes = groups.iter().map(Vec::len).collect();
    Ok(result)
}

fn mann_kendall(values: Vec<f64>, alternative: Alternative) -> Result<ClassicalTestResult, String> {
    validate(&values)?;
    let n = values.len();
    if n < 3 {
        return Err("Mann–Kendall requires at least three ordered observations".into());
    }
    let mut ordered = values.clone();
    ordered.sort_by(f64::total_cmp);
    ordered.dedup_by(|a, b| *a == *b);
    let mut tree = vec![0i64; ordered.len() + 1];
    let mut seen = 0i64;
    let mut s = 0i64;
    for value in &values {
        let rank = ordered
            .binary_search_by(|candidate| candidate.total_cmp(value))
            .expect("compressed value")
            + 1;
        let less = fenwick_sum(&tree, rank - 1);
        let less_or_equal = fenwick_sum(&tree, rank);
        s += less - (seen - less_or_equal);
        let mut index = rank;
        while index < tree.len() {
            tree[index] += 1;
            index += index & index.wrapping_neg();
        }
        seen += 1;
    }
    let (_, ties) = ranks(&values)?;
    let tie = ties.iter().map(|t| (t.pow(3) - t) as f64).sum::<f64>();
    let variance = (n * (n - 1) * (2 * n + 5)) as f64 / 18.0 - tie / 18.0;
    if variance <= 0.0 {
        return Err("Mann–Kendall variance is zero".into());
    }
    let corrected = if s > 0 {
        (s - 1) as f64
    } else if s < 0 {
        (s + 1) as f64
    } else {
        0.0
    };
    let z = corrected / variance.sqrt();
    let p = normal_p(z, alternative);
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
    result
        .details
        .insert("kendall_tau".into(), s as f64 / (n * (n - 1) / 2) as f64);
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

fn ranks(values: &[f64]) -> Result<(Vec<f64>, Vec<usize>), String> {
    if values.iter().any(|x| !x.is_finite()) {
        return Err("rank test input contains non-finite values".into());
    }
    let mut order = (0..values.len()).collect::<Vec<_>>();
    order.sort_by(|a, b| values[*a].total_cmp(&values[*b]));
    let mut output = vec![0.0; values.len()];
    let mut ties = Vec::new();
    let mut i = 0;
    while i < order.len() {
        let mut j = i + 1;
        while j < order.len() && values[order[i]] == values[order[j]] {
            j += 1;
        }
        let rank = (i + 1 + j) as f64 / 2.0;
        for index in &order[i..j] {
            output[*index] = rank;
        }
        ties.push(j - i);
        i = j;
    }
    Ok((output, ties))
}

fn validate(values: &[f64]) -> Result<(), String> {
    if values.iter().all(|x| x.is_finite()) {
        Ok(())
    } else {
        Err("rank test input contains non-finite values".into())
    }
}
fn normal_p(z: f64, a: Alternative) -> f64 {
    let n = Normal::new(0.0, 1.0).expect("standard normal");
    match a {
        Alternative::TwoSided => 2.0 * n.sf(z.abs()),
        Alternative::Greater => n.sf(z),
        Alternative::Less => n.cdf(z),
    }
    .clamp(0.0, 1.0)
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
        statistic: stat,
        degrees_of_freedom: df,
        p_value: p.clamp(0.0, 1.0),
        estimate: None,
        standard_error: None,
        sample_sizes: n,
        details: Default::default(),
    }
}
fn chi_table(
    method: &str,
    null: &str,
    observed: Vec<f64>,
    rows: usize,
    cols: usize,
) -> Result<ClassicalTestResult, String> {
    let n = observed.iter().sum::<f64>();
    let rt = (0..rows)
        .map(|r| observed[r * cols..(r + 1) * cols].iter().sum::<f64>())
        .collect::<Vec<_>>();
    let ct = (0..cols)
        .map(|c| (0..rows).map(|r| observed[r * cols + c]).sum::<f64>())
        .collect::<Vec<_>>();
    let mut x = 0.0;
    for r in 0..rows {
        for c in 0..cols {
            let e = rt[r] * ct[c] / n;
            if e <= 0.0 {
                return Err("Mood median table has an empty margin".into());
            }
            x += (observed[r * cols + c] - e).powi(2) / e;
        }
    }
    let df = ((rows - 1) * (cols - 1)) as f64;
    let p = ChiSquared::new(df).map_err(|_| "invalid chi-square")?.sf(x);
    Ok(base(method, null, "chi_squared", x, vec![df], p, vec![]))
}
