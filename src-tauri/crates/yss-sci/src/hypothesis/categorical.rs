//! Pearson, exact 2x2, and stratified categorical tests.
use statrs::distribution::{Binomial, ChiSquared, ContinuousCDF, DiscreteCDF};
use statrs::function::gamma::ln_gamma;
use std::collections::{BTreeMap, BTreeSet};
use yss_sci_contract::hypothesis::{CategoricalHypothesisTest as Input, ClassicalTestResult};

pub fn run(input: Input) -> Result<ClassicalTestResult, String> {
    match input {
        Input::Independence { row, column } => independence(row, column),
        Input::GoodnessOfFit { observed, expected } => goodness(observed, expected),
        Input::PearsonTable {
            observed,
            rows,
            columns,
        } => pearson_table(observed, rows, columns),
        Input::FisherExact { row, column } => fisher(row, column),
        Input::McNemar { before, after } => mcnemar(before, after),
        Input::Cmh {
            exposed,
            outcome,
            strata,
        } => cmh(exposed, outcome, strata),
        Input::MultipleProportions {
            successes_and_trials,
        } => multiple_proportions(successes_and_trials),
    }
}

fn independence(row: Vec<Box<str>>, column: Vec<Box<str>>) -> Result<ClassicalTestResult, String> {
    if row.len() != column.len() || row.is_empty() {
        return Err("crosstab requires aligned non-empty categorical columns".into());
    }
    let (rows, cols, table) = count_table(row, column)?;
    if rows.len() < 2 || cols.len() < 2 {
        return Err("independence test requires at least two levels in both variables".into());
    }
    let n = table.iter().flatten().sum::<u64>() as f64;
    let row_totals = table
        .iter()
        .map(|cells| cells.iter().sum::<u64>() as f64)
        .collect::<Vec<_>>();
    let col_totals = (0..cols.len())
        .map(|c| table.iter().map(|cells| cells[c]).sum::<u64>() as f64)
        .collect::<Vec<_>>();
    let mut statistic = 0.0;
    let mut min_expected = f64::INFINITY;
    for r in 0..rows.len() {
        for c in 0..cols.len() {
            let expected = row_totals[r] * col_totals[c] / n;
            if expected <= 0.0 {
                return Err("crosstab contains an empty marginal".into());
            }
            min_expected = min_expected.min(expected);
            statistic += (table[r][c] as f64 - expected).powi(2) / expected;
        }
    }
    let mut result = chi_result(
        "chisquare.crosstab",
        "row and column classifications are independent",
        statistic,
        ((rows.len() - 1) * (cols.len() - 1)) as f64,
        n as usize,
    )?;
    result
        .details
        .insert("minimum_expected_count".into(), min_expected);
    Ok(result)
}

fn goodness(observed: Vec<f64>, expected: Vec<f64>) -> Result<ClassicalTestResult, String> {
    if observed.len() != expected.len()
        || observed.len() < 2
        || observed
            .iter()
            .chain(&expected)
            .any(|v| !v.is_finite() || *v < 0.0)
        || expected.contains(&0.0)
    {
        return Err(
            "goodness-of-fit requires matching positive expected and non-negative observed counts"
                .into(),
        );
    }
    let statistic = observed
        .iter()
        .zip(&expected)
        .map(|(o, e)| (o - e).powi(2) / e)
        .sum::<f64>();
    let df = (observed.len() - 1) as f64;
    chi_result(
        "chisquare.goodness_of_fit",
        "observed proportions match expected counts",
        statistic,
        df,
        observed.iter().sum::<f64>() as usize,
    )
}

fn pearson_table(
    observed: Vec<f64>,
    rows: usize,
    columns: usize,
) -> Result<ClassicalTestResult, String> {
    if rows < 2
        || columns < 2
        || rows.checked_mul(columns) != Some(observed.len())
        || observed
            .iter()
            .any(|v| !v.is_finite() || *v < 0.0 || v.fract() != 0.0)
    {
        return Err(
            "Pearson table requires a rectangular count table with at least two rows and columns"
                .into(),
        );
    }
    let n = observed.iter().sum::<f64>();
    let row_totals = (0..rows)
        .map(|r| observed[r * columns..(r + 1) * columns].iter().sum::<f64>())
        .collect::<Vec<_>>();
    let col_totals = (0..columns)
        .map(|c| (0..rows).map(|r| observed[r * columns + c]).sum::<f64>())
        .collect::<Vec<_>>();
    let mut statistic = 0.0;
    let mut min_expected = f64::INFINITY;
    for r in 0..rows {
        for c in 0..columns {
            let expected = row_totals[r] * col_totals[c] / n;
            if expected <= 0.0 {
                return Err("Pearson table contains an empty marginal".into());
            }
            min_expected = min_expected.min(expected);
            statistic += (observed[r * columns + c] - expected).powi(2) / expected;
        }
    }
    let mut result = chi_result(
        "chisquare.general",
        "row and column classifications are independent",
        statistic,
        ((rows - 1) * (columns - 1)) as f64,
        n as usize,
    )?;
    result
        .details
        .insert("minimum_expected_count".into(), min_expected);
    Ok(result)
}

fn fisher(row: Vec<Box<str>>, column: Vec<Box<str>>) -> Result<ClassicalTestResult, String> {
    if row.len() != column.len() || row.is_empty() {
        return Err("Fisher exact test requires aligned categorical columns".into());
    }
    let (categories_row, categories_col, table) = count_table(row, column)?;
    if categories_row.len() != 2 || categories_col.len() != 2 {
        return Err("Fisher exact test currently requires a 2 by 2 table".into());
    }
    let [a, b, c, d] = [table[0][0], table[0][1], table[1][0], table[1][1]];
    let p_value = fisher_two_sided(a, b, c, d)?;
    Ok(ClassicalTestResult {
        method: "fisher_exact".into(),
        null_hypothesis: "row and column classifications are independent".into(),
        alternative: "two-sided".into(),
        statistic_name: "odds_ratio".into(),
        statistic: (a as f64 * d as f64) - (b as f64 * c as f64),
        degrees_of_freedom: vec![],
        p_value,
        estimate: None,
        standard_error: None,
        sample_sizes: vec![
            usize::try_from(a + b + c + d).map_err(|_| "Fisher table count overflow")?,
        ],
        details: [
            ("a".into(), a as f64),
            ("b".into(), b as f64),
            ("c".into(), c as f64),
            ("d".into(), d as f64),
        ]
        .into(),
    })
}

fn mcnemar(before: Vec<f64>, after: Vec<f64>) -> Result<ClassicalTestResult, String> {
    if before.is_empty()
        || before.len() != after.len()
        || before.iter().chain(&after).any(|v| *v != 0.0 && *v != 1.0)
    {
        return Err("McNemar test requires aligned paired binary observations".into());
    }
    let (mut b, mut c) = (0u64, 0u64);
    for (x, y) in before.iter().zip(&after) {
        if *x == 0.0 && *y == 1.0 {
            b += 1;
        } else if *x == 1.0 && *y == 0.0 {
            c += 1;
        }
    }
    let discordant = b + c;
    if discordant == 0 {
        return Err("McNemar test has no discordant pairs".into());
    }
    let statistic = (b as f64 - c as f64).powi(2) / discordant as f64;
    let binomial =
        Binomial::new(0.5, discordant).map_err(|_| "invalid McNemar exact distribution")?;
    let tail = binomial.cdf(b.min(c));
    let p_value = (2.0 * tail).min(1.0);
    let mut result = chi_result(
        "mcnemar",
        "paired marginal proportions are equal",
        statistic,
        1.0,
        before.len(),
    )?;
    result.p_value = p_value;
    result.details.insert("discordant_0_to_1".into(), b as f64);
    result.details.insert("discordant_1_to_0".into(), c as f64);
    result
        .details
        .insert("exact_binomial_p_value".into(), p_value);
    Ok(result)
}

fn cmh(
    exposed: Vec<f64>,
    outcome: Vec<f64>,
    strata: Vec<Box<str>>,
) -> Result<ClassicalTestResult, String> {
    if exposed.len() != outcome.len()
        || outcome.len() != strata.len()
        || strata.is_empty()
        || exposed
            .iter()
            .chain(&outcome)
            .any(|v| *v != 0.0 && *v != 1.0)
    {
        return Err(
            "CMH test requires aligned binary exposure, outcome and stratum columns".into(),
        );
    }
    let mut groups: BTreeMap<Box<str>, [f64; 4]> = BTreeMap::new();
    for ((x, y), stratum) in exposed.iter().zip(&outcome).zip(strata) {
        let table = groups.entry(stratum).or_default();
        let index = match (*x, *y) {
            (1.0, 1.0) => 0,
            (1.0, 0.0) => 1,
            (0.0, 1.0) => 2,
            _ => 3,
        };
        table[index] += 1.0;
    }
    let (mut numerator, mut variance) = (0.0, 0.0);
    for [a, b, c, d] in groups.values() {
        let n = a + b + c + d;
        if n <= 1.0 {
            continue;
        }
        let row1 = a + b;
        let col1 = a + c;
        let expected = row1 * col1 / n;
        numerator += a - expected;
        variance += row1 * (n - row1) * col1 * (n - col1) / (n * n * (n - 1.0));
    }
    if variance <= 0.0 {
        return Err("CMH test has no within-stratum information".into());
    }
    chi_result(
        "cmh",
        "exposure and outcome are conditionally independent within strata",
        numerator.powi(2) / variance,
        1.0,
        exposed.len(),
    )
}

fn multiple_proportions(values: Vec<f64>) -> Result<ClassicalTestResult, String> {
    if values.len() < 6
        || values.len() % 2 != 0
        || values
            .iter()
            .any(|v| !v.is_finite() || *v < 0.0 || v.fract() != 0.0)
    {
        return Err("multiple-proportion test requires at least three success/total pairs".into());
    }
    let groups = values.len() / 2;
    let mut counts = Vec::with_capacity(groups * 2);
    let mut total = 0.0;
    let mut successes = 0.0;
    for pair in values.chunks_exact(2) {
        let success = pair[0];
        let n = pair[1];
        if n == 0.0 || success > n {
            return Err("invalid success and total counts".into());
        }
        counts.push(success);
        counts.push(n - success);
        successes += success;
        total += n;
    }
    pearson_table(counts, 2, groups).map(|mut result| {
        result.method = "proportion.multiple".into();
        result.null_hypothesis = "all group proportions are equal".into();
        result.sample_sizes = vec![total as usize];
        result
            .details
            .insert("overall_proportion".into(), successes / total);
        result
    })
}

fn count_table(
    row: Vec<Box<str>>,
    column: Vec<Box<str>>,
) -> Result<(Vec<Box<str>>, Vec<Box<str>>, Vec<Vec<u64>>), String> {
    let rows = row
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let columns = column
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let ri = rows
        .iter()
        .enumerate()
        .map(|(i, v)| (v.as_ref(), i))
        .collect::<BTreeMap<_, _>>();
    let ci = columns
        .iter()
        .enumerate()
        .map(|(i, v)| (v.as_ref(), i))
        .collect::<BTreeMap<_, _>>();
    let mut table = vec![vec![0u64; columns.len()]; rows.len()];
    for (r, c) in row.iter().zip(&column) {
        table[ri[r.as_ref()]][ci[c.as_ref()]] += 1;
    }
    Ok((rows, columns, table))
}

fn fisher_two_sided(a: u64, b: u64, c: u64, d: u64) -> Result<f64, String> {
    let r1 = a + b;
    let r2 = c + d;
    let c1 = a + c;
    let n = r1 + r2;
    let low = c1.saturating_sub(r2);
    let high = r1.min(c1);
    let log_choose = |n: u64, k: u64| {
        ln_gamma(n as f64 + 1.0) - ln_gamma(k as f64 + 1.0) - ln_gamma((n - k) as f64 + 1.0)
    };
    let log_p = |x: u64| log_choose(r1, x) + log_choose(r2, c1 - x) - log_choose(n, c1);
    let observed = log_p(a);
    let p = (low..=high)
        .filter_map(|x| {
            let value = log_p(x);
            (value <= observed + 1e-12).then_some(value.exp())
        })
        .sum::<f64>();
    Ok(p.clamp(0.0, 1.0))
}

fn chi_result(
    method: &str,
    null: &str,
    statistic: f64,
    df: f64,
    observations: usize,
) -> Result<ClassicalTestResult, String> {
    if !statistic.is_finite() || statistic < 0.0 || df <= 0.0 || !df.is_finite() {
        return Err("invalid chi-square statistic or degrees of freedom".into());
    }
    let distribution = ChiSquared::new(df).map_err(|_| "invalid chi-square distribution")?;
    Ok(ClassicalTestResult {
        method: method.into(),
        null_hypothesis: null.into(),
        alternative: "upper-tail".into(),
        statistic_name: "chi_squared".into(),
        statistic,
        degrees_of_freedom: vec![df],
        p_value: distribution.sf(statistic).clamp(0.0, 1.0),
        estimate: None,
        standard_error: None,
        sample_sizes: vec![observations],
        details: Default::default(),
    })
}
