//! Finite-degree noncentral t/F tails from bounded integrals and centered Poisson/beta series.
use super::*;
use crate::distribution::{integrate_gauss, stirling_error};
use statrs::{
    distribution::{ChiSquared, ContinuousCDF, FisherSnedecor, Normal, StudentsT},
    function::{beta::beta_reg, gamma::ln_gamma},
};
const EPS: f64 = 2e-14;
pub(super) fn normal() -> Normal {
    Normal::new(0., 1.).expect("normal")
}
pub(super) fn normal_power(
    noncentrality: f64,
    alpha: f64,
    alternative: PowerAlternative,
) -> Result<f64> {
    let normal = normal();
    let value = match alternative {
        PowerAlternative::TwoSided => {
            let z = -normal.inverse_cdf(alpha / 2.);
            normal.sf(z - noncentrality) + normal.cdf(-z - noncentrality)
        }
        PowerAlternative::Greater => normal.sf(-normal.inverse_cdf(alpha) - noncentrality),
        PowerAlternative::Less => normal.cdf(normal.inverse_cdf(alpha) - noncentrality),
    };
    probability(value)
}
pub(super) fn t_power(
    df: f64,
    noncentrality: f64,
    alpha: f64,
    alternative: PowerAlternative,
    control: &Control,
) -> Result<f64> {
    if !df.is_finite() || df <= 0. {
        return Err(parameter());
    }
    let critical = crate::distribution::student_t::upper_quantile(
        if alternative == PowerAlternative::TwoSided {
            alpha / 2.
        } else {
            alpha
        },
        df,
    )
    .ok_or_else(failed)?;
    let power = match alternative {
        PowerAlternative::TwoSided => {
            t_tail(critical, df, noncentrality, control)?
                + t_tail(critical, df, -noncentrality, control)?
        }
        PowerAlternative::Greater => t_tail(critical, df, noncentrality, control)?,
        PowerAlternative::Less => t_tail(critical, df, -noncentrality, control)?,
    };
    probability(power)
}
fn t_tail(t: f64, df: f64, delta: f64, control: &Control) -> Result<f64> {
    control.check()?;
    if !t.is_finite() || !df.is_finite() || !delta.is_finite() {
        return Err(failed());
    }
    if delta == 0. {
        let distribution = StudentsT::new(0., 1., df).map_err(|_| parameter())?;
        return probability(crate::distribution::student_t_probability(
            &distribution,
            t,
            yss_sci_contract::hypothesis::Alternative::Greater,
        ));
    }
    if t < 0. {
        return probability(1. - t_tail(-t, df, -delta, control)?);
    }
    if t == 0. {
        return probability(normal().cdf(delta));
    }
    if normal().cdf(delta) < EPS {
        return Ok(0.);
    }
    if df >= 64. {
        return large_df_t_tail(t, df, delta, control);
    }
    // If Z >= -12, (Z+delta)^2 >= (delta-12)^2. The omitted normal tail
    // and this chi-square tail bound absolute error before returning one.
    if delta > 12. && upper_bound(df, ((delta - 12.) / t).powi(2))? < EPS {
        return Ok(1.);
    }
    let lambda = finite(delta * delta / 2.)?;
    let complement = 1. / (1. + (t / df.sqrt()).powi(2));
    series(lambda, Some(delta.signum()), control, |j, p, q| {
        0.5 * (p * beta_reg(df / 2., j as f64 + 0.5, complement)
            + q * beta_reg(df / 2., j as f64 + 1., complement))
    })
}

fn large_df_t_tail(t: f64, df: f64, delta: f64, control: &Control) -> Result<f64> {
    let scale = df.sqrt();
    let normalization = (-stirling_error(df / 2.)).exp() / std::f64::consts::PI.sqrt();
    let normal = normal();
    // w = sqrt(df) * log(sqrt(V/df)), V ~ chi-square(df). Its exact density
    // is normalization * exp(df/2 * (2w/sqrt(df) - expm1(2w/sqrt(df)))).
    // Log-concavity bounds both omitted tails outside [-8,8] below 2e-17
    // for df >= 64; the conditional Normal probability is at most one.
    let mut bounds = [0.; 24];
    for (i, bound) in bounds[..17].iter_mut().enumerate() {
        *bound = i as f64 - 8.;
    }
    let mut count = 17;
    // Also split the Normal transition, retaining narrow finite-degree tails
    // when t and delta are large. This keeps panel work independent of df.
    for z in [-8., -4., -2., 0., 2., 4., 8.] {
        if delta + z > 0. {
            let w = scale * ((delta + z - t) / t).ln_1p();
            if w > -8. && w < 8. {
                bounds[count] = w;
                count += 1;
            }
        }
    }
    bounds[..count].sort_unstable_by(f64::total_cmp);
    let mut total = 0.;
    for b in bounds[..count].windows(2) {
        control.check()?;
        total += integrate_gauss(b[0], b[1], |w| {
            let u = 2. * w / scale;
            let kernel = if u.abs() <= 0.001 {
                // Scale before squaring and avoid subtracting expm1(u) from u.
                -w * w * (1. + u * (1. / 3. + u * (1. / 12. + u * (1. / 60. + u / 360.))))
            } else {
                df / 2. * (u - u.exp_m1())
            };
            let threshold = (t - delta) + t * (w / scale).exp_m1();
            kernel.exp() * normal.sf(threshold)
        });
    }
    probability(normalization * total)
}

pub(super) fn f_power(
    df1: f64,
    df2: f64,
    lambda: f64,
    alpha: f64,
    control: &Control,
) -> Result<f64> {
    let f = FisherSnedecor::new(df1, df2).map_err(|_| parameter())?;
    let critical = 1.
        / FisherSnedecor::new(df2, df1)
            .map_err(|_| parameter())?
            .inverse_cdf(alpha);
    if lambda == 0. {
        return probability(crate::distribution::fisher_snedecor_sf(&f, critical));
    }
    let root = lambda.sqrt();
    if root > 12. && upper_bound(df2, (root - 12.).powi(2) / (df1 * critical))? < EPS {
        return Ok(1.);
    }
    let complement = 1. / (1. + df1 * critical / df2);
    series(lambda / 2., None, control, |j, p, _| {
        p * beta_reg(df2 / 2., df1 / 2. + j as f64, complement)
    })
}
fn upper_bound(df: f64, multiplier: f64) -> Result<f64> {
    probability(
        ChiSquared::new(df)
            .map_err(|_| parameter())?
            .sf(df * multiplier),
    )
}
fn probability(p: f64) -> Result<f64> {
    if !p.is_finite() || !(-1e-10..=1. + 1e-10).contains(&p) {
        return Err(failed());
    }
    Ok(p.clamp(0., 1.))
}
fn mode_weights(lambda: f64, mode: usize, twin: Option<f64>) -> (f64, f64) {
    let k = mode as f64;
    let logp = if mode < 16 {
        -lambda + k * lambda.ln() - ln_gamma(k + 1.)
    } else {
        let difference = lambda - k;
        k * (difference / k).ln_1p()
            - difference
            - 0.5 * (2. * std::f64::consts::PI * k).ln()
            - stirling_error(k)
    };
    let p = logp.exp();
    let q = twin.map_or(0., |sign| {
        let z = k + 1.;
        let ratio = if mode < 16 {
            ln_gamma(z) - ln_gamma(z + 0.5)
        } else {
            -0.5 * z.ln() - z * (0.5 / z).ln_1p() + 0.5 + stirling_error(z)
                - stirling_error(z + 0.5)
        };
        sign * (logp + 0.5 * lambda.ln() + ratio).exp()
    });
    (p, q)
}
fn series(
    lambda: f64,
    twin: Option<f64>,
    control: &Control,
    term: impl Fn(usize, f64, f64) -> f64,
) -> Result<f64> {
    if !lambda.is_finite() || !(0. ..=design::MAX_EXACT_SIZE as f64).contains(&lambda) {
        return Err(failed());
    }
    if lambda == 0. {
        return probability(term(0, 1., 0.));
    }
    let mode = lambda.floor() as usize;
    let (p0, q0) = mode_weights(lambda, mode, twin);
    let mut total = finite(term(mode, p0, q0))?;
    let (mut j, mut p, mut q) = (mode, p0, q0);
    while j > 0 {
        control.check()?;
        let (rp, rq) = (j as f64 / lambda, (j as f64 + 0.5) / lambda);
        if rp < 1. && rq < 1. && p * rp / (1. - rp) + q.abs() * rq / (1. - rq) < EPS {
            break;
        }
        p *= rp;
        q *= rq;
        j -= 1;
        total += finite(term(j, p, q))?;
    }
    let (mut j, mut p, mut q) = (mode, p0, q0);
    loop {
        control.check()?;
        let (rp, rq) = (lambda / (j as f64 + 1.), lambda / (j as f64 + 1.5));
        if rp < 1. && rq < 1. && p * rp / (1. - rp) + q.abs() * rq / (1. - rq) < EPS {
            break;
        }
        p *= rp;
        q *= rq;
        j = j.checked_add(1).ok_or_else(failed)?;
        total += finite(term(j, p, q))?;
    }
    probability(total)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};
    #[test]
    fn centered_noncentral_series_match_scipy_across_signs_degrees_and_extreme_noncentrality() {
        let reference: serde_json::Value =
            serde_json::from_str(include_str!("../../tests/fixtures/power_reference.json"))
                .unwrap();
        let control = Control {
            cancellation: Default::default(),
            deadline: Instant::now() + Duration::from_secs(30),
        };
        for row in reference["noncentral_t"].as_array().unwrap() {
            let get = |k: &str| row[k].as_f64().unwrap();
            let actual = t_tail(get("t"), get("df"), get("delta"), &control).unwrap();
            assert!((actual - get("tail")).abs() < 1e-8, "{row}: {actual}");
        }
        for row in reference["noncentral_f"].as_array().unwrap() {
            let get = |k: &str| row[k].as_f64().unwrap();
            let actual = f_power(
                get("df1"),
                get("df2"),
                get("lambda"),
                get("alpha"),
                &control,
            )
            .unwrap();
            assert!((actual - get("power")).abs() < 1e-8, "{row}: {actual}");
        }
    }
}
