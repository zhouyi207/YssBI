//! Mean/count tests with explicit data and option admission; observations are not dropped.
use super::{Error, Violation, checkpoint, failed, finite, invalid, parameter};
use statrs::distribution::{
    Binomial, ContinuousCDF, Discrete, DiscreteCDF, Normal, Poisson, StudentsT,
};
use yss_sci_contract::execution::ScientificExecutionControl;
use yss_sci_contract::hypothesis::{
    Alternative, ClassicalHypothesisTest, ClassicalTestResult, SummaryTDesign,
};

pub fn run(
    input: ClassicalHypothesisTest,
    control: &ScientificExecutionControl,
) -> Result<ClassicalTestResult, Error> {
    control.check()?;
    let result = match input {
        ClassicalHypothesisTest::OneSample {
            values,
            null_mean,
            alternative,
        } => {
            validate(&values, control)?;
            if values.len() < 2 {
                return Err(invalid(Violation::EmptyInput));
            }
            if !null_mean.is_finite() {
                return Err(parameter());
            }
            let n = values.len();
            let mean = sample_sum(&values, control)? / n as f64;
            let variance = sample_variance(&values, mean, control)?;
            finish_t(
                "t.one_sample",
                format!("mean = {null_mean}"),
                mean - null_mean,
                ((variance / n as f64).sqrt(), (n - 1) as f64),
                alternative,
                vec![n],
                control,
            )
        }
        ClassicalHypothesisTest::Independent {
            first,
            second,
            equal_variance,
            alternative,
        } => {
            validate(&first, control)?;
            validate(&second, control)?;
            if first.len() < 2 || second.len() < 2 {
                return Err(invalid(Violation::EmptyInput));
            }
            independent(
                &first,
                &second,
                equal_variance,
                0.0,
                alternative,
                "t.independent",
                control,
            )
        }
        ClassicalHypothesisTest::Paired {
            before,
            after,
            alternative,
        } => {
            if before.len() != after.len() {
                return Err(invalid(Violation::ShapeMismatch));
            }
            validate(&before, control)?;
            validate(&after, control)?;
            if before.len() < 2 {
                return Err(invalid(Violation::EmptyInput));
            }
            let differences = before
                .iter()
                .zip(after)
                .enumerate()
                .map(|(index, (a, b))| {
                    checkpoint(control, index)?;
                    finite(a - b)
                })
                .collect::<Result<Vec<_>, Error>>()?;
            let n = differences.len();
            let mean = sample_sum(&differences, control)? / n as f64;
            let variance = sample_variance(&differences, mean, control)?;
            finish_t(
                "t.paired",
                "mean(before - after) = 0".into(),
                mean,
                ((variance / n as f64).sqrt(), (n - 1) as f64),
                alternative,
                vec![n],
                control,
            )
        }
        ClassicalHypothesisTest::Summary {
            design,
            first_count,
            first_mean,
            first_sd,
            second_count,
            second_mean,
            second_sd,
            null_difference,
            equal_variance,
            alternative,
        } => {
            validate_summary(first_count, first_mean, first_sd)?;
            if !null_difference.is_finite() {
                return Err(parameter());
            }
            match (design, second_count, second_mean, second_sd) {
                (SummaryTDesign::Paired, None, None, None) => {
                    let se = first_sd / (first_count as f64).sqrt();
                    finish_t(
                        "t.summary_paired",
                        format!("mean difference = {null_difference}"),
                        first_mean - null_difference,
                        (se, (first_count - 1) as f64),
                        alternative,
                        vec![first_count],
                        control,
                    )
                }
                (SummaryTDesign::OneSample, None, None, None) => {
                    let se = first_sd / (first_count as f64).sqrt();
                    finish_t(
                        "t.summary_one_sample",
                        format!("mean = {null_difference}"),
                        first_mean - null_difference,
                        (se, (first_count - 1) as f64),
                        alternative,
                        vec![first_count],
                        control,
                    )
                }
                (SummaryTDesign::Independent, Some(n2), Some(mean2), Some(sd2)) => {
                    validate_summary(n2, mean2, sd2)?;
                    let estimate = first_mean - mean2;
                    let (se, df) =
                        summary_independent_se_df(first_count, first_sd, n2, sd2, equal_variance)?;
                    finish_t(
                        "t.summary_independent",
                        format!("mean1 - mean2 = {null_difference}"),
                        estimate - null_difference,
                        (se, df),
                        alternative,
                        vec![first_count, n2],
                        control,
                    )
                }
                _ => {
                    return Err(invalid(Violation::ShapeMismatch));
                }
            }
        }
        ClassicalHypothesisTest::OneSampleZ {
            values,
            null_mean,
            population_sd,
            alternative,
        } => {
            validate(&values, control)?;
            if !null_mean.is_finite() || !population_sd.is_finite() || population_sd <= 0.0 {
                return Err(parameter());
            }
            let n = values.len();
            let estimate = sample_sum(&values, control)? / n as f64 - null_mean;
            finish_normal(
                "z.mean",
                format!("mean = {null_mean}"),
                estimate,
                population_sd / (n as f64).sqrt(),
                alternative,
                vec![n],
                control,
            )
        }
        ClassicalHypothesisTest::OneProportionZ {
            successes,
            trials,
            null_probability,
            alternative,
        } => {
            validate_proportion(successes, trials, null_probability)?;
            let estimate = successes as f64 / trials as f64 - null_probability;
            let se = (null_probability * (1.0 - null_probability) / trials as f64).sqrt();
            finish_normal(
                "z.proportion",
                format!("p = {null_probability}"),
                estimate,
                se,
                alternative,
                vec![trials],
                control,
            )
        }
        ClassicalHypothesisTest::ExactBinomial {
            successes,
            trials,
            null_probability,
            alternative,
        } => {
            validate_proportion(successes, trials, null_probability)?;
            let p_value =
                binomial_p_value(successes, trials, null_probability, alternative, control)?;
            let estimate = successes as f64 / trials as f64;
            Ok(ClassicalTestResult {
                method: "test.binomial".into(),
                null_hypothesis: format!("p = {null_probability}"),
                alternative: alternative_name(alternative).into(),
                statistic_name: "successes".into(),
                statistic: Some(successes as f64),
                degrees_of_freedom: vec![],
                p_value,
                estimate: Some(estimate),
                standard_error: None,
                sample_sizes: vec![trials],
                details: [("successes".into(), successes as f64)].into(),
            })
        }
        ClassicalHypothesisTest::TwoProportions {
            first_successes,
            first_trials,
            second_successes,
            second_trials,
            null_difference,
            alternative,
        } => {
            validate_proportion(first_successes, first_trials, 0.5)?;
            validate_proportion(second_successes, second_trials, 0.5)?;
            if !null_difference.is_finite() {
                return Err(parameter());
            }
            let p1 = first_successes as f64 / first_trials as f64;
            let p2 = second_successes as f64 / second_trials as f64;
            let se = if null_difference == 0.0 {
                let pooled = (first_successes + second_successes) as f64
                    / (first_trials + second_trials) as f64;
                (pooled * (1.0 - pooled) * (1.0 / first_trials as f64 + 1.0 / second_trials as f64))
                    .sqrt()
            } else {
                (p1 * (1.0 - p1) / first_trials as f64 + p2 * (1.0 - p2) / second_trials as f64)
                    .sqrt()
            };
            finish_normal(
                "proportion.two",
                format!("p1 - p2 = {null_difference}"),
                p1 - p2 - null_difference,
                se,
                alternative,
                vec![first_trials, second_trials],
                control,
            )
        }
        ClassicalHypothesisTest::PoissonRate {
            counts,
            null_rate_per_observation,
            alternative,
        } => {
            validate(&counts, control)?;
            if !null_rate_per_observation.is_finite() || null_rate_per_observation < 0.0 {
                return Err(parameter());
            }
            for (index, count) in counts.iter().enumerate() {
                checkpoint(control, index)?;
                if *count < 0.0 || count.fract() != 0.0 {
                    return Err(invalid(Violation::DataOutOfRange));
                }
            }
            let events_f = sample_sum(&counts, control)?;
            if events_f >= u64::MAX as f64 {
                return Err(failed());
            }
            let events = events_f as u64;
            let expected = finite(null_rate_per_observation * counts.len() as f64)?;
            if expected > 500_000.0 || events > 1_000_000 {
                return Err(invalid(Violation::DataOutOfRange));
            }
            let p_value = poisson_p_value(events, expected, alternative, control)?;
            let rate = events as f64 / counts.len() as f64;
            let statistic = (events as f64 - expected) / expected.sqrt().max(1.0);
            Ok(ClassicalTestResult {
                method: "poisson".into(),
                null_hypothesis: format!("rate = {null_rate_per_observation} per observation"),
                alternative: alternative_name(alternative).into(),
                statistic_name: "count_score".into(),
                statistic: Some(statistic),
                degrees_of_freedom: vec![],
                p_value,
                estimate: Some(rate),
                standard_error: Some(rate.sqrt() / (counts.len() as f64).sqrt()),
                sample_sizes: vec![counts.len()],
                details: [
                    ("event_count".into(), events as f64),
                    ("expected_event_count".into(), expected),
                ]
                .into(),
            })
        }
        ClassicalHypothesisTest::Equivalence {
            values,
            lower_bound,
            upper_bound,
        } => {
            validate(&values, control)?;
            if values.len() < 2 {
                return Err(invalid(Violation::EmptyInput));
            }
            if !lower_bound.is_finite() || !upper_bound.is_finite() || lower_bound >= upper_bound {
                return Err(parameter());
            }
            let n = values.len();
            let mean = sample_sum(&values, control)? / n as f64;
            let se = (sample_variance(&values, mean, control)? / n as f64).sqrt();
            finite(se)?;
            if se <= 0.0 {
                return Err(invalid(Violation::DataOutOfRange));
            }
            let df = (n - 1) as f64;
            let dist = StudentsT::new(0.0, 1.0, df).map_err(|_| failed())?;
            let lower_stat = finite((mean - lower_bound) / se)?;
            let upper_stat = finite((mean - upper_bound) / se)?;
            control.check()?;
            let lower_p = dist.sf(lower_stat);
            control.check()?;
            let upper_p = dist.cdf(upper_stat);
            control.check()?;
            let p_value = lower_p.max(upper_p).clamp(0.0, 1.0);
            Ok(ClassicalTestResult {
                method: "equivalence".into(),
                null_hypothesis: format!("mean <= {lower_bound} or mean >= {upper_bound}"),
                alternative: "lower_bound < mean < upper_bound".into(),
                statistic_name: "t_lower_bound".into(),
                statistic: Some(lower_stat),
                degrees_of_freedom: vec![df],
                p_value,
                estimate: Some(mean),
                standard_error: Some(se),
                sample_sizes: vec![n],
                details: [
                    ("t_upper_bound".into(), upper_stat),
                    ("p_lower_bound".into(), lower_p),
                    ("p_upper_bound".into(), upper_p),
                ]
                .into(),
            })
        }
    };
    control.check()?;
    result
}

fn independent(
    first: &[f64],
    second: &[f64],
    equal_variance: bool,
    null_difference: f64,
    alternative: Alternative,
    method: &str,
    control: &ScientificExecutionControl,
) -> Result<ClassicalTestResult, Error> {
    let n1 = first.len();
    let n2 = second.len();
    let mean1 = sample_sum(first, control)? / n1 as f64;
    let mean2 = sample_sum(second, control)? / n2 as f64;
    let var1 = sample_variance(first, mean1, control)?;
    let var2 = sample_variance(second, mean2, control)?;
    let (se, df) = summary_independent_se_df(n1, var1.sqrt(), n2, var2.sqrt(), equal_variance)?;
    finish_t(
        method,
        format!("mean1 - mean2 = {null_difference}"),
        mean1 - mean2 - null_difference,
        (se, df),
        alternative,
        vec![n1, n2],
        control,
    )
}

fn summary_independent_se_df(
    n1: usize,
    sd1: f64,
    n2: usize,
    sd2: f64,
    equal_variance: bool,
) -> Result<(f64, f64), Error> {
    let a = sd1 * sd1 / n1 as f64;
    let b = sd2 * sd2 / n2 as f64;
    let variance = if equal_variance {
        let pooled =
            (((n1 - 1) as f64 * sd1 * sd1) + ((n2 - 1) as f64 * sd2 * sd2)) / (n1 + n2 - 2) as f64;
        pooled * (1.0 / n1 as f64 + 1.0 / n2 as f64)
    } else {
        a + b
    };
    finite(variance)?;
    if variance <= 0.0 {
        return Err(invalid(Violation::DataOutOfRange));
    }
    let df = if equal_variance {
        (n1 + n2 - 2) as f64
    } else {
        variance * variance / (a * a / (n1 - 1) as f64 + b * b / (n2 - 1) as f64)
    };
    if !df.is_finite() || df <= 0.0 {
        return Err(failed());
    }
    Ok((variance.sqrt(), df))
}

fn sample_sum(values: &[f64], control: &ScientificExecutionControl) -> Result<f64, Error> {
    let mut sum = -0.0;
    for (index, value) in values.iter().enumerate() {
        checkpoint(control, index)?;
        sum += value;
    }
    finite(sum)
}

fn sample_variance(
    values: &[f64],
    mean: f64,
    control: &ScientificExecutionControl,
) -> Result<f64, Error> {
    let mut sum = -0.0;
    for (index, value) in values.iter().enumerate() {
        checkpoint(control, index)?;
        sum += (value - mean).powi(2);
    }
    let variance = sum / (values.len() - 1) as f64;
    if variance.is_finite() {
        Ok(variance)
    } else {
        Err(failed())
    }
}

fn validate(values: &[f64], control: &ScientificExecutionControl) -> Result<(), Error> {
    control.check()?;
    if values.is_empty() {
        return Err(invalid(Violation::EmptyInput));
    }
    for (index, value) in values.iter().enumerate() {
        checkpoint(control, index)?;
        if !value.is_finite() {
            return Err(invalid(Violation::NonFiniteInput));
        }
    }
    Ok(())
}

fn finish_t(
    method: &str,
    null_hypothesis: String,
    estimate: f64,
    standard_error_df: (f64, f64),
    alternative: Alternative,
    sample_sizes: Vec<usize>,
    control: &ScientificExecutionControl,
) -> Result<ClassicalTestResult, Error> {
    let (standard_error, df) = standard_error_df;
    finite(estimate)?;
    finite(standard_error)?;
    finite(df)?;
    if standard_error <= 0.0 {
        return Err(invalid(Violation::DataOutOfRange));
    }
    if df <= 0.0 {
        return Err(failed());
    }
    let statistic = finite(estimate / standard_error)?;
    let distribution = StudentsT::new(0.0, 1.0, df).map_err(|_| failed())?;
    control.check()?;
    let p_value = match alternative {
        Alternative::TwoSided => 2.0 * (1.0 - distribution.cdf(statistic.abs())),
        Alternative::Greater => 1.0 - distribution.cdf(statistic),
        Alternative::Less => distribution.cdf(statistic),
    }
    .clamp(0.0, 1.0);
    control.check()?;
    let alternative = match alternative {
        Alternative::TwoSided => "two-sided",
        Alternative::Greater => "greater",
        Alternative::Less => "less",
    };
    Ok(ClassicalTestResult {
        method: method.into(),
        null_hypothesis,
        alternative: alternative.into(),
        statistic_name: "t".into(),
        statistic: Some(statistic),
        degrees_of_freedom: vec![df],
        p_value,
        estimate: Some(estimate),
        standard_error: Some(standard_error),
        sample_sizes,
        details: Default::default(),
    })
}

fn finish_normal(
    method: &str,
    null_hypothesis: String,
    estimate: f64,
    standard_error: f64,
    alternative: Alternative,
    sample_sizes: Vec<usize>,
    control: &ScientificExecutionControl,
) -> Result<ClassicalTestResult, Error> {
    finite(estimate)?;
    finite(standard_error)?;
    if standard_error <= 0.0 {
        return Err(invalid(Violation::DataOutOfRange));
    }
    let statistic = finite(estimate / standard_error)?;
    let distribution = Normal::new(0.0, 1.0).map_err(|_| failed())?;
    control.check()?;
    let p_value = match alternative {
        Alternative::TwoSided => 2.0 * distribution.sf(statistic.abs()),
        Alternative::Greater => distribution.sf(statistic),
        Alternative::Less => distribution.cdf(statistic),
    }
    .clamp(0.0, 1.0);
    control.check()?;
    Ok(ClassicalTestResult {
        method: method.into(),
        null_hypothesis,
        alternative: alternative_name(alternative).into(),
        statistic_name: "z".into(),
        statistic: Some(statistic),
        degrees_of_freedom: vec![],
        p_value,
        estimate: Some(estimate),
        standard_error: Some(standard_error),
        sample_sizes,
        details: Default::default(),
    })
}

fn alternative_name(alternative: Alternative) -> &'static str {
    match alternative {
        Alternative::TwoSided => "two-sided",
        Alternative::Greater => "greater",
        Alternative::Less => "less",
    }
}

fn validate_proportion(successes: usize, trials: usize, p: f64) -> Result<(), Error> {
    if trials == 0 {
        return Err(invalid(Violation::EmptyInput));
    }
    if successes > trials {
        return Err(invalid(Violation::DataOutOfRange));
    }
    if !p.is_finite() || !(0.0..=1.0).contains(&p) {
        return Err(parameter());
    }
    Ok(())
}

fn validate_summary(count: usize, mean: f64, sd: f64) -> Result<(), Error> {
    if count < 2 {
        return Err(invalid(Violation::EmptyInput));
    }
    if !mean.is_finite() || !sd.is_finite() {
        return Err(invalid(Violation::NonFiniteInput));
    }
    if sd < 0.0 {
        return Err(invalid(Violation::DataOutOfRange));
    }
    Ok(())
}

fn binomial_p_value(
    successes: usize,
    trials: usize,
    p: f64,
    alternative: Alternative,
    control: &ScientificExecutionControl,
) -> Result<f64, Error> {
    if trials > 1_000_000 {
        return Err(invalid(Violation::DataOutOfRange));
    }
    if p == 0.0 {
        return Ok(if successes == 0 { 1.0 } else { 0.0 });
    }
    if p == 1.0 {
        return Ok(if successes == trials { 1.0 } else { 0.0 });
    }
    let distribution = Binomial::new(p, trials as u64).map_err(|_| failed())?;
    control.check()?;
    let value = match alternative {
        Alternative::Less => distribution.cdf(successes as u64),
        Alternative::Greater => distribution.sf(successes.saturating_sub(1) as u64),
        Alternative::TwoSided => {
            let observed = distribution.pmf(successes as u64);
            let mut sum = -0.0;
            for k in 0..=trials {
                checkpoint(control, k)?;
                let mass = distribution.pmf(k as u64);
                if mass <= observed * (1.0 + 1e-12) {
                    sum += mass;
                }
            }
            sum
        }
    };
    control.check()?;
    Ok(value.clamp(0.0, 1.0))
}

fn poisson_p_value(
    observed: u64,
    mean: f64,
    alternative: Alternative,
    control: &ScientificExecutionControl,
) -> Result<f64, Error> {
    if mean == 0.0 {
        return Ok(if observed == 0 { 1.0 } else { 0.0 });
    }
    let distribution = Poisson::new(mean).map_err(|_| failed())?;
    control.check()?;
    let p_value = match alternative {
        Alternative::Less => distribution.cdf(observed),
        Alternative::Greater => distribution.sf(observed.saturating_sub(1)),
        Alternative::TwoSided => {
            let observed_mass = distribution.pmf(observed);
            let limit = ((mean + 12.0 * mean.sqrt() + 100.0).ceil() as u64).max(observed);
            let mut sum = -0.0;
            for k in 0..=limit {
                checkpoint(control, k as usize)?;
                let mass = distribution.pmf(k);
                if mass <= observed_mass * (1.0 + 1e-12) {
                    sum += mass;
                }
            }
            sum
        }
    };
    control.check()?;
    Ok(p_value.clamp(0.0, 1.0))
}
