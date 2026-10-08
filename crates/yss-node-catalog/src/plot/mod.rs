use super::builtin::{
    BuiltinAssemblyError, ProviderFragment, assembled_decimal, assembled_interface,
    assembled_parameters, leaf, sid,
};
use crate::Message;
use crate::builtin::{node_key, node_key_text};
use yss_data_contract::DataValue;
use yss_node_protocol::*;
use yss_node_registry::{CategoryRegistration, TypeRegistration};

const CATEGORY: &str = "plot";

#[derive(Clone, Copy)]
enum PlotInputs {
    Pair,
    NumericSeries,
    CorrelationSeries,
    Correlogram,
    Groups,
    Words,
    ErrorBars,
    Roc,
    Categories,
    Combination,
    Bubble,
    Model,
}

#[derive(Clone, Copy)]
struct PlotSpec {
    id: &'static str,
    en: &'static str,
    zh: &'static str,
    aliases: &'static [&'static str],
    zh_aliases: &'static [&'static str],
    inputs: PlotInputs,
}

const SPECS: &[PlotSpec] = &[
    PlotSpec {
        id: "yssbi.plot.scatter.view",
        en: "Scatter Plot",
        zh: "散点图",
        aliases: &["scatterplot", "XY plot", "points"],
        zh_aliases: &["散点图", "XY图", "点图"],
        inputs: PlotInputs::Pair,
    },
    PlotSpec {
        id: "yssbi.plot.line.view",
        en: "Line Plot",
        zh: "折线图",
        aliases: &["line chart", "time series plot", "curve"],
        zh_aliases: &["折线图", "时间序列图", "曲线图"],
        inputs: PlotInputs::Pair,
    },
    PlotSpec {
        id: "yssbi.plot.ecdf.view",
        en: "Empirical CDF",
        zh: "经验累积分布图",
        aliases: &["ECDF", "empirical cumulative distribution function", "CDF"],
        zh_aliases: &["经验分布函数", "累积分布", "ECDF"],
        inputs: PlotInputs::NumericSeries,
    },
    PlotSpec {
        id: "yssbi.plot.kde.view",
        en: "Kernel Density Estimate",
        zh: "核密度估计图",
        aliases: &[
            "KDE",
            "kernel density estimation",
            "density plot",
            "Silverman bandwidth",
        ],
        zh_aliases: &["核密度估计", "密度图", "KDE"],
        inputs: PlotInputs::NumericSeries,
    },
    PlotSpec {
        id: "yssbi.plot.histogram.view",
        en: "Histogram",
        zh: "直方图",
        aliases: &["frequency distribution", "bins", "Sturges rule"],
        zh_aliases: &["频数分布", "分箱", "斯特吉斯规则"],
        inputs: PlotInputs::NumericSeries,
    },
    PlotSpec {
        id: "yssbi.plot.correlation.view",
        en: "Correlation Plot",
        zh: "相关性图",
        aliases: &[
            "correlation matrix",
            "Pearson correlation",
            "p-value",
            "heatmap",
        ],
        zh_aliases: &["相关矩阵", "皮尔逊相关", "P值", "热力图"],
        inputs: PlotInputs::CorrelationSeries,
    },
    PlotSpec {
        id: "yssbi.plot.correlogram.view",
        en: "Correlogram (ACF & PACF)",
        zh: "相关图（ACF 与 PACF）",
        aliases: &["correlogram", "ACF", "PACF", "Ljung-Box", "autocorrelation"],
        zh_aliases: &["相关图", "自相关", "偏自相关", "Ljung-Box检验"],
        inputs: PlotInputs::Correlogram,
    },
    PlotSpec {
        id: "yssbi.plot.boxplot.view",
        en: "Box Plot",
        zh: "箱线图",
        aliases: &["boxplot", "box-and-whisker"],
        zh_aliases: &["箱线图", "四分位数"],
        inputs: PlotInputs::Groups,
    },
    PlotSpec {
        id: "yssbi.plot.wordcloud.view",
        en: "Word Cloud",
        zh: "词云",
        aliases: &["word cloud", "term frequency"],
        zh_aliases: &["词云", "词频"],
        inputs: PlotInputs::Words,
    },
    PlotSpec {
        id: "yssbi.plot.errorbar.view",
        en: "Error Bar Plot",
        zh: "误差线图",
        aliases: &["error bars", "intervals"],
        zh_aliases: &["误差线", "区间图"],
        inputs: PlotInputs::ErrorBars,
    },
    PlotSpec {
        id: "yssbi.plot.pp_qq.view",
        en: "P-P / Q-Q Plot",
        zh: "P-P/Q-Q图",
        aliases: &["probability plot", "quantile plot"],
        zh_aliases: &["概率图", "分位数图"],
        inputs: PlotInputs::NumericSeries,
    },
    PlotSpec {
        id: "yssbi.plot.roc.view",
        en: "ROC Curve",
        zh: "ROC曲线",
        aliases: &["ROC", "AUC", "receiver operating characteristic"],
        zh_aliases: &["ROC曲线", "AUC"],
        inputs: PlotInputs::Roc,
    },
    PlotSpec {
        id: "yssbi.plot.quadrant.view",
        en: "Quadrant Plot",
        zh: "象限图",
        aliases: &["quadrant chart"],
        zh_aliases: &["象限图", "四象限"],
        inputs: PlotInputs::Pair,
    },
    PlotSpec {
        id: "yssbi.plot.pareto.view",
        en: "Pareto Chart",
        zh: "帕累托图",
        aliases: &["Pareto", "cumulative frequency"],
        zh_aliases: &["帕累托图", "累计频数"],
        inputs: PlotInputs::Categories,
    },
    PlotSpec {
        id: "yssbi.plot.combination.view",
        en: "Combination Chart",
        zh: "组合图",
        aliases: &["combo chart", "bar and line"],
        zh_aliases: &["组合图", "柱线图"],
        inputs: PlotInputs::Combination,
    },
    PlotSpec {
        id: "yssbi.plot.bubble.view",
        en: "Bubble Chart",
        zh: "气泡图",
        aliases: &["bubble plot", "size"],
        zh_aliases: &["气泡图", "气泡大小"],
        inputs: PlotInputs::Bubble,
    },
    PlotSpec {
        id: "yssbi.plot.violin.view",
        en: "Violin Plot",
        zh: "小提琴图",
        aliases: &["violin plot", "distribution"],
        zh_aliases: &["小提琴图", "分布密度"],
        inputs: PlotInputs::Groups,
    },
    PlotSpec {
        id: "yssbi.plot.heatmap.view",
        en: "Heatmap",
        zh: "热力图",
        aliases: &["heat map", "matrix"],
        zh_aliases: &["热力图", "矩阵图"],
        inputs: PlotInputs::Groups,
    },
    PlotSpec {
        id: "yssbi.plot.coefficient.view",
        en: "Coefficient Plot",
        zh: "系数图",
        aliases: &["coefficient plot", "coefplot", "confidence intervals"],
        zh_aliases: &["系数图", "Coef图", "置信区间"],
        inputs: PlotInputs::Model,
    },
];

pub(crate) fn build_provider_fragment() -> Result<ProviderFragment, BuiltinAssemblyError> {
    let mut nodes = Vec::with_capacity(SPECS.len());
    let mut messages = vec![
        (
            "en-US",
            "categories.plot.title".to_owned(),
            Message::Text("Visualization"),
        ),
        (
            "zh-CN",
            "categories.plot.title".to_owned(),
            Message::Text("可视化"),
        ),
        (
            "en-US",
            "parameters.plot.maximum_lag.title".to_owned(),
            Message::Text("Maximum lag"),
        ),
        (
            "zh-CN",
            "parameters.plot.maximum_lag.title".to_owned(),
            Message::Text("最大滞后阶数"),
        ),
        (
            "en-US",
            "types.plot_data.title".to_owned(),
            Message::Text("Plot data"),
        ),
        (
            "zh-CN",
            "types.plot_data.title".to_owned(),
            Message::Text("绘图数据"),
        ),
    ];
    for (key, en, zh) in [
        ("bins", "Bins (0 = automatic)", "分箱数（0 为自动）"),
        ("grid_points", "Density grid points", "密度网格点数"),
        ("max_words", "Maximum words", "最多显示词数"),
        ("mode", "Plot type", "图形类型"),
        (
            "estimate_parameters",
            "Estimate normal reference parameters",
            "估计正态参考分布参数",
        ),
        ("reference_mean", "Reference mean", "参考均值"),
        (
            "reference_standard_deviation",
            "Reference standard deviation",
            "参考标准差",
        ),
        ("x_cut", "X split", "X 分界值"),
        ("y_cut", "Y split", "Y 分界值"),
        ("dual_axis", "Separate line axis", "折线使用独立纵轴"),
        ("confidence_level", "Confidence level", "置信水平"),
        ("include_intercept", "Include intercept", "显示截距"),
    ] {
        messages.push((
            "en-US",
            format!("parameters.plot.{key}.title"),
            Message::Text(en),
        ));
        messages.push((
            "zh-CN",
            format!("parameters.plot.{key}.title"),
            Message::Text(zh),
        ));
    }
    let categories = vec![CategoryRegistration {
        id: category_id(CATEGORY)?,
        title_key: i18n_key("categories.plot.title")?,
        parent: None,
        order: 70,
    }];
    for spec in SPECS {
        add_messages(&mut messages, spec);
        nodes.push(leaf(protocol(spec)?, spec.id));
    }
    let fragment = ProviderFragment {
        categories,
        nodes,
        messages,
        types: vec![TypeRegistration {
            id: type_id("plot.data")?,
            title_key: i18n_key("types.plot_data.title")?,
            classes: Default::default(),
        }],
        ..ProviderFragment::default()
    };
    Ok(fragment)
}

fn protocol(spec: &PlotSpec) -> Result<NodeProtocol, BuiltinAssemblyError> {
    let mut ports = Vec::new();
    match spec.inputs {
        PlotInputs::Pair => {
            ports.push(data_port(
                "x",
                "X",
                PortDirection::Input,
                numeric_data_series_type(),
                PortCardinality::Declared,
            )?);
            ports.push(data_port(
                "y",
                "Y",
                PortDirection::Input,
                numeric_data_series_type(),
                PortCardinality::Declared,
            )?);
        }
        PlotInputs::NumericSeries => ports.push(data_port(
            "values",
            "Values",
            PortDirection::Input,
            numeric_data_series_type(),
            PortCardinality::Declared,
        )?),
        PlotInputs::CorrelationSeries => ports.push(data_port(
            "series",
            "DataSeries",
            PortDirection::Input,
            numeric_data_series_type(),
            PortCardinality::UserCreated {
                min: 2,
                max: Some(64),
            },
        )?),
        PlotInputs::Correlogram => {
            ports.push(data_port(
                "values",
                "DataSeries",
                PortDirection::Input,
                numeric_data_series_type(),
                PortCardinality::Declared,
            )?);
        }
        PlotInputs::Groups => ports.push(data_port(
            "series",
            "Series",
            PortDirection::Input,
            numeric_data_series_type(),
            PortCardinality::UserCreated {
                min: 1,
                max: Some(64),
            },
        )?),
        PlotInputs::Words | PlotInputs::Categories => ports.push(data_port(
            if matches!(spec.inputs, PlotInputs::Words) {
                "words"
            } else {
                "categories"
            },
            "Text",
            PortDirection::Input,
            data_series_type(concrete("core.text")?),
            PortCardinality::Declared,
        )?),
        PlotInputs::ErrorBars => {
            for (key, title) in [
                ("x", "X"),
                ("y", "Estimate"),
                ("lower", "Lower"),
                ("upper", "Upper"),
            ] {
                ports.push(data_port(
                    key,
                    title,
                    PortDirection::Input,
                    numeric_data_series_type(),
                    PortCardinality::Declared,
                )?);
            }
        }
        PlotInputs::Roc => {
            ports.push(data_port(
                "labels",
                "Labels",
                PortDirection::Input,
                data_series_type(concrete("core.binary")?),
                PortCardinality::Declared,
            )?);
            ports.push(data_port(
                "scores",
                "Scores",
                PortDirection::Input,
                numeric_data_series_type(),
                PortCardinality::Declared,
            )?);
        }
        PlotInputs::Combination => {
            ports.push(data_port(
                "categories",
                "Categories",
                PortDirection::Input,
                data_series_type(concrete("core.text")?),
                PortCardinality::Declared,
            )?);
            for (key, title) in [("bars", "Bars"), ("line", "Line")] {
                ports.push(data_port(
                    key,
                    title,
                    PortDirection::Input,
                    numeric_data_series_type(),
                    PortCardinality::Declared,
                )?);
            }
        }
        PlotInputs::Bubble => {
            for (key, title) in [("x", "X"), ("y", "Y"), ("size", "Size")] {
                ports.push(data_port(
                    key,
                    title,
                    PortDirection::Input,
                    numeric_data_series_type(),
                    PortCardinality::Declared,
                )?);
            }
        }
        PlotInputs::Model => ports.push(data_port(
            "model",
            "Linear model",
            PortDirection::Input,
            concrete("statistics.model.linear")?,
            PortCardinality::Declared,
        )?),
    }
    ports.push(data_port(
        "result",
        "Result",
        PortDirection::Output,
        concrete("plot.data")?,
        PortCardinality::Declared,
    )?);
    Ok(NodeProtocol {
        type_id: node_id(spec.id)?,
        catalog: NodeCatalogProtocol {
            title_key: node_key(spec.id, "title")?,
            documentation_key: Some(node_key(spec.id, "documentation")?),
            aliases_key: Some(node_key(spec.id, "aliases")?),
            category_id: category_id(CATEGORY)?,
            icon_id: icon_id("builtin.plot")?,
            style_id: style_id("builtin.plot")?,
            hidden: false,
        },
        interface: assembled_interface(spec.id, ports, vec![], vec![])?,
        parameters: assembled_parameters(spec.id, parameters(spec)?)?,
        instance_display: NodeInstanceDisplaySpec::Static,
        execution: ExecutionSemantics {
            determinism: Determinism::Deterministic,
            cache: CachePolicy::PerRun,
        },
        typing: NodeTypingSpec::Fixed,
        scope: NodeScope::Any,
        managed_role: None,
    })
}

fn parameter(
    key: &'static str,
    value_type: TypeExpr,
    value: DataValue,
    constraints: Vec<ParameterConstraint>,
    editor: ParameterEditorSpec,
) -> Result<Parameter, BuiltinAssemblyError> {
    Ok(Parameter {
        key: sid(key, ParameterKey::new)?,
        title_key: i18n_key(format!("parameters.plot.{key}.title"))?,
        description_key: None,
        default_value: Some(TypedValue {
            value_type: value_type.clone(),
            value,
        }),
        value_type,
        constraints,
        editor,
        presentation: ParameterPresentation::DetailPanel,
        visible_when: None,
    })
}

fn integer_parameter(
    key: &'static str,
    default: i64,
    min: i64,
    max: i64,
) -> Result<Parameter, BuiltinAssemblyError> {
    parameter(
        key,
        concrete("core.numeric")?,
        DataValue::Integer(default),
        vec![ParameterConstraint::IntegerRange {
            min: Some(min),
            max: Some(max),
        }],
        ParameterEditorSpec::Number,
    )
}

fn number_parameter(
    key: &'static str,
    default: &'static str,
) -> Result<Parameter, BuiltinAssemblyError> {
    parameter(
        key,
        concrete("core.numeric")?,
        DataValue::Decimal(assembled_decimal("plot.parameter", default)?),
        if matches!(key, "reference_standard_deviation" | "confidence_level") {
            vec![ParameterConstraint::Positive]
        } else {
            vec![]
        },
        ParameterEditorSpec::Number,
    )
}

fn boolean_parameter(key: &'static str, default: bool) -> Result<Parameter, BuiltinAssemblyError> {
    parameter(
        key,
        concrete("core.binary")?,
        DataValue::Bool(default),
        vec![],
        ParameterEditorSpec::Toggle,
    )
}

fn parameters(spec: &PlotSpec) -> Result<Vec<Parameter>, BuiltinAssemblyError> {
    Ok(match spec.id {
        "yssbi.plot.histogram.view" => vec![integer_parameter("bins", 0, 0, 128)?],
        "yssbi.plot.kde.view" => vec![integer_parameter("grid_points", 256, 16, 512)?],
        "yssbi.plot.correlogram.view" => vec![integer_parameter("maximum_lag", 20, 1, 40)?],
        "yssbi.plot.wordcloud.view" => vec![integer_parameter("max_words", 100, 1, 256)?],
        "yssbi.plot.pp_qq.view" => vec![
            parameter(
                "mode",
                concrete("core.text")?,
                DataValue::String("qq".into()),
                vec![ParameterConstraint::OneOf(vec![
                    DataValue::String("qq".into()),
                    DataValue::String("pp".into()),
                ])],
                ParameterEditorSpec::Select,
            )?,
            boolean_parameter("estimate_parameters", true)?,
            number_parameter("reference_mean", "0")?,
            number_parameter("reference_standard_deviation", "1")?,
        ],
        "yssbi.plot.quadrant.view" => vec![
            number_parameter("x_cut", "0")?,
            number_parameter("y_cut", "0")?,
        ],
        "yssbi.plot.combination.view" => vec![boolean_parameter("dual_axis", true)?],
        "yssbi.plot.coefficient.view" => vec![
            number_parameter("confidence_level", "0.95")?,
            boolean_parameter("include_intercept", false)?,
        ],
        _ => vec![],
    })
}

fn data_port(
    key: &'static str,
    title: &'static str,
    direction: PortDirection,
    value_type: TypeExpr,
    cardinality: PortCardinality,
) -> Result<PortSpec, BuiltinAssemblyError> {
    Ok(PortSpec {
        key: port_key(key)?,
        title: title.into(),
        direction,
        value_type,
        cardinality,
        connections: crate::data_connections(direction),
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
    })
}

fn add_messages(out: &mut Vec<(&'static str, String, Message)>, spec: &PlotSpec) {
    let title = node_key_text(spec.id, "title");
    let documentation = node_key_text(spec.id, "documentation");
    let aliases = node_key_text(spec.id, "aliases");
    out.extend([
        ("en-US", title.to_owned(), Message::Text(spec.en)),
        ("zh-CN", title.to_owned(), Message::Text(spec.zh)),
        ("en-US", documentation.to_owned(), Message::Text("This dataflow view node returns a presentation result that can be opened from the graph output.")),
        ("zh-CN", documentation.to_owned(), Message::Text("此数据流视图节点返回可从图结果中打开的展示结果。")),
        ("en-US", aliases.to_owned(), Message::Aliases(spec.aliases)),
        ("zh-CN", aliases.to_owned(), Message::Aliases(spec.zh_aliases)),
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
use crate::builtin::iid as i18n_key;
