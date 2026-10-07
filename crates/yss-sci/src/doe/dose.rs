//! Four-parameter log-logistic least squares; zero dose uses its continuous limit.
use super::*;
use crate::regression::models::nonlinear_formula;
use yss_sci_contract::regression::models::{
    IterationOptions, RegressionCoefficient, RegressionDetails,
};

pub fn dose_response(
    y: &[f64],
    dose: &[f64],
    iteration: IterationOptions,
    control: &Control,
) -> Result<DoseResponseResult> {
    validate(y, &[dose.to_vec()], control)?;
    if y.len() <= 4 || dose.iter().any(|&x| x < 0.) {
        return Err(parameter());
    }
    let min_dose = dose.iter().copied().fold(f64::INFINITY, f64::min);
    let max_dose = dose.iter().copied().fold(0., f64::max);
    if min_dose == max_dose {
        return Err(parameter());
    }
    let endpoint_mean = |endpoint| {
        let count = dose.iter().filter(|&&x| x == endpoint).count() as f64;
        dose.iter()
            .zip(y)
            .filter(|(x, _)| **x == endpoint)
            .map(|(_, r)| r / count)
            .sum::<f64>()
    };
    let (mut at_zero, mut at_infinity) = (endpoint_mean(min_dose), endpoint_mean(max_dose));
    if at_zero == at_infinity {
        // Equal observed endpoint means can occur by chance. Only a constant
        // response prevents any four-parameter curve from being identified.
        at_zero = y.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        at_infinity = y.iter().copied().fold(f64::INFINITY, f64::min);
        if at_zero == at_infinity {
            return Err(parameter());
        }
    }
    let mut positive: Vec<_> = dose.iter().copied().filter(|&x| x > 0.).collect();
    positive.sort_by(f64::total_cmp);
    control.check()?;
    let initial = [
        at_infinity,
        at_zero,
        0.,
        positive[positive.len() / 2].ln() - max_dose.ln(),
    ];
    let x = dose.iter().map(|x| x / max_dose).collect();
    // Positive Hill exponent and ED50 remove the sign/asymptote symmetry without
    // restricting direction: at_zero may be either above or below at_infinity.
    let mut model = nonlinear_formula(
        y,
        &[x],
        "b1+(b2-b1)/(1+(x1/exp(b4))^exp(b3))",
        &initial,
        &[],
        &[],
        iteration,
        control,
    )?;
    model.method = "dose_response_log_logistic_4".into();
    for (c, name) in model.coefficients.iter_mut().zip([
        "asymptote_infinite",
        "asymptote_zero",
        "log_hill",
        "log_ed50",
    ]) {
        c.term = name.into();
    }
    let shift = max_dose.ln();
    model.coefficients[3].estimate = finite(model.coefficients[3].estimate + shift)?;
    if let Some(ci) = model.coefficients[3].confidence_interval.as_mut() {
        ci[0] = finite(ci[0] + shift)?;
        ci[1] = finite(ci[1] + shift)?;
    }
    // A zero test on a log-dose parameter depends on the arbitrary physical unit.
    for c in &mut model.coefficients[2..] {
        c.statistic = None;
        c.p_value = None;
    }
    if let RegressionDetails::Nonlinear {
        formula,
        initial_values,
        ..
    } = &mut model.details
    {
        *formula = "c+(d-c)/(1+(dose/exp(log_ed50))^exp(log_hill)); c=asymptote_infinite, d=asymptote_zero".into();
        initial_values[3] += shift;
    }
    let ed50 = positive_parameter(&model.coefficients[3])?;
    let parameters = DoseResponseParameters {
        hill: positive_parameter(&model.coefficients[2])?,
        increasing: model.coefficients[0].estimate > model.coefficients[1].estimate,
        ed50_inside_dose_range: ed50.estimate >= min_dose && ed50.estimate <= max_dose,
        ed50,
    };
    control.check()?;
    Ok(DoseResponseResult { model, parameters })
}

fn positive_parameter(coefficient: &RegressionCoefficient) -> Result<PositiveCurveParameter> {
    let estimate = finite(coefficient.estimate.exp())?;
    if estimate <= 0. {
        return Err(parameter());
    }
    let standard_error = coefficient.standard_error.and_then(|s| {
        let se = estimate * s;
        se.is_finite().then_some(se)
    });
    let confidence_interval = coefficient.confidence_interval.and_then(|ci| {
        let result = [ci[0].exp(), ci[1].exp()];
        result.iter().all(|v| v.is_finite()).then_some(result)
    });
    Ok(PositiveCurveParameter {
        estimate,
        standard_error,
        confidence_interval,
    })
}
