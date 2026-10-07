//! Balanced crossed random-effects ANOVA. Error denominators differ from fixed-effects ANOVA.
use super::*;
use statrs::distribution::{ContinuousCDF, FisherSnedecor};
struct CrossedStudy {
    n: usize,
    a: usize,
    b: usize,
    r: usize,
    scale: f64,
    part_ss: f64,
    operator_ss: f64,
    interaction_ss: f64,
    error_ss: f64,
}
pub fn measurement_system(
    values: &[f64],
    parts: &[usize],
    operators: &[usize],
    include_interaction: bool,
    control: &Control,
) -> Result<GageResult> {
    let study = prepare(values, parts, operators, control)?;
    let CrossedStudy {
        n,
        a,
        b,
        r,
        scale,
        part_ss,
        operator_ss,
        interaction_ss,
        error_ss,
    } = study;
    let (part_df, operator_df, interaction_df, error_df) =
        (a - 1, b - 1, (a - 1) * (b - 1), a * b * (r - 1));
    let (part_ms, operator_ms, interaction_ms, error_ms) = (
        part_ss / part_df as f64,
        operator_ss / operator_df as f64,
        interaction_ss / interaction_df as f64,
        error_ss / error_df as f64,
    );
    let (residual_ss, residual_df, denominator, denominator_df) = if include_interaction {
        (error_ss, error_df, interaction_ms, interaction_df)
    } else {
        (
            error_ss + interaction_ss,
            error_df + interaction_df,
            (error_ss + interaction_ss) / (error_df + interaction_df) as f64,
            error_df + interaction_df,
        )
    };
    let repeatability = residual_ss / residual_df as f64;
    let part = (part_ms - denominator) / (b * r) as f64;
    let operator = (operator_ms - denominator) / (a * r) as f64;
    let interaction = if include_interaction {
        (interaction_ms - error_ms) / r as f64
    } else {
        0.
    };
    let negative_components_truncated = [
        ("part", part),
        ("operator", operator),
        ("interaction", interaction),
    ]
    .into_iter()
    .filter_map(|(name, v)| (v < 0.).then_some(name))
    .collect();
    let (part, operator, interaction) = (part.max(0.), operator.max(0.), interaction.max(0.));
    let reproducibility = operator + interaction;
    let gage = repeatability + reproducibility;
    let total = gage + part;
    let components = [
        ("repeatability", repeatability),
        ("operator", operator),
        ("interaction", interaction),
        ("reproducibility", reproducibility),
        ("total_gage", gage),
        ("part", part),
        ("total", total),
    ]
    .into_iter()
    .map(|(source, v)| {
        control.check()?;
        let sd = finite(v.sqrt() * scale)?;
        Ok(GageComponent {
            source,
            variance: finite((v * scale) * scale)?,
            standard_deviation: sd,
            study_variation: finite(6. * sd)?,
            contribution_percent: (total > 0.).then(|| 100. * v / total),
            study_variation_percent: (total > 0.).then(|| 100. * (v / total).sqrt()),
        })
    })
    .collect::<Result<Vec<_>>>()?;
    let mut anova = vec![
        term(
            "part",
            part_ss,
            part_df,
            Some((denominator, denominator_df)),
            scale,
        )?,
        term(
            "operator",
            operator_ss,
            operator_df,
            Some((denominator, denominator_df)),
            scale,
        )?,
    ];
    if include_interaction {
        anova.push(term(
            "interaction",
            interaction_ss,
            interaction_df,
            Some((error_ms, error_df)),
            scale,
        )?);
    }
    anova.push(term(
        "repeatability",
        residual_ss,
        residual_df,
        None,
        scale,
    )?);
    Ok(GageResult {
        method: "balanced_crossed_random_anova",
        observations: n,
        parts: a,
        operators: b,
        repetitions: r,
        include_interaction,
        total_sum_squares: finite(
            ((part_ss + operator_ss + interaction_ss + error_ss) * scale) * scale,
        )?,
        anova,
        components,
        negative_components_truncated,
    })
}
fn term(
    source: &'static str,
    ss: f64,
    df: usize,
    denominator: Option<(f64, usize)>,
    scale: f64,
) -> Result<GageAnovaTerm> {
    let ms = ss / df as f64;
    let test = denominator
        .filter(|(value, _)| *value > 0.)
        .map(|(value, df2)| {
            let f = finite(ms / value)?;
            let p = FisherSnedecor::new(df as f64, df2 as f64)
                .map_err(|_| failed())?
                .sf(f);
            Ok((f, finite(p)?))
        })
        .transpose()?;
    Ok(GageAnovaTerm {
        source,
        sum_squares: finite((ss * scale) * scale)?,
        degrees_of_freedom: df,
        mean_square: finite((ms * scale) * scale)?,
        f_statistic: test.map(|t| t.0),
        denominator_degrees_of_freedom: denominator.map(|d| d.1),
        p_value: test.map(|t| t.1),
    })
}
fn prepare(
    values: &[f64],
    parts: &[usize],
    operators: &[usize],
    control: &Control,
) -> Result<CrossedStudy> {
    validate(values, &[], control)?;
    let n = values.len();
    if parts.len() != n || operators.len() != n {
        return Err(parameter());
    }
    let levels = |v: &[usize]| {
        v.iter()
            .copied()
            .max()
            .and_then(|v| v.checked_add(1))
            .ok_or_else(parameter)
    };
    let (a, b) = (levels(parts)?, levels(operators)?);
    let cells = a.checked_mul(b).ok_or_else(parameter)?;
    if a < 2 || b < 2 || cells > n / 2 || !n.is_multiple_of(cells) {
        return Err(parameter());
    }
    let r = n / cells;
    let scale = values
        .iter()
        .map(|v| v.abs())
        .fold(0., f64::max)
        .max(f64::MIN_POSITIVE);
    let anchor = values[0] / scale;
    let mut means = vec![0.; cells];
    let mut counts = vec![0usize; cells];
    for (i, ((&v, &part), &operator)) in values.iter().zip(parts).zip(operators).enumerate() {
        if i.is_multiple_of(1024) {
            control.check()?;
        }
        let j = part * b + operator;
        counts[j] += 1;
        means[j] += ((v / scale - anchor) - means[j]) / counts[j] as f64;
    }
    if counts.iter().any(|&n| n != r) {
        return Err(parameter());
    }
    let mut part_means = vec![0.; a];
    let mut operator_means = vec![0.; b];
    for (i, row) in means.chunks_exact(b).enumerate() {
        control.check()?;
        for (j, &v) in row.iter().enumerate() {
            part_means[i] += v / b as f64;
            operator_means[j] += v / a as f64;
        }
    }
    let mean = means.iter().map(|v| v / cells as f64).sum::<f64>();
    let part_ss = (b * r) as f64 * part_means.iter().map(|v| (v - mean).powi(2)).sum::<f64>();
    let operator_ss = (a * r) as f64
        * operator_means
            .iter()
            .map(|v| (v - mean).powi(2))
            .sum::<f64>();
    let mut interaction_ss = 0.;
    for (i, row) in means.chunks_exact(b).enumerate() {
        control.check()?;
        interaction_ss += r as f64
            * row
                .iter()
                .enumerate()
                .map(|(j, v)| (v - part_means[i] - operator_means[j] + mean).powi(2))
                .sum::<f64>();
    }
    let mut error_ss = 0.;
    for (i, ((&v, &part), &operator)) in values.iter().zip(parts).zip(operators).enumerate() {
        if i.is_multiple_of(1024) {
            control.check()?;
        }
        error_ss += ((v / scale - anchor) - means[part * b + operator]).powi(2);
    }
    Ok(CrossedStudy {
        n,
        a,
        b,
        r,
        scale,
        part_ss,
        operator_ss,
        interaction_ss,
        error_ss,
    })
}
