use super::*;
use crate::inference::intervals::critical;

/// Reflection coefficients map finite unconstrained values into a stable AR polynomial.
pub(super) fn stable_coefficients(raw: &[f64]) -> Vec<f64> {
    let mut coefficients: Vec<f64> = Vec::new();
    for value in raw {
        let reflection = value.tanh().clamp(-1.0 + f64::EPSILON, 1.0 - f64::EPSILON);
        let mut next = vec![0.0; coefficients.len() + 1];
        for j in 0..coefficients.len() {
            next[j] = coefficients[j] - reflection * coefficients[coefficients.len() - 1 - j];
        }
        next[coefficients.len()] = reflection;
        coefficients = next;
    }
    coefficients
}
fn multiply(a: &[f64], b: &[f64]) -> Vec<f64> {
    let mut result = vec![0.0; a.len() + b.len() - 1];
    for (i, x) in a.iter().enumerate() {
        for (j, y) in b.iter().enumerate() {
            result[i + j] += x * y;
        }
    }
    result
}
fn polynomial(coefs: &[f64], period: usize, sign: f64) -> Vec<f64> {
    let mut poly = vec![0.0; coefs.len() * period + 1];
    poly[0] = 1.0;
    for (j, v) in coefs.iter().enumerate() {
        poly[(j + 1) * period] = sign * v;
    }
    poly
}
struct Model {
    ar: Vec<f64>,
    ma: Vec<f64>,
    mean: f64,
}
fn decode(raw: &[f64], o: ArimaOptions) -> Model {
    let mut offset = 0;
    let mut read = |n| {
        let c = stable_coefficients(&raw[offset..offset + n]);
        offset += n;
        c
    };
    let ar = read(o.p);
    let ma = read(o.q);
    let sar = read(o.seasonal_p);
    let sma = read(o.seasonal_q);
    let mean = if o.constant { raw[offset] } else { 0.0 };
    Model {
        ar: multiply(&polynomial(&ar, 1, -1.0), &polynomial(&sar, o.period, -1.0)),
        ma: multiply(&polynomial(&ma, 1, -1.0), &polynomial(&sma, o.period, -1.0)),
        mean,
    }
}
fn residuals(y: &[f64], model: &Model, burn: usize, control: &Control) -> Result<Vec<f64>> {
    let mut errors = vec![0.0; y.len()];
    for t in burn..y.len() {
        if t % 256 == 0 {
            control.check()?;
        }
        let ar = (1..model.ar.len())
            .map(|j| model.ar[j] * (y[t - j] - model.mean))
            .sum::<f64>();
        let ma = (1..model.ma.len())
            .map(|j| model.ma[j] * errors[t - j])
            .sum::<f64>();
        errors[t] = finite(y[t] - model.mean + ar - ma)?;
    }
    Ok(errors)
}

pub fn arima(y: &[f64], o: ArimaOptions, control: &Control) -> Result<ForecastResult> {
    validate(y, &[], control)?;
    check_iteration(o.iteration)?;
    horizon(y.len(), o.horizon)?;
    let z = critical(o.confidence, None)?;
    if o.period == 0 || ((o.seasonal_p > 0 || o.seasonal_d > 0 || o.seasonal_q > 0) && o.period < 2)
    {
        return Err(parameter());
    }
    let difference = o
        .seasonal_d
        .checked_mul(o.period)
        .and_then(|v| v.checked_add(o.d))
        .ok_or_else(parameter)?;
    let ar_degree = o
        .seasonal_p
        .checked_mul(o.period)
        .and_then(|v| v.checked_add(o.p))
        .ok_or_else(parameter)?;
    let ma_degree = o
        .seasonal_q
        .checked_mul(o.period)
        .and_then(|v| v.checked_add(o.q))
        .ok_or_else(parameter)?;
    let burn = ar_degree.max(ma_degree);
    let count = [
        o.p,
        o.q,
        o.seasonal_p,
        o.seasonal_q,
        usize::from(o.constant),
    ]
    .into_iter()
    .try_fold(0usize, |a, b| a.checked_add(b))
    .ok_or_else(parameter)?;
    let start = difference.checked_add(burn).ok_or_else(parameter)?;
    if start >= y.len() || count >= y.len() - start {
        return Err(parameter());
    }
    let mut diff = vec![1.0];
    for _ in 0..o.d {
        control.check()?;
        diff = multiply(&diff, &[1.0, -1.0]);
    }
    if o.seasonal_d > 0 {
        let mut seasonal = vec![0.0; o.period + 1];
        seasonal[0] = 1.0;
        seasonal[o.period] = -1.0;
        for _ in 0..o.seasonal_d {
            control.check()?;
            diff = multiply(&diff, &seasonal);
        }
    }
    let mut w = Vec::with_capacity(y.len() - difference);
    for t in difference..y.len() {
        if t % 256 == 0 {
            control.check()?;
        }
        w.push(finite(
            diff.iter().enumerate().map(|(j, v)| v * y[t - j]).sum(),
        )?);
    }
    let scale = scale(&w)?;
    for v in &mut w {
        *v /= scale;
    }
    let mut initial = vec![0.0; count];
    if o.constant {
        initial[count - 1] = mean(&w);
    }
    let objective = |b: &[f64]| {
        let errors = residuals(&w, &decode(b, o), burn, control)?;
        finite(
            errors[burn..]
                .iter()
                .map(|v| v * v / (w.len() - burn) as f64)
                .sum(),
        )
    };
    let (beta, iterations) = if count == 0 {
        (initial, 0)
    } else {
        let fit = minimize(&objective, initial, o.iteration, control)?;
        (fit.beta, fit.iterations)
    };
    let model = decode(&beta, o);
    let mut parameters = Vec::new();
    let mut offset = 0;
    for (name, width, sign) in [
        ("ar", o.p, 1.0),
        ("ma", o.q, -1.0),
        ("seasonal_ar", o.seasonal_p, 1.0),
        ("seasonal_ma", o.seasonal_q, -1.0),
    ] {
        for (j, value) in stable_coefficients(&beta[offset..offset + width])
            .iter()
            .enumerate()
        {
            parameters.push(estimate(format!("{name}{}", j + 1), sign * value));
        }
        offset += width;
    }
    if o.constant {
        parameters.push(estimate("differenced_mean", model.mean * scale));
    }
    let mut errors = residuals(&w, &model, burn, control)?;
    let mut fitted = vec![None; y.len()];
    for t in start..y.len() {
        fitted[t] = Some(finite(y[t] - errors[t - difference] * scale)?);
    }
    let mut levels = y.to_vec();
    let mut forecasts = Vec::with_capacity(o.horizon);
    for _ in 0..o.horizon {
        control.check()?;
        let t = w.len();
        let next = model.mean
            - (1..model.ar.len())
                .map(|j| model.ar[j] * (w[t - j] - model.mean))
                .sum::<f64>()
            + (1..model.ma.len())
                .map(|j| model.ma[j] * errors[t - j])
                .sum::<f64>();
        let level = finite(
            next * scale
                - (1..diff.len())
                    .map(|j| diff[j] * levels[levels.len() - j])
                    .sum::<f64>(),
        )?;
        w.push(next);
        errors.push(0.0);
        levels.push(level);
        forecasts.push(level);
    }
    let mut r = report(
        if o.seasonal_p > 0 || o.seasonal_q > 0 || o.seasonal_d > 0 {
            "sarima_css"
        } else {
            "arima_css"
        },
        y,
        fitted,
        forecasts,
        parameters,
        iterations,
        control,
    )?;
    likelihood(&mut r, count + 1)?;
    r.parameters
        .push(estimate("innovation_variance", r.innovation_variance));
    let denominator = multiply(&model.ar, &diff);
    let mut psi = Vec::with_capacity(o.horizon);
    let mut sum = 0.0;
    let mut lower = Vec::new();
    let mut upper = Vec::new();
    for h in 0..o.horizon {
        if h % 256 == 0 {
            control.check()?;
        }
        let v = model.ma.get(h).copied().unwrap_or(0.0)
            - (1..denominator.len().min(h + 1))
                .map(|j| denominator[j] * psi[h - j])
                .sum::<f64>();
        psi.push(v);
        sum = finite(sum + v * v)?;
        let margin = z * r.innovation_variance.sqrt() * sum.sqrt();
        lower.push(finite(r.forecasts[h] - margin)?);
        upper.push(finite(r.forecasts[h] + margin)?);
    }
    r.lower = Some(lower);
    r.upper = Some(upper);
    control.check()?;
    Ok(r)
}
