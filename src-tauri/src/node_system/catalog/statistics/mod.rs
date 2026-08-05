//! Statistical node protocols staged for aggregation into the built-in provider.
//!
//! Algorithms are lowered to runtime kernel handles and remain independent of
//! legacy graph/executor state. Runtime adapters consume the `sci` and `tabular`
//! application boundaries.

mod families;

use super::builtin::{
    BuiltinAssemblyError, ProviderFragment, assembled_decimal, assembled_interface,
    assembled_parameters, iid, leaf, sid,
};
use super::localization::{Aliases, Message, Text};
use crate::node_system::protocol::*;
use crate::node_system::registry::{CategoryRegistration, TypeRegistration};

#[cfg(test)]
pub use families::LEGACY_NODE_IDS;
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
            description_key: Some(node_key(spec.id, "description")?),
            documentation_key: Some(node_key(spec.id, "documentation")?),
            aliases_key: Some(node_key(spec.id, "aliases")?),
            category_id: sid(category(spec.family), NodeCategoryId::new)?,
            icon_id: sid("builtin.statistics", IconId::new)?,
            style_id: sid("builtin.dataframe", NodeStyleId::new)?,
            hidden: false,
        },
        interface: assembled_interface(spec.id, ports(spec)?, vec![], vec![], vec![])?,
        parameters: assembled_parameters(spec.id, parameters(spec)?)?,
        execution: execution(spec.stage),
        scope: NodeScope::Any,
        managed_role: None,
    })
}

fn ports(spec: &NodeSpec) -> Result<Vec<PortSpec>, BuiltinAssemblyError> {
    match spec.stage {
        Stage::Constant => Ok(vec![data_output("covariance", config_type()?)?]),
        Stage::Configure => configure_ports(spec.family),
        Stage::Fit => fit_ports(spec),
        Stage::Summary => summary_ports(spec.family),
        Stage::Predict => prediction_ports(spec.family),
        Stage::Test => test_ports(spec.family),
    }
}

fn configure_ports(family: Family) -> Result<Vec<PortSpec>, BuiltinAssemblyError> {
    if family == Family::Ols {
        return Ok(vec![
            optional_data_input("covariance", config_type()?)?,
            data_output("configuration", config_type()?)?,
        ]);
    }
    Ok(vec![data_output("configuration", config_type()?)?])
}

fn fit_ports(spec: &NodeSpec) -> Result<Vec<PortSpec>, BuiltinAssemblyError> {
    let mut ports = vec![control_input("enter")?];
    if matches!(spec.family, Family::Vec) {
        ports.push(user_data_input("variables", series_type()?, 2)?);
    } else {
        ports.extend(regression_inputs(spec.family)?);
    }
    if spec.id == "yssbi.statistics.wls.fit" {
        ports.push(data_input("weights", series_type()?)?);
    }
    ports.push(optional_data_input("configuration", config_type()?)?);
    ports.push(data_output("model", model_type()?)?);
    ports.push(data_output("fitted", series_type()?)?);
    ports.push(data_output("residuals", series_type()?)?);
    ports.push(control_output("then")?);
    Ok(ports)
}

fn summary_ports(family: Family) -> Result<Vec<PortSpec>, BuiltinAssemblyError> {
    let mut ports = vec![control_input("enter")?];
    match family {
        Family::Adf => ports.push(data_input("test_result", result_type()?)?),
        Family::PanelDid => {
            ports.extend(regression_inputs(Family::Panel)?);
            ports.push(data_input("treatment", series_type()?)?);
        }
        Family::Var => ports.push(user_data_input("variables", series_type()?, 2)?),
        Family::Iv2sls | Family::IvLiml => {
            ports.extend(regression_inputs(family)?);
            ports.push(user_data_input("endogenous", series_type()?, 1)?);
            ports.push(user_data_input("instruments", series_type()?, 1)?);
        }
        _ => ports.extend(regression_inputs(family)?),
    }
    ports.push(optional_data_input("configuration", config_type()?)?);
    ports.push(data_output("result", result_type()?)?);
    ports.push(data_output("report", report_type()?)?);
    ports.push(control_output("then")?);
    Ok(ports)
}

fn prediction_ports(_family: Family) -> Result<Vec<PortSpec>, BuiltinAssemblyError> {
    Ok(vec![
        control_input("enter")?,
        data_input("model", model_type()?)?,
        user_data_input("predictors", series_type()?, 1)?,
        data_output("prediction", series_type()?)?,
        control_output("then")?,
    ])
}

fn test_ports(family: Family) -> Result<Vec<PortSpec>, BuiltinAssemblyError> {
    let mut ports = vec![control_input("enter")?];
    match family {
        Family::Adf => ports.push(data_input("series", series_type()?)?),
        Family::Var | Family::VecRank => {
            ports.push(user_data_input("variables", series_type()?, 2)?)
        }
        _ => ports.push(data_input("series", series_type()?)?),
    }
    ports.push(data_output("result", result_type()?)?);
    ports.push(control_output("then")?);
    Ok(ports)
}

fn regression_inputs(family: Family) -> Result<Vec<PortSpec>, BuiltinAssemblyError> {
    let mut ports = vec![
        data_input("response", series_type()?)?,
        user_data_input("predictors", series_type()?, 1)?,
    ];
    if matches!(family, Family::Panel | Family::PanelDid) {
        ports.push(data_input("entity", series_type()?)?);
        ports.push(data_input("time", series_type()?)?);
    }
    Ok(ports)
}

fn parameters(spec: &NodeSpec) -> Result<Vec<ParameterSpec>, BuiltinAssemblyError> {
    let parameters = match spec.stage {
        Stage::Constant => vec![],
        Stage::Predict | Stage::Fit | Stage::Summary if spec.family == Family::Prediction => vec![],
        _ if spec.id == "yssbi.statistics.ols.vce.fixed_scale" => {
            vec![decimal_parameter("scale", "1")?]
        }
        _ if spec.id == "yssbi.statistics.ols.vce.cluster" => {
            vec![text_parameter("cluster", false, true)?]
        }
        _ if spec.id == "yssbi.statistics.ols.vce.hac" => vec![
            select_parameter("kernel", "bartlett")?,
            positive_integer_parameter("bandwidth", 1)?,
        ],
        _ if spec.id == "yssbi.statistics.ols.vce.newey_west" => {
            vec![positive_integer_parameter("lag", 1)?]
        }
        Stage::Configure => return configure_parameters(spec.family),
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
        Stage::Summary if spec.family == Family::Var => vec![
            positive_integer_parameter("lags", 1)?,
            select_parameter("trend", "constant")?,
        ],
        Stage::Summary if spec.family == Family::PanelDid => vec![
            toggle_parameter("event_study", false)?,
            positive_integer_parameter("placebo_repetitions", 100)?,
        ],
        _ => vec![],
    };
    Ok(parameters)
}

fn configure_parameters(family: Family) -> Result<Vec<ParameterSpec>, BuiltinAssemblyError> {
    let mut parameters = vec![toggle_parameter("constant", true)?];
    match family {
        Family::Logit | Family::Probit => {
            parameters.push(positive_integer_parameter("max_iterations", 100)?);
            parameters.push(decimal_parameter("tolerance", "0.000001")?);
        }
        Family::Gls => parameters.push(select_parameter("covariance_structure", "identity")?),
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

fn execution(stage: Stage) -> ExecutionSemantics {
    let effectful = !matches!(stage, Stage::Constant | Stage::Configure);
    ExecutionSemantics {
        determinism: Determinism::Deterministic,
        purity: if effectful {
            Purity::Effectful
        } else {
            Purity::Pure
        },
        evaluation: if effectful {
            EvaluationPolicy::EagerWhenRegionEntered
        } else {
            EvaluationPolicy::DemandDriven
        },
        cache: if effectful {
            CachePolicy::None
        } else {
            CachePolicy::PerRun
        },
        effects: if effectful {
            EffectSemantics::Ordered
        } else {
            EffectSemantics::None
        },
    }
}

fn control_input(key: &'static str) -> Result<PortSpec, BuiltinAssemblyError> {
    control_port(key, PortDirection::Input)
}
fn control_output(key: &'static str) -> Result<PortSpec, BuiltinAssemblyError> {
    control_port(key, PortDirection::Output)
}
fn control_port(
    key: &'static str,
    direction: PortDirection,
) -> Result<PortSpec, BuiltinAssemblyError> {
    Ok(PortSpec {
        key: port_key(key)?,
        label_key: iid(leak(format!("ports.{key}.label")))?,
        direction,
        kind: PortKind::Control,
        value_type: TypeExpr::Unknown,
        instances: PortInstances::Declared,
        connections: ConnectionsPerPort::Single,
        input_binding: None,
        consumption: None,
        production: None,
        editor: PortEditorSpec::Default,
        schema: None,
    })
}
fn data_input(key: &'static str, value_type: TypeExpr) -> Result<PortSpec, BuiltinAssemblyError> {
    data_port(
        key,
        PortDirection::Input,
        value_type,
        PortInstances::Declared,
        false,
    )
}
fn optional_data_input(
    key: &'static str,
    value_type: TypeExpr,
) -> Result<PortSpec, BuiltinAssemblyError> {
    data_port(
        key,
        PortDirection::Input,
        value_type,
        PortInstances::Declared,
        true,
    )
}
fn user_data_input(
    key: &'static str,
    value_type: TypeExpr,
    min: u16,
) -> Result<PortSpec, BuiltinAssemblyError> {
    data_port(
        key,
        PortDirection::Input,
        value_type,
        PortInstances::UserCreated { min, max: None },
        false,
    )
}
fn data_output(key: &'static str, value_type: TypeExpr) -> Result<PortSpec, BuiltinAssemblyError> {
    data_port(
        key,
        PortDirection::Output,
        value_type,
        PortInstances::Declared,
        false,
    )
}
fn data_port(
    key: &'static str,
    direction: PortDirection,
    value_type: TypeExpr,
    instances: PortInstances,
    optional: bool,
) -> Result<PortSpec, BuiltinAssemblyError> {
    Ok(PortSpec {
        key: port_key(key)?,
        label_key: iid(leak(format!("ports.{key}.label")))?,
        direction,
        kind: PortKind::Data,
        value_type: value_type.clone(),
        instances,
        connections: ConnectionsPerPort::Single,
        input_binding: (direction == PortDirection::Input).then_some(InputBindingSpec {
            literal_policy: if optional {
                LiteralPolicy::Allowed
            } else {
                LiteralPolicy::Forbidden
            },
            default_value: optional.then(|| TypedValue {
                value_type: value_type.clone(),
                value: Value::Null,
            }),
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
        concrete("core.int64")?,
        ParameterEditorSpec::Number,
        Value::Integer(default),
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
        concrete("core.float64")?,
        ParameterEditorSpec::Number,
        Value::Decimal(assembled_decimal("statistics.parameter", default)?),
        vec![],
    )
}
fn toggle_parameter(
    key: &'static str,
    default: bool,
) -> Result<ParameterSpec, BuiltinAssemblyError> {
    parameter(
        key,
        concrete("core.bool")?,
        ParameterEditorSpec::Toggle,
        Value::Bool(default),
        vec![],
    )
}
fn select_parameter(
    key: &'static str,
    default: &'static str,
) -> Result<ParameterSpec, BuiltinAssemblyError> {
    parameter(
        key,
        concrete("core.string")?,
        ParameterEditorSpec::Select,
        Value::String(default.into()),
        vec![ParameterConstraint::Required],
    )
}
fn text_parameter(
    key: &'static str,
    multiline: bool,
    required: bool,
) -> Result<ParameterSpec, BuiltinAssemblyError> {
    parameter(
        key,
        concrete("core.string")?,
        ParameterEditorSpec::Text { multiline },
        Value::String("".into()),
        required
            .then_some(ParameterConstraint::Required)
            .into_iter()
            .collect(),
    )
}
fn parameter(
    key: &'static str,
    value_type: TypeExpr,
    editor: ParameterEditorSpec,
    value: Value,
    constraints: Vec<ParameterConstraint>,
) -> Result<ParameterSpec, BuiltinAssemblyError> {
    Ok(ParameterSpec {
        key: sid(key, ParameterKey::new)?,
        title_key: iid(leak(format!("parameters.statistics.{key}.title")))?,
        description_key: Some(iid(leak(format!(
            "parameters.statistics.{key}.description"
        )))?),
        default_value: Some(ParameterValue {
            value_type: value_type.clone(),
            value,
        }),
        value_type,
        constraints,
        editor,
    })
}

fn statistics_types() -> Result<Vec<TypeRegistration>, BuiltinAssemblyError> {
    [
        (
            "statistics.configuration",
            "types.statistics_configuration.title",
        ),
        ("statistics.model", "types.statistics_model.title"),
        ("statistics.result", "types.statistics_result.title"),
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
fn statistics_categories() -> Result<Vec<CategoryRegistration>, BuiltinAssemblyError> {
    [
        ("statistics", None, 70),
        ("statistics.regression", Some("statistics"), 71),
        ("statistics.panel", Some("statistics"), 72),
        ("statistics.timeseries", Some("statistics"), 73),
    ]
    .into_iter()
    .map(|(id, parent, order)| {
        Ok(CategoryRegistration {
            id: sid(id, NodeCategoryId::new)?,
            title_key: iid(leak(format!("categories.{id}.title")))?,
            parent: parent
                .map(|value| sid(value, NodeCategoryId::new))
                .transpose()?,
            order,
        })
    })
    .collect()
}
fn category(family: Family) -> &'static str {
    match family {
        Family::Panel | Family::PanelDid => "statistics.panel",
        Family::Adf | Family::Var | Family::Vec | Family::VecRank => "statistics.timeseries",
        _ => "statistics.regression",
    }
}
fn concrete(id: &'static str) -> Result<TypeExpr, BuiltinAssemblyError> {
    Ok(TypeExpr::Concrete(sid(id, TypeId::new)?))
}
fn series_type() -> Result<TypeExpr, BuiltinAssemblyError> {
    concrete("tabular.series")
}
fn config_type() -> Result<TypeExpr, BuiltinAssemblyError> {
    concrete("statistics.configuration")
}
fn model_type() -> Result<TypeExpr, BuiltinAssemblyError> {
    concrete("statistics.model")
}
fn result_type() -> Result<TypeExpr, BuiltinAssemblyError> {
    concrete("statistics.result")
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
    let description = leak(format!("nodes.{}.description", spec.id));
    let documentation = leak(format!("nodes.{}.documentation", spec.id));
    let aliases = leak(format!("nodes.{}.aliases", spec.id));
    out.extend([
        ("en-US", title, Text(spec.title)),
        ("zh-CN", title, Text(spec.zh_title)),
        (
            "en-US",
            description,
            Text("Runs a typed statistical operation."),
        ),
        ("zh-CN", description, Text("执行类型化统计操作。")),
        (
            "en-US",
            documentation,
            Text("Lowered to the scientific runtime API without legacy graph state."),
        ),
        (
            "zh-CN",
            documentation,
            Text("降低到科学计算运行时 API，不依赖旧图状态。"),
        ),
        ("en-US", aliases, Aliases(spec.aliases)),
        ("zh-CN", aliases, Aliases(spec.zh_aliases)),
    ]);
}

fn add_shared_messages(out: &mut Vec<(&'static str, &'static str, Message)>) {
    for (key, en, zh) in [
        (
            "types.statistics_configuration.title",
            "Statistical Configuration",
            "统计配置",
        ),
        (
            "types.statistics_model.title",
            "Statistical Model",
            "统计模型",
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
        (
            "categories.statistics.regression.title",
            "Regression",
            "回归",
        ),
        (
            "categories.statistics.panel.title",
            "Panel Statistics",
            "面板统计",
        ),
        (
            "categories.statistics.timeseries.title",
            "Time-Series Statistics",
            "时间序列统计",
        ),
    ] {
        out.push(("en-US", key, Text(en)));
        out.push(("zh-CN", key, Text(zh)));
    }
    for key in [
        "covariance",
        "configuration",
        "response",
        "predictors",
        "weights",
        "model",
        "fitted",
        "residuals",
        "test_result",
        "endogenous",
        "instruments",
        "entity",
        "time",
        "treatment",
        "variables",
        "report",
        "prediction",
        "series",
    ] {
        let label = leak(format!("ports.{key}.label"));
        out.push(("en-US", label, Text(key)));
        out.push(("zh-CN", label, Text(key)));
    }
    for key in [
        "scale",
        "cluster",
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
        out.push(("en-US", title, Text(key)));
        out.push(("zh-CN", title, Text(key)));
        out.push(("en-US", description, Text("Typed statistical parameter.")));
        out.push(("zh-CN", description, Text("类型化统计参数。")));
    }
}

#[cfg(test)]
mod tests;
