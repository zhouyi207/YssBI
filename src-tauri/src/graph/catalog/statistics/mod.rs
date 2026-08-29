//! Statistical node protocols staged for aggregation into the built-in provider.
//!
//! Algorithms are lowered to runtime kernel handles and depend on the current
//! node-system contracts. Runtime adapters consume the `sci` and `tabular`
//! application boundaries.

mod families;

use super::builtin::{
    BuiltinAssemblyError, ProviderFragment, assembled_decimal, assembled_interface,
    assembled_parameters, iid, leaf, sid,
};
use crate::graph::catalog::{Aliases, Message, Text};
use crate::graph::protocol::*;
use crate::graph::registry::{CategoryRegistration, TypeRegistration};

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
            category_id: sid(category(spec.family), NodeCategoryId::new)?,
            icon_id: sid("builtin.statistics", IconId::new)?,
            style_id: sid("builtin.dataframe", NodeStyleId::new)?,
            hidden: false,
        },
        interface: assembled_interface(spec.id, ports(spec)?, vec![], vec![], vec![])?,
        parameters: assembled_parameters(spec.id, parameters(spec)?)?,
        instance_display: NodeInstanceDisplaySpec::Static,
        execution: execution(spec.stage),
        scope: NodeScope::Any,
        managed_role: None,
    })
}

fn ports(spec: &NodeSpec) -> Result<Vec<PortSpec>, BuiltinAssemblyError> {
    match spec.stage {
        Stage::Constant => Ok(vec![data_output(
            "covariance",
            "Covariance",
            config_type()?,
        )?]),
        Stage::Configure => configure_ports(spec.family),
        Stage::Fit => fit_ports(spec),
        Stage::Summary => summary_ports(spec),
        Stage::Predict => prediction_ports(spec.family),
        Stage::Test => test_ports(spec.family),
    }
}

fn configure_ports(family: Family) -> Result<Vec<PortSpec>, BuiltinAssemblyError> {
    if family == Family::Ols {
        return Ok(vec![
            optional_data_input("covariance", "Covariance", config_type()?)?,
            data_output("configuration", "Config", config_type()?)?,
        ]);
    }
    Ok(vec![data_output(
        "configuration",
        "Config",
        config_type()?,
    )?])
}

fn fit_ports(spec: &NodeSpec) -> Result<Vec<PortSpec>, BuiltinAssemblyError> {
    let mut ports = vec![control_input("enter", "Enter")?];
    if matches!(spec.family, Family::Vec) {
        ports.push(user_data_input(
            "variables",
            "Variables",
            series_type()?,
            2,
        )?);
    } else {
        ports.extend(regression_inputs(spec.family)?);
    }
    if spec.id == "yssbi.statistics.wls.fit" {
        ports.push(data_input("weights", "Weights", series_type()?)?);
    }
    ports.push(optional_data_input(
        "configuration",
        "Config",
        config_type()?,
    )?);
    ports.push(data_output("model", "Model", model_type(spec)?)?);
    ports.push(data_output("fitted", "Fitted", float_series_type()?)?);
    ports.push(data_output("residuals", "Residuals", float_series_type()?)?);
    ports.push(control_output("then", "Then")?);
    Ok(ports)
}

fn summary_ports(spec: &NodeSpec) -> Result<Vec<PortSpec>, BuiltinAssemblyError> {
    let family = spec.family;
    let mut ports = vec![control_input("enter", "Enter")?];
    match family {
        Family::Adf => ports.push(data_input(
            "test_result",
            "Test Result",
            result_type(Family::Adf)?,
        )?),
        Family::PanelDid => {
            ports.extend(regression_inputs(Family::Panel)?);
            ports.push(data_input("treatment", "Treatment", series_type()?)?);
        }
        Family::Var => ports.push(user_data_input(
            "variables",
            "Variables",
            series_type()?,
            2,
        )?),
        Family::Iv2sls | Family::IvLiml => {
            ports.extend(regression_inputs(family)?);
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
        _ => ports.extend(regression_inputs(family)?),
    }
    ports.push(optional_data_input(
        "configuration",
        "Config",
        config_type()?,
    )?);
    ports.push(data_output("result", "Result", summary_result_type(spec)?)?);
    ports.push(data_output("report", "Report", report_type()?)?);
    ports.push(control_output("then", "Then")?);
    Ok(ports)
}

fn prediction_ports(family: Family) -> Result<Vec<PortSpec>, BuiltinAssemblyError> {
    Ok(vec![
        control_input("enter", "Enter")?,
        data_input("model", "Model", prediction_model_type(family)?)?,
        user_data_input("predictors", "Predictors", series_type()?, 1)?,
        data_output("prediction", "Prediction", float_series_type()?)?,
        control_output("then", "Then")?,
    ])
}

fn test_ports(family: Family) -> Result<Vec<PortSpec>, BuiltinAssemblyError> {
    let mut ports = vec![control_input("enter", "Enter")?];
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
    ports.push(control_output("then", "Then")?);
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
    let mut parameters = match spec.stage {
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
    if matches!(
        spec.stage,
        Stage::Fit | Stage::Summary | Stage::Predict | Stage::Test
    ) {
        parameters.push(inherited_decimal_parameter("convergence_tolerance")?);
        parameters.push(inherited_select_parameter("missing_value_policy")?);
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
            CachePolicy::Disabled
        } else {
            CachePolicy::PerRun
        },
        effects: if effectful {
            EffectSemantics::Ordered
        } else {
            EffectSemantics::None
        },
        idempotent: false,
        retry: None,
    }
}

fn control_input(key: &'static str, title: &'static str) -> Result<PortSpec, BuiltinAssemblyError> {
    control_port(key, title, PortDirection::Input)
}
fn control_output(
    key: &'static str,
    title: &'static str,
) -> Result<PortSpec, BuiltinAssemblyError> {
    control_port(key, title, PortDirection::Output)
}
fn control_port(
    key: &'static str,
    title: &'static str,
    direction: PortDirection,
) -> Result<PortSpec, BuiltinAssemblyError> {
    Ok(PortSpec {
        key: port_key(key)?,
        title: title.into(),
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
        PortInstances::Declared,
        false,
    )
}
fn optional_data_input(
    key: &'static str,
    title: &'static str,
    value_type: TypeExpr,
) -> Result<PortSpec, BuiltinAssemblyError> {
    data_port(
        key,
        title,
        PortDirection::Input,
        value_type,
        PortInstances::Declared,
        true,
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
        PortInstances::UserCreated { min, max },
        false,
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
        PortInstances::Declared,
        false,
    )
}
fn data_port(
    key: &'static str,
    title: &'static str,
    direction: PortDirection,
    value_type: TypeExpr,
    instances: PortInstances,
    optional: bool,
) -> Result<PortSpec, BuiltinAssemblyError> {
    Ok(PortSpec {
        key: port_key(key)?,
        title: title.into(),
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
fn inherited_decimal_parameter(key: &'static str) -> Result<ParameterSpec, BuiltinAssemblyError> {
    optional_parameter(
        key,
        concrete("core.float64")?,
        ParameterEditorSpec::Number,
        vec![],
    )
}
fn inherited_select_parameter(key: &'static str) -> Result<ParameterSpec, BuiltinAssemblyError> {
    optional_parameter(
        key,
        concrete("core.string")?,
        ParameterEditorSpec::Select,
        vec![ParameterConstraint::OneOf(vec![
            Value::String("Listwise".into()),
            Value::String("Reject".into()),
        ])],
    )
}
fn optional_parameter(
    key: &'static str,
    value_type: TypeExpr,
    editor: ParameterEditorSpec,
    constraints: Vec<ParameterConstraint>,
) -> Result<ParameterSpec, BuiltinAssemblyError> {
    Ok(ParameterSpec {
        key: sid(key, ParameterKey::new)?,
        title_key: iid(leak(format!("parameters.statistics.{key}.title")))?,
        description_key: Some(iid(leak(format!(
            "parameters.statistics.{key}.description"
        )))?),
        default_value: None,
        value_type,
        constraints,
        editor,
        presentation: ParameterPresentation::DetailPanel,
    })
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
        presentation: ParameterPresentation::DetailPanel,
    })
}

fn statistics_types() -> Result<Vec<TypeRegistration>, BuiltinAssemblyError> {
    [
        (
            "statistics.configuration",
            "types.statistics_configuration.title",
        ),
        ("statistics.model.ols", "types.statistics_model_ols.title"),
        ("statistics.model.gls", "types.statistics_model_gls.title"),
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
        ("statistics.model.wls", "types.statistics_model_wls.title"),
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
        ("statistics.model.adf", "types.statistics_model_adf.title"),
        (
            "statistics.model.vec_rank",
            "types.statistics_model_vec_rank.title",
        ),
        ("statistics.result.ols", "types.statistics_result.title"),
        ("statistics.result.gls", "types.statistics_result.title"),
        ("statistics.result.logit", "types.statistics_result.title"),
        ("statistics.result.probit", "types.statistics_result.title"),
        ("statistics.result.prais", "types.statistics_result.title"),
        ("statistics.result.wls", "types.statistics_result.title"),
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
    Ok(numeric_data_series_type())
}
fn config_type() -> Result<TypeExpr, BuiltinAssemblyError> {
    concrete("statistics.configuration")
}
fn model_type(spec: &NodeSpec) -> Result<TypeExpr, BuiltinAssemblyError> {
    let id = if spec.id == "yssbi.statistics.wls.fit" {
        "statistics.model.wls"
    } else {
        match spec.family {
            Family::Ols | Family::Prediction => "statistics.model.ols",
            Family::Gls => "statistics.model.gls",
            Family::Logit => "statistics.model.logit",
            Family::Probit => "statistics.model.probit",
            Family::Prais => "statistics.model.prais",
            Family::Vec => "statistics.model.vec",
            Family::Adf
            | Family::Iv2sls
            | Family::IvLiml
            | Family::Panel
            | Family::PanelDid
            | Family::Var
            | Family::VecRank => {
                return Err(BuiltinAssemblyError::UnsupportedBuiltinConfiguration {
                    context: "statistics fit model family",
                    value: format!("{:?}", spec.family).into(),
                });
            }
        }
    };
    concrete(id)
}
fn prediction_model_type(family: Family) -> Result<TypeExpr, BuiltinAssemblyError> {
    let id = match family {
        Family::Ols | Family::Prediction => "statistics.model.ols",
        Family::Logit => "statistics.model.logit",
        Family::Probit => "statistics.model.probit",
        Family::Adf
        | Family::Gls
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
    Ok(data_series_type(concrete("core.float64")?))
}
fn summary_result_type(spec: &NodeSpec) -> Result<TypeExpr, BuiltinAssemblyError> {
    if spec.id == "yssbi.statistics.wls.summary" {
        concrete("statistics.result.wls")
    } else {
        result_type(spec.family)
    }
}
fn result_type(family: Family) -> Result<TypeExpr, BuiltinAssemblyError> {
    concrete(match family {
        Family::Adf => "statistics.model.adf",
        Family::Ols | Family::Prediction => "statistics.result.ols",
        Family::Gls => "statistics.result.gls",
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
            "types.statistics_configuration.title",
            "Statistical Configuration",
            "统计配置",
        ),
        ("types.statistics_model_ols.title", "OLS Model", "OLS 模型"),
        ("types.statistics_model_gls.title", "GLS Model", "GLS 模型"),
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
        ("types.statistics_model_wls.title", "WLS Model", "WLS 模型"),
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
        ("types.statistics_model_adf.title", "ADF Result", "ADF 结果"),
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
        "convergence_tolerance",
        "missing_value_policy",
    ] {
        let title = leak(format!("parameters.statistics.{key}.title"));
        let description = leak(format!("parameters.statistics.{key}.description"));
        out.push(("en-US", title, Text(key)));
        out.push(("zh-CN", title, Text(key)));
        out.push(("en-US", description, Text("Typed statistical parameter.")));
        out.push(("zh-CN", description, Text("类型化统计参数。")));
    }
}
