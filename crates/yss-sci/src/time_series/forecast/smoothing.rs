use super::*;

fn logistic(x: f64) -> f64 {
    if x >= 0.0 {
        1.0 / (1.0 + (-x).exp())
    } else {
        let v = x.exp();
        v / (1.0 + v)
    }
}
fn logit(x: f64) -> f64 {
    let x = x.clamp(1e-6, 1.0 - 1e-6);
    (x / (1.0 - x)).ln()
}
fn decode(raw: &[f64], mut o: SmoothingOptions) -> SmoothingOptions {
    let mut j = 0;
    o.alpha = logistic(raw[j]);
    j += 1;
    if o.trend {
        o.beta = logistic(raw[j]);
        j += 1;
    }
    if o.seasonality != Seasonality::None {
        o.gamma = (1.0 - o.alpha) * logistic(raw[j]);
        j += 1;
    }
    if o.damped {
        o.phi = logistic(raw[j]);
    }
    o
}
struct Path {
    fitted: Vec<Option<f64>>,
    forecast: Vec<f64>,
}
fn smooth(y: &[f64], o: SmoothingOptions, control: &Control) -> Result<Path> {
    let seasonal = o.seasonality != Seasonality::None;
    let m = if seasonal { o.period } else { 1 };
    let mut level = if seasonal { mean(&y[..m]) } else { y[0] };
    let mut trend = if o.trend {
        if seasonal {
            (mean(&y[m..2 * m]) - level) / m as f64
        } else {
            y[1] - y[0]
        }
    } else {
        0.0
    };
    let phi = if o.damped { o.phi } else { 1.0 };
    if seasonal {
        level += trend * (m - 1) as f64 / 2.0;
    }
    let mut seasons = vec![
        if o.seasonality == Seasonality::Multiplicative {
            1.0
        } else {
            0.0
        };
        m
    ];
    if seasonal {
        for i in 0..m {
            let baseline = level + trend * (i as f64 - (m - 1) as f64);
            if o.seasonality == Seasonality::Multiplicative && baseline <= 0.0 {
                return Err(parameter());
            }
            seasons[i] = if o.seasonality == Seasonality::Additive {
                y[i] - baseline
            } else {
                y[i] / baseline
            };
        }
    }
    let start = if seasonal { m } else { 1 };
    let mut fitted = vec![None; y.len()];
    for t in start..y.len() {
        if t % 256 == 0 {
            control.check()?;
        }
        let previous = level;
        let base = level + phi * trend;
        let s = seasons[t % m];
        let prediction = match o.seasonality {
            Seasonality::None => base,
            Seasonality::Additive => base + s,
            Seasonality::Multiplicative => base * s,
        };
        fitted[t] = Some(finite(prediction)?);
        let adjusted = match o.seasonality {
            Seasonality::None => y[t],
            Seasonality::Additive => y[t] - s,
            Seasonality::Multiplicative => {
                if s <= 0.0 {
                    return Err(failed());
                }
                y[t] / s
            }
        };
        level = finite(o.alpha * adjusted + (1.0 - o.alpha) * base)?;
        if o.trend {
            trend = finite(o.beta * (level - previous) + (1.0 - o.beta) * phi * trend)?;
        }
        match o.seasonality {
            Seasonality::None => (),
            Seasonality::Additive => {
                seasons[t % m] = finite(o.gamma * (y[t] - base) + (1.0 - o.gamma) * s)?
            }
            Seasonality::Multiplicative => {
                if base <= 0.0 {
                    return Err(failed());
                }
                seasons[t % m] = finite(o.gamma * y[t] / base + (1.0 - o.gamma) * s)?;
            }
        }
    }
    let mut forecast = Vec::with_capacity(o.horizon);
    let mut power = 1.0;
    let mut sum = 0.0;
    for h in 0..o.horizon {
        if h % 256 == 0 {
            control.check()?;
        }
        power *= phi;
        sum += power;
        let base = level + sum * trend;
        let v = match o.seasonality {
            Seasonality::None => base,
            Seasonality::Additive => base + seasons[(y.len() + h) % m],
            Seasonality::Multiplicative => base * seasons[(y.len() + h) % m],
        };
        forecast.push(finite(v)?);
    }
    Ok(Path { fitted, forecast })
}

pub fn exponential_smoothing(
    y: &[f64],
    o: SmoothingOptions,
    control: &Control,
) -> Result<ForecastResult> {
    validate(y, &[], control)?;
    horizon(y.len(), o.horizon)?;
    check_iteration(o.iteration)?;
    if y.len() < 2
        || o.damped && !o.trend
        || [o.alpha, o.beta, o.gamma, o.phi]
            .iter()
            .any(|v| !v.is_finite() || *v < 0.0 || *v > 1.0)
    {
        return Err(parameter());
    }
    if o.seasonality != Seasonality::None
        && (o.alpha + o.gamma > 1.0
            || o.period < 2
            || o.period.checked_mul(2).is_none_or(|v| v > y.len()))
    {
        return Err(parameter());
    }
    if o.seasonality == Seasonality::Multiplicative && y.iter().any(|v| *v <= 0.0) {
        return Err(parameter());
    }
    let (response, response_scale) = normalized_series(y, control)?;
    let normalized = response.as_slice();
    let start = if o.seasonality == Seasonality::None {
        1
    } else {
        o.period
    };
    let (options, iterations) = if o.optimize {
        let mut raw = vec![logit(o.alpha)];
        if o.trend {
            raw.push(logit(o.beta));
        }
        if o.seasonality != Seasonality::None {
            raw.push(logit(o.gamma / (1.0 - o.alpha).max(1e-6)));
        }
        if o.damped {
            raw.push(logit(o.phi));
        }
        if y.len() - start <= raw.len() {
            return Err(parameter());
        }
        // Level variance grows with a trend and can flatten the normalized objective.
        // Scale by the initial one-step errors so convergence is invariant to trend length.
        let scaling = {
            let initial = smooth(normalized, SmoothingOptions { horizon: 0, ..o }, control)?;
            let (rms, _) = root_mean_square(
                normalized
                    .iter()
                    .zip(&initial.fitted)
                    .filter_map(|(v, f)| f.map(|f| v - f)),
                control,
            )?;
            if rms > 0.0 { rms } else { 1.0 }
        };
        let objective = |b: &[f64]| {
            let mut candidate = decode(b, o);
            candidate.horizon = 0;
            let path = smooth(normalized, candidate, control)?;
            let (rms, _) = root_mean_square(
                normalized
                    .iter()
                    .zip(&path.fitted)
                    .filter_map(|(v, f)| f.map(|f| (v - f) / scaling)),
                control,
            )?;
            finite(rms * rms)
        };
        let fit = minimize(&objective, raw, o.iteration, control)?;
        (decode(&fit.beta, o), fit.iterations)
    } else {
        (o, 0)
    };
    let mut path = smooth(normalized, options, control)?;
    for (i, value) in path.fitted.iter_mut().enumerate() {
        if i % 1024 == 0 {
            control.check()?;
        }
        if let Some(value) = value {
            *value = finite(*value * response_scale)?;
        }
    }
    for (i, value) in path.forecast.iter_mut().enumerate() {
        if i % 1024 == 0 {
            control.check()?;
        }
        *value = finite(*value * response_scale)?;
    }
    let mut parameters = vec![estimate("alpha", options.alpha)];
    if o.trend {
        parameters.push(estimate("beta", options.beta));
    }
    if o.seasonality != Seasonality::None {
        parameters.push(estimate("gamma", options.gamma));
    }
    if o.damped {
        parameters.push(estimate("phi", options.phi));
    }
    let method = match o.seasonality {
        Seasonality::Additive => "holt_winters_additive",
        Seasonality::Multiplicative => "holt_winters_multiplicative",
        Seasonality::None => {
            if o.trend {
                "holt_linear"
            } else {
                "simple_exponential_smoothing"
            }
        }
    };
    let mut r = report(
        method,
        y,
        path.fitted,
        path.forecast,
        parameters,
        iterations,
        control,
    )?;
    if r.innovation_variance > 0.0 {
        let count = if o.optimize {
            r.parameters.len() + 1
        } else {
            1
        };
        likelihood(&mut r, count)?;
    }
    control.check()?;
    Ok(r)
}
