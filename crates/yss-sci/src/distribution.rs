//! Sampling and shared reference-distribution tails; no node IDs or runtime containers here.
use rand::{
    Rng, RngExt,
    distr::{Distribution, Open01},
};
use statrs::distribution::{self as native, ContinuousCDF};
use statrs::function::beta::ln_beta;
use yss_sci_contract::distribution::{SampleValue, SamplingDistribution};
use yss_sci_contract::execution::{
    ScientificComputationError as Error, ScientificExecutionControl, ScientificInputViolation,
};
use yss_sci_contract::hypothesis::Alternative;

pub(crate) mod student_t;

/// Stirling log-Gamma remainder through 1/z^7, for z >= 16.
pub(crate) fn stirling_error(z: f64) -> f64 {
    let q = 1. / (z * z);
    (1. / 12. - q * (1. / 360. - q * (1. / 1260. - q / 1680.))) / z
}

/// Standard-normal two-sided tails round once, including subnormal probabilities.
pub(crate) fn normal_two_sided_p(statistic: f64) -> f64 {
    statrs::function::erf::erfc(statistic.abs() / std::f64::consts::SQRT_2)
}

/// Directed probabilities for validated standard Student-t reference distributions.
pub(crate) fn student_t_probability(
    distribution: &native::StudentsT,
    statistic: f64,
    alternative: Alternative,
) -> f64 {
    let magnitude = statistic.abs();
    let df = distribution.freedom();
    if df.is_finite() && magnitude > df.sqrt() / f64::EPSILON.sqrt() {
        // I_h(a, 1/2) = h^a / (a B(a, 1/2)) * (1 + O(h)), with
        // h = df/(df+t^2) < EPSILON. Avoid both t^2 overflow and h underflow,
        // and include the one-sided factor before rounding subnormal probabilities.
        let log_magnitude = magnitude.ln();
        let log_tail = if df >= 5.0
            && 25.0_f64.ln() - 4.0 * log_magnitude < f64::from_bits(1).ln() - std::f64::consts::LN_2
        {
            // E[T^4] <= 25 for df >= 5: Markov bounds even the two-sided
            // probability below half the smallest representable positive value.
            f64::NEG_INFINITY
        } else {
            let a = df / 2.0;
            a * (df.ln() - 2.0 * log_magnitude) - a.ln() - ln_beta(a, 0.5)
        };
        return match alternative {
            Alternative::TwoSided => log_tail.exp(),
            Alternative::Greater | Alternative::Less => {
                let tail = (log_tail - std::f64::consts::LN_2).exp();
                if (statistic >= 0.0) == (alternative == Alternative::Greater) {
                    tail
                } else {
                    1.0 - tail
                }
            }
        };
    }
    match alternative {
        Alternative::TwoSided => 2.0 * distribution.sf(magnitude),
        Alternative::Greater => distribution.sf(statistic),
        Alternative::Less => distribution.cdf(statistic),
    }
}

/// F upper tails avoid the native SF's subtraction when its beta argument is near one.
pub(crate) fn fisher_snedecor_sf(distribution: &native::FisherSnedecor, statistic: f64) -> f64 {
    if statistic <= distribution.freedom_2() / distribution.freedom_1() {
        return distribution.sf(statistic);
    }
    let reciprocal =
        native::FisherSnedecor::new(distribution.freedom_2(), distribution.freedom_1())
            .expect("validated F degrees remain valid when swapped");
    reciprocal.cdf(1.0 / statistic)
}

fn invalid() -> Error {
    Error::InvalidInput {
        violation: ScientificInputViolation::ParameterOutOfRange,
    }
}
fn finite(value: f64) -> Result<f64, Error> {
    value.is_finite().then_some(value).ok_or_else(invalid)
}
fn positive(value: f64) -> Result<f64, Error> {
    if value.is_finite() && value > 0.0 {
        Ok(value)
    } else {
        Err(invalid())
    }
}
fn probability(value: f64, zero: bool) -> Result<f64, Error> {
    if value.is_finite() && value <= 1.0 && (value > 0.0 || (zero && value == 0.0)) {
        Ok(value)
    } else {
        Err(invalid())
    }
}

// Native discrete samplers that use floating-point arithmetic cannot represent wider counts.
const MAX_EXACT_COUNT: u64 = 1 << 53;

pub fn sample_into(
    distribution: SamplingDistribution,
    output: &mut [SampleValue],
    control: &ScientificExecutionControl,
) -> Result<(), Error> {
    sample_with_rng(distribution, output, control, &mut rand::rng())
}

fn sample_with_rng<R: Rng + ?Sized>(
    spec: SamplingDistribution,
    output: &mut [SampleValue],
    control: &ScientificExecutionControl,
    rng: &mut R,
) -> Result<(), Error> {
    use SamplingDistribution::*;
    control.check()?;
    if output.is_empty() {
        return Err(invalid());
    }
    macro_rules! floats {
        ($distribution:expr) => {{
            let distribution = $distribution.map_err(|_| invalid())?;
            for value in output.iter_mut() {
                control.check()?;
                let sample: f64 = distribution.sample(rng);
                let positive_support = matches!(
                    spec,
                    Gamma { .. }
                        | ChiSquared { .. }
                        | LogNormal { .. }
                        | Weibull { .. }
                        | Pareto { .. }
                        | InverseGamma { .. }
                        | FisherSnedecor { .. }
                        | Erlang { .. }
                );
                if !sample.is_finite() || (positive_support && sample <= 0.0) {
                    return Err(Error::ComputationFailed);
                }
                *value = SampleValue::Float(sample);
            }
        }};
    }
    match spec {
        Normal {
            mean,
            standard_deviation,
        } => floats!(native::Normal::new(
            finite(mean)?,
            positive(standard_deviation)?
        )),
        Uniform { lower, upper } => {
            finite(lower)?;
            finite(upper)?;
            positive(upper - lower)?;
            floats!(native::Uniform::new(lower, upper));
        }
        Exponential { rate } => floats!(native::Exp::new(positive(rate)?)),
        Gamma { shape, rate } => floats!(native::Gamma::new(positive(shape)?, positive(rate)?)),
        Beta { alpha, beta } => {
            // The native sampler normalizes two Gamma variates by their sum.
            finite(alpha + beta)?;
            floats!(native::Beta::new(positive(alpha)?, positive(beta)?));
        }
        StudentsT { degrees_of_freedom } => floats!(native::StudentsT::new(
            0.0,
            1.0,
            positive(degrees_of_freedom)?
        )),
        Cauchy { location, scale } => {
            floats!(native::Cauchy::new(finite(location)?, positive(scale)?))
        }
        ChiSquared { degrees_of_freedom } => {
            floats!(native::ChiSquared::new(positive(degrees_of_freedom)?))
        }
        LogNormal { mu, sigma } => floats!(native::LogNormal::new(finite(mu)?, positive(sigma)?)),
        Weibull { shape, scale } => {
            floats!(native::Weibull::new(positive(shape)?, positive(scale)?))
        }
        Laplace { location, scale } => {
            floats!(native::Laplace::new(finite(location)?, positive(scale)?))
        }
        Pareto { shape, scale } => floats!(native::Pareto::new(positive(scale)?, positive(shape)?)),
        // statrs calls beta 'rate', but its inverse-gamma density contains exp(-beta / x).
        InverseGamma { shape, scale } => floats!(native::InverseGamma::new(
            positive(shape)?,
            positive(scale)?
        )),
        Triangular {
            minimum,
            maximum,
            mode,
        } => {
            finite(minimum)?;
            finite(maximum)?;
            finite(mode)?;
            positive(maximum - minimum)?;
            floats!(native::Triangular::new(minimum, maximum, mode));
        }
        FisherSnedecor {
            numerator,
            denominator,
        } => floats!(native::FisherSnedecor::new(
            positive(numerator)?,
            positive(denominator)?
        )),
        Erlang { shape, rate } => {
            if shape == 0 || shape > MAX_EXACT_COUNT {
                return Err(invalid());
            }
            floats!(native::Erlang::new(shape, positive(rate)?));
        }
        Bernoulli { probability: p } => {
            let distribution =
                native::Bernoulli::new(probability(p, true)?).map_err(|_| invalid())?;
            for value in output.iter_mut() {
                control.check()?;
                let sample: bool = distribution.sample(rng);
                *value = SampleValue::Integer(i64::from(sample));
            }
        }
        Binomial {
            trials,
            probability: p,
        } => {
            if trials > MAX_EXACT_COUNT {
                return Err(invalid());
            }
            let distribution =
                native::Binomial::new(probability(p, true)?, trials).map_err(|_| invalid())?;
            let sampler = distribution
                .sampler(native::BinomialAlgorithm::Automatic)
                .map_err(|_| invalid())?;
            for value in output.iter_mut() {
                control.check()?;
                let sample: u64 = sampler.sample(rng);
                *value = SampleValue::Integer(
                    i64::try_from(sample).map_err(|_| Error::ComputationFailed)?,
                );
            }
        }
        Poisson { rate } => {
            if !rate.is_finite() || rate < 0.0 || rate > MAX_EXACT_COUNT as f64 {
                return Err(invalid());
            }
            for value in output.iter_mut() {
                control.check()?;
                *value = SampleValue::Integer(poisson(rate, rng)?);
            }
        }
        Geometric { probability: p } => {
            probability(p, false)?;
            for value in output.iter_mut() {
                control.check()?;
                // ln_1p preserves small probabilities; Open01 avoids a zero-trial endpoint.
                let sample = if p == 1.0 {
                    1.0
                } else {
                    let u: f64 = rng.sample(Open01);
                    (u.ln() / (-p).ln_1p()).floor() + 1.0
                };
                if !sample.is_finite() || sample < 1.0 || sample > MAX_EXACT_COUNT as f64 {
                    return Err(Error::ComputationFailed);
                }
                *value = SampleValue::Integer(sample as i64);
            }
        }
        NegativeBinomial {
            successes,
            probability: p,
        } => {
            positive(successes)?;
            probability(p, false)?;
            let gamma = if p == 1.0 {
                None
            } else {
                Some(
                    native::Gamma::new(successes, positive(p / (1.0 - p))?)
                        .map_err(|_| invalid())?,
                )
            };
            for value in output.iter_mut() {
                control.check()?;
                let rate = gamma.as_ref().map_or(0.0, |gamma| gamma.sample(rng));
                *value = SampleValue::Integer(poisson(rate, rng)?);
            }
        }
        DiscreteUniform { lower, upper } => {
            if lower > upper {
                return Err(invalid());
            }
            for value in output.iter_mut() {
                control.check()?;
                *value = SampleValue::Integer(rng.random_range(lower..=upper));
            }
        }
        Hypergeometric {
            population,
            successes,
            draws,
        } => {
            if successes > population || draws > population || population > i64::MAX as u64 {
                return Err(invalid());
            }
            for value in output.iter_mut() {
                control.check()?;
                // Sample the smaller complement using exact integer urn draws. Unlike the native
                // per-draw f64 loop this handles zero draws and permits cooperative cancellation.
                let complement = draws > population - draws;
                let steps = draws.min(population - draws);
                let (mut remaining, mut available, mut hits) = (population, successes, 0u64);
                for index in 0..steps {
                    if index % 1024 == 0 {
                        control.check()?;
                    }
                    if available == 0 {
                        break;
                    }
                    if available == remaining {
                        hits += steps - index;
                        break;
                    }
                    if rng.random_range(0..remaining) < available {
                        hits += 1;
                        available -= 1;
                    }
                    remaining -= 1;
                }
                let sample = if complement { successes - hits } else { hits };
                *value = SampleValue::Integer(sample as i64);
            }
        }
    }
    control.check()
}

fn poisson<R: Rng + ?Sized>(rate: f64, rng: &mut R) -> Result<i64, Error> {
    if rate == 0.0 {
        return Ok(0);
    }
    if !rate.is_finite() || rate < 0.0 || rate > MAX_EXACT_COUNT as f64 {
        return Err(Error::ComputationFailed);
    }
    let distribution = native::Poisson::new(rate).map_err(|_| invalid())?;
    let value: f64 = distribution.sample(rng);
    if !value.is_finite() || value < 0.0 || value > MAX_EXACT_COUNT as f64 || value.fract() != 0.0 {
        return Err(Error::ComputationFailed);
    }
    Ok(value as i64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use std::{
        sync::{Arc, atomic::AtomicBool},
        time::{Duration, Instant},
    };

    #[test]
    fn sampling_preserves_parameter_conventions_integer_edges_and_control() {
        use SamplingDistribution::*;
        let control = ScientificExecutionControl::from_shared(
            Arc::new(AtomicBool::new(false)),
            Instant::now() + Duration::from_secs(10),
        );
        let mut rng = rand::rngs::StdRng::seed_from_u64(37);
        let mut samples = vec![SampleValue::Integer(0); 8192];
        // These are adapter conventions, not tests of the library's distribution functions.
        for (distribution, expected) in [
            (
                Gamma {
                    shape: 4.0,
                    rate: 2.0,
                },
                2.0,
            ),
            (
                InverseGamma {
                    shape: 4.0,
                    scale: 6.0,
                },
                2.0,
            ),
            (
                Pareto {
                    shape: 4.0,
                    scale: 3.0,
                },
                4.0,
            ),
            (
                NegativeBinomial {
                    successes: 2.5,
                    probability: 0.5,
                },
                2.5,
            ),
            (Geometric { probability: 0.25 }, 4.0),
            (
                Hypergeometric {
                    population: 20,
                    successes: 8,
                    draws: 15,
                },
                6.0,
            ),
        ] {
            sample_with_rng(distribution, &mut samples, &control, &mut rng).unwrap();
            let mean = samples
                .iter()
                .map(|sample| match sample {
                    SampleValue::Integer(n) => *n as f64,
                    SampleValue::Float(n) => *n,
                })
                .sum::<f64>()
                / samples.len() as f64;
            assert!((mean - expected).abs() < 0.15, "{distribution:?}: {mean}");
        }
        let mut samples = [SampleValue::Integer(-1); 8];
        for (distribution, expected) in [
            (Bernoulli { probability: 0.0 }, 0),
            (Bernoulli { probability: 1.0 }, 1),
            (
                Binomial {
                    trials: 0,
                    probability: 0.5,
                },
                0,
            ),
            (
                Binomial {
                    trials: 7,
                    probability: 1.0,
                },
                7,
            ),
            (Poisson { rate: 0.0 }, 0),
            (Geometric { probability: 1.0 }, 1),
            (
                NegativeBinomial {
                    successes: 2.5,
                    probability: 1.0,
                },
                0,
            ),
            (
                Hypergeometric {
                    population: 0,
                    successes: 0,
                    draws: 0,
                },
                0,
            ),
            (
                Hypergeometric {
                    population: 20,
                    successes: 8,
                    draws: 20,
                },
                8,
            ),
            (
                DiscreteUniform {
                    lower: i64::MAX,
                    upper: i64::MAX,
                },
                i64::MAX,
            ),
            (
                DiscreteUniform {
                    lower: i64::MIN,
                    upper: i64::MIN,
                },
                i64::MIN,
            ),
        ] {
            sample_with_rng(distribution, &mut samples, &control, &mut rng).unwrap();
            assert_eq!(
                samples,
                [SampleValue::Integer(expected); 8],
                "{distribution:?}"
            );
        }
        sample_with_rng(
            DiscreteUniform {
                lower: i64::MIN,
                upper: i64::MAX,
            },
            &mut samples,
            &control,
            &mut rng,
        )
        .unwrap();
        for distribution in [
            Normal {
                mean: 0.0,
                standard_deviation: 0.0,
            },
            Uniform {
                lower: -f64::MAX,
                upper: f64::MAX,
            },
            Gamma {
                shape: f64::INFINITY,
                rate: 1.0,
            },
            Triangular {
                minimum: 0.0,
                maximum: 1.0,
                mode: 2.0,
            },
            Geometric { probability: 0.0 },
            Binomial {
                trials: MAX_EXACT_COUNT + 1,
                probability: 0.5,
            },
            Hypergeometric {
                population: 5,
                successes: 6,
                draws: 1,
            },
        ] {
            assert!(
                matches!(
                    sample_with_rng(distribution, &mut samples, &control, &mut rng),
                    Err(Error::InvalidInput { .. })
                ),
                "{distribution:?}"
            );
        }
        let timeout = ScientificExecutionControl::from_shared(
            Arc::new(AtomicBool::new(false)),
            Instant::now(),
        );
        assert_eq!(
            sample_with_rng(Poisson { rate: 1.0 }, &mut samples, &timeout, &mut rng),
            Err(Error::DeadlineExceeded)
        );
        let cancellation = control.cancellation.clone();
        let worker = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(2));
            cancellation.cancel();
        });
        assert_eq!(
            sample_with_rng(
                Hypergeometric {
                    population: 20_000_000_000,
                    successes: 10_000_000_000,
                    draws: 10_000_000_000
                },
                &mut samples,
                &control,
                &mut rng
            ),
            Err(Error::Cancelled)
        );
        worker.join().unwrap();
    }
}
