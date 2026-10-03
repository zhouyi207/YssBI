use super::preparation::{CenteredColumns, center_columns};
use super::*;
use crate::regression::models::common::ols;
mod probing;

/// Columns: X, W, optional Z, then additive continuous covariates.
pub fn moderation(
    y: &[f64],
    columns: &[Vec<f64>],
    options: ModerationOptions,
    control: &Control,
) -> Result<ModerationResult> {
    validate(y, columns, control)?;
    let primary = 2 + usize::from(options.second_moderator);
    if columns.len() < primary
        || y.len() < 2
        || !options.probe_sd.is_finite()
        || options.probe_sd <= 0.
    {
        return Err(parameter());
    }
    let prepared = center_columns(columns, control)?;
    let centered = &prepared.values;
    let mut basis = centered[..primary].to_vec();
    let mut names = vec!["intercept".to_string(), "x".into(), "w".into()];
    if options.second_moderator {
        names.push("z".into());
    }
    let interactions = if options.second_moderator {
        &[(0, 1, "x:w"), (0, 2, "x:z"), (1, 2, "w:z")][..]
    } else {
        &[(0, 1, "x:w")][..]
    };
    for &(a, b, name) in interactions {
        control.check()?;
        basis.push(
            centered[a]
                .iter()
                .zip(&centered[b])
                .map(|(a, b)| finite(a * b))
                .collect::<Result<_>>()?,
        );
        names.push(name.into());
    }
    if options.second_moderator {
        basis.push(
            (0..y.len())
                .map(|i| finite(centered[0][i] * centered[1][i] * centered[2][i]))
                .collect::<Result<_>>()?,
        );
        names.push("x:w:z".into());
    }
    for (j, x) in centered[primary..].iter().enumerate() {
        basis.push(x.clone());
        names.push(format!("covariate{}", j + 1));
    }
    if y.len() <= basis.len() + 1 {
        return Err(parameter());
    }
    let mut model = ols(y, &basis, true, control)?;
    model.method = if options.second_moderator {
        "moderation_three_way"
    } else {
        "moderation_two_way"
    }
    .into();
    for (c, name) in model.coefficients.iter_mut().zip(names) {
        c.term = name;
    }
    let diagnostics = probing::probe(&model, columns, &prepared, options, control)?;
    control.check()?;
    Ok(ModerationResult { model, diagnostics })
}
