use super::builtin::{BuiltinAssemblyError, ProviderFragment, assembled_interface, leaf, sid};
use crate::Message;
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
    title: &'static str,
    value_type: NumericRepresentation,
}

#[derive(Clone, Copy)]
struct DistributionSpec {
    id: &'static str,
    category: &'static str,
    en: &'static str,
    zh: &'static str,
    aliases: &'static [&'static str],
    zh_aliases: &'static [&'static str],
    parameters: &'static [DistributionParameter],
}

const F: NumericRepresentation = NumericRepresentation::Float64;
const I: NumericRepresentation = NumericRepresentation::Int64;
const fn setting(
    key: &'static str,
    title: &'static str,
    value_type: NumericRepresentation,
) -> DistributionParameter {
    DistributionParameter {
        key,
        title,
        value_type,
    }
}

const SPECS: &[DistributionSpec] = &[
    DistributionSpec {
        id: "yssbi.distribution.normal.sample",
        category: CONTINUOUS_CATEGORY,
        en: "Normal Samples",
        zh: "正态分布采样",
        aliases: &["normal distribution", "Gaussian", "random normal"],
        zh_aliases: &["正态分布", "高斯分布", "随机采样"],
        parameters: &[
            setting("mean", "Mean", F),
            setting("standard_deviation", "Standard Deviation", F),
            setting("sample_count", "Sample Count", I),
        ],
    },
    DistributionSpec {
        id: "yssbi.distribution.uniform.sample",
        category: CONTINUOUS_CATEGORY,
        en: "Uniform Samples",
        zh: "连续均匀分布采样",
        aliases: &["continuous uniform distribution", "random uniform"],
        zh_aliases: &["连续均匀分布", "均匀采样"],
        parameters: &[
            setting("lower_bound", "Lower Bound", F),
            setting("upper_bound", "Upper Bound", F),
            setting("sample_count", "Sample Count", I),
        ],
    },
    DistributionSpec {
        id: "yssbi.distribution.exponential.sample",
        category: CONTINUOUS_CATEGORY,
        en: "Exponential Samples",
        zh: "指数分布采样",
        aliases: &["exponential distribution", "Exp", "rate"],
        zh_aliases: &["指数分布", "率参数"],
        parameters: &[
            setting("rate", "Rate", F),
            setting("sample_count", "Sample Count", I),
        ],
    },
    DistributionSpec {
        id: "yssbi.distribution.gamma.sample",
        category: CONTINUOUS_CATEGORY,
        en: "Gamma Samples",
        zh: "伽马分布采样",
        aliases: &["gamma distribution", "shape rate"],
        zh_aliases: &["伽马分布", "形状率参数"],
        parameters: &[
            setting("shape", "Shape", F),
            setting("rate", "Rate", F),
            setting("sample_count", "Sample Count", I),
        ],
    },
    DistributionSpec {
        id: "yssbi.distribution.beta.sample",
        category: CONTINUOUS_CATEGORY,
        en: "Beta Samples",
        zh: "贝塔分布采样",
        aliases: &["beta distribution", "alpha beta"],
        zh_aliases: &["贝塔分布", "阿尔法贝塔"],
        parameters: &[
            setting("alpha", "Alpha", F),
            setting("beta", "Beta", F),
            setting("sample_count", "Sample Count", I),
        ],
    },
    DistributionSpec {
        id: "yssbi.distribution.students_t.sample",
        category: CONTINUOUS_CATEGORY,
        en: "Student's t Samples",
        zh: "学生 t 分布采样",
        aliases: &[
            "Student t distribution",
            "t distribution",
            "degrees of freedom",
        ],
        zh_aliases: &["学生t分布", "t分布", "自由度"],
        parameters: &[
            setting("degrees_of_freedom", "Degrees of Freedom", F),
            setting("sample_count", "Sample Count", I),
        ],
    },
    DistributionSpec {
        id: "yssbi.distribution.cauchy.sample",
        category: CONTINUOUS_CATEGORY,
        en: "Cauchy Samples",
        zh: "柯西分布采样",
        aliases: &["Cauchy distribution", "location scale"],
        zh_aliases: &["柯西分布", "位置尺度"],
        parameters: &[
            setting("location", "Location", F),
            setting("scale", "Scale", F),
            setting("sample_count", "Sample Count", I),
        ],
    },
    DistributionSpec {
        id: "yssbi.distribution.chi_squared.sample",
        category: CONTINUOUS_CATEGORY,
        en: "Chi-squared Samples",
        zh: "卡方分布采样",
        aliases: &[
            "chi squared distribution",
            "chi-square",
            "degrees of freedom",
        ],
        zh_aliases: &["卡方分布", "自由度"],
        parameters: &[
            setting("degrees_of_freedom", "Degrees of Freedom", F),
            setting("sample_count", "Sample Count", I),
        ],
    },
    DistributionSpec {
        id: "yssbi.distribution.log_normal.sample",
        category: CONTINUOUS_CATEGORY,
        en: "Log-normal Samples",
        zh: "对数正态分布采样",
        aliases: &["lognormal distribution", "log normal", "mu sigma"],
        zh_aliases: &["对数正态分布", "缪西格玛"],
        parameters: &[
            setting("mu", "Mu", F),
            setting("sigma", "Sigma", F),
            setting("sample_count", "Sample Count", I),
        ],
    },
    DistributionSpec {
        id: "yssbi.distribution.weibull.sample",
        category: CONTINUOUS_CATEGORY,
        en: "Weibull Samples",
        zh: "威布尔分布采样",
        aliases: &["Weibull distribution", "shape scale"],
        zh_aliases: &["威布尔分布", "形状尺度"],
        parameters: &[
            setting("shape", "Shape", F),
            setting("scale", "Scale", F),
            setting("sample_count", "Sample Count", I),
        ],
    },
    DistributionSpec {
        id: "yssbi.distribution.laplace.sample",
        category: CONTINUOUS_CATEGORY,
        en: "Laplace Samples",
        zh: "拉普拉斯分布采样",
        aliases: &["Laplace distribution", "double exponential"],
        zh_aliases: &["拉普拉斯分布", "双指数分布"],
        parameters: &[
            setting("location", "Location", F),
            setting("scale", "Scale", F),
            setting("sample_count", "Sample Count", I),
        ],
    },
    DistributionSpec {
        id: "yssbi.distribution.pareto.sample",
        category: CONTINUOUS_CATEGORY,
        en: "Pareto Samples",
        zh: "帕累托分布采样",
        aliases: &["Pareto distribution", "power law"],
        zh_aliases: &["帕累托分布", "幂律分布"],
        parameters: &[
            setting("shape", "Shape", F),
            setting("scale", "Scale", F),
            setting("sample_count", "Sample Count", I),
        ],
    },
    DistributionSpec {
        id: "yssbi.distribution.inverse_gamma.sample",
        category: CONTINUOUS_CATEGORY,
        en: "Inverse-gamma Samples",
        zh: "逆伽马分布采样",
        aliases: &["inverse gamma distribution", "reciprocal gamma"],
        zh_aliases: &["逆伽马分布", "倒数伽马"],
        parameters: &[
            setting("shape", "Shape", F),
            setting("scale", "Scale", F),
            setting("sample_count", "Sample Count", I),
        ],
    },
    DistributionSpec {
        id: "yssbi.distribution.triangular.sample",
        category: CONTINUOUS_CATEGORY,
        en: "Triangular Samples",
        zh: "三角分布采样",
        aliases: &["triangular distribution", "minimum maximum mode"],
        zh_aliases: &["三角分布", "最小值最大值众数"],
        parameters: &[
            setting("minimum", "Minimum", F),
            setting("maximum", "Maximum", F),
            setting("mode", "Mode", F),
            setting("sample_count", "Sample Count", I),
        ],
    },
    DistributionSpec {
        id: "yssbi.distribution.fisher_snedecor.sample",
        category: CONTINUOUS_CATEGORY,
        en: "F-distribution Samples",
        zh: "F 分布采样",
        aliases: &[
            "Fisher Snedecor distribution",
            "F distribution",
            "degrees of freedom",
        ],
        zh_aliases: &["费舍尔斯内德克分布", "F分布", "自由度"],
        parameters: &[
            setting(
                "numerator_degrees_of_freedom",
                "Numerator Degrees of Freedom",
                F,
            ),
            setting(
                "denominator_degrees_of_freedom",
                "Denominator Degrees of Freedom",
                F,
            ),
            setting("sample_count", "Sample Count", I),
        ],
    },
    DistributionSpec {
        id: "yssbi.distribution.erlang.sample",
        category: CONTINUOUS_CATEGORY,
        en: "Erlang Samples",
        zh: "爱尔朗分布采样",
        aliases: &["Erlang distribution", "integer shape gamma"],
        zh_aliases: &["爱尔朗分布", "整数形状伽马"],
        parameters: &[
            setting("shape", "Integer Shape", I),
            setting("rate", "Rate", F),
            setting("sample_count", "Sample Count", I),
        ],
    },
    DistributionSpec {
        id: "yssbi.distribution.bernoulli.sample",
        category: DISCRETE_CATEGORY,
        en: "Bernoulli Samples",
        zh: "伯努利分布采样",
        aliases: &["Bernoulli distribution", "binary trial", "probability"],
        zh_aliases: &["伯努利分布", "二元试验", "概率"],
        parameters: &[
            setting("probability", "Success Probability", F),
            setting("sample_count", "Sample Count", I),
        ],
    },
    DistributionSpec {
        id: "yssbi.distribution.binomial.sample",
        category: DISCRETE_CATEGORY,
        en: "Binomial Samples",
        zh: "二项分布采样",
        aliases: &["binomial distribution", "trials probability"],
        zh_aliases: &["二项分布", "试验次数概率"],
        parameters: &[
            setting("trial_count", "Trial Count", I),
            setting("probability", "Success Probability", F),
            setting("sample_count", "Sample Count", I),
        ],
    },
    DistributionSpec {
        id: "yssbi.distribution.poisson.sample",
        category: DISCRETE_CATEGORY,
        en: "Poisson Samples",
        zh: "泊松分布采样",
        aliases: &["Poisson distribution", "lambda", "count distribution"],
        zh_aliases: &["泊松分布", "兰布达", "计数分布"],
        parameters: &[
            setting("rate", "Lambda", F),
            setting("sample_count", "Sample Count", I),
        ],
    },
    DistributionSpec {
        id: "yssbi.distribution.geometric.sample",
        category: DISCRETE_CATEGORY,
        en: "Geometric Samples",
        zh: "几何分布采样",
        aliases: &["geometric distribution", "waiting time", "probability"],
        zh_aliases: &["几何分布", "等待次数", "概率"],
        parameters: &[
            setting("probability", "Success Probability", F),
            setting("sample_count", "Sample Count", I),
        ],
    },
    DistributionSpec {
        id: "yssbi.distribution.negative_binomial.sample",
        category: DISCRETE_CATEGORY,
        en: "Negative-binomial Samples",
        zh: "负二项分布采样",
        aliases: &["negative binomial distribution", "Pascal distribution"],
        zh_aliases: &["负二项分布", "帕斯卡分布"],
        parameters: &[
            setting("success_count", "Success Count", F),
            setting("probability", "Success Probability", F),
            setting("sample_count", "Sample Count", I),
        ],
    },
    DistributionSpec {
        id: "yssbi.distribution.discrete_uniform.sample",
        category: DISCRETE_CATEGORY,
        en: "Discrete-uniform Samples",
        zh: "离散均匀分布采样",
        aliases: &["discrete uniform distribution", "random integer"],
        zh_aliases: &["离散均匀分布", "随机整数"],
        parameters: &[
            setting("lower_bound", "Lower Bound", I),
            setting("upper_bound", "Upper Bound", I),
            setting("sample_count", "Sample Count", I),
        ],
    },
    DistributionSpec {
        id: "yssbi.distribution.hypergeometric.sample",
        category: DISCRETE_CATEGORY,
        en: "Hypergeometric Samples",
        zh: "超几何分布采样",
        aliases: &[
            "hypergeometric distribution",
            "sampling without replacement",
        ],
        zh_aliases: &["超几何分布", "不放回抽样"],
        parameters: &[
            setting("population_size", "Population Size", I),
            setting("success_population", "Successes in Population", I),
            setting("draw_count", "Draw Count", I),
            setting("sample_count", "Sample Count", I),
        ],
    },
];

pub(crate) fn build_provider_fragment() -> Result<ProviderFragment, BuiltinAssemblyError> {
    let mut nodes = Vec::with_capacity(SPECS.len());
    let mut messages = vec![
        (
            "en-US",
            "categories.distribution.title",
            Message::Text("Probability Distributions"),
        ),
        (
            "zh-CN",
            "categories.distribution.title",
            Message::Text("概率分布"),
        ),
        (
            "en-US",
            "categories.distribution.continuous.title",
            Message::Text("Continuous Distributions"),
        ),
        (
            "zh-CN",
            "categories.distribution.continuous.title",
            Message::Text("连续分布"),
        ),
        (
            "en-US",
            "categories.distribution.discrete.title",
            Message::Text("Discrete Distributions"),
        ),
        (
            "zh-CN",
            "categories.distribution.discrete.title",
            Message::Text("离散分布"),
        ),
    ];
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
        add_messages(&mut messages, spec);
        for field in spec.parameters {
            let key = Box::leak(field_key(spec.id, field.key)?.to_string().into_boxed_str());
            messages.push(("en-US", key, Message::Text(field.title)));
            messages.push((
                "zh-CN",
                key,
                Message::Text(match field.key {
                    "mean" => "均值",
                    "standard_deviation" => "标准差",
                    "sample_count" => "样本数",
                    "lower_bound" => "下界",
                    "upper_bound" => "上界",
                    "rate" => "率参数",
                    "shape" => "形状参数",
                    "scale" => "尺度参数",
                    "alpha" => "Alpha",
                    "beta" => "Beta",
                    "degrees_of_freedom" => "自由度",
                    "location" => "位置参数",
                    "mu" => "Mu",
                    "sigma" => "Sigma",
                    "minimum" => "最小值",
                    "maximum" => "最大值",
                    "mode" => "众数",
                    "numerator_degrees_of_freedom" => "分子自由度",
                    "denominator_degrees_of_freedom" => "分母自由度",
                    "probability" => "成功概率",
                    "trial_count" => "试验次数",
                    "success_count" => "成功次数",
                    "population_size" => "总体大小",
                    "success_population" => "总体成功数",
                    "draw_count" => "抽取次数",
                    _ => field.title,
                }),
            ));
        }
        nodes.push(leaf(protocol(spec)?, spec.id));
    }
    Ok(ProviderFragment {
        types,
        categories,
        nodes,
        messages,
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
        data_series_type(concrete("core.numeric")?),
    )?];
    Ok(NodeProtocol {
        type_id: node_id(spec.id)?,
        catalog: NodeCatalogProtocol {
            title_key: node_key(spec.id, "title")?,
            documentation_key: Some(node_key(spec.id, "documentation")?),
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
            cache: CachePolicy::Disabled,
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
    i18n_key(Box::leak(
        format!("nodes.{node}.parameters.{field}.title").into_boxed_str(),
    ))
}

fn data_port(
    key: &'static str,
    title: &'static str,
    value_type: TypeExpr,
) -> Result<PortSpec, BuiltinAssemblyError> {
    Ok(PortSpec {
        key: port_key(key)?,
        title: title.into(),
        direction: PortDirection::Output,
        value_type,
        cardinality: PortCardinality::Declared,
        connections: ConnectionsPerPort::Multiple {
            max: None,
            ordered: false,
        },
        input_binding: None,
        consumption: None,
        production: Some(OutputProduction::FullyMaterialized),
        editor: PortEditorSpec::Default,
        schema: None,
    })
}

fn add_messages(out: &mut Vec<(&'static str, &'static str, Message)>, spec: &DistributionSpec) {
    let title = key_text(spec.id, "title");
    let documentation = key_text(spec.id, "documentation");
    let aliases = key_text(spec.id, "aliases");
    out.extend([
        ("en-US", title, Message::Text(spec.en)),
        ("zh-CN", title, Message::Text(spec.zh)),
        ("en-US", documentation, Message::Text("Distribution parameters and sample count are edited in the Distribution and Sampling groups in Detail. Sample count must be a positive integer.")),
        ("zh-CN", documentation, Message::Text("在 Detail 的分布参数和采样设置两组中设置分布参数和样本数；样本数必须为正整数。")),
        ("en-US", aliases, Message::Aliases(spec.aliases)),
        ("zh-CN", aliases, Message::Aliases(spec.zh_aliases)),
    ]);
}

fn concrete(value: &'static str) -> Result<TypeExpr, BuiltinAssemblyError> {
    Ok(TypeExpr::Concrete(type_id(value)?))
}
fn node_id(value: &'static str) -> Result<NodeTypeId, BuiltinAssemblyError> {
    sid(value, NodeTypeId::new)
}
fn type_id(value: &'static str) -> Result<TypeId, BuiltinAssemblyError> {
    sid(value, TypeId::new)
}
fn port_key(value: &'static str) -> Result<PortKey, BuiltinAssemblyError> {
    sid(value, PortKey::new)
}
fn category_id(value: &'static str) -> Result<NodeCategoryId, BuiltinAssemblyError> {
    sid(value, NodeCategoryId::new)
}
fn icon_id(value: &'static str) -> Result<IconId, BuiltinAssemblyError> {
    sid(value, IconId::new)
}
fn style_id(value: &'static str) -> Result<NodeStyleId, BuiltinAssemblyError> {
    sid(value, NodeStyleId::new)
}
fn i18n_key(value: &'static str) -> Result<I18nKey, BuiltinAssemblyError> {
    sid(value, I18nKey::new)
}
fn node_key(id: &'static str, suffix: &'static str) -> Result<I18nKey, BuiltinAssemblyError> {
    i18n_key(key_text(id, suffix))
}
fn key_text(id: &'static str, suffix: &'static str) -> &'static str {
    Box::leak(format!("nodes.{id}.{suffix}").into_boxed_str())
}

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
