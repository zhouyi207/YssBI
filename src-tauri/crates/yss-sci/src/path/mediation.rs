//! One observed mediator, with optional first- or second-stage moderation.
use super::preparation::{center_columns, range, sample_sd};
use super::*;
use crate::regression::models::common::{Design, least_squares, ols};
use yss_sci_contract::regression::models::RegressionModelResult;

/// Columns are X, M, optional W, then additive covariates in both equations.
pub fn mediation(
    y: &[f64],
    columns: &[Vec<f64>],
    options: MediationOptions,
    control: &Control,
) -> Result<MediationResult> {
    validate(y, columns, control)?;
    let moderated = options.moderated_stage != MediatedStage::None;
    if columns.len() < 2 + usize::from(moderated)
        || y.len() < 2
        || options.replications == 1
        || !options.probe_sd.is_finite()
        || options.probe_sd <= 0.
    {
        return Err(parameter());
    }
    let prepared = center_columns(columns, control)?;
    let design = Equations::new(&prepared.values, options.moderated_stage)?;
    let mut mediator = ols(&columns[1], &design.mediator, true, control)?;
    let mut outcome = ols(y, &design.outcome, true, control)?;
    rename(&mut mediator, &design.mediator_names);
    rename(&mut outcome, &design.outcome_names);
    mediator.method = "mediation_mediator_ols".into();
    outcome.method = "mediation_outcome_ols".into();
    let probes = if moderated {
        let distance = finite(options.probe_sd * sample_sd(&prepared.values[2])?)?;
        vec![-distance, 0., distance]
    } else {
        vec![0.]
    };
    let a: Vec<_> = mediator.coefficients.iter().map(|c| c.estimate).collect();
    let b: Vec<_> = outcome.coefficients.iter().map(|c| c.estimate).collect();
    let points = effect_values(&a, &b, &probes, options.moderated_stage)?;
    let intervals =
        super::bootstrap::percentile_effects(y.len(), &points, options, control, |rows| {
            // Keep observed centering/probe coordinates fixed across replications.
            let a = bootstrap_fit(&columns[1], &design.mediator, rows, control)?;
            let b = bootstrap_fit(y, &design.outcome, rows, control)?;
            effect_values(&a, &b, &probes, options.moderated_stage)
        })?;
    let mut intervals = intervals.into_iter();
    let direct = intervals.next().ok_or_else(parameter)?;
    let wrange = if moderated {
        range(&columns[2])
    } else {
        [0., 0.]
    };
    let mut effects = Vec::with_capacity(probes.len());
    for w in probes {
        let moderator = if moderated {
            Some(finite(w + prepared.centers[2])?)
        } else {
            None
        };
        effects.push(IndirectEffect {
            moderator,
            in_observed_range: moderator.is_none_or(|w| w >= wrange[0] && w <= wrange[1]),
            indirect: intervals.next().ok_or_else(parameter)?,
            total: intervals.next().ok_or_else(parameter)?,
        });
    }
    Ok(MediationResult {
        mediator,
        outcome,
        diagnostics: MediationDiagnostics {
            moderated_stage: options.moderated_stage,
            input_centers: prepared.centers,
            replications: options.replications,
            seed: options.seed,
            inference: if options.replications == 0 {
                "point_estimates"
            } else {
                "iid_pairs_bootstrap_percentile_95"
            }
            .into(),
            direct,
            effects,
            moderated_mediation_index: intervals.next(),
        },
    })
}

struct Equations {
    mediator: Vec<Vec<f64>>,
    outcome: Vec<Vec<f64>>,
    mediator_names: Vec<String>,
    outcome_names: Vec<String>,
}
impl Equations {
    fn new(x: &[Vec<f64>], stage: MediatedStage) -> Result<Self> {
        let mut m = vec![x[0].clone()];
        let mut y = vec![x[0].clone(), x[1].clone()];
        let mut mn = vec!["intercept".into(), "x".into()];
        let mut yn = vec!["intercept".into(), "x".into(), "m".into()];
        let moderated = stage != MediatedStage::None;
        if moderated {
            m.push(x[2].clone());
            mn.push("w".into());
            y.push(x[2].clone());
            yn.push("w".into());
            let interacting = if stage == MediatedStage::First { 0 } else { 1 };
            let interaction = x[interacting]
                .iter()
                .zip(&x[2])
                .map(|(v, w)| finite(v * w))
                .collect::<Result<_>>()?;
            if stage == MediatedStage::First {
                m.push(interaction);
                mn.push("x:w".into());
            } else {
                y.push(interaction);
                yn.push("m:w".into());
            }
        }
        for (j, cov) in x[2 + usize::from(moderated)..].iter().enumerate() {
            m.push(cov.clone());
            y.push(cov.clone());
            mn.push(format!("covariate{}", j + 1));
            yn.push(format!("covariate{}", j + 1));
        }
        Ok(Self {
            mediator: m,
            outcome: y,
            mediator_names: mn,
            outcome_names: yn,
        })
    }
}
fn rename(model: &mut RegressionModelResult, names: &[String]) {
    for (c, name) in model.coefficients.iter_mut().zip(names) {
        c.term = name.clone();
    }
}
fn bootstrap_fit(
    y: &[f64],
    columns: &[Vec<f64>],
    rows: &[usize],
    control: &Control,
) -> Result<Vec<f64>> {
    let y: Vec<_> = rows.iter().map(|&i| y[i]).collect();
    let x: Vec<Vec<_>> = columns
        .iter()
        .map(|x| rows.iter().map(|&i| x[i]).collect())
        .collect();
    let design = Design::new(&x, y.len(), true, true, true, control)?;
    let (beta, _) = least_squares(&design.x, &y, None, control)?;
    let (beta, _) = design.raw(&beta, None);
    Ok(beta)
}
fn effect_values(a: &[f64], b: &[f64], probes: &[f64], stage: MediatedStage) -> Result<Vec<f64>> {
    let mut values = vec![b[1]];
    for &w in probes {
        let indirect = match stage {
            MediatedStage::None => a[1] * b[2],
            MediatedStage::First => (a[1] + a[3] * w) * b[2],
            MediatedStage::Second => a[1] * (b[2] + b[4] * w),
        };
        values.extend([finite(indirect)?, finite(b[1] + indirect)?]);
    }
    match stage {
        MediatedStage::None => (),
        MediatedStage::First => values.push(finite(a[3] * b[2])?),
        MediatedStage::Second => values.push(finite(a[1] * b[4])?),
    }
    Ok(values)
}
