use super::builtin::ProviderFragment;
use super::localization::Message;
use crate::node_system::compiler::{
    FragmentMetadata, FragmentResult, KernelFragment as LoweredKernelFragment, LoweredKernel,
    LoweredNode, LoweringContext, LoweringError, NodeImplementation, NodeLowerer,
};
use crate::node_system::document::PortRef;
use crate::node_system::plan::{CompiledParameterHandle, KernelHandle};
use crate::node_system::protocol::*;
use crate::node_system::registry::{CategoryRegistration, RegisteredNode, TypeRegistration};
use std::collections::BTreeSet;
use std::sync::Arc;

const CATEGORY: &str = "distribution";
const FLOAT_SERIES: &str = "core.data_series.float64";
const INTEGER_SERIES: &str = "core.data_series.int64";

#[derive(Clone, Copy)]
enum ScalarType {
    Float64,
    Int64,
}

impl ScalarType {
    fn type_id(self) -> &'static str {
        match self {
            Self::Float64 => "core.float64",
            Self::Int64 => "core.int64",
        }
    }
}

#[derive(Clone, Copy)]
struct InputSpec {
    key: &'static str,
    en: &'static str,
    zh: &'static str,
    value_type: ScalarType,
}

#[derive(Clone, Copy)]
struct DistributionSpec {
    #[allow(dead_code)]
    legacy_name: &'static str,
    id: &'static str,
    kernel: &'static str,
    en: &'static str,
    zh: &'static str,
    aliases: &'static [&'static str],
    zh_aliases: &'static [&'static str],
    inputs: &'static [InputSpec],
    output: ScalarType,
}

const F: ScalarType = ScalarType::Float64;
const I: ScalarType = ScalarType::Int64;
const fn input(
    key: &'static str,
    en: &'static str,
    zh: &'static str,
    value_type: ScalarType,
) -> InputSpec {
    InputSpec {
        key,
        en,
        zh,
        value_type,
    }
}

const SPECS: &[DistributionSpec] = &[
    DistributionSpec {
        legacy_name: "Normal",
        id: "yssbi.distribution.normal.sample",
        kernel: "yssbi.distribution.normal.sample",
        en: "Normal Samples",
        zh: "正态分布采样",
        aliases: &["normal distribution", "Gaussian", "random normal"],
        zh_aliases: &["正态分布", "高斯分布", "随机采样"],
        inputs: &[
            input("mean", "Mean", "均值", F),
            input("standard_deviation", "Standard Deviation", "标准差", F),
            input("sample_count", "Sample Count", "样本数", I),
        ],
        output: F,
    },
    DistributionSpec {
        legacy_name: "Uniform",
        id: "yssbi.distribution.uniform.sample",
        kernel: "yssbi.distribution.uniform.sample",
        en: "Uniform Samples",
        zh: "连续均匀分布采样",
        aliases: &["continuous uniform distribution", "random uniform"],
        zh_aliases: &["连续均匀分布", "均匀采样"],
        inputs: &[
            input("lower_bound", "Lower Bound", "下界", F),
            input("upper_bound", "Upper Bound", "上界", F),
            input("sample_count", "Sample Count", "样本数", I),
        ],
        output: F,
    },
    DistributionSpec {
        legacy_name: "Exponential",
        id: "yssbi.distribution.exponential.sample",
        kernel: "yssbi.distribution.exponential.sample",
        en: "Exponential Samples",
        zh: "指数分布采样",
        aliases: &["exponential distribution", "Exp", "rate"],
        zh_aliases: &["指数分布", "率参数"],
        inputs: &[
            input("rate", "Rate", "率参数", F),
            input("sample_count", "Sample Count", "样本数", I),
        ],
        output: F,
    },
    DistributionSpec {
        legacy_name: "Gamma",
        id: "yssbi.distribution.gamma.sample",
        kernel: "yssbi.distribution.gamma.sample",
        en: "Gamma Samples",
        zh: "伽马分布采样",
        aliases: &["gamma distribution", "shape rate"],
        zh_aliases: &["伽马分布", "形状率参数"],
        inputs: &[
            input("shape", "Shape", "形状参数", F),
            input("rate", "Rate", "率参数", F),
            input("sample_count", "Sample Count", "样本数", I),
        ],
        output: F,
    },
    DistributionSpec {
        legacy_name: "Beta",
        id: "yssbi.distribution.beta.sample",
        kernel: "yssbi.distribution.beta.sample",
        en: "Beta Samples",
        zh: "贝塔分布采样",
        aliases: &["beta distribution", "alpha beta"],
        zh_aliases: &["贝塔分布", "阿尔法贝塔"],
        inputs: &[
            input("alpha", "Alpha", "Alpha", F),
            input("beta", "Beta", "Beta", F),
            input("sample_count", "Sample Count", "样本数", I),
        ],
        output: F,
    },
    DistributionSpec {
        legacy_name: "StudentsT",
        id: "yssbi.distribution.students_t.sample",
        kernel: "yssbi.distribution.students_t.sample",
        en: "Student's t Samples",
        zh: "学生 t 分布采样",
        aliases: &[
            "Student t distribution",
            "t distribution",
            "degrees of freedom",
        ],
        zh_aliases: &["学生t分布", "t分布", "自由度"],
        inputs: &[
            input("degrees_of_freedom", "Degrees of Freedom", "自由度", F),
            input("sample_count", "Sample Count", "样本数", I),
        ],
        output: F,
    },
    DistributionSpec {
        legacy_name: "Cauchy",
        id: "yssbi.distribution.cauchy.sample",
        kernel: "yssbi.distribution.cauchy.sample",
        en: "Cauchy Samples",
        zh: "柯西分布采样",
        aliases: &["Cauchy distribution", "location scale"],
        zh_aliases: &["柯西分布", "位置尺度"],
        inputs: &[
            input("location", "Location", "位置参数", F),
            input("scale", "Scale", "尺度参数", F),
            input("sample_count", "Sample Count", "样本数", I),
        ],
        output: F,
    },
    DistributionSpec {
        legacy_name: "ChiSquared",
        id: "yssbi.distribution.chi_squared.sample",
        kernel: "yssbi.distribution.chi_squared.sample",
        en: "Chi-squared Samples",
        zh: "卡方分布采样",
        aliases: &[
            "chi squared distribution",
            "chi-square",
            "degrees of freedom",
        ],
        zh_aliases: &["卡方分布", "自由度"],
        inputs: &[
            input("degrees_of_freedom", "Degrees of Freedom", "自由度", F),
            input("sample_count", "Sample Count", "样本数", I),
        ],
        output: F,
    },
    DistributionSpec {
        legacy_name: "LogNormal",
        id: "yssbi.distribution.log_normal.sample",
        kernel: "yssbi.distribution.log_normal.sample",
        en: "Log-normal Samples",
        zh: "对数正态分布采样",
        aliases: &["lognormal distribution", "log normal", "mu sigma"],
        zh_aliases: &["对数正态分布", "缪西格玛"],
        inputs: &[
            input("mu", "Mu", "Mu", F),
            input("sigma", "Sigma", "Sigma", F),
            input("sample_count", "Sample Count", "样本数", I),
        ],
        output: F,
    },
    DistributionSpec {
        legacy_name: "Weibull",
        id: "yssbi.distribution.weibull.sample",
        kernel: "yssbi.distribution.weibull.sample",
        en: "Weibull Samples",
        zh: "威布尔分布采样",
        aliases: &["Weibull distribution", "shape scale"],
        zh_aliases: &["威布尔分布", "形状尺度"],
        inputs: &[
            input("shape", "Shape", "形状参数", F),
            input("scale", "Scale", "尺度参数", F),
            input("sample_count", "Sample Count", "样本数", I),
        ],
        output: F,
    },
    DistributionSpec {
        legacy_name: "Laplace",
        id: "yssbi.distribution.laplace.sample",
        kernel: "yssbi.distribution.laplace.sample",
        en: "Laplace Samples",
        zh: "拉普拉斯分布采样",
        aliases: &["Laplace distribution", "double exponential"],
        zh_aliases: &["拉普拉斯分布", "双指数分布"],
        inputs: &[
            input("location", "Location", "位置参数", F),
            input("scale", "Scale", "尺度参数", F),
            input("sample_count", "Sample Count", "样本数", I),
        ],
        output: F,
    },
    DistributionSpec {
        legacy_name: "Pareto",
        id: "yssbi.distribution.pareto.sample",
        kernel: "yssbi.distribution.pareto.sample",
        en: "Pareto Samples",
        zh: "帕累托分布采样",
        aliases: &["Pareto distribution", "power law"],
        zh_aliases: &["帕累托分布", "幂律分布"],
        inputs: &[
            input("shape", "Shape", "形状参数", F),
            input("scale", "Scale", "尺度参数", F),
            input("sample_count", "Sample Count", "样本数", I),
        ],
        output: F,
    },
    DistributionSpec {
        legacy_name: "InverseGamma",
        id: "yssbi.distribution.inverse_gamma.sample",
        kernel: "yssbi.distribution.inverse_gamma.sample",
        en: "Inverse-gamma Samples",
        zh: "逆伽马分布采样",
        aliases: &["inverse gamma distribution", "reciprocal gamma"],
        zh_aliases: &["逆伽马分布", "倒数伽马"],
        inputs: &[
            input("shape", "Shape", "形状参数", F),
            input("scale", "Scale", "尺度参数", F),
            input("sample_count", "Sample Count", "样本数", I),
        ],
        output: F,
    },
    DistributionSpec {
        legacy_name: "Triangular",
        id: "yssbi.distribution.triangular.sample",
        kernel: "yssbi.distribution.triangular.sample",
        en: "Triangular Samples",
        zh: "三角分布采样",
        aliases: &["triangular distribution", "minimum maximum mode"],
        zh_aliases: &["三角分布", "最小值最大值众数"],
        inputs: &[
            input("minimum", "Minimum", "最小值", F),
            input("maximum", "Maximum", "最大值", F),
            input("mode", "Mode", "众数", F),
            input("sample_count", "Sample Count", "样本数", I),
        ],
        output: F,
    },
    DistributionSpec {
        legacy_name: "FisherSnedecor",
        id: "yssbi.distribution.fisher_snedecor.sample",
        kernel: "yssbi.distribution.fisher_snedecor.sample",
        en: "F-distribution Samples",
        zh: "F 分布采样",
        aliases: &[
            "Fisher Snedecor distribution",
            "F distribution",
            "degrees of freedom",
        ],
        zh_aliases: &["费舍尔斯内德克分布", "F分布", "自由度"],
        inputs: &[
            input(
                "numerator_degrees_of_freedom",
                "Numerator Degrees of Freedom",
                "分子自由度",
                F,
            ),
            input(
                "denominator_degrees_of_freedom",
                "Denominator Degrees of Freedom",
                "分母自由度",
                F,
            ),
            input("sample_count", "Sample Count", "样本数", I),
        ],
        output: F,
    },
    DistributionSpec {
        legacy_name: "Erlang",
        id: "yssbi.distribution.erlang.sample",
        kernel: "yssbi.distribution.erlang.sample",
        en: "Erlang Samples",
        zh: "爱尔朗分布采样",
        aliases: &["Erlang distribution", "integer shape gamma"],
        zh_aliases: &["爱尔朗分布", "整数形状伽马"],
        inputs: &[
            input("shape", "Integer Shape", "整数形状参数", I),
            input("rate", "Rate", "率参数", F),
            input("sample_count", "Sample Count", "样本数", I),
        ],
        output: F,
    },
    DistributionSpec {
        legacy_name: "Bernoulli",
        id: "yssbi.distribution.bernoulli.sample",
        kernel: "yssbi.distribution.bernoulli.sample",
        en: "Bernoulli Samples",
        zh: "伯努利分布采样",
        aliases: &["Bernoulli distribution", "binary trial", "probability"],
        zh_aliases: &["伯努利分布", "二元试验", "概率"],
        inputs: &[
            input("probability", "Success Probability", "成功概率", F),
            input("sample_count", "Sample Count", "样本数", I),
        ],
        output: I,
    },
    DistributionSpec {
        legacy_name: "Binomial",
        id: "yssbi.distribution.binomial.sample",
        kernel: "yssbi.distribution.binomial.sample",
        en: "Binomial Samples",
        zh: "二项分布采样",
        aliases: &["binomial distribution", "trials probability"],
        zh_aliases: &["二项分布", "试验次数概率"],
        inputs: &[
            input("trial_count", "Trial Count", "试验次数", I),
            input("probability", "Success Probability", "成功概率", F),
            input("sample_count", "Sample Count", "样本数", I),
        ],
        output: I,
    },
    DistributionSpec {
        legacy_name: "Poisson",
        id: "yssbi.distribution.poisson.sample",
        kernel: "yssbi.distribution.poisson.sample",
        en: "Poisson Samples",
        zh: "泊松分布采样",
        aliases: &["Poisson distribution", "lambda", "count distribution"],
        zh_aliases: &["泊松分布", "兰布达", "计数分布"],
        inputs: &[
            input("rate", "Lambda", "Lambda", F),
            input("sample_count", "Sample Count", "样本数", I),
        ],
        output: I,
    },
    DistributionSpec {
        legacy_name: "Geometric",
        id: "yssbi.distribution.geometric.sample",
        kernel: "yssbi.distribution.geometric.sample",
        en: "Geometric Samples",
        zh: "几何分布采样",
        aliases: &["geometric distribution", "waiting time", "probability"],
        zh_aliases: &["几何分布", "等待次数", "概率"],
        inputs: &[
            input("probability", "Success Probability", "成功概率", F),
            input("sample_count", "Sample Count", "样本数", I),
        ],
        output: I,
    },
    DistributionSpec {
        legacy_name: "NegativeBinomial",
        id: "yssbi.distribution.negative_binomial.sample",
        kernel: "yssbi.distribution.negative_binomial.sample",
        en: "Negative-binomial Samples",
        zh: "负二项分布采样",
        aliases: &["negative binomial distribution", "Pascal distribution"],
        zh_aliases: &["负二项分布", "帕斯卡分布"],
        inputs: &[
            input("success_count", "Success Count", "成功次数", F),
            input("probability", "Success Probability", "成功概率", F),
            input("sample_count", "Sample Count", "样本数", I),
        ],
        output: I,
    },
    DistributionSpec {
        legacy_name: "DiscreteUniform",
        id: "yssbi.distribution.discrete_uniform.sample",
        kernel: "yssbi.distribution.discrete_uniform.sample",
        en: "Discrete-uniform Samples",
        zh: "离散均匀分布采样",
        aliases: &["discrete uniform distribution", "random integer"],
        zh_aliases: &["离散均匀分布", "随机整数"],
        inputs: &[
            input("lower_bound", "Lower Bound", "下界", I),
            input("upper_bound", "Upper Bound", "上界", I),
            input("sample_count", "Sample Count", "样本数", I),
        ],
        output: I,
    },
    DistributionSpec {
        legacy_name: "Hypergeometric",
        id: "yssbi.distribution.hypergeometric.sample",
        kernel: "yssbi.distribution.hypergeometric.sample",
        en: "Hypergeometric Samples",
        zh: "超几何分布采样",
        aliases: &[
            "hypergeometric distribution",
            "sampling without replacement",
        ],
        zh_aliases: &["超几何分布", "不放回抽样"],
        inputs: &[
            input("population_size", "Population Size", "总体大小", I),
            input(
                "success_population",
                "Successes in Population",
                "总体成功数",
                I,
            ),
            input("draw_count", "Draw Count", "抽取数", I),
            input("sample_count", "Sample Count", "样本数", I),
        ],
        output: I,
    },
];

#[cfg(test)]
pub(crate) fn legacy_manifest() -> impl Iterator<Item = (&'static str, &'static str)> {
    SPECS.iter().map(|spec| (spec.legacy_name, spec.id))
}

pub(crate) fn build_provider_fragment() -> ProviderFragment {
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
            "types.data_series.float64.title",
            Message::Text("Float64 Data Series"),
        ),
        (
            "zh-CN",
            "types.data_series.float64.title",
            Message::Text("Float64 数据序列"),
        ),
        (
            "en-US",
            "types.data_series.int64.title",
            Message::Text("Int64 Data Series"),
        ),
        (
            "zh-CN",
            "types.data_series.int64.title",
            Message::Text("Int64 数据序列"),
        ),
    ];
    let types = vec![
        TypeRegistration {
            id: type_id(FLOAT_SERIES),
            title_key: i18n_key("types.data_series.float64.title"),
            classes: BTreeSet::new(),
        },
        TypeRegistration {
            id: type_id(INTEGER_SERIES),
            title_key: i18n_key("types.data_series.int64.title"),
            classes: BTreeSet::new(),
        },
    ];
    let categories = vec![CategoryRegistration {
        id: category_id(CATEGORY),
        title_key: i18n_key("categories.distribution.title"),
        parent: None,
        order: 60,
    }];
    for spec in SPECS {
        add_messages(&mut messages, spec);
        nodes.push(RegisteredNode::leaf(
            Arc::new(protocol(spec)),
            Arc::new(NodeImplementation::new(DistributionLowerer {
                kernel: spec.kernel,
            })),
        ));
    }
    ProviderFragment {
        types,
        categories,
        nodes,
        messages,
        ..ProviderFragment::default()
    }
}

fn protocol(spec: &DistributionSpec) -> NodeProtocol {
    let mut ports = spec
        .inputs
        .iter()
        .map(|input| {
            data_port(
                spec.id,
                input.key,
                PortDirection::Input,
                concrete(input.value_type.type_id()),
            )
        })
        .collect::<Vec<_>>();
    let output_type = match spec.output {
        ScalarType::Float64 => FLOAT_SERIES,
        ScalarType::Int64 => INTEGER_SERIES,
    };
    ports.push(data_port(
        spec.id,
        "samples",
        PortDirection::Output,
        concrete(output_type),
    ));
    NodeProtocol {
        type_id: node_id(spec.id),
        catalog: NodeCatalogProtocol {
            title_key: node_key(spec.id, "title"),
            description_key: Some(node_key(spec.id, "description")),
            documentation_key: Some(node_key(spec.id, "documentation")),
            aliases_key: Some(node_key(spec.id, "aliases")),
            category_id: category_id(CATEGORY),
            icon_id: icon_id("builtin.distribution"),
            style_id: style_id("builtin.value"),
            hidden: false,
        },
        interface: NodeInterfaceProtocol::new(ports, vec![], vec![])
            .expect("distribution protocol interface"),
        parameters: ParameterSchema::new(vec![]).expect("empty distribution parameters"),
        execution: ExecutionSemantics {
            determinism: Determinism::NonDeterministic,
            purity: Purity::Pure,
            evaluation: EvaluationPolicy::DemandDriven,
            cache: CachePolicy::None,
            effects: EffectSemantics::None,
        },
        scope: NodeScope::Any,
        managed_role: None,
    }
}

fn data_port(
    id: &'static str,
    key: &'static str,
    direction: PortDirection,
    value_type: TypeExpr,
) -> PortSpec {
    PortSpec {
        key: port_key(key),
        label_key: node_port_key(id, key),
        direction,
        kind: PortKind::Data,
        value_type,
        instances: PortInstances::Declared,
        connections: if direction == PortDirection::Input {
            ConnectionsPerPort::Single
        } else {
            ConnectionsPerPort::Multiple {
                max: None,
                ordered: false,
            }
        },
        input_binding: (direction == PortDirection::Input).then_some(InputBindingSpec {
            literal_policy: LiteralPolicy::Allowed,
            default_value: None,
        }),
        consumption: (direction == PortDirection::Input)
            .then_some(InputConsumption::FullyMaterialized),
        production: (direction == PortDirection::Output)
            .then_some(OutputProduction::FullyMaterialized),
        editor: PortEditorSpec::Default,
        schema: None,
    }
}

struct DistributionLowerer {
    kernel: &'static str,
}

impl NodeLowerer for DistributionLowerer {
    fn lower(&self, context: &LoweringContext<'_>) -> Result<LoweredNode, LoweringError> {
        let output = context
            .outputs
            .iter()
            .find(|(address, _)| matches!(&address.port, PortRef::Declared { key } if key.as_str() == "samples"))
            .map(|(address, _)| address.clone())
            .ok_or_else(|| LoweringError::new("distribution output 'samples' was not materialized"))?;
        Ok(LoweredNode {
            kernel: LoweredKernel::Kernel(LoweredKernelFragment {
                kernel: KernelHandle::new(self.kernel)
                    .map_err(|error| LoweringError::new(error.to_string()))?,
                metadata: FragmentMetadata {
                    effect: EffectSemantics::None,
                    resources: Box::new([]),
                    results: vec![FragmentResult {
                        name: "samples".into(),
                        output,
                    }]
                    .into_boxed_slice(),
                },
            }),
            parameters: CompiledParameterHandle::new(format!("node.{}", context.node_id))
                .map_err(|error| LoweringError::new(error.to_string()))?,
        })
    }
}

fn add_messages(out: &mut Vec<(&'static str, &'static str, Message)>, spec: &DistributionSpec) {
    let title = key_text(spec.id, "title");
    let description = key_text(spec.id, "description");
    let documentation = key_text(spec.id, "documentation");
    let aliases = key_text(spec.id, "aliases");
    out.extend([
        ("en-US", title, Message::Text(spec.en)),
        ("zh-CN", title, Message::Text(spec.zh)),
        ("en-US", description, Message::Text("Draws an in-memory data series from the selected probability distribution.")),
        ("zh-CN", description, Message::Text("从所选概率分布生成内存数据序列。")),
        ("en-US", documentation, Message::Text("Parameters use the conventional statistical parameterization shown by the port names. Sample count must be non-negative.")),
        ("zh-CN", documentation, Message::Text("参数采用端口名称所示的标准统计参数化；样本数必须为非负整数。")),
        ("en-US", aliases, Message::Aliases(spec.aliases)),
        ("zh-CN", aliases, Message::Aliases(spec.zh_aliases)),
    ]);
    for input in spec.inputs {
        let key = port_label_text(spec.id, input.key);
        out.push(("en-US", key, Message::Text(input.en)));
        out.push(("zh-CN", key, Message::Text(input.zh)));
    }
    let samples = port_label_text(spec.id, "samples");
    out.push(("en-US", samples, Message::Text("Samples")));
    out.push(("zh-CN", samples, Message::Text("样本")));
}

fn concrete(value: &'static str) -> TypeExpr {
    TypeExpr::Concrete(type_id(value))
}
fn node_id(value: &'static str) -> NodeTypeId {
    NodeTypeId::new(value).expect("distribution node id")
}
fn type_id(value: &'static str) -> TypeId {
    TypeId::new(value).expect("distribution type id")
}
fn port_key(value: &'static str) -> PortKey {
    PortKey::new(value).expect("distribution port key")
}
fn category_id(value: &'static str) -> NodeCategoryId {
    NodeCategoryId::new(value).expect("distribution category id")
}
fn icon_id(value: &'static str) -> IconId {
    IconId::new(value).expect("distribution icon id")
}
fn style_id(value: &'static str) -> NodeStyleId {
    NodeStyleId::new(value).expect("distribution style id")
}
fn i18n_key(value: &'static str) -> I18nKey {
    I18nKey::new(value).expect("distribution i18n key")
}
fn node_key(id: &'static str, suffix: &'static str) -> I18nKey {
    i18n_key(key_text(id, suffix))
}
fn node_port_key(id: &'static str, port: &'static str) -> I18nKey {
    I18nKey::new(port_label_text(id, port)).expect("distribution port i18n key")
}
fn key_text(id: &'static str, suffix: &'static str) -> &'static str {
    Box::leak(format!("nodes.{id}.{suffix}").into_boxed_str())
}
fn port_label_text(id: &'static str, port: &'static str) -> &'static str {
    Box::leak(format!("nodes.{id}.ports.{port}.label").into_boxed_str())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    const LEGACY_NODES: &[&str] = &[
        "Normal",
        "Uniform",
        "Exponential",
        "Gamma",
        "Beta",
        "StudentsT",
        "Cauchy",
        "ChiSquared",
        "LogNormal",
        "Weibull",
        "Laplace",
        "Pareto",
        "InverseGamma",
        "Triangular",
        "FisherSnedecor",
        "Erlang",
        "Bernoulli",
        "Binomial",
        "Poisson",
        "Geometric",
        "NegativeBinomial",
        "DiscreteUniform",
        "Hypergeometric",
    ];

    #[test]
    fn migration_covers_every_legacy_distribution_node_once() {
        assert_eq!(SPECS.len(), LEGACY_NODES.len());
        let migrated = SPECS
            .iter()
            .map(|spec| spec.legacy_name)
            .collect::<BTreeSet<_>>();
        assert_eq!(migrated, LEGACY_NODES.iter().copied().collect());
        assert_eq!(
            SPECS
                .iter()
                .map(|spec| spec.id)
                .collect::<BTreeSet<_>>()
                .len(),
            SPECS.len()
        );
        assert!(
            SPECS
                .iter()
                .all(|spec| spec.id.starts_with("yssbi.distribution.")
                    && spec.id.ends_with(".sample"))
        );
    }

    #[test]
    fn protocols_use_semantic_unique_port_keys() {
        for spec in SPECS {
            let keys = spec.inputs.iter().map(|input| input.key).chain(["samples"]);
            let keys = keys.collect::<BTreeSet<_>>();
            assert_eq!(keys.len(), spec.inputs.len() + 1, "{}", spec.id);
            assert!(
                !keys
                    .iter()
                    .any(|key| key.starts_with("input") || key.chars().all(char::is_numeric))
            );
        }
    }
}
