use super::*;
use statrs::distribution::{ChiSquared, FisherSnedecor};
use yss_sci_contract::association::*;

fn interval(
    estimate: f64,
    margin: f64,
    confidence: f64,
    method: &'static str,
) -> Result<ConfidenceInterval, Error> {
    Ok(ConfidenceInterval {
        lower: finite(estimate - margin)?,
        upper: finite(estimate + margin)?,
        level: confidence,
        method,
    })
}
fn optional_interval(lower: f64, upper: f64, confidence: f64) -> Option<ConfidenceInterval> {
    (lower.is_finite() && upper.is_finite() && lower <= upper).then_some(ConfidenceInterval {
        lower,
        upper,
        level: confidence,
        method: "f_distribution",
    })
}
fn f_quantile(p: f64, a: f64, b: f64) -> Result<f64, Error> {
    finite(
        FisherSnedecor::new(a, b)
            .map_err(|_| Error::ComputationFailed)?
            .inverse_cdf(p),
    )
}

pub fn kappa(
    ratings: &[Vec<usize>],
    categories: usize,
    options: KappaOptions,
    control: &Control,
) -> Result<KappaResult, Error> {
    control.check()?;
    level(options.confidence_level)?;
    let m = ratings.len();
    if m < 2 || categories < 2 {
        return Err(invalid());
    }
    let n = ratings[0].len();
    if n < 2
        || (options.method == KappaMethod::Cohen && m != 2)
        || (options.method == KappaMethod::Fleiss && options.weighting != KappaWeighting::None)
    {
        return Err(invalid());
    }
    let mut counts = vec![vec![0usize; categories]; m];
    for (rater, column) in ratings.iter().enumerate() {
        if column.len() != n {
            return Err(Error::InvalidInput {
                violation: Violation::ShapeMismatch,
            });
        }
        for (i, &category) in column.iter().enumerate() {
            checkpoint(control, i)?;
            if category >= categories {
                return Err(invalid());
            }
            counts[rater][category] += 1;
        }
    }
    let (po, pe, variance, null_variance, contingency) = match options.method {
        KappaMethod::Cohen => {
            let mut table = vec![vec![0usize; categories]; categories];
            for i in 0..n {
                checkpoint(control, i)?;
                table[ratings[0][i]][ratings[1][i]] += 1;
            }
            let weights = (0..categories)
                .map(|a| {
                    (0..categories)
                        .map(|b| {
                            let distance = a.abs_diff(b) as f64 / (categories - 1) as f64;
                            match options.weighting {
                                KappaWeighting::None => f64::from(a == b),
                                KappaWeighting::Linear => 1.0 - distance,
                                KappaWeighting::Quadratic => 1.0 - distance * distance,
                            }
                        })
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>();
            let row = counts[0]
                .iter()
                .map(|&count| count as f64 / n as f64)
                .collect::<Vec<_>>();
            let col = counts[1]
                .iter()
                .map(|&count| count as f64 / n as f64)
                .collect::<Vec<_>>();
            let mut observed = Sum::default();
            let mut expected = Sum::default();
            for a in 0..categories {
                for b in 0..categories {
                    observed.add(weights[a][b] * table[a][b] as f64 / n as f64);
                    expected.add(weights[a][b] * row[a] * col[b]);
                }
            }
            let (po, pe) = (
                bounded(observed.total, 0.0, 1.0)?,
                bounded(expected.total, 0.0, 1.0)?,
            );
            if pe >= 1.0 {
                return Err(invalid());
            }
            let coefficient = finite((po - pe) / (1.0 - pe))?;
            let u = (0..categories)
                .map(|a| sum((0..categories).map(|b| weights[a][b] * col[b])))
                .collect::<Vec<_>>();
            let v = (0..categories)
                .map(|b| sum((0..categories).map(|a| weights[a][b] * row[a])))
                .collect::<Vec<_>>();
            let mut average_gradient = Sum::default();
            for a in 0..categories {
                for b in 0..categories {
                    let gradient =
                        (weights[a][b] - (1.0 - coefficient) * (u[a] + v[b])) / (1.0 - pe);
                    average_gradient.add(table[a][b] as f64 / n as f64 * gradient);
                }
            }
            let mut variance = Sum::default();
            let mut null_variance = Sum::default();
            for a in 0..categories {
                for b in 0..categories {
                    let gradient =
                        (weights[a][b] - (1.0 - coefficient) * (u[a] + v[b])) / (1.0 - pe);
                    variance.add(
                        table[a][b] as f64 / n as f64 * (gradient - average_gradient.total).powi(2),
                    );
                    let null_gradient = (weights[a][b] - u[a] - v[b] + pe) / (1.0 - pe);
                    null_variance.add(row[a] * col[b] * null_gradient.powi(2));
                }
            }
            (
                po,
                pe,
                finite(variance.total / n as f64)?,
                finite(null_variance.total / n as f64)?,
                Some(table),
            )
        }
        KappaMethod::Fleiss => {
            let p = (0..categories)
                .map(|c| {
                    counts
                        .iter()
                        .map(|row| row[c] as f64 / (n * m) as f64)
                        .sum::<f64>()
                })
                .collect::<Vec<_>>();
            let pe = bounded(sum(p.iter().map(|p| p * p)), 0.0, 1.0)?;
            if pe >= 1.0 {
                return Err(invalid());
            }
            let mut subject_counts = vec![0usize; categories];
            let mut agreements = Vec::with_capacity(n);
            let mut observed = Sum::default();
            for i in 0..n {
                checkpoint(control, i)?;
                subject_counts.fill(0);
                for column in ratings {
                    subject_counts[column[i]] += 1;
                }
                let agreement = subject_counts
                    .iter()
                    .map(|&count| (count as f64) * (count as f64 - 1.0))
                    .sum::<f64>()
                    / (m * (m - 1)) as f64;
                agreements.push(agreement);
                observed.add(agreement / n as f64);
            }
            let po = bounded(observed.total, 0.0, 1.0)?;
            let coefficient = finite((po - pe) / (1.0 - pe))?;
            let mut influences = Sum::default();
            for i in 0..n {
                checkpoint(control, i)?;
                subject_counts.fill(0);
                for column in ratings {
                    subject_counts[column[i]] += 1;
                }
                let margin =
                    sum((0..categories)
                        .map(|c| p[c] * (subject_counts[c] as f64 / m as f64 - p[c])));
                let influence =
                    (agreements[i] - po - 2.0 * (1.0 - coefficient) * margin) / (1.0 - pe);
                influences.add(influence * influence);
            }
            let variance = finite(influences.total / (n * (n - 1)) as f64)?;
            let numerator =
                (1.0 - pe).powi(2) - sum(p.iter().map(|p| p * (1.0 - p) * (1.0 - 2.0 * p)));
            let null_variance =
                finite(2.0 * numerator.max(0.0) / ((1.0 - pe).powi(2) * (n * m * (m - 1)) as f64))?;
            (po, pe, variance, null_variance, None)
        }
    };
    let coefficient = finite((po - pe) / (1.0 - pe))?;
    let standard_error = finite(variance.sqrt())?;
    let confidence_interval = Some(interval(
        coefficient,
        normal_quantile(options.confidence_level)? * standard_error,
        options.confidence_level,
        "normal_delta",
    )?);
    let inference = if null_variance > 0.0 {
        let z = finite(coefficient / null_variance.sqrt())?;
        Some(TestInference {
            statistic_name: "z",
            statistic: Some(z),
            degrees_of_freedom: vec![],
            p_value: Some(normal_tail(z, Alternative::TwoSided)?),
            method: "normal_null_variance",
        })
    } else {
        None
    };
    control.check()?;
    Ok(KappaResult {
        method: match options.method {
            KappaMethod::Cohen => "cohen_kappa",
            KappaMethod::Fleiss => "fleiss_kappa",
        },
        weighting: match options.weighting {
            KappaWeighting::None => "none",
            KappaWeighting::Linear => "linear",
            KappaWeighting::Quadratic => "quadratic",
        },
        coefficient,
        observations: n,
        raters: m,
        observed_agreement: po,
        expected_agreement: pe,
        standard_error: Some(standard_error),
        confidence_interval,
        inference,
        categories: (0..categories).collect(),
        category_counts_by_rater: counts,
        contingency_table: contingency,
    })
}

pub fn icc(
    ratings: &[Vec<f64>],
    kind: IccType,
    confidence: f64,
    control: &Control,
) -> Result<IccResult, Error> {
    let n = matrix(ratings, 2, control)?;
    level(confidence)?;
    let k = ratings.len();
    let scale = ratings
        .iter()
        .flat_map(|column| column.iter())
        .map(|x| x.abs())
        .fold(0.0_f64, f64::max);
    if scale == 0.0 {
        return Err(invalid());
    }
    let column_means = ratings
        .iter()
        .map(|column| sum(column.iter().map(|x| x / scale)) / n as f64)
        .collect::<Vec<_>>();
    let grand = sum(column_means.iter().map(|x| x / k as f64));
    let mut row_means = Vec::with_capacity(n);
    for i in 0..n {
        checkpoint(control, i)?;
        row_means.push(sum(ratings
            .iter()
            .map(|column| column[i] / scale / k as f64)));
    }
    let ss_subjects = k as f64 * sum(row_means.iter().map(|mean| (mean - grand).powi(2)));
    let ss_raters = n as f64 * sum(column_means.iter().map(|mean| (mean - grand).powi(2)));
    let mut residual = Sum::default();
    let mut within = Sum::default();
    for i in 0..n {
        checkpoint(control, i)?;
        for j in 0..k {
            let value = ratings[j][i] / scale;
            residual.add((value - row_means[i] - column_means[j] + grand).powi(2));
            within.add((value - row_means[i]).powi(2));
        }
    }
    let msb = ss_subjects / (n - 1) as f64;
    let msj = ss_raters / (k - 1) as f64;
    let mse = residual.total / ((n - 1) * (k - 1)) as f64;
    let msw = within.total / (n * (k - 1)) as f64;
    let average = matches!(kind, IccType::Icc1k | IccType::Icc2k | IccType::Icc3k);
    let one_way = matches!(kind, IccType::Icc1 | IccType::Icc1k);
    let absolute = matches!(kind, IccType::Icc2 | IccType::Icc2k);
    let denominator = if one_way {
        if average {
            msb
        } else {
            msb + (k - 1) as f64 * msw
        }
    } else if absolute {
        if average {
            msb + (msj - mse) / n as f64
        } else {
            msb + (k - 1) as f64 * mse + k as f64 * (msj - mse) / n as f64
        }
    } else if average {
        msb
    } else {
        msb + (k - 1) as f64 * mse
    };
    if denominator <= 0.0 {
        return Err(invalid());
    }
    let coefficient = finite((msb - if one_way { msw } else { mse }) / denominator)?;
    let error = if one_way { msw } else { mse };
    let df1 = (n - 1) as f64;
    let df2 = if one_way {
        (n * (k - 1)) as f64
    } else {
        ((n - 1) * (k - 1)) as f64
    };
    let (statistic, p_value) = if error > 0.0 {
        let f = msb / error;
        let distribution = FisherSnedecor::new(df1, df2).map_err(|_| Error::ComputationFailed)?;
        (
            f.is_finite().then_some(f),
            Some(bounded(distribution.sf(f), 0.0, 1.0)?),
        )
    } else {
        (None, (msb > 0.0).then_some(0.0))
    };
    let mut confidence_interval = None;
    if let Some(f) = statistic {
        let probability = (1.0 + confidence) / 2.0;
        if one_way || !absolute {
            let lower_f = f / f_quantile(probability, df1, df2)?;
            let upper_f = f * f_quantile(probability, df2, df1)?;
            let (lower, upper) = if average {
                (1.0 - 1.0 / lower_f, 1.0 - 1.0 / upper_f)
            } else {
                (
                    (lower_f - 1.0) / (lower_f + (k - 1) as f64),
                    (upper_f - 1.0) / (upper_f + (k - 1) as f64),
                )
            };
            confidence_interval = optional_interval(lower, upper, confidence);
        } else if mse > 0.0 {
            let single =
                (msb - mse) / (msb + (k - 1) as f64 * mse + k as f64 * (msj - mse) / n as f64);
            let fj = msj / mse;
            let base = n as f64 * (1.0 + (k - 1) as f64 * single) - k as f64 * single;
            let v = df2 * (k as f64 * single * fj + base).powi(2)
                / (df1 * (k as f64 * single * fj).powi(2) + base * base);
            if v.is_finite() && v > 0.0 {
                let fu = f_quantile(probability, df1, v)?;
                let fl = f_quantile(probability, v, df1)?;
                let correction = k as f64 * msj + (k * n - k - n) as f64 * mse;
                let mut lower = n as f64 * (msb - fu * mse) / (fu * correction + n as f64 * msb);
                let mut upper = n as f64 * (fl * msb - mse) / (correction + n as f64 * fl * msb);
                if average {
                    lower = lower * k as f64 / (1.0 + lower * (k - 1) as f64);
                    upper = upper * k as f64 / (1.0 + upper * (k - 1) as f64);
                }
                confidence_interval = optional_interval(lower, upper, confidence);
            }
        }
    }
    control.check()?;
    let raw = |square| finite((square * scale) * scale);
    Ok(IccResult {
        method: match kind {
            IccType::Icc1 => "ICC1",
            IccType::Icc2 => "ICC2",
            IccType::Icc3 => "ICC3",
            IccType::Icc1k => "ICC1k",
            IccType::Icc2k => "ICC2k",
            IccType::Icc3k => "ICC3k",
        },
        model: if one_way {
            "one_way_random"
        } else if absolute {
            "two_way_random"
        } else {
            "two_way_mixed"
        },
        measurement: if average { "average" } else { "single" },
        definition: if one_way || absolute {
            "absolute_agreement"
        } else {
            "consistency"
        },
        coefficient,
        observations: n,
        raters: k,
        subject_mean_square: raw(msb)?,
        rater_mean_square: raw(msj)?,
        error_mean_square: raw(mse)?,
        within_mean_square: raw(msw)?,
        inference: TestInference {
            statistic_name: "F",
            statistic,
            degrees_of_freedom: vec![df1, df2],
            p_value,
            method: "f_distribution",
        },
        confidence_interval,
    })
}

pub fn bland_altman(
    x: &[f64],
    y: &[f64],
    coverage: f64,
    confidence: f64,
    control: &Control,
) -> Result<BlandAltmanResult, Error> {
    paired(x, y, 2, control)?;
    level(coverage)?;
    level(confidence)?;
    let n = x.len();
    let mut differences = Vec::with_capacity(n);
    for i in 0..n {
        checkpoint(control, i)?;
        differences.push(finite(x[i] - y[i])?);
    }
    let (centered, scale, mean) = centered(&differences, control)?;
    let bias = finite(mean * scale)?;
    let sd = finite(scale * (sum(centered.iter().map(|x| x * x)) / (n - 1) as f64).sqrt())?;
    let z = normal_quantile(coverage)?;
    let lower_limit = finite(bias - z * sd)?;
    let upper_limit = finite(bias + z * sd)?;
    let t = finite(
        StudentsT::new(0.0, 1.0, (n - 1) as f64)
            .map_err(|_| Error::ComputationFailed)?
            .inverse_cdf((1.0 + confidence) / 2.0),
    )?;
    let bias_margin = t * sd / (n as f64).sqrt();
    let limit_margin = t * sd * (1.0 / n as f64 + z * z / (2 * (n - 1)) as f64).sqrt();
    let take = n.min(MAX_BLAND_ALTMAN_POINTS);
    let mut points = Vec::with_capacity(take);
    for i in 0..take {
        checkpoint(control, i)?;
        let row = (i as u128 * (n - 1) as u128 / (take - 1) as u128) as usize;
        points.push(BlandAltmanPoint {
            observation: row,
            mean: finite(x[row] / 2.0 + y[row] / 2.0)?,
            difference: differences[row],
        });
    }
    control.check()?;
    Ok(BlandAltmanResult {
        method: "bland_altman",
        observations: n,
        bias,
        standard_deviation: sd,
        coverage,
        lower_limit,
        upper_limit,
        bias_confidence_interval: interval(bias, bias_margin, confidence, "student_t")?,
        lower_limit_confidence_interval: interval(
            lower_limit,
            limit_margin,
            confidence,
            "approximate_student_t",
        )?,
        upper_limit_confidence_interval: interval(
            upper_limit,
            limit_margin,
            confidence,
            "approximate_student_t",
        )?,
        points,
        sampled: take < n,
    })
}

pub fn kendall_w(ratings: &[Vec<f64>], control: &Control) -> Result<ConcordanceResult, Error> {
    let n = matrix(ratings, 2, control)?;
    let m = ratings.len();
    let mut rank_sums = vec![0.0; n];
    let mut variance = Sum::default();
    let mean = (n + 1) as f64 / 2.0;
    for column in ratings {
        let ranked = ranks(column, control)?;
        for (i, &rank) in ranked.values.iter().enumerate() {
            checkpoint(control, i)?;
            rank_sums[i] += rank;
            variance.add((rank - mean).powi(2));
        }
    }
    if variance.total <= 0.0 {
        return Err(invalid());
    }
    let r = bounded(
        sum(rank_sums.iter().map(|r| (r - m as f64 * mean).powi(2))) / (m as f64 * variance.total),
        0.0,
        1.0,
    )?;
    let statistic = finite(m as f64 * (n - 1) as f64 * r)?;
    let p = ChiSquared::new((n - 1) as f64)
        .map_err(|_| Error::ComputationFailed)?
        .sf(statistic);
    let tie_correction = bounded(
        12.0 * variance.total / (m as f64 * n as f64 * ((n as f64).powi(2) - 1.0)),
        0.0,
        1.0,
    )?;
    control.check()?;
    Ok(ConcordanceResult {
        method: "kendall_w",
        coefficient: r,
        observations: n,
        raters: m,
        tie_correction,
        inference: TestInference {
            statistic_name: "chi_squared",
            statistic: Some(statistic),
            degrees_of_freedom: vec![(n - 1) as f64],
            p_value: Some(bounded(p, 0.0, 1.0)?),
            method: "chi_square_approximation_with_ties",
        },
    })
}

pub fn rwg(items: &[Vec<f64>], null: AgreementNull, control: &Control) -> Result<RwgResult, Error> {
    let n = matrix(items, 1, control)?;
    let (expected_variance, distribution, scale_points) = match null {
        AgreementNull::Uniform { scale_points } if (2..=10000).contains(&scale_points) => (
            ((scale_points as f64).powi(2) - 1.0) / 12.0,
            "uniform",
            Some(scale_points),
        ),
        AgreementNull::SpecifiedVariance { variance } if variance.is_finite() && variance > 0.0 => {
            (variance, "specified_variance", None)
        }
        _ => return Err(invalid()),
    };
    let mut output = Vec::with_capacity(items.len());
    for (item, values) in items.iter().enumerate() {
        if let Some(points) = scale_points {
            for (i, &value) in values.iter().enumerate() {
                checkpoint(control, i)?;
                if value.fract() != 0.0 || value < 1.0 || value > points as f64 {
                    return Err(invalid());
                }
            }
        }
        let (centered, scale, _) = centered(values, control)?;
        let variance =
            finite((sum(centered.iter().map(|x| x * x)) / (n - 1) as f64 * scale) * scale)?;
        let raw_rwg = finite(1.0 - variance / expected_variance)?;
        output.push(RwgItem {
            item,
            observed_variance: variance,
            raw_rwg,
            rwg: raw_rwg.max(0.0),
        });
    }
    let average = finite(sum(output
        .iter()
        .map(|item| item.observed_variance / items.len() as f64)))?;
    let ratio = finite(average / expected_variance)?;
    let truncated = ratio > 1.0;
    let ratio = ratio.min(1.0);
    let numerator = items.len() as f64 * (1.0 - ratio);
    let coefficient = bounded(numerator / (numerator + ratio), 0.0, 1.0)?;
    control.check()?;
    Ok(RwgResult {
        method: "rwg_j",
        observations: n,
        item_count: items.len(),
        null_distribution: distribution,
        expected_variance,
        mean_observed_variance: average,
        rwg_j: coefficient,
        variance_truncated: truncated,
        items: output,
    })
}
