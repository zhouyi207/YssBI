use super::builtin::{BuiltinAssemblyError, ProviderFragment, assembled_interface, leaf, sid};
use crate::builtin::node_key;
use crate::builtin::{category_id, concrete, data_port, icon_id, node_type_id};
use yss_node_protocol::*;
use yss_node_registry::CategoryRegistration;

const CATEGORY: &str = "distribution";
const CONTINUOUS_CATEGORY: &str = "distribution.continuous";
const DISCRETE_CATEGORY: &str = "distribution.discrete";

#[derive(Clone, Copy)]
enum NumericRepresentation {
    Float64,
    Int64,
}

#[derive(Clone, Copy)]
struct DistributionParameter {
    key: &'static str,
    value_type: NumericRepresentation,
}

#[derive(Clone, Copy)]
struct DistributionSpec {
    id: &'static str,
    category: &'static str,
    parameters: &'static [DistributionParameter],
}

const F: NumericRepresentation = NumericRepresentation::Float64;
const I: NumericRepresentation = NumericRepresentation::Int64;
const fn setting(key: &'static str, value_type: NumericRepresentation) -> DistributionParameter {
    DistributionParameter { key, value_type }
}

const SPECS: &[DistributionSpec] = &[
    DistributionSpec {
        id: "yssbi.distribution.normal.sample",
        category: CONTINUOUS_CATEGORY,
        parameters: &[
            setting("mean", F),
            setting("standard_deviation", F),
            setting("sample_count", I),
        ],
    },
    DistributionSpec {
        id: "yssbi.distribution.uniform.sample",
        category: CONTINUOUS_CATEGORY,
        parameters: &[
            setting("lower_bound", F),
            setting("upper_bound", F),
            setting("sample_count", I),
        ],
    },
    DistributionSpec {
        id: "yssbi.distribution.exponential.sample",
        category: CONTINUOUS_CATEGORY,
        parameters: &[setting("rate", F), setting("sample_count", I)],
    },
    DistributionSpec {
        id: "yssbi.distribution.gamma.sample",
        category: CONTINUOUS_CATEGORY,
        parameters: &[
            setting("shape", F),
            setting("rate", F),
            setting("sample_count", I),
        ],
    },
    DistributionSpec {
        id: "yssbi.distribution.beta.sample",
        category: CONTINUOUS_CATEGORY,
        parameters: &[
            setting("alpha", F),
            setting("beta", F),
            setting("sample_count", I),
        ],
    },
    DistributionSpec {
        id: "yssbi.distribution.students_t.sample",
        category: CONTINUOUS_CATEGORY,
        parameters: &[setting("degrees_of_freedom", F), setting("sample_count", I)],
    },
    DistributionSpec {
        id: "yssbi.distribution.cauchy.sample",
        category: CONTINUOUS_CATEGORY,
        parameters: &[
            setting("location", F),
            setting("scale", F),
            setting("sample_count", I),
        ],
    },
    DistributionSpec {
        id: "yssbi.distribution.chi_squared.sample",
        category: CONTINUOUS_CATEGORY,
        parameters: &[setting("degrees_of_freedom", F), setting("sample_count", I)],
    },
    DistributionSpec {
        id: "yssbi.distribution.log_normal.sample",
        category: CONTINUOUS_CATEGORY,
        parameters: &[
            setting("mu", F),
            setting("sigma", F),
            setting("sample_count", I),
        ],
    },
    DistributionSpec {
        id: "yssbi.distribution.weibull.sample",
        category: CONTINUOUS_CATEGORY,
        parameters: &[
            setting("shape", F),
            setting("scale", F),
            setting("sample_count", I),
        ],
    },
    DistributionSpec {
        id: "yssbi.distribution.laplace.sample",
        category: CONTINUOUS_CATEGORY,
        parameters: &[
            setting("location", F),
            setting("scale", F),
            setting("sample_count", I),
        ],
    },
    DistributionSpec {
        id: "yssbi.distribution.pareto.sample",
        category: CONTINUOUS_CATEGORY,
        parameters: &[
            setting("shape", F),
            setting("scale", F),
            setting("sample_count", I),
        ],
    },
    DistributionSpec {
        id: "yssbi.distribution.inverse_gamma.sample",
        category: CONTINUOUS_CATEGORY,
        parameters: &[
            setting("shape", F),
            setting("scale", F),
            setting("sample_count", I),
        ],
    },
    DistributionSpec {
        id: "yssbi.distribution.triangular.sample",
        category: CONTINUOUS_CATEGORY,
        parameters: &[
            setting("minimum", F),
            setting("maximum", F),
            setting("mode", F),
            setting("sample_count", I),
        ],
    },
    DistributionSpec {
        id: "yssbi.distribution.fisher_snedecor.sample",
        category: CONTINUOUS_CATEGORY,
        parameters: &[
            setting("numerator_degrees_of_freedom", F),
            setting("denominator_degrees_of_freedom", F),
            setting("sample_count", I),
        ],
    },
    DistributionSpec {
        id: "yssbi.distribution.erlang.sample",
        category: CONTINUOUS_CATEGORY,
        parameters: &[
            setting("shape", I),
            setting("rate", F),
            setting("sample_count", I),
        ],
    },
    DistributionSpec {
        id: "yssbi.distribution.bernoulli.sample",
        category: DISCRETE_CATEGORY,
        parameters: &[setting("probability", F), setting("sample_count", I)],
    },
    DistributionSpec {
        id: "yssbi.distribution.binomial.sample",
        category: DISCRETE_CATEGORY,
        parameters: &[
            setting("trial_count", I),
            setting("probability", F),
            setting("sample_count", I),
        ],
    },
    DistributionSpec {
        id: "yssbi.distribution.poisson.sample",
        category: DISCRETE_CATEGORY,
        parameters: &[setting("rate", F), setting("sample_count", I)],
    },
    DistributionSpec {
        id: "yssbi.distribution.geometric.sample",
        category: DISCRETE_CATEGORY,
        parameters: &[setting("probability", F), setting("sample_count", I)],
    },
    DistributionSpec {
        id: "yssbi.distribution.negative_binomial.sample",
        category: DISCRETE_CATEGORY,
        parameters: &[
            setting("success_count", F),
            setting("probability", F),
            setting("sample_count", I),
        ],
    },
    DistributionSpec {
        id: "yssbi.distribution.discrete_uniform.sample",
        category: DISCRETE_CATEGORY,
        parameters: &[
            setting("lower_bound", I),
            setting("upper_bound", I),
            setting("sample_count", I),
        ],
    },
    DistributionSpec {
        id: "yssbi.distribution.hypergeometric.sample",
        category: DISCRETE_CATEGORY,
        parameters: &[
            setting("population_size", I),
            setting("success_population", I),
            setting("draw_count", I),
            setting("sample_count", I),
        ],
    },
];

pub(crate) fn build_provider_fragment() -> Result<ProviderFragment, BuiltinAssemblyError> {
    let mut nodes = Vec::with_capacity(SPECS.len());
    let types = Vec::new();
    let categories = vec![
        CategoryRegistration {
            id: category_id(CATEGORY)?,
            title_key: i18n_key("categories.distribution.title")?,
            parent: None,
            order: 60,
        },
        CategoryRegistration {
            id: category_id(CONTINUOUS_CATEGORY)?,
            title_key: i18n_key("categories.distribution.continuous.title")?,
            parent: Some(category_id(CATEGORY)?),
            order: 61,
        },
        CategoryRegistration {
            id: category_id(DISCRETE_CATEGORY)?,
            title_key: i18n_key("categories.distribution.discrete.title")?,
            parent: Some(category_id(CATEGORY)?),
            order: 62,
        },
    ];
    for spec in SPECS {
        nodes.push(leaf(protocol(spec)?, spec.id));
    }
    Ok(ProviderFragment {
        types,
        categories,
        nodes,
        ..ProviderFragment::default()
    })
}

fn protocol(spec: &DistributionSpec) -> Result<NodeProtocol, BuiltinAssemblyError> {
    let mut parameters = spec
        .parameters
        .iter()
        .map(|field| parameter(spec, field))
        .collect::<Result<Vec<_>, BuiltinAssemblyError>>()?;
    let ports = vec![data_port(
        "samples",
        "Samples",
        PortDirection::Output,
        data_series_type(concrete("core.numeric")?),
        PortCardinality::Declared,
        LiteralPolicy::Forbidden,
    )?];
    Ok(NodeProtocol {
        type_id: node_type_id(spec.id)?,
        catalog: NodeCatalogProtocol {
            title_key: node_key(spec.id, "title")?,
            aliases_key: Some(node_key(spec.id, "aliases")?),
            category_id: category_id(spec.category)?,
            icon_id: icon_id("builtin.distribution")?,
            style_id: style_id("builtin.value")?,
            hidden: false,
        },
        interface: assembled_interface(spec.id, ports, vec![], vec![])?,
        parameters: {
            let sampling = parameters.remove(
                parameters
                    .iter()
                    .position(|parameter| parameter.key.as_str() == "sample_count")
                    .expect("sampling count"),
            );
            Parameters::new([
                ParameterGroup::new("distribution", parameters),
                ParameterGroup::new("sampling", [sampling]),
            ])
            .map_err(|source| BuiltinAssemblyError::InvalidParameterSchema {
                node_type: spec.id.into(),
                source,
            })?
        },
        instance_display: NodeInstanceDisplaySpec::Static,
        execution: ExecutionSemantics {
            determinism: Determinism::NonDeterministic,
        },
        typing: NodeTypingSpec::Fixed,
        scope: NodeScope::Any,
        managed_role: None,
    })
}

fn parameter(
    spec: &DistributionSpec,
    field: &DistributionParameter,
) -> Result<Parameter, BuiltinAssemblyError> {
    let parameter = Parameter::number(field.key).title(field_key(spec.id, field.key)?);
    Ok(match field.value_type {
        NumericRepresentation::Int64 => {
            let parameter = parameter.int();
            if field.key == "sample_count" {
                parameter.positive().min(1).default(100)
            } else {
                parameter.default(match field.key {
                    "population_size" | "trial_count" => 10,
                    "success_population" => 5,
                    "lower_bound" => 0,
                    _ => 1,
                })
            }
        }
        NumericRepresentation::Float64 => {
            let parameter = parameter.float();
            let parameter = if matches!(
                field.key,
                "standard_deviation"
                    | "scale"
                    | "shape"
                    | "alpha"
                    | "beta"
                    | "sigma"
                    | "degrees_of_freedom"
                    | "numerator_degrees_of_freedom"
                    | "denominator_degrees_of_freedom"
            ) {
                parameter.positive()
            } else {
                parameter
            };
            parameter.default(match field.key {
                "mean" | "location" | "mu" | "minimum" | "lower_bound" => 0.0,
                "probability" | "mode" => 0.5,
                _ => 1.0,
            })
        }
    })
}

fn field_key(node: &'static str, field: &'static str) -> Result<I18nKey, BuiltinAssemblyError> {
    i18n_key(format!("nodes.{node}.parameters.{field}.title"))
}

fn style_id(value: &'static str) -> Result<NodeStyleId, BuiltinAssemblyError> {
    sid(value, NodeStyleId::new)
}
use crate::builtin::iid as i18n_key;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sampling_groups_have_valid_flat_defaults() {
        for spec in SPECS {
            let protocol = protocol(spec).unwrap();
            assert_eq!(protocol.interface.ports.len(), 1, "{}", spec.id);
            assert_eq!(protocol.interface.ports[0].direction, PortDirection::Output);
            assert_eq!(protocol.interface.ports[0].key.as_str(), "samples");
            assert_eq!(protocol.parameters.groups.len(), 2);
            assert_eq!(protocol.parameters.iter().count(), spec.parameters.len());
            assert!(
                protocol
                    .parameters
                    .iter()
                    .all(|field| matches!(field.editor, ParameterEditorSpec::Number))
            );
            let mut defaults: ParameterValues = protocol
                .parameters
                .iter()
                .map(|parameter| (parameter.key.clone(), parameter.default_json().unwrap()))
                .collect();
            assert!(
                validate_and_prepare_parameter_values::<()>(&protocol, &defaults, |_, _| None)
                    .issues
                    .is_empty()
            );
            let count = ParameterKey::new("sample_count").unwrap();
            assert_eq!(defaults[&count], 100);
            defaults.insert(count, 0.into());
            assert!(
                !validate_and_prepare_parameter_values::<()>(&protocol, &defaults, |_, _| None)
                    .issues
                    .is_empty()
            );
        }
    }
}
