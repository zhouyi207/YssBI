//! Panel design preparation, estimator selection and projection of computed facts.
use super::*;
use crate::error::{computation_failed, invalid_input};
use crate::regression::design::design_matrix;
use yss_sci_contract::panel::{
    PanelEffects as Effects, PanelEstimator as Estimator, PanelFit, PanelOptions,
};
use yss_sci_contract::{SciError, SciInputViolation, SciOperationCode};
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
        return Err(invalid_input(op, SciInputViolation::ShapeMismatch));
    }
    if response
        .iter()
        .chain(&entity)
        .chain(&time)
        .chain(predictors.iter().flatten())
        .any(|v| !v.is_finite())
    {
        return Err(invalid_input(op, SciInputViolation::NonFiniteInput));
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
        return Err(invalid_input(op, SciInputViolation::ParameterOutOfRange));
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
        return Err(invalid_input(op, SciInputViolation::ParameterOutOfRange));
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
                return Err(invalid_input(op, SciInputViolation::ParameterOutOfRange));
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
                return Err(invalid_input(op, SciInputViolation::ParameterOutOfRange));
            }
            (
                "panel_fd",
                fit_panel_fd(&y, &x, &entities, &original_times, c, cov, None),
            )
        }
        _ => return Err(invalid_input(op, SciInputViolation::ParameterOutOfRange)),
    };
    let mut result = result.map_err(|_| computation_failed(op))?;
    result.family = family.into();
    Ok(result)
}
