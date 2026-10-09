//! Pearson, exact 2x2, and stratified categorical tests.
use super::{Error, Violation, checkpoint, failed, invalid};
use statrs::distribution::{Binomial, ChiSquared, ContinuousCDF, DiscreteCDF};
use statrs::function::gamma::ln_gamma;
use std::collections::BTreeMap;
use yss_sci_contract::execution::ScientificExecutionControl;
use yss_sci_contract::hypothesis::{CategoricalHypothesisTest as Input, ClassicalTestResult};

pub fn run(
    input: Input,
    control: &ScientificExecutionControl,
) -> Result<ClassicalTestResult, Error> {
    control.check()?;
    let result = match input {
        Input::Independence { row, column } => independence(row, column, control),
        Input::GoodnessOfFit { observed, expected } => goodness(observed, expected, control),
        Input::PearsonTable {
            observed,
            rows,
            columns,
        } => pearson_table(observed, rows, columns, control),
        Input::FisherExact { row, column } => fisher(row, column, control),
        Input::McNemar { before, after } => mcnemar(before, after, control),
        Input::Cmh {
            exposed,
            outcome,
            strata,
        } => cmh(exposed, outcome, strata, control),
        Input::MultipleProportions {
            successes_and_trials,
        } => multiple_proportions(successes_and_trials, control),
    };
    control.check()?;
    result
}

fn independence(
    row: Vec<Box<str>>,
    column: Vec<Box<str>>,
    control: &ScientificExecutionControl,
) -> Result<ClassicalTestResult, Error> {
    if row.len() != column.len() {
        return Err(invalid(Violation::ShapeMismatch));
    }
    if row.is_empty() {
        return Err(invalid(Violation::EmptyInput));
    }
    let table = count_table(row, column, control)?;
    let rows = table.len();
    let cols = table.first().map_or(0, Vec::len);
    if rows < 2 || cols < 2 {
        return Err(invalid(Violation::DataOutOfRange));
    }
    let mut n = 0u64;
    let mut row_totals = vec![0.0; rows];
    let mut col_totals = vec![0.0; cols];
    for (r, cells) in table.iter().enumerate() {
        for (c, count) in cells.iter().enumerate() {
            checkpoint(control, r * cols + c)?;
            n += count;
            row_totals[r] += *count as f64;
            col_totals[c] += *count as f64;
        }
    }
    let n = n as f64;
    let mut statistic = 0.0;
    let mut min_expected = f64::INFINITY;
    for r in 0..rows {
        for c in 0..cols {
            checkpoint(control, r * cols + c)?;
            let expected = row_totals[r] * col_totals[c] / n;
            if expected <= 0.0 {
                return Err(invalid(Violation::DataOutOfRange));
            }
            min_expected = min_expected.min(expected);
            statistic += (table[r][c] as f64 - expected).powi(2) / expected;
        }
    }
    let mut result = chi_result(
        "chisquare.crosstab",
        "row and column classifications are independent",
        statistic,
        ((rows - 1) * (cols - 1)) as f64,
        n as usize,
        control,
    )?;
    result
        .details
        .insert("minimum_expected_count".into(), min_expected);
    Ok(result)
}

fn goodness(
    observed: Vec<f64>,
    expected: Vec<f64>,
    control: &ScientificExecutionControl,
) -> Result<ClassicalTestResult, Error> {
    if observed.len() != expected.len() {
        return Err(invalid(Violation::ShapeMismatch));
    }
    if observed.len() < 2 {
        return Err(invalid(Violation::EmptyInput));
    }
    let mut statistic = 0.0;
    let mut n = 0.0;
    let mut expected_total = 0.0;
    for (i, (o, e)) in observed.iter().zip(&expected).enumerate() {
        checkpoint(control, i)?;
        if !o.is_finite() || !e.is_finite() {
            return Err(invalid(Violation::NonFiniteInput));
        }
        if *o < 0.0 || *e <= 0.0 {
            return Err(invalid(Violation::DataOutOfRange));
        }
        statistic += (o - e).powi(2) / e;
        n += o;
        expected_total += e;
    }
    if !n.is_finite() || !expected_total.is_finite() {
        return Err(failed());
    }
    if n <= 0.0 || (n - expected_total).abs() / n.min(expected_total) > f64::EPSILON.sqrt() {
        return Err(invalid(Violation::DataOutOfRange));
    }
    let df = (observed.len() - 1) as f64;
    chi_result(
        "chisquare.goodness_of_fit",
        "observed proportions match expected counts",
        statistic,
        df,
        n as usize,
        control,
    )
}

fn pearson_table(
    observed: Vec<f64>,
    rows: usize,
    columns: usize,
    control: &ScientificExecutionControl,
) -> Result<ClassicalTestResult, Error> {
    if rows.checked_mul(columns) != Some(observed.len()) {
        return Err(invalid(Violation::ShapeMismatch));
    }
    if rows < 2 || columns < 2 {
        return Err(invalid(Violation::DataOutOfRange));
    }
    let mut n = 0.0;
    let mut row_totals = vec![0.0; rows];
    let mut col_totals = vec![0.0; columns];
    for (i, count) in observed.iter().enumerate() {
        checkpoint(control, i)?;
        if !count.is_finite() {
            return Err(invalid(Violation::NonFiniteInput));
        }
        if *count < 0.0 || count.fract() != 0.0 {
            return Err(invalid(Violation::DataOutOfRange));
        }
        n += count;
        row_totals[i / columns] += count;
        col_totals[i % columns] += count;
    }
    let mut statistic = 0.0;
    let mut min_expected = f64::INFINITY;
    for r in 0..rows {
        for c in 0..columns {
            checkpoint(control, r * columns + c)?;
            let expected = row_totals[r] * col_totals[c] / n;
            if expected <= 0.0 {
                return Err(invalid(Violation::DataOutOfRange));
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
        control,
    )?;
    result
        .details
        .insert("minimum_expected_count".into(), min_expected);
    Ok(result)
}

fn fisher(
    row: Vec<Box<str>>,
    column: Vec<Box<str>>,
    control: &ScientificExecutionControl,
) -> Result<ClassicalTestResult, Error> {
    if row.len() != column.len() {
        return Err(invalid(Violation::ShapeMismatch));
    }
    if row.is_empty() {
        return Err(invalid(Violation::EmptyInput));
    }
    let table = count_table(row, column, control)?;
    if table.len() != 2 || table[0].len() != 2 {
        return Err(invalid(Violation::DataOutOfRange));
    }
    let [a, b, c, d] = [table[0][0], table[0][1], table[1][0], table[1][1]];
    let p_value = fisher_two_sided(a, b, c, d, control)?;
    let statistic = if b == 0 || c == 0 {
        None
    } else {
        Some((a as f64 * d as f64) / (b as f64 * c as f64))
    };
    Ok(ClassicalTestResult {
        method: "fisher_exact".into(),
        null_hypothesis: "row and column classifications are independent".into(),
        alternative: "two-sided".into(),
        statistic_name: "odds_ratio".into(),
        statistic,
        degrees_of_freedom: vec![],
        p_value,
        estimate: None,
        standard_error: None,
        sample_sizes: vec![usize::try_from(a + b + c + d).map_err(|_| failed())?],
        details: [
            ("a".into(), a as f64),
            ("b".into(), b as f64),
            ("c".into(), c as f64),
            ("d".into(), d as f64),
        ]
        .into(),
    })
}

fn mcnemar(
    before: Vec<f64>,
    after: Vec<f64>,
    control: &ScientificExecutionControl,
) -> Result<ClassicalTestResult, Error> {
    if before.len() != after.len() {
        return Err(invalid(Violation::ShapeMismatch));
    }
    if before.is_empty() {
        return Err(invalid(Violation::EmptyInput));
    }
    let (mut b, mut c) = (0u64, 0u64);
    for (i, (x, y)) in before.iter().zip(&after).enumerate() {
        checkpoint(control, i)?;
        if !x.is_finite() || !y.is_finite() {
            return Err(invalid(Violation::NonFiniteInput));
        }
        if (*x != 0.0 && *x != 1.0) || (*y != 0.0 && *y != 1.0) {
            return Err(invalid(Violation::DataOutOfRange));
        }
        if *x == 0.0 && *y == 1.0 {
            b += 1;
        } else if *x == 1.0 && *y == 0.0 {
            c += 1;
        }
    }
    let discordant = b + c;
    if discordant == 0 {
        return Err(invalid(Violation::DataOutOfRange));
    }
    let statistic = (b as f64 - c as f64).powi(2) / discordant as f64;
    let binomial = Binomial::new(0.5, discordant).map_err(|_| failed())?;
    control.check()?;
    let tail = binomial.cdf(b.min(c));
    control.check()?;
    let p_value = (2.0 * tail).min(1.0);
    let mut result = chi_result(
        "mcnemar",
        "paired marginal proportions are equal",
        statistic,
        1.0,
        before.len(),
        control,
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
    control: &ScientificExecutionControl,
) -> Result<ClassicalTestResult, Error> {
    if exposed.len() != outcome.len() || outcome.len() != strata.len() {
        return Err(invalid(Violation::ShapeMismatch));
    }
    if strata.is_empty() {
        return Err(invalid(Violation::EmptyInput));
    }
    let mut groups: BTreeMap<Box<str>, [f64; 4]> = BTreeMap::new();
    for (i, ((x, y), stratum)) in exposed.iter().zip(&outcome).zip(strata).enumerate() {
        checkpoint(control, i)?;
        if !x.is_finite() || !y.is_finite() {
            return Err(invalid(Violation::NonFiniteInput));
        }
        if (*x != 0.0 && *x != 1.0) || (*y != 0.0 && *y != 1.0) {
            return Err(invalid(Violation::DataOutOfRange));
        }
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
    for (i, [a, b, c, d]) in groups.values().enumerate() {
        checkpoint(control, i)?;
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
        return Err(invalid(Violation::DataOutOfRange));
    }
    chi_result(
        "cmh",
        "exposure and outcome are conditionally independent within strata",
        numerator.powi(2) / variance,
        1.0,
        exposed.len(),
        control,
    )
}

fn multiple_proportions(
    values: Vec<f64>,
    control: &ScientificExecutionControl,
) -> Result<ClassicalTestResult, Error> {
    if !values.len().is_multiple_of(2) {
        return Err(invalid(Violation::ShapeMismatch));
    }
    if values.len() < 6 {
        return Err(invalid(Violation::EmptyInput));
    }
    let groups = values.len() / 2;
    let mut counts = Vec::with_capacity(groups * 2);
    let mut total = 0.0;
    let mut successes = 0.0;
    for (i, pair) in values.chunks_exact(2).enumerate() {
        checkpoint(control, i)?;
        if pair.iter().any(|v| !v.is_finite()) {
            return Err(invalid(Violation::NonFiniteInput));
        }
        if pair.iter().any(|v| *v < 0.0 || v.fract() != 0.0) {
            return Err(invalid(Violation::DataOutOfRange));
        }
        let success = pair[0];
        let n = pair[1];
        if n == 0.0 || success > n {
            return Err(invalid(Violation::DataOutOfRange));
        }
        counts.push(success);
        counts.push(n - success);
        successes += success;
        total += n;
    }
    pearson_table(counts, 2, groups, control).map(|mut result| {
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
    control: &ScientificExecutionControl,
) -> Result<Vec<Vec<u64>>, Error> {
    let mut rows = BTreeMap::<&str, usize>::new();
    let mut columns = BTreeMap::<&str, usize>::new();
    for (i, (r, c)) in row.iter().zip(&column).enumerate() {
        checkpoint(control, i)?;
        rows.entry(r.as_ref()).or_default();
        columns.entry(c.as_ref()).or_default();
    }
    for (i, index) in rows.values_mut().enumerate() {
        checkpoint(control, i)?;
        *index = i;
    }
    for (i, index) in columns.values_mut().enumerate() {
        checkpoint(control, i)?;
        *index = i;
    }
    let mut table = Vec::with_capacity(rows.len());
    for _ in 0..rows.len() {
        control.check()?;
        table.push(vec![0u64; columns.len()]);
    }
    for (i, (r, c)) in row.iter().zip(&column).enumerate() {
        checkpoint(control, i)?;
        table[rows[r.as_ref()]][columns[c.as_ref()]] += 1;
    }
    Ok(table)
}

fn fisher_two_sided(
    a: u64,
    b: u64,
    c: u64,
    d: u64,
    control: &ScientificExecutionControl,
) -> Result<f64, Error> {
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
    let mut p = 0.0;
    for (i, x) in (low..=high).enumerate() {
        checkpoint(control, i)?;
        let value = log_p(x);
        if value <= observed + 1e-12 {
            p += value.exp();
        }
    }
    Ok(p.clamp(0.0, 1.0))
}

fn chi_result(
    method: &str,
    null: &str,
    statistic: f64,
    df: f64,
    observations: usize,
    control: &ScientificExecutionControl,
) -> Result<ClassicalTestResult, Error> {
    if !statistic.is_finite() || statistic < 0.0 || df <= 0.0 || !df.is_finite() {
        return Err(failed());
    }
    let distribution = ChiSquared::new(df).map_err(|_| failed())?;
    control.check()?;
    let p_value = distribution.sf(statistic).clamp(0.0, 1.0);
    control.check()?;
    Ok(ClassicalTestResult {
        method: method.into(),
        null_hypothesis: null.into(),
        alternative: "upper-tail".into(),
        statistic_name: "chi_squared".into(),
        statistic: Some(statistic),
        degrees_of_freedom: vec![df],
        p_value,
        estimate: None,
        standard_error: None,
        sample_sizes: vec![observations],
        details: Default::default(),
    })
}
