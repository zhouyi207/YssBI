//! Statistical node protocols staged for aggregation into the built-in provider.
//!
//! Algorithms are lowered to runtime kernel handles and depend on the current
//! node-system contracts. Runtime adapters consume the `sci` and `tabular`
//! application boundaries.

use yss_data_contract::DataValue;
mod families;

use super::builtin::{
    BuiltinAssemblyError, ProviderFragment, assembled_decimal, assembled_interface,
    assembled_parameters, configuration_parameter, iid, leaf, sid,
};
use crate::{Aliases, Message, Text};
use yss_node_protocol::*;
use yss_node_registry::{CategoryRegistration, TypeRegistration};

use families::{Family, NODES, NodeSpec, Stage};

pub(crate) fn build_provider_fragment() -> Result<ProviderFragment, BuiltinAssemblyError> {
    let mut messages = Vec::new();
    add_shared_messages(&mut messages);
    let nodes = NODES
        .iter()
        .map(|spec| {
            add_node_messages(&mut messages, spec);
            Ok(leaf(protocol(spec)?, spec.id))
        })
        .collect::<Result<Vec<_>, BuiltinAssemblyError>>()?;
    Ok(ProviderFragment {
        types: statistics_types()?,
        categories: statistics_categories()?,
        nodes,
        messages,
        ..ProviderFragment::default()
    })
}

fn protocol(spec: &NodeSpec) -> Result<NodeProtocol, BuiltinAssemblyError> {
    Ok(NodeProtocol {
        type_id: sid(spec.id, NodeTypeId::new)?,
        catalog: NodeCatalogProtocol {
            title_key: node_key(spec.id, "title")?,
            documentation_key: Some(node_key(spec.id, "documentation")?),
            aliases_key: Some(node_key(spec.id, "aliases")?),
            category_id: sid(category(spec), NodeCategoryId::new)?,
            icon_id: sid("builtin.statistics", IconId::new)?,
            style_id: sid("builtin.dataframe", NodeStyleId::new)?,
            hidden: false,
        },
        interface: assembled_interface(spec.id, ports(spec)?, vec![], vec![])?,
        parameters: assembled_parameters(spec.id, parameters(spec)?)?,
        instance_display: NodeInstanceDisplaySpec::Static,
        execution: execution(),
        typing: NodeTypingSpec::Fixed,
        scope: NodeScope::Any,
        managed_role: None,
    })
}

fn ports(spec: &NodeSpec) -> Result<Vec<PortSpec>, BuiltinAssemblyError> {
    match spec.stage {
        Stage::Fit => fit_ports(spec),
        Stage::Summary => summary_ports(spec),
        Stage::Predict => prediction_ports(spec.family),
        Stage::Test => test_ports(spec.family),
    }
}

fn fit_ports(spec: &NodeSpec) -> Result<Vec<PortSpec>, BuiltinAssemblyError> {
    let mut ports = Vec::new();
    if matches!(spec.family, Family::Vec | Family::Var) {
        ports.push(user_data_input(
            "variables",
            "Variables",
            series_type()?,
            2,
        )?);
    } else {
        ports.extend(regression_inputs(spec.family)?);
    }
    if matches!(spec.family, Family::Iv2sls | Family::IvLiml) {
        ports.push(bounded_user_data_input(
            "endogenous",
            "Endogenous",
            series_type()?,
            1,
            Some(1),
        )?);
        ports.push(bounded_user_data_input(
            "instruments",
            "Instruments",
            series_type()?,
            1,
            Some(1),
        )?);
    }
    if spec.family == Family::PanelDid {
        ports.push(data_input("treatment", "Treatment", series_type()?)?);
    }
    if spec.family == Family::Linear {
        ports.push(bounded_user_data_input(
            "weights",
            "Weights (WLS)",
            series_type()?,
            0,
            Some(1),
        )?);
        ports.push(user_data_input(
            "sigma",
            "Covariance column (GLS)",
            series_type()?,
            0,
        )?);
    }
    ports.push(data_output("model", "Model", model_type(spec)?)?);
    ports.push(data_output("fitted", "Fitted", float_series_type()?)?);
    ports.push(data_output("residuals", "Residuals", float_series_type()?)?);
    if spec.family == Family::PanelDid {
        ports.push(data_output("report", "Report", report_type()?)?);
    }
    Ok(ports)
}

fn summary_ports(spec: &NodeSpec) -> Result<Vec<PortSpec>, BuiltinAssemblyError> {
    Ok(vec![
        data_input("model", "Model", model_type(spec)?)?,
        data_output("result", "Result", result_type(spec.family)?)?,
        data_output("report", "Report", report_type()?)?,
    ])
}

fn prediction_ports(family: Family) -> Result<Vec<PortSpec>, BuiltinAssemblyError> {
    Ok(vec![
        data_input("model", "Model", prediction_model_type(family)?)?,
        user_data_input("predictors", "Predictors", series_type()?, 1)?,
        data_output("prediction", "Prediction", float_series_type()?)?,
    ])
}

fn test_ports(family: Family) -> Result<Vec<PortSpec>, BuiltinAssemblyError> {
    let mut ports = Vec::new();
    match family {
        Family::Adf => ports.push(data_input("series", "DataSeries", series_type()?)?),
        Family::Var | Family::VecRank => ports.push(user_data_input(
            "variables",
            "Variables",
            series_type()?,
            2,
        )?),
        _ => ports.push(data_input("series", "DataSeries", series_type()?)?),
    }
    ports.push(data_output("result", "Result", result_type(family)?)?);
    if family == Family::Adf {
        ports.push(data_output("report", "Report", report_type()?)?);
    }
    Ok(ports)
}

fn regression_inputs(family: Family) -> Result<Vec<PortSpec>, BuiltinAssemblyError> {
    let mut ports = vec![
        data_input("response", "Response", series_type()?)?,
        user_data_input("predictors", "Predictors", series_type()?, 1)?,
    ];
    if matches!(family, Family::Panel | Family::PanelDid) {
        ports.push(data_input("entity", "Entity", series_type()?)?);
        ports.push(data_input("time", "Time", series_type()?)?);
    }
    Ok(ports)
}

fn parameters(spec: &NodeSpec) -> Result<Vec<ParameterSpec>, BuiltinAssemblyError> {
    if spec.stage == Stage::Summary {
        return Ok(vec![]);
    }
    let mut parameters = match spec.stage {
        Stage::Test if spec.family == Family::Adf => vec![
            positive_integer_parameter("lags", 1)?,
            select_parameter("regression", "constant")?,
        ],
        Stage::Test if matches!(spec.family, Family::Var | Family::VecRank) => vec![
            positive_integer_parameter("max_lags", 4)?,
            select_parameter("trend", "constant")?,
        ],
        Stage::Fit if spec.family == Family::Vec => vec![
            positive_integer_parameter("rank", 1)?,
            positive_integer_parameter("lags", 1)?,
            select_parameter("trend", "constant")?,
        ],
        Stage::Fit if spec.family == Family::Var => vec![
            positive_integer_parameter("lags", 1)?,
            select_parameter("trend", "constant")?,
        ],
        Stage::Fit if spec.family == Family::PanelDid => vec![
            toggle_parameter("event_study", false)?,
            positive_integer_parameter("placebo_repetitions", 100)?,
        ],
        _ => vec![],
    };
    if spec.stage == Stage::Fit
        && matches!(
            spec.family,
            Family::Linear
                | Family::Iv2sls
                | Family::IvLiml
                | Family::Logit
                | Family::Probit
                | Family::Panel
                | Family::Prais
        )
    {
        let schema = if spec.family == Family::Linear {
            linear_configuration_schema()?
        } else {
            ConfigurationSchema {
                fields: configure_parameters(spec.family)?
                    .into_iter()
                    .map(|parameter| ConfigurationFieldSpec {
                        parameter,
                        visible_when: None,
                    })
                    .collect(),
            }
        };
        parameters.push(configuration_parameter(
            "parameters.statistics.configuration.title",
            schema,
        )?);
    }
    Ok(parameters)
}

fn configure_parameters(family: Family) -> Result<Vec<ParameterSpec>, BuiltinAssemblyError> {
    let mut parameters = vec![toggle_parameter("constant", true)?];
    match family {
        Family::Logit | Family::Probit => {
            parameters.push(positive_integer_parameter("max_iterations", 100)?);
            parameters.push(decimal_parameter("tolerance", "0.000001")?);
        }
        Family::Iv2sls | Family::IvLiml => {
            parameters.push(select_parameter("covariance", "non_robust")?)
        }
        Family::Panel => {
            parameters.push(select_parameter("estimator", "fixed_effects")?);
            parameters.push(select_parameter("effects", "entity")?);
        }
        Family::Prais => parameters.push(select_parameter("transform", "prais_winsten")?),
        _ => {}
    }
    Ok(parameters)
}

fn linear_configuration_schema() -> Result<ConfigurationSchema, BuiltinAssemblyError> {
    let defaults = yss_sci_contract::regression::OlsOptions::default();
    let choice = |key, default, choices: &[&'static str]| {
        let mut parameter = select_parameter(key, default)?;
        parameter.constraints.push(ParameterConstraint::OneOf(
            choices
                .iter()
                .map(|value| DataValue::String((*value).into()))
                .collect(),
        ));
        Ok::<_, BuiltinAssemblyError>(parameter)
    };
    let conditional = |parameter, covariance: &'static str| -> Result<_, BuiltinAssemblyError> {
        Ok(ConfigurationFieldSpec {
            parameter,
            visible_when: Some(ConfigurationCondition {
                key: sid("covariance", ParameterKey::new)?,
                values: vec![DataValue::String(covariance.into())].into_boxed_slice(),
            }),
        })
    };
    let mut scale = decimal_parameter("scale", "1")?;
    scale.constraints.push(ParameterConstraint::Positive);
    Ok(ConfigurationSchema {
        fields: vec![
            ConfigurationFieldSpec {
                parameter: choice("method", "OLS", &["OLS", "WLS", "GLS"])?,
                visible_when: None,
            },
            ConfigurationFieldSpec {
                parameter: toggle_parameter("constant", defaults.constant)?,
                visible_when: None,
            },
            ConfigurationFieldSpec {
                parameter: choice(
                    "covariance",
                    defaults.covariance.name(),
                    &[
                        "nonrobust",
                        "HC0",
                        "HC1",
                        "HC2",
                        "HC3",
                        "HAC",
                        "newey",
                        "fixed scale",
                    ],
                )?,
                visible_when: None,
            },
            conditional(
                choice(
                    "kernel",
                    "bartlett",
                    &["bartlett", "parzen", "quadratic spectral"],
                )?,
                "HAC",
            )?,
            conditional(positive_integer_parameter("bandwidth", 1)?, "HAC")?,
            conditional(positive_integer_parameter("lag", 1)?, "newey")?,
            conditional(scale, "fixed scale")?,
        ]
        .into_boxed_slice(),
    })
}

fn execution() -> ExecutionSemantics {
    ExecutionSemantics {
        determinism: Determinism::Deterministic,
        cache: CachePolicy::PerRun,
    }
}
fn data_input(
    key: &'static str,
    title: &'static str,
    value_type: TypeExpr,
) -> Result<PortSpec, BuiltinAssemblyError> {
    data_port(
        key,
        title,
        PortDirection::Input,
        value_type,
        PortCardinality::Declared,
    )
}
fn user_data_input(
    key: &'static str,
    title: &'static str,
    value_type: TypeExpr,
    min: u16,
) -> Result<PortSpec, BuiltinAssemblyError> {
    bounded_user_data_input(key, title, value_type, min, None)
}
fn bounded_user_data_input(
    key: &'static str,
    title: &'static str,
    value_type: TypeExpr,
    min: u16,
    max: Option<u16>,
) -> Result<PortSpec, BuiltinAssemblyError> {
    data_port(
        key,
        title,
        PortDirection::Input,
        value_type,
        PortCardinality::UserCreated { min, max },
    )
}
fn data_output(
    key: &'static str,
    title: &'static str,
    value_type: TypeExpr,
) -> Result<PortSpec, BuiltinAssemblyError> {
    data_port(
        key,
        title,
        PortDirection::Output,
        value_type,
        PortCardinality::Declared,
    )
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
            literal_policy: LiteralPolicy::Forbidden,
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

fn positive_integer_parameter(
    key: &'static str,
    default: i64,
) -> Result<ParameterSpec, BuiltinAssemblyError> {
    parameter(
        key,
        concrete("core.numeric")?,
        ParameterEditorSpec::Number,
        DataValue::Integer(default),
        vec![ParameterConstraint::IntegerRange {
            min: Some(1),
            max: None,
        }],
    )
}
fn decimal_parameter(
    key: &'static str,
    default: &'static str,
) -> Result<ParameterSpec, BuiltinAssemblyError> {
    parameter(
        key,
        concrete("core.numeric")?,
        ParameterEditorSpec::Number,
        DataValue::Decimal(assembled_decimal("statistics.parameter", default)?),
        vec![],
    )
}
fn toggle_parameter(
    key: &'static str,
    default: bool,
) -> Result<ParameterSpec, BuiltinAssemblyError> {
    parameter(
        key,
        concrete("core.binary")?,
        ParameterEditorSpec::Toggle,
        DataValue::Bool(default),
        vec![],
    )
}
fn select_parameter(
    key: &'static str,
    default: &'static str,
) -> Result<ParameterSpec, BuiltinAssemblyError> {
    parameter(
        key,
        concrete("core.text")?,
        ParameterEditorSpec::Select,
        DataValue::String(default.into()),
        vec![ParameterConstraint::Required],
    )
}
fn parameter(
    key: &'static str,
    value_type: TypeExpr,
    editor: ParameterEditorSpec,
    value: DataValue,
    constraints: Vec<ParameterConstraint>,
) -> Result<ParameterSpec, BuiltinAssemblyError> {
    Ok(ParameterSpec {
        key: sid(key, ParameterKey::new)?,
        title_key: iid(leak(format!("parameters.statistics.{key}.title")))?,
        description_key: Some(iid(leak(format!(
            "parameters.statistics.{key}.description"
        )))?),
        default_value: Some(TypedValue {
            value_type: value_type.clone(),
            value,
        }),
        value_type,
        constraints,
        editor,
        presentation: ParameterPresentation::DetailPanel,
    })
}

fn statistics_types() -> Result<Vec<TypeRegistration>, BuiltinAssemblyError> {
    [
        (
            "statistics.model.linear",
            "types.statistics_model_linear.title",
        ),
        (
            "statistics.model.logit",
            "types.statistics_model_logit.title",
        ),
        (
            "statistics.model.probit",
            "types.statistics_model_probit.title",
        ),
        (
            "statistics.model.prais",
            "types.statistics_model_prais.title",
        ),
        ("statistics.model.vec", "types.statistics_model_vec.title"),
        (
            "statistics.model.iv_2sls",
            "types.statistics_model_iv_2sls.title",
        ),
        (
            "statistics.model.iv_liml",
            "types.statistics_model_iv_liml.title",
        ),
        (
            "statistics.model.panel",
            "types.statistics_model_panel.title",
        ),
        (
            "statistics.model.panel_did",
            "types.statistics_model_panel_did.title",
        ),
        ("statistics.model.var", "types.statistics_model_var.title"),
        ("statistics.result.adf", "types.statistics_result_adf.title"),
        (
            "statistics.model.vec_rank",
            "types.statistics_model_vec_rank.title",
        ),
        ("statistics.result.linear", "types.statistics_result.title"),
        ("statistics.result.logit", "types.statistics_result.title"),
        ("statistics.result.probit", "types.statistics_result.title"),
        ("statistics.result.prais", "types.statistics_result.title"),
        ("statistics.report", "types.statistics_report.title"),
    ]
    .into_iter()
    .map(|(id, title)| {
        Ok(TypeRegistration {
            id: sid(id, TypeId::new)?,
            title_key: iid(title)?,
            classes: Default::default(),
        })
    })
    .collect()
}
// Navigation categories are independent of method families and execution stages.
const CATEGORIES: &[(&str, &str, &str)] = &[
    (
        "statistics.descriptive",
        "Descriptive Statistics",
        "描述统计",
    ),
    ("statistics.tests", "Hypothesis Tests", "假设检验"),
    (
        "statistics.association",
        "Association and Agreement",
        "相关与一致性",
    ),
    ("statistics.regression", "Regression Models", "回归模型"),
    ("statistics.anova", "Analysis of Variance", "方差分析"),
    (
        "statistics.multivariate",
        "Multivariate Analysis",
        "多元分析",
    ),
    (
        "statistics.longitudinal",
        "Longitudinal and Multilevel Models",
        "纵向与多层模型",
    ),
    ("statistics.panel", "Panel Models", "面板模型"),
    (
        "statistics.causal",
        "Econometrics and Causal Analysis",
        "计量与因果分析",
    ),
    ("statistics.timeseries", "Time Series", "时间序列"),
    ("statistics.survival", "Survival Analysis", "生存分析"),
    ("statistics.spatial", "Spatial Analysis", "空间分析"),
    (
        "statistics.psychometrics",
        "Psychometrics and Structural Equation Models",
        "测量、问卷与结构方程",
    ),
    (
        "statistics.decision",
        "Evaluation and Decision Analysis",
        "综合评价与决策",
    ),
    (
        "statistics.machine_learning",
        "Machine Learning",
        "机器学习",
    ),
    ("statistics.meta", "Meta-analysis", "Meta 分析"),
    (
        "statistics.design_quality",
        "Experimental Design and Quality Control",
        "实验设计与质量控制",
    ),
    ("statistics.power", "Power and Sample Size", "功效与样本量"),
    (
        "statistics.survey",
        "Complex Survey Analysis",
        "复杂抽样分析",
    ),
    (
        "statistics.inference",
        "Inference and Resampling",
        "推断与重抽样",
    ),
    (
        "statistics.diagnostics",
        "Model Diagnostics and Comparison",
        "模型诊断与比较",
    ),
    (
        "statistics.postestimation",
        "Prediction and Post-estimation",
        "预测与估计后分析",
    ),
];

fn statistics_categories() -> Result<Vec<CategoryRegistration>, BuiltinAssemblyError> {
    let mut categories = vec![CategoryRegistration {
        id: sid("statistics", NodeCategoryId::new)?,
        title_key: iid("categories.statistics.title")?,
        parent: None,
        order: 70,
    }];
    for (index, &(id, _, _)) in CATEGORIES.iter().enumerate() {
        categories.push(CategoryRegistration {
            id: sid(id, NodeCategoryId::new)?,
            title_key: iid(leak(format!("categories.{id}.title")))?,
            parent: Some(sid("statistics", NodeCategoryId::new)?),
            order: 71 + index as i32,
        });
    }
    Ok(categories)
}
fn category(spec: &NodeSpec) -> &'static str {
    if spec.stage == Stage::Predict {
        return "statistics.postestimation";
    }
    match spec.family {
        Family::Iv2sls | Family::IvLiml | Family::PanelDid => "statistics.causal",
        Family::Panel => "statistics.panel",
        Family::Adf | Family::Var | Family::Vec | Family::VecRank => "statistics.timeseries",
        Family::Linear | Family::Logit | Family::Probit | Family::Prais => "statistics.regression",
    }
}
fn concrete(id: &'static str) -> Result<TypeExpr, BuiltinAssemblyError> {
    Ok(TypeExpr::Concrete(sid(id, TypeId::new)?))
}
fn series_type() -> Result<TypeExpr, BuiltinAssemblyError> {
    Ok(numeric_data_series_type())
}
fn model_type(spec: &NodeSpec) -> Result<TypeExpr, BuiltinAssemblyError> {
    let id = match spec.family {
        Family::Linear => "statistics.model.linear",
        Family::Logit => "statistics.model.logit",
        Family::Probit => "statistics.model.probit",
        Family::Prais => "statistics.model.prais",
        Family::Vec => "statistics.model.vec",
        Family::Iv2sls => "statistics.model.iv_2sls",
        Family::IvLiml => "statistics.model.iv_liml",
        Family::Panel => "statistics.model.panel",
        Family::PanelDid => "statistics.model.panel_did",
        Family::Var => "statistics.model.var",
        Family::Adf | Family::VecRank => {
            return Err(BuiltinAssemblyError::UnsupportedBuiltinConfiguration {
                context: "statistics fit model family",
                value: format!("{:?}", spec.family).into(),
            });
        }
    };
    concrete(id)
}
fn prediction_model_type(family: Family) -> Result<TypeExpr, BuiltinAssemblyError> {
    let id = match family {
        Family::Linear => "statistics.model.linear",
        Family::Logit => "statistics.model.logit",
        Family::Probit => "statistics.model.probit",
        Family::Adf
        | Family::Iv2sls
        | Family::IvLiml
        | Family::Prais
        | Family::Panel
        | Family::PanelDid
        | Family::Var
        | Family::Vec
        | Family::VecRank => {
            return Err(
                BuiltinAssemblyError::UnsupportedStatisticsPredictionFamily {
                    family: format!("{family:?}").into(),
                },
            );
        }
    };
    concrete(id)
}
fn float_series_type() -> Result<TypeExpr, BuiltinAssemblyError> {
    Ok(data_series_type(concrete("core.numeric")?))
}
fn result_type(family: Family) -> Result<TypeExpr, BuiltinAssemblyError> {
    concrete(match family {
        Family::Adf => "statistics.result.adf",
        Family::Linear => "statistics.result.linear",
        Family::Iv2sls => "statistics.model.iv_2sls",
        Family::IvLiml => "statistics.model.iv_liml",
        Family::Logit => "statistics.result.logit",
        Family::Probit => "statistics.result.probit",
        Family::Prais => "statistics.result.prais",
        Family::Panel => "statistics.model.panel",
        Family::PanelDid => "statistics.model.panel_did",
        Family::Var => "statistics.model.var",
        Family::Vec => "statistics.model.vec",
        Family::VecRank => "statistics.model.vec_rank",
    })
}
fn report_type() -> Result<TypeExpr, BuiltinAssemblyError> {
    concrete("statistics.report")
}
fn port_key(key: &'static str) -> Result<PortKey, BuiltinAssemblyError> {
    sid(key, PortKey::new)
}
fn node_key(id: &'static str, suffix: &'static str) -> Result<I18nKey, BuiltinAssemblyError> {
    iid(leak(format!("nodes.{id}.{suffix}")))
}
fn leak(value: String) -> &'static str {
    Box::leak(value.into_boxed_str())
}

fn add_node_messages(out: &mut Vec<(&'static str, &'static str, Message)>, spec: &NodeSpec) {
    let title = leak(format!("nodes.{}.title", spec.id));
    let documentation = leak(format!("nodes.{}.documentation", spec.id));
    let aliases = leak(format!("nodes.{}.aliases", spec.id));
    out.extend([
        ("en-US", title, Text(spec.title)),
        ("zh-CN", title, Text(spec.zh_title)),
        (
            "en-US",
            documentation,
            Text("Lowered through the scientific runtime API and node-system contracts."),
        ),
        (
            "zh-CN",
            documentation,
            Text("通过科学计算运行时 API 和节点系统契约执行。"),
        ),
        ("en-US", aliases, Aliases(spec.aliases)),
        ("zh-CN", aliases, Aliases(spec.zh_aliases)),
    ]);
}

fn add_shared_messages(out: &mut Vec<(&'static str, &'static str, Message)>) {
    for (key, en, zh) in [
        (
            "types.statistics_model_linear.title",
            "Linear Regression Model",
            "线性回归模型",
        ),
        (
            "types.statistics_model_logit.title",
            "Logit Model",
            "Logit 模型",
        ),
        (
            "types.statistics_model_probit.title",
            "Probit Model",
            "Probit 模型",
        ),
        (
            "types.statistics_model_prais.title",
            "Prais Model",
            "Prais 模型",
        ),
        ("types.statistics_model_vec.title", "VEC Model", "VEC 模型"),
        (
            "types.statistics_model_iv_2sls.title",
            "IV 2SLS Model",
            "IV 2SLS 模型",
        ),
        (
            "types.statistics_model_iv_liml.title",
            "IV LIML Model",
            "IV LIML 模型",
        ),
        (
            "types.statistics_model_panel.title",
            "Panel Model",
            "面板模型",
        ),
        (
            "types.statistics_model_panel_did.title",
            "Panel DID Model",
            "面板 DID 模型",
        ),
        ("types.statistics_model_var.title", "VAR Model", "VAR 模型"),
        (
            "types.statistics_result_adf.title",
            "ADF Result",
            "ADF 结果",
        ),
        (
            "types.statistics_model_vec_rank.title",
            "VEC Rank Result",
            "VEC 秩检验结果",
        ),
        (
            "types.statistics_result.title",
            "Statistical Result",
            "统计结果",
        ),
        (
            "types.statistics_report.title",
            "Statistical Report",
            "统计报告",
        ),
        ("categories.statistics.title", "Statistics", "统计"),
    ] {
        out.push(("en-US", key, Text(en)));
        out.push(("zh-CN", key, Text(zh)));
    }
    for &(id, en, zh) in CATEGORIES {
        let key = leak(format!("categories.{id}.title"));
        out.push(("en-US", key, Text(en)));
        out.push(("zh-CN", key, Text(zh)));
    }
    for key in [
        "method",
        "scale",
        "configuration",
        "kernel",
        "bandwidth",
        "lag",
        "constant",
        "max_iterations",
        "tolerance",
        "covariance_structure",
        "covariance",
        "estimator",
        "effects",
        "transform",
        "lags",
        "regression",
        "max_lags",
        "trend",
        "rank",
        "event_study",
        "placebo_repetitions",
    ] {
        let title = leak(format!("parameters.statistics.{key}.title"));
        let description = leak(format!("parameters.statistics.{key}.description"));
        let (en, zh) = match key {
            "method" => ("Estimation method", "估计方法"),
            "configuration" => ("Model configuration", "模型配置"),
            "constant" => ("Include intercept", "包含常数项"),
            "covariance" => (
                "Standard error method (GLS: nonrobust only)",
                "标准误方法（GLS 仅支持 nonrobust）",
            ),
            "kernel" => ("Kernel", "核函数"),
            "bandwidth" => ("Bandwidth", "带宽"),
            "lag" => ("Lag order", "滞后阶数"),
            "scale" => ("Variance scale", "方差尺度"),
            _ => (key, key),
        };
        out.push(("en-US", title, Text(en)));
        out.push(("zh-CN", title, Text(zh)));
        out.push(("en-US", description, Text("Typed statistical parameter.")));
        out.push(("zh-CN", description, Text("类型化统计参数。")));
    }
}
