//! Prospective F power from finite-Beta log densities and a controlled Poisson mixture.
use super::{
    distributions::{EPS, probability, series, upper_bound},
    *,
};
use crate::distribution::{integrate_gauss, stirling_error};
use statrs::function::{beta::ln_beta, gamma::ln_gamma};

struct Density {
    a: f64,
    b: f64,
    p: f64,
    scale: f64,
    log_normalization: f64,
    reflected: bool,
}

impl Density {
    fn new(a: f64, b: f64) -> Result<Self> {
        // All numerator and residual degrees in admitted Power designs are >= 1.
        if !a.is_finite() || !b.is_finite() || a < 0.5 || b < 0.5 {
            return Err(parameter());
        }
        let reflected = a > b;
        let (a, b) = if reflected { (b, a) } else { (a, b) };
        let ratio = a / b;
        let scale = (a / (1. + ratio)).sqrt();
        let remainder = stirling_error(a + b) - stirling_error(b);
        let log_normalization = if a >= 16. {
            -0.5 * (2. * std::f64::consts::PI).ln() + remainder - stirling_error(a)
        } else if b >= 16. {
            (a - 0.5) * a.ln() - a - ln_gamma(a) + remainder
        } else {
            a * (a / (a + b)).ln() + b * (b / (a + b)).ln() - ln_beta(a, b) - scale.ln()
        };
        Ok(Self {
            a,
            b,
            p: ratio / (1. + ratio),
            scale,
            log_normalization,
            reflected,
        })
    }

    fn kernel(&self, w: f64) -> f64 {
        let u = w / self.scale;
        if u.abs() <= 0.001 {
            let pq = self.p * (1. - self.p);
            return -0.5
                * w
                * w
                * (1.
                    + u * ((1. - 2. * self.p) / 3.
                        + u * ((1. - 6. * pq) / 12.
                            + u * ((1. - 2. * self.p) * (1. - 12. * pq) / 60.
                                + u * (1. - 30. * pq + 120. * pq * pq) / 360.))));
        }
        let log_denominator = if u > 40. {
            u + self.p.ln() + ((1. - self.p) / self.p * (-u).exp()).ln_1p()
        } else {
            (self.p * u.exp_m1()).ln_1p()
        };
        self.a * u - (self.a + self.b) * log_denominator
    }

    fn slope(&self, w: f64) -> f64 {
        let u = w / self.scale;
        let ratio = if u > 40. {
            (1. - (-u).exp()) / (self.p + (1. - self.p) * (-u).exp())
        } else {
            u.exp_m1() / (1. + self.p * u.exp_m1())
        };
        -self.a / self.scale * (1. - self.p) * ratio
    }

    fn log_probability(&self, w: f64, upper: bool, control: &Control) -> Result<f64> {
        let right = w >= 0.;
        let direction = if right { 1. } else { -1. };
        let origin = self.kernel(w);
        let rate = self.slope(w).abs().max(1.);
        let mut integral = 0.;
        // Log-concavity scales the tail by its tangent. Near the mode the
        // minimum admitted asymptotic slope is 1/sqrt(2); v=63 retains its tail.
        for b in [0., 1., 3., 7., 15., 31., 63.].windows(2) {
            control.check()?;
            integral += integrate_gauss(b[0], b[1], |v| {
                (self.kernel(w + direction * v / rate) - origin).exp()
            });
        }
        let log_small = self.log_normalization + origin + integral.ln() - rate.ln();
        finite(if right == upper {
            log_small
        } else {
            (-log_small.exp()).ln_1p()
        })
    }

    fn log_sf(&self, log_f: f64, control: &Control) -> Result<f64> {
        let w = self.scale * if self.reflected { -log_f } else { log_f };
        self.log_probability(w, !self.reflected, control)
    }

    fn log_critical(&self, alpha: f64, control: &Control) -> Result<f64> {
        let upper = (alpha <= 0.5) != self.reflected;
        let target = if alpha <= 0.5 { alpha } else { 1. - alpha }.ln();
        let direction = if upper { -1. } else { 1. };
        let (mut low, mut high) = (-1., 1.);
        let mut bracketed = false;
        for _ in 0..64 {
            control.check()?;
            if direction * (self.log_probability(low, upper, control)? - target) > 0. {
                low *= 2.;
            } else if direction * (self.log_probability(high, upper, control)? - target) < 0. {
                high *= 2.;
            } else {
                bracketed = true;
                break;
            }
        }
        if !bracketed {
            return Err(failed());
        }
        let mut w = 0.;
        for _ in 0..64 {
            control.check()?;
            let value = self.log_probability(w, upper, control)?;
            let error = target - value;
            if error.abs() <= 32. * f64::EPSILON * (1. + target.abs()) {
                return finite(if self.reflected { -w } else { w } / self.scale);
            }
            if direction * (value - target) < 0. {
                low = w;
            } else {
                high = w;
            }
            let derivative = direction * (self.log_normalization + self.kernel(w) - value).exp();
            let next = w + error / derivative;
            w = if next > low && next < high {
                next
            } else {
                low + (high - low) / 2.
            };
        }
        Err(failed())
    }
}

fn saturated(df1: f64, df2: f64, lambda: f64, log_f: f64) -> Result<bool> {
    let mean = df1 + lambda;
    let log_ratio = (lambda / df1).ln_1p() - log_f;
    if log_ratio <= 0. {
        return Ok(false);
    }
    let ratio = log_ratio.exp();
    let denominator_bound = upper_bound(df2, (1. + ratio) / 2.)?;
    if denominator_bound >= EPS {
        return Ok(false);
    }
    // A union bound splits at half the excess alternative mean. The exact
    // noncentral chi-square MGF yields the lower-tail Chernoff bound below.
    let fraction = (1. + ratio.recip()) / 2.;
    let a = df1 / mean;
    let b = lambda / mean;
    let v = 2. * (1. - fraction) / (a + 2. * b + a.hypot(2. * (b * fraction).sqrt()));
    let numerator_bound = (-(0.25 * df1 + 0.5 * lambda) * v * v).exp();
    Ok(denominator_bound + numerator_bound < EPS)
}

pub(super) fn power(df1: f64, df2: f64, lambda: f64, alpha: f64, control: &Control) -> Result<f64> {
    control.check()?;
    let a = df1 / 2.;
    let b = df2 / 2.;
    let density = Density::new(a, b)?;
    if lambda == 0. {
        return Ok(alpha);
    }
    let log_critical = density.log_critical(alpha, control)?;
    if saturated(df1, df2, lambda, log_critical)? {
        return Ok(1.);
    }
    series(lambda / 2., None, control, |j, p, _| {
        let component = Density::new(a + j as f64, b)?;
        let shifted = log_critical - (j as f64 / a).ln_1p();
        probability(p * component.log_sf(shifted, control)?.exp())
    })
}
