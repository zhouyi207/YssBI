use super::*;
use rand::{RngExt, SeedableRng, rngs::StdRng};

struct Model {
    mean: f64,
    omega: f64,
    alpha: Vec<f64>,
    gamma: Vec<f64>,
    beta: Vec<f64>,
}
fn asymmetric(o: VolatilityOptions) -> bool {
    matches!(
        o.method,
        VolatilityMethod::Egarch | VolatilityMethod::GjrGarch
    )
}
fn decode(raw: &[f64], o: VolatilityOptions) -> Result<Model> {
    let offset = usize::from(o.constant);
    let mean = if o.constant { raw[0] } else { 0.0 };
    let omega = if o.method == VolatilityMethod::Egarch {
        raw[offset]
    } else {
        finite(raw[offset].exp())?
    };
    let c = &raw[offset + 1..];
    if o.method == VolatilityMethod::Egarch {
        return Ok(Model {
            mean,
            omega,
            alpha: c[..o.p].to_vec(),
            gamma: c[o.p..2 * o.p].to_vec(),
            beta: super::arima::stable_coefficients(&c[2 * o.p..]),
        });
    }
    // The unused simplex mass enforces finite unconditional variance under symmetric innovations.
    let max = c.iter().copied().fold(0.0, f64::max);
    let denominator = (-max).exp() + c.iter().map(|v| (v - max).exp()).sum::<f64>();
    let weights: Vec<_> = c.iter().map(|v| (v - max).exp() / denominator).collect();
    if weights.iter().sum::<f64>() >= 1.0 {
        return Err(failed());
    }
    let a = if asymmetric(o) { o.p } else { 0 };
    Ok(Model {
        mean,
        omega,
        alpha: weights[..o.p].to_vec(),
        gamma: weights[o.p..o.p + a].iter().map(|v| 2.0 * v).collect(),
        beta: weights[o.p + a..].to_vec(),
    })
}
fn next_variance(
    m: &Model,
    o: VolatilityOptions,
    e: &[f64],
    variance: &[f64],
    backcast: f64,
) -> Result<f64> {
    let t = variance.len();
    let mut h = m.omega;
    for (j, a) in m.alpha.iter().enumerate() {
        if o.method == VolatilityMethod::Egarch {
            // E|Z| = sqrt(2/pi), so the presample centered shock is zero.
            if t > j {
                let z = e[t - j - 1] / variance[t - j - 1].sqrt();
                h += a * (z.abs() - (2.0 / std::f64::consts::PI).sqrt()) + m.gamma[j] * z;
            }
        } else {
            let shock = if t > j {
                e[t - j - 1].powi(2)
            } else {
                backcast
            };
            h += a * shock;
            if !m.gamma.is_empty() {
                h += m.gamma[j]
                    * if t > j {
                        if e[t - j - 1] < 0.0 { shock } else { 0.0 }
                    } else {
                        0.5 * backcast
                    };
            }
        }
    }
    for (j, b) in m.beta.iter().enumerate() {
        let v = if t > j { variance[t - j - 1] } else { backcast };
        h += b * if o.method == VolatilityMethod::Egarch {
            v.ln()
        } else {
            v
        };
    }
    let h = finite(if o.method == VolatilityMethod::Egarch {
        h.exp()
    } else {
        h
    })?;
    if h <= 0.0 {
        return Err(failed());
    }
    Ok(h)
}
fn path(
    y: &[f64],
    m: &Model,
    o: VolatilityOptions,
    backcast: f64,
    control: &Control,
) -> Result<(Vec<f64>, Vec<f64>, f64)> {
    let e: Vec<_> = y.iter().map(|v| v - m.mean).collect();
    let mut variance = Vec::with_capacity(y.len());
    let mut loss = 0.0;
    for (i, v) in e.iter().enumerate() {
        if i % 256 == 0 {
            control.check()?;
        }
        let h = next_variance(m, o, &e, &variance, backcast)?;
        variance.push(h);
        loss += 0.5 * (h.ln() + v * v / h) / y.len() as f64;
    }
    Ok((e, variance, finite(loss)?))
}
pub fn volatility(y: &[f64], o: VolatilityOptions, control: &Control) -> Result<VolatilityResult> {
    validate(y, &[], control)?;
    horizon(y.len(), o.horizon)?;
    check_iteration(o.iteration)?;
    let asym = if asymmetric(o) { o.p } else { 0 };
    let count =
        o.p.checked_add(o.q)
            .and_then(|v| v.checked_add(asym))
            .and_then(|v| v.checked_add(1 + usize::from(o.constant)))
            .ok_or_else(parameter)?;
    if o.p == 0
        || y.len() <= count
        || o.p >= y.len()
        || o.q >= y.len()
        || (o.method == VolatilityMethod::Arch && o.q != 0)
        || (o.method == VolatilityMethod::Egarch && o.simulations == 0)
    {
        return Err(parameter());
    }
    let scale = scale(y)?;
    let scaled: Vec<_> = y.iter().map(|v| v / scale).collect();
    let sample_mean = mean(&scaled);
    let backcast = finite(
        scaled
            .iter()
            .map(|v| (v - sample_mean).powi(2) / y.len() as f64)
            .sum(),
    )?;
    let mut initial = Vec::with_capacity(count);
    if o.constant {
        initial.push(sample_mean);
    }
    if o.method == VolatilityMethod::Egarch {
        initial.push(0.0);
        initial.extend(vec![0.1 / o.p as f64; o.p]);
        initial.extend(vec![-0.03 / o.p as f64; o.p]);
        initial.extend((0..o.q).map(|j| if j == 0 { 0.85f64.atanh() } else { 0.0 }));
    } else {
        let a = 0.1;
        let g = if asym > 0 { 0.05 } else { 0.0 };
        let b = if o.q > 0 { 0.8 } else { 0.0 };
        let slack: f64 = 1.0 - a - g - b;
        initial.push((slack * backcast).ln());
        initial.extend(vec![(a / o.p as f64 / slack).ln(); o.p]);
        if asym > 0 {
            initial.extend(vec![(g / asym as f64 / slack).ln(); asym]);
        }
        if o.q > 0 {
            initial.extend(vec![(b / o.q as f64 / slack).ln(); o.q]);
        }
    }
    let objective = |b: &[f64]| path(&scaled, &decode(b, o)?, o, backcast, control).map(|v| v.2);
    let fit = minimize(&objective, initial, o.iteration, control)?;
    let m = decode(&fit.beta, o)?;
    let (e, variance, loss) = path(&scaled, &m, o, backcast, control)?;
    let mut forecasts = vec![0.0; o.horizon];
    if o.method == VolatilityMethod::Egarch {
        let mut rng = StdRng::seed_from_u64(o.seed);
        for _ in 0..o.simulations {
            control.check()?;
            let mut residuals = e.clone();
            let mut h = variance.clone();
            for forecast in &mut forecasts {
                if h.len() % 256 == 0 {
                    control.check()?;
                }
                let v = next_variance(&m, o, &residuals, &h, backcast)?;
                *forecast += v / o.simulations as f64;
                let u = 1.0 - rng.random::<f64>();
                let z = (-2.0 * u.ln()).sqrt()
                    * (2.0 * std::f64::consts::PI * rng.random::<f64>()).cos();
                h.push(v);
                residuals.push(z * v.sqrt());
            }
        }
    } else {
        let mut h = variance.clone();
        let n = y.len();
        for forecast in &mut forecasts {
            control.check()?;
            let t = h.len();
            let mut v = m.omega;
            for (j, a) in m.alpha.iter().enumerate() {
                let lag = t - j - 1;
                let shock = if lag < n { e[lag] * e[lag] } else { h[lag] };
                v += a * shock;
                if !m.gamma.is_empty() {
                    v += m.gamma[j]
                        * if lag < n {
                            if e[lag] < 0.0 { shock } else { 0.0 }
                        } else {
                            0.5 * shock
                        };
                }
            }
            for (j, b) in m.beta.iter().enumerate() {
                v += b * h[t - j - 1];
            }
            h.push(finite(v)?);
            *forecast = v;
        }
    }
    let variance_scale = finite(scale * scale)?;
    let ll =
        finite(-(loss + scale.ln() + 0.5 * (2.0 * std::f64::consts::PI).ln()) * y.len() as f64)?;
    let mut parameters = Vec::new();
    if o.constant {
        parameters.push(estimate("mean", finite(m.mean * scale)?));
    }
    let omega = if o.method == VolatilityMethod::Egarch {
        m.omega + (1.0 - m.beta.iter().sum::<f64>()) * variance_scale.ln()
    } else {
        m.omega * variance_scale
    };
    parameters.push(estimate("omega", finite(omega)?));
    for (name, coefs) in [("alpha", &m.alpha), ("gamma", &m.gamma), ("beta", &m.beta)] {
        for (j, v) in coefs.iter().enumerate() {
            parameters.push(estimate(format!("{name}{}", j + 1), *v));
        }
    }
    let standardized_residuals = e
        .iter()
        .zip(&variance)
        .map(|(v, h)| finite(v / h.sqrt()))
        .collect::<Result<Vec<_>>>()?;
    let residuals = e
        .iter()
        .map(|v| finite(v * scale))
        .collect::<Result<Vec<_>>>()?;
    let conditional_variances = variance
        .iter()
        .map(|v| finite(v * variance_scale))
        .collect::<Result<Vec<_>>>()?;
    let forecast_variances = forecasts
        .iter()
        .map(|v| finite(v * variance_scale))
        .collect::<Result<Vec<_>>>()?;
    control.check()?;
    Ok(VolatilityResult {
        method: o.method,
        observations: y.len(),
        parameters,
        residuals,
        standardized_residuals,
        conditional_variances,
        forecast_variances,
        log_likelihood: ll,
        aic: finite(-2.0 * ll + 2.0 * count as f64)?,
        bic: finite(-2.0 * ll + (y.len() as f64).ln() * count as f64)?,
        iterations: fit.iterations,
        simulations: (o.method == VolatilityMethod::Egarch).then_some(o.simulations),
        seed: (o.method == VolatilityMethod::Egarch).then_some(o.seed),
    })
}
