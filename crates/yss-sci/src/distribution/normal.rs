//! Standard-normal reference probabilities; sampling remains with the native distributions.
use std::f64::consts::SQRT_2;

const LOG_SQRT_2_PI: f64 = 0.9189385332046727;

fn log_density(z: f64) -> f64 {
    -(0.5 * z) * z - LOG_SQRT_2_PI
}

fn mills_fraction(z: f64) -> f64 {
    // Laplace's continued fraction for SF(z)/phi(z), z >= 8 (DLMF 7.9.1).
    // The fixed depth converges beyond f64 precision; scaling keeps z*z out
    // of the ratio and retains finite values after the probability underflows.
    let mut fraction = 0.0;
    for k in (1..=32).rev() {
        fraction = f64::from(k) / (z + fraction);
    }
    fraction
}

fn log_mills_ratio(z: f64) -> f64 {
    -z.ln() - (mills_fraction(z) / z).ln_1p()
}

fn positive_tail(z: f64, factor: f64) -> f64 {
    if z >= 40.0 {
        return 0.0;
    }
    let square = z * z;
    let error = z.mul_add(z, -square);
    let half_density = (-0.25 * square).exp() * (-0.25 * error).exp();
    // Compensate the square before exponentiation and include the one/two-sided
    // factor before the final product, which alone may round to a subnormal.
    half_density * (half_density * (0.3989422804014327 * factor / (z + mills_fraction(z))))
}

fn ordinary_tail(z: f64, factor: f64) -> f64 {
    let argument = z / SQRT_2;
    // Correct the rounded sqrt(2) conversion. Its argument error otherwise
    // becomes many probability ulps near the tail branch.
    let error = argument.mul_add(SQRT_2, -z) + argument * -9.667293313452913e-17;
    0.5 * factor * libm::erfc(argument)
        + (0.3989422804014327 * factor * (-0.5 * z * z).exp()) * error
}

pub(crate) fn sf(z: f64) -> f64 {
    if z >= 8.0 {
        positive_tail(z, 1.0)
    } else if z <= -8.0 {
        1.0 - positive_tail(-z, 1.0)
    } else {
        ordinary_tail(z, 1.0)
    }
}

pub(crate) fn cdf(z: f64) -> f64 {
    sf(-z)
}

pub(crate) fn log_sf(z: f64) -> f64 {
    if z >= 8.0 {
        log_density(z) + log_mills_ratio(z)
    } else if z > 0.0 {
        sf(z).ln()
    } else {
        (-sf(-z)).ln_1p()
    }
}

pub(crate) fn log_cdf(z: f64) -> f64 {
    log_sf(-z)
}

pub(crate) fn sum_sf(first: f64, second: f64) -> f64 {
    let first = log_sf(first);
    let second = log_sf(second);
    let (large, small) = if first >= second {
        (first, second)
    } else {
        (second, first)
    };
    if large == f64::NEG_INFINITY {
        0.0
    } else {
        (large + (small - large).exp().ln_1p()).exp()
    }
}

pub(crate) fn log_cdf_density_ratio(z: f64) -> f64 {
    if z <= -8.0 {
        log_mills_ratio(-z)
    } else {
        log_cdf(z) - log_density(z)
    }
}

pub(crate) fn two_sided_p(z: f64) -> f64 {
    let magnitude = z.abs();
    if magnitude >= 8.0 {
        positive_tail(magnitude, 2.0)
    } else {
        ordinary_tail(magnitude, 2.0)
    }
}

pub(crate) fn upper_quantile(tail: f64) -> f64 {
    SQRT_2 * statrs::function::erf::erfc_inv(2.0 * tail)
}

pub(crate) fn two_sided_critical(alpha: f64) -> f64 {
    // Preserve alpha when halving it would round a subnormal to zero.
    SQRT_2 * statrs::function::erf::erfc_inv(alpha)
}
