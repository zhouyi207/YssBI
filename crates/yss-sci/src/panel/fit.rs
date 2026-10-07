//! Panel design preparation, estimator selection and projection of computed facts.
use super::*;
use crate::error::{computation_failed, invalid_input};
use crate::regression::design::design_matrix;
use yss_sci_contract::panel::{
    PanelEffects as Effects, PanelEstimator as Estimator, PanelFit, PanelOptions,
};
use yss_sci_contract::{SciError, SciOperationCode, execution::ScientificInputViolation};
use yss_sci_linalg::Col;

pub fn fit_panel(
    response: Vec<f64>,
    predictors: Vec<Vec<f64>>,
    entity: Vec<f64>,
    time: Vec<f64>,
    options: PanelOptions,
) -> Result<PanelFit, SciError> {
    let op = SciOperationCode::Panel;
    let n = response.len();
    if n == 0 || entity.len() != n || time.len() != n || predictors.iter().any(|v| v.len() != n) {
        return Err(invalid_input(op, ScientificInputViolation::ShapeMismatch));
    }
    if response
        .iter()
        .chain(&entity)
        .chain(&time)
        .chain(predictors.iter().flatten())
        .any(|v| !v.is_finite())
    {
        return Err(invalid_input(op, ScientificInputViolation::NonFiniteInput));
    }
    // Numeric identity treats both signed zeros alike, including sorting and level lookup.
    let normalize = |values: Vec<f64>| {
        values
            .into_iter()
            .map(|v| if v == 0.0 { 0.0 } else { v })
            .collect::<Vec<_>>()
    };
    let entity = normalize(entity);
    let time = normalize(time);
    let c = options.constant;
    let cov = options.covariance.as_str();
    if !matches!(cov, "nonrobust" | "HC0" | "HC1" | "HC2" | "HC3" | "cluster")
        || (matches!(
            options.estimator,
            Estimator::Between | Estimator::MaximumLikelihood
        ) && cov != "nonrobust")
        || (matches!(options.estimator, Estimator::Lsdv) && !c)
        || (matches!(options.estimator, Estimator::FirstDifference)
            && options.effects != Effects::Entity)
        || (matches!(options.estimator, Estimator::Between) && options.effects == Effects::TwoWay)
    {
        return Err(invalid_input(
            op,
            ScientificInputViolation::ParameterOutOfRange,
        ));
    }
    // Sorting is required by the FD estimator. Validate duplicate keys before every estimator.
    let mut order: Vec<_> = (0..n).collect();
    order.sort_by(|&a, &b| {
        entity[a]
            .total_cmp(&entity[b])
            .then(time[a].total_cmp(&time[b]))
    });
    if order
        .windows(2)
        .any(|w| entity[w[0]] == entity[w[1]] && time[w[0]] == time[w[1]])
    {
        return Err(invalid_input(
            op,
            ScientificInputViolation::ParameterOutOfRange,
        ));
    }
    let ids = |values: &[f64]| -> Vec<usize> {
        let mut levels = values.to_vec();
        levels.sort_by(f64::total_cmp);
        levels.dedup();
        order
            .iter()
            .map(|&i| {
                levels
                    .binary_search_by(|v| v.total_cmp(&values[i]))
                    .expect("observed level")
            })
            .collect()
    };
    let entities = ids(&entity);
    let times = ids(&time);
    let y = Col::from_iter(order.iter().map(|&i| response[i]));
    let columns = predictors
        .iter()
        .map(|v| order.iter().map(|&i| v[i]).collect())
        .collect::<Vec<Vec<f64>>>();
    let x = design_matrix(&columns, n, c, op)?;
    let (family, result) = match (options.estimator, options.effects) {
        (Estimator::FixedEffects, Effects::Entity) => (
            "panel_fe_entity",
            fit_panel_fe(&y, &x, &entities, c, cov, None),
        ),
        (Estimator::FixedEffects, Effects::Time) => (
            "panel_fe_time",
            fit_panel_fe_time(&y, &x, &entities, &times, c, cov, None),
        ),
        (Estimator::FixedEffects, Effects::TwoWay) => (
            "panel_fe_twoway",
            fit_panel_fe_twoway(&y, &x, &entities, &times, c, cov, None),
        ),
        (Estimator::Lsdv, Effects::Entity) => (
            "panel_lsdv_entity",
            fit_panel_lsdv(&y, &x, &entities, c, cov, None),
        ),
        (Estimator::Lsdv, Effects::Time) => (
            "panel_lsdv_time",
            fit_panel_lsdv_time(&y, &x, &entities, &times, c, cov, None),
        ),
        (Estimator::Lsdv, Effects::TwoWay) => (
            "panel_lsdv_twoway",
            fit_panel_lsdv_twoway(&y, &x, &entities, &times, c, cov, None),
        ),
        (Estimator::RandomEffects, Effects::Entity) => (
            "panel_re_entity",
            fit_panel_re_fgls(&y, &x, &entities, c, cov, None),
        ),
        (Estimator::RandomEffects, Effects::Time) => (
            "panel_re_time",
            fit_panel_re_fgls_time(&y, &x, &entities, &times, c, cov, None),
        ),
        (Estimator::RandomEffects, Effects::TwoWay) => (
            "panel_re_twoway",
            fit_panel_re_fgls_twoway(&y, &x, &entities, &times, c, cov, None),
        ),
        (Estimator::MaximumLikelihood, Effects::Entity) => {
            ("panel_mle_entity", fit_panel_re_mle(&y, &x, &entities, c))
        }
        (Estimator::MaximumLikelihood, Effects::Time) => (
            "panel_mle_time",
            fit_panel_re_mle_time(&y, &x, &entities, &times, c),
        ),
        (Estimator::MaximumLikelihood, Effects::TwoWay) => (
            "panel_mle_twoway",
            fit_panel_re_mle_twoway(&y, &x, &entities, &times, c),
        ),
        (Estimator::Between, Effects::Entity) => (
            "panel_between_entity",
            fit_panel_re_be(&y, &x, &entities, c),
        ),
        (Estimator::Between, Effects::Time) => (
            "panel_between_time",
            fit_panel_re_be_time(&y, &x, &entities, &times, c),
        ),
        (Estimator::FirstDifference, Effects::Entity) => {
            if time
                .iter()
                .any(|v| v.fract() != 0.0 || *v < i64::MIN as f64 || *v >= i64::MAX as f64)
            {
                return Err(invalid_input(
                    op,
                    ScientificInputViolation::ParameterOutOfRange,
                ));
            }
            let original_times = order.iter().map(|&i| time[i] as i64).collect::<Vec<_>>();
            if original_times
                .iter()
                .copied()
                .max()
                .unwrap()
                .checked_sub(original_times.iter().copied().min().unwrap())
                .is_none()
            {
                return Err(invalid_input(
                    op,
                    ScientificInputViolation::ParameterOutOfRange,
                ));
            }
            (
                "panel_fd",
                fit_panel_fd(&y, &x, &entities, &original_times, c, cov, None),
            )
        }
        _ => {
            return Err(invalid_input(
                op,
                ScientificInputViolation::ParameterOutOfRange,
            ));
        }
    };
    let mut result = result.map_err(|_| computation_failed(op))?;
    result.family = family.into();
    result.statistics.time_periods = times
        .iter()
        .copied()
        .collect::<std::collections::HashSet<_>>()
        .len();
    let mut labels: Vec<(String, Option<String>)> = std::iter::once(("_cons".into(), None))
        .take(usize::from(c))
        .chain((0..predictors.len()).map(|j| (format!("x{}", j + 1), None)))
        .collect();
    if options.estimator == Estimator::Lsdv {
        for (variable, values, applies) in [
            ("entity", &entity, options.effects != Effects::Time),
            ("time", &time, options.effects != Effects::Entity),
        ] {
            if applies {
                let mut levels = values.clone();
                levels.sort_by(f64::total_cmp);
                levels.dedup();
                labels.extend(
                    levels
                        .iter()
                        .skip(1)
                        .map(|v| (format!("{variable}[{v}]"), Some(v.to_string()))),
                );
            }
        }
    }
    let omitted = result.omitted_indices.clone().unwrap_or_default();
    result.omitted_terms = omitted
        .iter()
        .map(|&index| yss_sci_contract::panel::PanelOmittedTerm {
            index,
            variable: labels[index].0.clone(),
            category: labels[index].1.clone(),
            reason: if options.estimator == Estimator::FixedEffects {
                "absorbed_or_collinear"
            } else {
                "collinear"
            }
            .into(),
        })
        .collect();
    if options.estimator == Estimator::FirstDifference && c {
        result
            .omitted_terms
            .push(yss_sci_contract::panel::PanelOmittedTerm {
                index: 0,
                variable: "_cons".into(),
                category: None,
                reason: "removed_by_differencing".into(),
            });
    }
    let retained = labels
        .into_iter()
        .enumerate()
        .filter(|(i, _)| {
            !(omitted.contains(i)
                || (options.estimator == Estimator::FirstDifference && c && *i == 0))
        })
        .map(|(_, v)| v)
        .collect::<Vec<_>>();
    result.parameter_names = retained.iter().map(|v| v.0.clone()).collect();
    result.parameter_categories = retained.into_iter().map(|v| v.1).collect();
    if result.parameter_names.len() != result.coefficients.len() {
        return Err(computation_failed(op));
    }
    result.estimation.source_rows = match options.estimator {
        Estimator::FirstDifference => order
            .windows(2)
            .filter(|w| entity[w[0]] == entity[w[1]] && time[w[1]] - time[w[0]] == 1.0)
            .map(|w| w.to_vec())
            .collect(),
        Estimator::Between => {
            let ids = if options.effects == Effects::Entity {
                &entities
            } else {
                &times
            };
            let count = ids.iter().max().map_or(0, |v| v + 1);
            (0..count)
                .map(|id| {
                    order
                        .iter()
                        .enumerate()
                        .filter(|(i, _)| ids[*i] == id)
                        .map(|(_, r)| *r)
                        .collect()
                })
                .collect()
        }
        _ => order.iter().map(|i| vec![*i]).collect(),
    };
    if result.estimation.source_rows.len() != result.estimation.fitted.len() {
        return Err(computation_failed(op));
    }
    Ok(result)
}

/// Predict on the exact estimator scale. Absorbed effects are not extrapolated;
/// callers provide within/quasi-demeaned/differenced/mean inputs as appropriate.
pub fn predict(fit: &PanelFit, predictors: &[Vec<f64>]) -> Result<Vec<f64>, SciError> {
    let n = predictors.first().map_or(0, Vec::len);
    let x = design_matrix(
        predictors,
        n,
        fit.estimation.constant,
        SciOperationCode::Panel,
    )?;
    if x.ncols() != fit.estimation.coefficients.len()
        || fit.estimation.coefficients.iter().any(|v| !v.is_finite())
    {
        return Err(invalid_input(
            SciOperationCode::Panel,
            ScientificInputViolation::ShapeMismatch,
        ));
    }
    Ok(
        (&x * &Col::from_iter(fit.estimation.coefficients.iter().copied()))
            .iter()
            .copied()
            .collect(),
    )
}
