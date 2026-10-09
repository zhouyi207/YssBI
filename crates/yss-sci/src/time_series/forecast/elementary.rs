use super::*;

pub fn ecm(y: &[f64], x: &[Vec<f64>], options: EcmOptions, control: &Control) -> Result<EcmResult> {
    validate(y, x, control)?;
    let start = options.lags.checked_add(1).ok_or_else(parameter)?;
    let width = x
        .len()
        .checked_mul(start)
        .and_then(|v| {
            v.checked_add(start)?
                .checked_add(usize::from(options.constant))
        })
        .ok_or_else(parameter)?;
    if x.is_empty() || start >= y.len() || y.len() - start <= width {
        return Err(parameter());
    }
    let mut long_run = ols(y, x, options.constant, control)?;
    // Cointegrating OLS does not have the ordinary stationary-regression t law.
    long_run.covariance = None;
    for coefficient in &mut long_run.coefficients {
        coefficient.standard_error = None;
        coefficient.statistic = None;
        coefficient.p_value = None;
        coefficient.confidence_interval = None;
    }
    let dy: Vec<_> = (start..y.len()).map(|i| y[i] - y[i - 1]).collect();
    let mut predictors = vec![
        (start..y.len())
            .map(|i| long_run.residuals[i - 1])
            .collect(),
    ];
    let mut names = vec!["error_correction_lag1".to_owned()];
    for lag in 1..=options.lags {
        control.check()?;
        predictors.push(
            (start..y.len())
                .map(|i| y[i - lag] - y[i - lag - 1])
                .collect(),
        );
        names.push(format!("delta_y_lag{lag}"));
    }
    for (j, col) in x.iter().enumerate() {
        for lag in 0..=options.lags {
            control.check()?;
            predictors.push(
                (start..y.len())
                    .map(|i| col[i - lag] - col[i - lag - 1])
                    .collect(),
            );
            names.push(format!("delta_x{}_lag{lag}", j + 1));
        }
    }
    let mut short_run = ols(&dy, &predictors, options.constant, control)?;
    for (coefficient, name) in short_run
        .coefficients
        .iter_mut()
        .skip(usize::from(options.constant))
        .zip(names)
    {
        coefficient.term = name;
    }
    short_run.method = "ecm_short_run".into();
    control.check()?;
    Ok(EcmResult {
        observations: y.len(),
        first_short_run_row: start,
        long_run,
        short_run,
    })
}

pub fn grey_prediction(y: &[f64], steps: usize, control: &Control) -> Result<ForecastResult> {
    validate(y, &[], control)?;
    horizon(y.len(), steps)?;
    if y.len() < 4 || y.iter().any(|v| *v <= 0.0) {
        return Err(parameter());
    }
    let mut sum = y[0];
    let mut background = Vec::with_capacity(y.len() - 1);
    for (i, &v) in y.iter().enumerate().skip(1) {
        if i % 1024 == 0 {
            control.check()?;
        }
        background.push(finite(-(sum + 0.5 * v))?);
        sum = finite(sum + v)?;
    }
    let design = Design::new(&[background], y.len() - 1, true, true, true, control)?;
    let (beta, _) = least_squares(&design.x, &y[1..], None, control)?;
    let (beta, _) = design.raw(&beta, None);
    let (b, a) = (beta[0], beta[1]);
    let factor = if a.abs() < 1e-8 {
        1.0 - a / 2.0 + a * a / 6.0
    } else {
        -(-a).exp_m1() / a
    };
    let predict = |i: usize| finite((b - a * y[0]) * factor * (-a * (i - 1) as f64).exp());
    let mut fitted = vec![None];
    for i in 1..y.len() {
        if i % 1024 == 0 {
            control.check()?;
        }
        fitted.push(Some(predict(i)?));
    }
    let mut forecast = Vec::with_capacity(steps);
    for i in y.len()..y.len() + steps {
        if i % 1024 == 0 {
            control.check()?;
        }
        forecast.push(predict(i)?);
    }
    report(
        "gm_1_1",
        y,
        fitted,
        forecast,
        vec![estimate("development_a", a), estimate("input_b", b)],
        1,
        control,
    )
}

pub fn markov_prediction(
    states: &[usize],
    state_count: usize,
    steps: usize,
    pseudocount: f64,
    control: &Control,
) -> Result<MarkovResult> {
    control.check()?;
    horizon(states.len(), steps)?;
    if states.len() < 2
        || state_count == 0
        || state_count > states.len()
        || state_count.checked_mul(state_count).is_none()
        || state_count.checked_mul(steps).is_none()
        || !pseudocount.is_finite()
        || pseudocount < 0.0
        || states.iter().any(|s| *s >= state_count)
    {
        return Err(parameter());
    }
    let mut counts = vec![vec![0; state_count]; state_count];
    for (i, w) in states.windows(2).enumerate() {
        if i % 1024 == 0 {
            control.check()?;
        }
        counts[w[0]][w[1]] += 1;
    }
    let mut transition = Vec::with_capacity(state_count);
    for row in &counts {
        control.check()?;
        let denominator =
            finite(row.iter().sum::<usize>() as f64 + pseudocount * state_count as f64)?;
        if denominator <= 0.0 {
            return Err(parameter());
        }
        transition.push(
            row.iter()
                .map(|c| (*c as f64 + pseudocount) / denominator)
                .collect::<Vec<_>>(),
        );
    }
    let last_state = states[states.len() - 1];
    let mut distribution = vec![0.0; state_count];
    distribution[last_state] = 1.0;
    let mut forecast_probabilities: Vec<Vec<f64>> = Vec::with_capacity(steps);
    let mut forecast_states = Vec::with_capacity(steps);
    for _ in 0..steps {
        control.check()?;
        let mut next = vec![0.0; state_count];
        let previous = forecast_probabilities.last().unwrap_or(&distribution);
        for (i, &weight) in previous.iter().enumerate() {
            if i % 64 == 0 {
                control.check()?;
            }
            for (j, p) in next.iter_mut().enumerate() {
                *p += weight * transition[i][j];
            }
        }
        let total = finite(next.iter().sum())?;
        if total <= 0.0 {
            return Err(failed());
        }
        for p in &mut next {
            *p /= total;
        }
        let mode = (0..state_count).fold(0, |best, j| if next[j] > next[best] { j } else { best });
        forecast_states.push(mode);
        forecast_probabilities.push(next);
    }
    Ok(MarkovResult {
        observations: states.len(),
        states: state_count,
        counts,
        transition_probabilities: transition,
        last_state,
        forecast_probabilities,
        forecast_states,
        pseudocount,
    })
}
