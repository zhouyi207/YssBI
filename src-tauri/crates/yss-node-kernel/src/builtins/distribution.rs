use crate::{KernelError, KernelInvocation, RuntimeValue};
use std::collections::BTreeMap;
use yss_data_contract::TabularScalar;
use yss_sci_contract::distribution::{SampleValue, SamplingDistribution};
use yss_sci_contract::scientific::{ScientificComputationError, ScientificExecutionControl};

#[derive(Clone, Copy)]
pub(crate) enum DistributionKernel {
    Normal,
    Uniform,
    Exponential,
    Gamma,
    Beta,
    StudentsT,
    Cauchy,
    ChiSquared,
    LogNormal,
    Weibull,
    Laplace,
    Pareto,
    InverseGamma,
    Triangular,
    FisherSnedecor,
    Erlang,
    Bernoulli,
    Binomial,
    Poisson,
    Geometric,
    NegativeBinomial,
    DiscreteUniform,
    Hypergeometric,
}

fn integer(values: &BTreeMap<Box<str>, RuntimeValue>, key: &str) -> Result<i64, KernelError> {
    match values.get(key) {
        Some(RuntimeValue::Scalar(TabularScalar::Integer(value))) => Ok(*value),
        Some(RuntimeValue::Scalar(TabularScalar::Unsigned(value))) => {
            i64::try_from(*value).map_err(|_| KernelError::InvalidParameter)
        }
        _ => Err(KernelError::InvalidParameter),
    }
}

pub(crate) fn execute(
    kind: DistributionKernel,
    invocation: &KernelInvocation<'_>,
) -> Result<RuntimeValue, KernelError> {
    use DistributionKernel::*;
    use SamplingDistribution as D;
    invocation.check_control()?;
    let Some(RuntimeValue::Record(config)) = invocation.parameter("configuration") else {
        return Err(KernelError::InvalidParameter);
    };
    let number = |key: &str| {
        // Configuration decimals retain the protocol's exact textual spelling at the wire.
        let value = match config.get(key) {
            Some(RuntimeValue::Scalar(TabularScalar::String(value))) => value
                .parse::<f64>()
                .map_err(|_| KernelError::InvalidParameter)?,
            value => super::numeric_input(value).map_err(|_| KernelError::InvalidParameter)?,
        };
        if value.is_finite() {
            Ok(value)
        } else {
            Err(KernelError::InvalidParameter)
        }
    };
    let natural =
        |key| u64::try_from(integer(config, key)?).map_err(|_| KernelError::InvalidParameter);
    let count = usize::try_from(integer(config, "sample_count")?)
        .ok()
        .filter(|n| *n > 0)
        .ok_or(KernelError::InvalidParameter)?;
    invocation
        .control
        .check_bytes(count.checked_mul(size_of::<SampleValue>() + size_of::<RuntimeValue>() * 2))?;
    let distribution = match kind {
        Normal => D::Normal {
            mean: number("mean")?,
            standard_deviation: number("standard_deviation")?,
        },
        Uniform => D::Uniform {
            lower: number("lower_bound")?,
            upper: number("upper_bound")?,
        },
        Exponential => D::Exponential {
            rate: number("rate")?,
        },
        Gamma => D::Gamma {
            shape: number("shape")?,
            rate: number("rate")?,
        },
        Beta => D::Beta {
            alpha: number("alpha")?,
            beta: number("beta")?,
        },
        StudentsT => D::StudentsT {
            degrees_of_freedom: number("degrees_of_freedom")?,
        },
        Cauchy => D::Cauchy {
            location: number("location")?,
            scale: number("scale")?,
        },
        ChiSquared => D::ChiSquared {
            degrees_of_freedom: number("degrees_of_freedom")?,
        },
        LogNormal => D::LogNormal {
            mu: number("mu")?,
            sigma: number("sigma")?,
        },
        Weibull => D::Weibull {
            shape: number("shape")?,
            scale: number("scale")?,
        },
        Laplace => D::Laplace {
            location: number("location")?,
            scale: number("scale")?,
        },
        Pareto => D::Pareto {
            shape: number("shape")?,
            scale: number("scale")?,
        },
        InverseGamma => D::InverseGamma {
            shape: number("shape")?,
            scale: number("scale")?,
        },
        Triangular => D::Triangular {
            minimum: number("minimum")?,
            maximum: number("maximum")?,
            mode: number("mode")?,
        },
        FisherSnedecor => D::FisherSnedecor {
            numerator: number("numerator_degrees_of_freedom")?,
            denominator: number("denominator_degrees_of_freedom")?,
        },
        Erlang => D::Erlang {
            shape: natural("shape")?,
            rate: number("rate")?,
        },
        Bernoulli => D::Bernoulli {
            probability: number("probability")?,
        },
        Binomial => D::Binomial {
            trials: natural("trial_count")?,
            probability: number("probability")?,
        },
        Poisson => D::Poisson {
            rate: number("rate")?,
        },
        Geometric => D::Geometric {
            probability: number("probability")?,
        },
        NegativeBinomial => D::NegativeBinomial {
            successes: number("success_count")?,
            probability: number("probability")?,
        },
        DiscreteUniform => D::DiscreteUniform {
            lower: integer(config, "lower_bound")?,
            upper: integer(config, "upper_bound")?,
        },
        Hypergeometric => D::Hypergeometric {
            population: natural("population_size")?,
            successes: natural("success_population")?,
            draws: natural("draw_count")?,
        },
    };
    let mut samples = invocation.control.reserve(count)?;
    samples.resize(count, SampleValue::Integer(0));
    yss_sci_runtime::distribution::sample_into(
        distribution,
        &mut samples,
        &ScientificExecutionControl::from_shared(
            invocation.control.cancellation.clone(),
            invocation.control.deadline,
        ),
    )
    .map_err(|error| match error {
        ScientificComputationError::InvalidInput { .. } => KernelError::InvalidParameter,
        ScientificComputationError::Cancelled => KernelError::Cancelled,
        ScientificComputationError::DeadlineExceeded => KernelError::DeadlineExceeded,
        ScientificComputationError::ComputationFailed => KernelError::NonFiniteResult,
    })?;
    let mut values = invocation.control.reserve(count)?;
    for (index, sample) in samples.into_iter().enumerate() {
        if index % 1024 == 0 {
            invocation.check_control()?;
        }
        values.push(match sample {
            SampleValue::Integer(value) => RuntimeValue::Scalar(TabularScalar::Integer(value)),
            SampleValue::Float(value) => {
                RuntimeValue::float64(value).map_err(|_| KernelError::NonFiniteResult)?
            }
        });
    }
    Ok(RuntimeValue::List(values.into()))
}
