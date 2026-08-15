use super::KernelFragment;
use crate::node_system::protocol::{CanonicalDecimal, Value};
use crate::node_system::runtime::{
    ArtifactKind, DataSeriesBuilder, DataSeriesElementType, Kernel, KernelContext, KernelError,
    RuntimeValue,
};
use rand::distr::Distribution;

#[derive(Clone, Copy)]
enum DistributionKind {
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

#[derive(Clone, Copy)]
struct KernelSpec {
    handle: &'static str,
    kind: DistributionKind,
}

const KERNELS: &[KernelSpec] = &[
    KernelSpec {
        handle: "yssbi.distribution.normal.sample",
        kind: DistributionKind::Normal,
    },
    KernelSpec {
        handle: "yssbi.distribution.uniform.sample",
        kind: DistributionKind::Uniform,
    },
    KernelSpec {
        handle: "yssbi.distribution.exponential.sample",
        kind: DistributionKind::Exponential,
    },
    KernelSpec {
        handle: "yssbi.distribution.gamma.sample",
        kind: DistributionKind::Gamma,
    },
    KernelSpec {
        handle: "yssbi.distribution.beta.sample",
        kind: DistributionKind::Beta,
    },
    KernelSpec {
        handle: "yssbi.distribution.students_t.sample",
        kind: DistributionKind::StudentsT,
    },
    KernelSpec {
        handle: "yssbi.distribution.cauchy.sample",
        kind: DistributionKind::Cauchy,
    },
    KernelSpec {
        handle: "yssbi.distribution.chi_squared.sample",
        kind: DistributionKind::ChiSquared,
    },
    KernelSpec {
        handle: "yssbi.distribution.log_normal.sample",
        kind: DistributionKind::LogNormal,
    },
    KernelSpec {
        handle: "yssbi.distribution.weibull.sample",
        kind: DistributionKind::Weibull,
    },
    KernelSpec {
        handle: "yssbi.distribution.laplace.sample",
        kind: DistributionKind::Laplace,
    },
    KernelSpec {
        handle: "yssbi.distribution.pareto.sample",
        kind: DistributionKind::Pareto,
    },
    KernelSpec {
        handle: "yssbi.distribution.inverse_gamma.sample",
        kind: DistributionKind::InverseGamma,
    },
    KernelSpec {
        handle: "yssbi.distribution.triangular.sample",
        kind: DistributionKind::Triangular,
    },
    KernelSpec {
        handle: "yssbi.distribution.fisher_snedecor.sample",
        kind: DistributionKind::FisherSnedecor,
    },
    KernelSpec {
        handle: "yssbi.distribution.erlang.sample",
        kind: DistributionKind::Erlang,
    },
    KernelSpec {
        handle: "yssbi.distribution.bernoulli.sample",
        kind: DistributionKind::Bernoulli,
    },
    KernelSpec {
        handle: "yssbi.distribution.binomial.sample",
        kind: DistributionKind::Binomial,
    },
    KernelSpec {
        handle: "yssbi.distribution.poisson.sample",
        kind: DistributionKind::Poisson,
    },
    KernelSpec {
        handle: "yssbi.distribution.geometric.sample",
        kind: DistributionKind::Geometric,
    },
    KernelSpec {
        handle: "yssbi.distribution.negative_binomial.sample",
        kind: DistributionKind::NegativeBinomial,
    },
    KernelSpec {
        handle: "yssbi.distribution.discrete_uniform.sample",
        kind: DistributionKind::DiscreteUniform,
    },
    KernelSpec {
        handle: "yssbi.distribution.hypergeometric.sample",
        kind: DistributionKind::Hypergeometric,
    },
];

pub(crate) fn build_kernel_fragment() -> KernelFragment {
    let mut fragment = KernelFragment::default();
    for spec in KERNELS {
        fragment.register(spec.handle, DistributionKernel { kind: spec.kind });
    }
    fragment
}

struct DistributionKernel {
    kind: DistributionKind,
}

impl Kernel for DistributionKernel {
    fn execute(
        &self,
        context: &KernelContext<'_>,
        inputs: &[RuntimeValue],
    ) -> Result<Vec<RuntimeValue>, KernelError> {
        context
            .cancellation
            .check()
            .map_err(|error| KernelError::cancelled(error.to_string()))?;
        let mut rng = rand::rng();
        let output = match self.kind {
            DistributionKind::Normal => sample_float(inputs, 3, "normal", |count| {
                let distribution = statrs::distribution::Normal::new(
                    float_input(inputs, 0)?,
                    float_input(inputs, 1)?,
                )
                .map_err(|error| invalid("Normal", error))?;
                Ok((0..count).map(|_| distribution.sample(&mut rng)).collect())
            })?,
            DistributionKind::Uniform => sample_float(inputs, 3, "uniform", |count| {
                let distribution = statrs::distribution::Uniform::new(
                    float_input(inputs, 0)?,
                    float_input(inputs, 1)?,
                )
                .map_err(|error| invalid("Uniform", error))?;
                Ok((0..count).map(|_| distribution.sample(&mut rng)).collect())
            })?,
            DistributionKind::Exponential => sample_float(inputs, 2, "exponential", |count| {
                let distribution = statrs::distribution::Exp::new(float_input(inputs, 0)?)
                    .map_err(|error| invalid("Exponential", error))?;
                Ok((0..count).map(|_| distribution.sample(&mut rng)).collect())
            })?,
            DistributionKind::Gamma => sample_float(inputs, 3, "gamma", |count| {
                let distribution = statrs::distribution::Gamma::new(
                    float_input(inputs, 0)?,
                    float_input(inputs, 1)?,
                )
                .map_err(|error| invalid("Gamma", error))?;
                Ok((0..count).map(|_| distribution.sample(&mut rng)).collect())
            })?,
            DistributionKind::Beta => sample_float(inputs, 3, "beta", |count| {
                let distribution = statrs::distribution::Beta::new(
                    float_input(inputs, 0)?,
                    float_input(inputs, 1)?,
                )
                .map_err(|error| invalid("Beta", error))?;
                Ok((0..count).map(|_| distribution.sample(&mut rng)).collect())
            })?,
            DistributionKind::StudentsT => sample_float(inputs, 2, "students_t", |count| {
                let distribution =
                    statrs::distribution::StudentsT::new(0.0, 1.0, float_input(inputs, 0)?)
                        .map_err(|error| invalid("StudentsT", error))?;
                Ok((0..count).map(|_| distribution.sample(&mut rng)).collect())
            })?,
            DistributionKind::Cauchy => sample_float(inputs, 3, "cauchy", |count| {
                let distribution = statrs::distribution::Cauchy::new(
                    float_input(inputs, 0)?,
                    float_input(inputs, 1)?,
                )
                .map_err(|error| invalid("Cauchy", error))?;
                Ok((0..count).map(|_| distribution.sample(&mut rng)).collect())
            })?,
            DistributionKind::ChiSquared => sample_float(inputs, 2, "chi_squared", |count| {
                let distribution = statrs::distribution::ChiSquared::new(float_input(inputs, 0)?)
                    .map_err(|error| invalid("ChiSquared", error))?;
                Ok((0..count).map(|_| distribution.sample(&mut rng)).collect())
            })?,
            DistributionKind::LogNormal => sample_float(inputs, 3, "log_normal", |count| {
                let distribution = statrs::distribution::LogNormal::new(
                    float_input(inputs, 0)?,
                    float_input(inputs, 1)?,
                )
                .map_err(|error| invalid("LogNormal", error))?;
                Ok((0..count).map(|_| distribution.sample(&mut rng)).collect())
            })?,
            DistributionKind::Weibull => sample_float(inputs, 3, "weibull", |count| {
                let distribution = statrs::distribution::Weibull::new(
                    float_input(inputs, 0)?,
                    float_input(inputs, 1)?,
                )
                .map_err(|error| invalid("Weibull", error))?;
                Ok((0..count).map(|_| distribution.sample(&mut rng)).collect())
            })?,
            DistributionKind::Laplace => sample_float(inputs, 3, "laplace", |count| {
                let distribution = statrs::distribution::Laplace::new(
                    float_input(inputs, 0)?,
                    float_input(inputs, 1)?,
                )
                .map_err(|error| invalid("Laplace", error))?;
                Ok((0..count).map(|_| distribution.sample(&mut rng)).collect())
            })?,
            DistributionKind::Pareto => sample_float(inputs, 3, "pareto", |count| {
                let distribution = statrs::distribution::Pareto::new(
                    float_input(inputs, 0)?,
                    float_input(inputs, 1)?,
                )
                .map_err(|error| invalid("Pareto", error))?;
                Ok((0..count).map(|_| distribution.sample(&mut rng)).collect())
            })?,
            DistributionKind::InverseGamma => sample_float(inputs, 3, "inverse_gamma", |count| {
                let distribution = statrs::distribution::InverseGamma::new(
                    float_input(inputs, 0)?,
                    float_input(inputs, 1)?,
                )
                .map_err(|error| invalid("InverseGamma", error))?;
                Ok((0..count).map(|_| distribution.sample(&mut rng)).collect())
            })?,
            DistributionKind::Triangular => sample_float(inputs, 4, "triangular", |count| {
                let distribution = statrs::distribution::Triangular::new(
                    float_input(inputs, 0)?,
                    float_input(inputs, 1)?,
                    float_input(inputs, 2)?,
                )
                .map_err(|error| invalid("Triangular", error))?;
                Ok((0..count).map(|_| distribution.sample(&mut rng)).collect())
            })?,
            DistributionKind::FisherSnedecor => {
                sample_float(inputs, 3, "fisher_snedecor", |count| {
                    let distribution = statrs::distribution::FisherSnedecor::new(
                        float_input(inputs, 0)?,
                        float_input(inputs, 1)?,
                    )
                    .map_err(|error| invalid("FisherSnedecor", error))?;
                    Ok((0..count).map(|_| distribution.sample(&mut rng)).collect())
                })?
            }
            DistributionKind::Erlang => sample_float(inputs, 3, "erlang", |count| {
                let shape = non_negative_integer(inputs, 0, "shape")?;
                if shape == 0 {
                    return Err(KernelError::new("Erlang: shape must be at least 1"));
                }
                let distribution =
                    statrs::distribution::Erlang::new(shape, float_input(inputs, 1)?)
                        .map_err(|error| invalid("Erlang", error))?;
                Ok((0..count).map(|_| distribution.sample(&mut rng)).collect())
            })?,
            DistributionKind::Bernoulli => sample_integer(inputs, 2, "bernoulli", |count| {
                let distribution = statrs::distribution::Bernoulli::new(float_input(inputs, 0)?)
                    .map_err(|error| invalid("Bernoulli", error))?;
                Ok((0..count)
                    .map(|_| Distribution::<bool>::sample(&distribution, &mut rng) as i64)
                    .collect())
            })?,
            DistributionKind::Binomial => sample_integer(inputs, 3, "binomial", |count| {
                let distribution = statrs::distribution::Binomial::new(
                    float_input(inputs, 1)?,
                    non_negative_integer(inputs, 0, "trial_count")?,
                )
                .map_err(|error| invalid("Binomial", error))?;
                unsigned_samples(count, || distribution.sample(&mut rng), "Binomial")
            })?,
            DistributionKind::Poisson => sample_integer(inputs, 2, "poisson", |count| {
                let distribution = statrs::distribution::Poisson::new(float_input(inputs, 0)?)
                    .map_err(|error| invalid("Poisson", error))?;
                unsigned_samples(count, || distribution.sample(&mut rng), "Poisson")
            })?,
            DistributionKind::Geometric => sample_integer(inputs, 2, "geometric", |count| {
                let distribution = statrs::distribution::Geometric::new(float_input(inputs, 0)?)
                    .map_err(|error| invalid("Geometric", error))?;
                unsigned_samples(count, || distribution.sample(&mut rng), "Geometric")
            })?,
            DistributionKind::NegativeBinomial => {
                sample_integer(inputs, 3, "negative_binomial", |count| {
                    let distribution = statrs::distribution::NegativeBinomial::new(
                        float_input(inputs, 0)?,
                        float_input(inputs, 1)?,
                    )
                    .map_err(|error| invalid("NegativeBinomial", error))?;
                    unsigned_samples(count, || distribution.sample(&mut rng), "NegativeBinomial")
                })?
            }
            DistributionKind::DiscreteUniform => {
                sample_integer(inputs, 3, "discrete_uniform", |count| {
                    let distribution = statrs::distribution::DiscreteUniform::new(
                        integer_input(inputs, 0)?,
                        integer_input(inputs, 1)?,
                    )
                    .map_err(|error| invalid("DiscreteUniform", error))?;
                    Ok((0..count).map(|_| distribution.sample(&mut rng)).collect())
                })?
            }
            DistributionKind::Hypergeometric => {
                sample_integer(inputs, 4, "hypergeometric", |count| {
                    let distribution = statrs::distribution::Hypergeometric::new(
                        non_negative_integer(inputs, 0, "population_size")?,
                        non_negative_integer(inputs, 1, "success_population")?,
                        non_negative_integer(inputs, 2, "draw_count")?,
                    )
                    .map_err(|error| invalid("Hypergeometric", error))?;
                    unsigned_samples(count, || distribution.sample(&mut rng), "Hypergeometric")
                })?
            }
        };
        context
            .cancellation
            .check()
            .map_err(|error| KernelError::cancelled(error.to_string()))?;
        Ok(vec![RuntimeValue::Artifact(output)])
    }
}

fn sample_float(
    inputs: &[RuntimeValue],
    arity: usize,
    name: &str,
    sample: impl FnOnce(usize) -> Result<Vec<f64>, KernelError>,
) -> Result<crate::node_system::runtime::Artifact, KernelError> {
    expect_arity(inputs, arity)?;
    let count = sample_count(inputs, arity - 1)?;
    let values = sample(count)?
        .into_iter()
        .map(decimal_value)
        .collect::<Result<Vec<_>, _>>()?;
    build_series(DataSeriesElementType::Float64, name, values)
}

fn sample_integer(
    inputs: &[RuntimeValue],
    arity: usize,
    name: &str,
    sample: impl FnOnce(usize) -> Result<Vec<i64>, KernelError>,
) -> Result<crate::node_system::runtime::Artifact, KernelError> {
    expect_arity(inputs, arity)?;
    let count = sample_count(inputs, arity - 1)?;
    build_series(
        DataSeriesElementType::Int64,
        name,
        sample(count)?.into_iter().map(Value::Integer).collect(),
    )
}

fn build_series(
    element_type: DataSeriesElementType,
    name: &str,
    values: Vec<Value>,
) -> Result<crate::node_system::runtime::Artifact, KernelError> {
    DataSeriesBuilder::new(element_type)
        .name(name)
        .format("number")
        .values(values)
        .build(ArtifactKind::Collected)
        .map_err(|error| KernelError::new(error.to_string()))
}

fn unsigned_samples(
    count: usize,
    mut sample: impl FnMut() -> u64,
    distribution: &str,
) -> Result<Vec<i64>, KernelError> {
    (0..count)
        .map(|_| {
            i64::try_from(sample()).map_err(|_| {
                KernelError::new(format!("{distribution}: sampled value is outside int64"))
            })
        })
        .collect()
}

fn expect_arity(inputs: &[RuntimeValue], expected: usize) -> Result<(), KernelError> {
    if inputs.len() == expected {
        Ok(())
    } else {
        Err(KernelError::new(format!(
            "distribution kernel received {} inputs; expected {expected}",
            inputs.len()
        )))
    }
}

fn scalar(inputs: &[RuntimeValue], index: usize) -> Result<&Value, KernelError> {
    match inputs.get(index) {
        Some(RuntimeValue::Scalar(value)) => Ok(value),
        Some(_) => Err(KernelError::new(format!("input {index} must be a scalar"))),
        None => Err(KernelError::new(format!("input {index} is missing"))),
    }
}

fn float_input(inputs: &[RuntimeValue], index: usize) -> Result<f64, KernelError> {
    let value = match scalar(inputs, index)? {
        Value::Decimal(value) => value
            .as_str()
            .parse::<f64>()
            .map_err(|_| KernelError::new(format!("input {index} is not a float64")))?,
        Value::Integer(value) => *value as f64,
        _ => return Err(KernelError::new(format!("input {index} must be numeric"))),
    };
    value
        .is_finite()
        .then_some(value)
        .ok_or_else(|| KernelError::new(format!("input {index} must be finite")))
}

fn integer_input(inputs: &[RuntimeValue], index: usize) -> Result<i64, KernelError> {
    match scalar(inputs, index)? {
        Value::Integer(value) => Ok(*value),
        _ => Err(KernelError::new(format!(
            "input {index} must be an Int64 scalar"
        ))),
    }
}

fn non_negative_integer(
    inputs: &[RuntimeValue],
    index: usize,
    name: &str,
) -> Result<u64, KernelError> {
    let value = integer_input(inputs, index)?;
    u64::try_from(value).map_err(|_| KernelError::new(format!("{name} must be non-negative")))
}

fn sample_count(inputs: &[RuntimeValue], index: usize) -> Result<usize, KernelError> {
    let count = non_negative_integer(inputs, index, "sample_count")?;
    if count == 0 {
        return Err(KernelError::new("sample_count must be positive"));
    }
    usize::try_from(count).map_err(|_| KernelError::new("sample_count exceeds platform capacity"))
}

fn decimal_value(value: f64) -> Result<Value, KernelError> {
    if !value.is_finite() {
        return Err(KernelError::new(
            "distribution produced a non-finite sample",
        ));
    }
    let mut text = format!("{value:.17}");
    if text.contains('.') {
        while text.ends_with('0') {
            text.pop();
        }
        if text.ends_with('.') {
            text.pop();
        }
    }
    if text == "-0" {
        text = "0".to_owned();
    }
    CanonicalDecimal::new(text)
        .map(Value::Decimal)
        .map_err(|error| KernelError::new(error.to_string()))
}

fn invalid(distribution: &str, error: impl std::fmt::Display) -> KernelError {
    KernelError::new(format!("{distribution}: invalid parameters: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn kernel_fragment_matches_current_distribution_catalog_inventory() {
        let node_system = crate::node_system::catalog::build_builtin_node_system().unwrap();
        let catalog_handles = node_system
            .registry
            .iter()
            .map(|(id, _)| id.as_str())
            .filter(|handle| {
                handle.starts_with("yssbi.distribution.") && handle.ends_with(".sample")
            })
            .collect::<BTreeSet<_>>();
        let spec_handles = KERNELS
            .iter()
            .map(|spec| spec.handle)
            .collect::<BTreeSet<_>>();
        let fragment = build_kernel_fragment();
        let fragment_handles = fragment
            .handles()
            .map(|handle| handle.as_str())
            .collect::<BTreeSet<_>>();

        assert_eq!(fragment_handles, spec_handles);
        assert_eq!(fragment_handles, catalog_handles);
    }
}
