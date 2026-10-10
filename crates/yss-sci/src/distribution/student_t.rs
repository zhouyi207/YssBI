//! Student-t quantiles and large-df probabilities from bounded hyperbolic-density integration.
//! Substituting t=sqrt(df)*sinh(u) makes the two-sided density proportional to cosh(u)^(-df).

// Positive half of the 16-point Gauss-Legendre rule (DLMF 3.5).
const GAUSS: [(f64, f64); 8] = [
    (0.9894009349916499, 0.027152459411754096),
    (0.9445750230732326, 0.062253523938647894),
    (0.8656312023878318, 0.09515851168249279),
    (0.755404408355003, 0.12462897125553388),
    (0.6178762444026438, 0.14959598881657674),
    (0.45801677765722737, 0.16915651939500254),
    (0.2816035507792589, 0.18260341504492358),
    (0.09501250983763744, 0.1894506104550685),
];
use super::stirling_error;
use statrs::function::{
    beta::ln_beta,
    erf::{erf_inv, erfc_inv},
};

fn inverse_slope(df: f64) -> f64 {
    if df < f64::EPSILON.sqrt() {
        df.sqrt().recip() * (1. + df * std::f64::consts::LN_2)
    } else if df >= 64. {
        let z = df / 2.;
        std::f64::consts::FRAC_PI_2.sqrt()
            * (0.5 - z * (0.5 / z).ln_1p() + stirling_error(z) - stirling_error(z + 0.5)).exp()
    } else {
        (0.5 * df.ln() + ln_beta(df / 2., 0.5) - std::f64::consts::LN_2).exp()
    }
}

fn center_quantile(confidence: f64, df: f64, slope: f64) -> Option<f64> {
    if confidence * confidence * std::f64::consts::FRAC_PI_2 > f64::EPSILON.cbrt() {
        return None;
    }
    let leading = confidence * slope;
    let curvature = (leading * df.sqrt().recip().hypot(1.)).powi(2);
    if !curvature.is_finite() || curvature > f64::EPSILON.cbrt() {
        return None;
    }
    Some(leading * (1. + curvature * (1. / 6. + curvature * (7. - 6. / (df + 1.)) / 120.)))
}

fn gamma_ratio_per_df(df: f64, slope: f64) -> f64 {
    if df > 1. / 16. {
        return (0.5 * df.ln() + slope.ln()) / df;
    }
    // Gamma duplication and DLMF 5.7.3 give this zeta expansion.
    // With df <= 1/16, the omitted normalized remainder is below 2e-17.
    let zeta = [
        1.6449340668482264,
        1.2020569031595942,
        1.0823232337111382,
        1.03692775514337,
        1.0173430619844492,
        1.0083492773819228,
        1.0040773561979443,
        1.0020083928260821,
        1.000994575127818,
        1.0004941886041194,
        1.000246086553308,
        1.0001227133475785,
    ];
    let mut result = std::f64::consts::LN_2;
    let mut power = df;
    for (i, value) in zeta.into_iter().enumerate() {
        let k = (i + 2) as i32;
        let sign = if k % 2 == 0 { 1. } else { -1. };
        result += sign * (2_f64.powi(1 - k) - 1.) * value / f64::from(k) * power;
        power *= df;
    }
    result
}

fn integrate(left: f64, right: f64, f: impl Fn(f64) -> f64) -> f64 {
    let mid = (left + right) / 2.;
    let half = (right - left) / 2.;
    half * GAUSS
        .iter()
        .map(|&(x, w)| w * (f(mid - half * x) + f(mid + half * x)))
        .sum::<f64>()
}

struct Density {
    df: f64,
    coordinate_scale: f64,
    normalization: f64,
}
impl Density {
    fn new(df: f64, inverse_slope: f64) -> Self {
        let scale = if df >= 1. { df.sqrt() } else { 1. };
        Self {
            df,
            coordinate_scale: scale,
            normalization: if df >= 1. {
                inverse_slope.recip()
            } else {
                df.sqrt() / inverse_slope
            },
        }
    }

    fn kernel(&self, w: f64) -> f64 {
        let u = w / self.coordinate_scale;
        // Scale the log-cosh series before subnormal squaring; its next relative
        // term is at most (17/1260)*EPSILON in this range.
        if self.df >= 1. && u * u <= f64::EPSILON.cbrt() {
            let square = u * u;
            return -0.5 * w * w * (1. - square / 6. + 2. * square * square / 45.);
        }
        let log_cosh = if u > 20. {
            u + (-2. * u).exp().ln_1p() - std::f64::consts::LN_2
        } else {
            let s = (u / 2.).sinh();
            (2. * s * s).ln_1p()
        };
        -self.df * log_cosh
    }
    fn central_integral(&self, w: f64) -> f64 {
        // The central target is <= 1/2; fixed panels also bound bracket work.
        let parts = w.ceil().clamp(1., 32.) as usize;
        let step = w / parts as f64;
        (0..parts)
            .map(|i| {
                integrate(i as f64 * step, (i + 1) as f64 * step, |x| {
                    self.kernel(x).exp()
                })
            })
            .sum()
    }
    fn tail_integral(&self, w: f64) -> f64 {
        let u = w / self.coordinate_scale;
        let slope = self.df / self.coordinate_scale * u.tanh();
        let origin = self.kernel(w);
        // The log-density is concave. Rescaling by its slope bounds the
        // integrand by exp(-v), including the omitted integral after v=39.
        let bounds = [0., 1., 3., 7., 15., 31., 39.];
        bounds
            .windows(2)
            .map(|b| integrate(b[0], b[1], |v| (self.kernel(w + v / slope) - origin).exp()))
            .sum::<f64>()
            / slope
    }

    fn log_two_sided_tail(&self, w: f64) -> f64 {
        if w <= 1. {
            return (-self.normalization * self.central_integral(w)).ln_1p();
        }
        let origin = self.kernel(w);
        let slope = self.df / self.coordinate_scale * (w / self.coordinate_scale).tanh();
        let log_normalization = self.normalization.ln();
        // Log-concavity bounds the remaining integral by exp(origin)/slope.
        // Only probabilities below half the smallest positive value round to zero.
        if log_normalization + origin - slope.ln() < f64::from_bits(1).ln() - std::f64::consts::LN_2
        {
            return f64::NEG_INFINITY;
        }
        log_normalization + origin + self.tail_integral(w).ln()
    }
    fn project(&self, w: f64) -> Option<f64> {
        let u = w / self.coordinate_scale;
        let q = if u <= 20. {
            self.df.sqrt() * u.sinh()
        } else {
            (0.5 * self.df.ln() + u - std::f64::consts::LN_2 + (-(-2. * u).exp()).ln_1p()).exp()
        };
        (q.is_finite() && q > 0.).then_some(q)
    }
    fn solve(&self, confidence: f64, tail: f64) -> Option<f64> {
        let target = if confidence <= 0.5 {
            (confidence / self.normalization).ln()
        } else {
            (2. * tail).ln()
        };
        let evaluate = |w: f64| {
            if confidence <= 0.5 {
                let integral = self.central_integral(w);
                (target - integral.ln(), integral * (-self.kernel(w)).exp())
            } else {
                let integral = self.tail_integral(w);
                (
                    self.normalization.ln() + self.kernel(w) + integral.ln() - target,
                    integral,
                )
            }
        };
        let normal = std::f64::consts::SQRT_2
            * if confidence <= 0.5 {
                erf_inv(confidence)
            } else {
                erfc_inv(2. * tail)
            };
        let initial = (self.coordinate_scale * (normal / self.df.sqrt()).asinh()).max(1.);
        let (mut low, mut high) = (0., initial);
        let mut bracket = None;
        for _ in 0..64 {
            let (error, _) = evaluate(high);
            if !error.is_finite() {
                return None;
            }
            if error <= 0. {
                bracket = Some((low, high));
                break;
            }
            low = high;
            high *= 2.;
        }
        let (mut low, mut high) = bracket?;
        let mut w = initial;
        for _ in 0..64 {
            let (error, inverse) = evaluate(w);
            if !error.is_finite() || !inverse.is_finite() {
                return None;
            }
            if error.abs() <= 8. * f64::EPSILON * (1. + target.abs()) {
                return self.project(w);
            }
            if error > 0. {
                low = w;
            } else {
                high = w;
            }
            let next = w + error * inverse;
            let next = if next > low && next < high {
                next
            } else {
                low + (high - low) / 2.
            };
            if next == w {
                return None;
            }
            w = next;
        }
        None
    }
}

fn quantile(confidence: f64, tail: f64, df: f64) -> Option<f64> {
    if df == 1. {
        let q = if confidence <= 0.5 {
            (std::f64::consts::FRAC_PI_2 * confidence).tan()
        } else {
            (std::f64::consts::PI * tail).tan().recip()
        };
        return (q.is_finite() && q > 0.).then_some(q);
    }
    if df == 2. {
        let q = (confidence / (2. * tail).sqrt()) / (1. - tail).sqrt();
        return (q.is_finite() && q > 0.).then_some(q);
    }
    let slope = inverse_slope(df);
    if let Some(q) = center_quantile(confidence, df, slope) {
        return Some(q);
    }
    let log_tail = if confidence <= 0.5 {
        (-confidence).ln_1p()
    } else {
        (2. * tail).ln()
    };
    let far = -log_tail / df - gamma_ratio_per_df(df, slope);
    // The power-law inverse includes its first correction. At log(t/sqrt(df))
    // >= 18 the next term is O(exp(-72)), avoiding overflow in intermediate t^2.
    if far >= 18. {
        let correction = 0.5 * (1. - 1. / (df + 2.)) * (-2. * far).exp();
        let q = (0.5 * df.ln() + far + (-correction).ln_1p()).exp();
        return (q.is_finite() && q > 0.).then_some(q);
    }
    let density = Density::new(df, slope);
    density.solve(confidence, tail)
}

/// Two-sided log tail in the stable large-df normalization regime (df >= 64).
pub(super) fn log_two_sided_tail(magnitude: f64, df: f64) -> f64 {
    let density = Density::new(df, inverse_slope(df));
    let w = df.sqrt() * (magnitude / df.sqrt()).asinh();
    density.log_two_sided_tail(w)
}

pub(crate) fn confidence_quantile(confidence: f64, df: f64) -> Option<f64> {
    quantile(confidence, (1. - confidence) / 2., df)
}
pub(crate) fn upper_quantile(tail: f64, df: f64) -> Option<f64> {
    if tail == 0.5 {
        Some(0.)
    } else if tail > 0.5 {
        quantile((tail - 0.5) * 2., 1. - tail, df).map(|q| -q)
    } else {
        quantile((0.5 - tail) * 2., tail, df)
    }
}
