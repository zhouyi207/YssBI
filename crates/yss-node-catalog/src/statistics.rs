//! Statistical node protocols staged for aggregation into the built-in provider.
//!
//! This module owns graph-independent interfaces and localization. Node Kernel
//! owns executable adapters; Graph Analysis owns connected types and schemas.

use crate::builtin::node_key;
use crate::builtin::node_scoped_keys;
use crate::builtin::{choice_condition, concrete, data_port};
use yss_data_contract::DataValue;
mod analyses;
mod anova;
mod association;
mod causal_models;
mod classical;
mod decision;
mod descriptive;
mod doe;
mod families;
mod inference;
mod inventory;
mod longitudinal;
mod meta;
mod multivariate;
mod panel_models;
mod path;
mod ports;
mod power;
mod psychometrics;
mod quality;
mod regression_models;
mod spatial;
mod survey;
mod survival;
mod time_series;

pub(crate) use inventory::ENTRIES as INVENTORY;

use super::builtin::{
    BuiltinAssemblyError, ProviderFragment, assembled_decimal, assembled_interface,
    assembled_parameters, iid, leaf, pure, sid,
};
use yss_node_protocol::*;
use yss_node_registry::{CategoryRegistration, TypeRegistration};

use families::{Family, NODES, NodeSpec, Stage};
use ports::{
    fitted_regression_type, fixed_numeric_table, fixed_table, label_series,
    numeric_or_binary_series,
};

pub(crate) fn build_provider_fragment() -> Result<ProviderFragment, BuiltinAssemblyError> {
    let mut fragment = defined_provider_fragment()?;
    inventory::append(&mut fragment)?;
    Ok(fragment)
}

fn defined_provider_fragment() -> Result<ProviderFragment, BuiltinAssemblyError> {
    let nodes = NODES
        .iter()
        .map(|spec| Ok(leaf(protocol(spec)?, spec.id)))
        .collect::<Result<Vec<_>, BuiltinAssemblyError>>()?;
    let mut fragment = ProviderFragment {
        types: statistics_types()?,
        categories: statistics_categories()?,
        nodes,
        ..ProviderFragment::default()
    };
    decision::append(&mut fragment)?;
    psychometrics::append(&mut fragment)?;
    path::append(&mut fragment)?;
    power::append(&mut fragment)?;
    survey::append(&mut fragment)?;
    quality::append(&mut fragment)?;
    doe::append(&mut fragment)?;
    time_series::append(&mut fragment)?;
    longitudinal::append(&mut fragment)?;
    analyses::append(&mut fragment)?;
    anova::append(&mut fragment)?;
    multivariate::append(&mut fragment)?;
    association::append(&mut fragment)?;
    regression_models::append(&mut fragment)?;
    panel_models::append(&mut fragment)?;
    causal_models::append(&mut fragment)?;
    spatial::append(&mut fragment)?;
    survival::append(&mut fragment)?;
    classical::append(&mut fragment)?;
    meta::append(&mut fragment)?;
    inference::append(&mut fragment)?;
    descriptive::append(&mut fragment)?;
    Ok(fragment)
}

fn protocol(spec: &NodeSpec) -> Result<NodeProtocol, BuiltinAssemblyError> {
    Ok(NodeProtocol {
        type_id: sid(spec.id, NodeTypeId::new)?,
        catalog: NodeCatalogProtocol {
            title_key: node_key(spec.id, "title")?,
            aliases_key: Some(node_key(spec.id, "aliases")?),
            category_id: sid(category(spec), NodeCategoryId::new)?,
            icon_id: sid("builtin.statistics", IconId::new)?,
            style_id: sid("builtin.dataframe", NodeStyleId::new)?,
            hidden: false,
        },
        interface: assembled_interface(spec.id, ports(spec)?, vec![], vec![])?,
        parameters: {
            let parameters = parameters(spec)?;
            if spec.family == Family::Linear && spec.stage == Stage::Fit {
                let (model, covariance): (Vec<_>, Vec<_>) = parameters
                    .into_iter()
                    .partition(|parameter| matches!(parameter.key.as_str(), "method" | "constant"));
                Parameters::new([
                    ParameterGroup::new("model", model),
                    ParameterGroup::new("covariance", covariance),
                ])
                .map_err(|source| {
                    BuiltinAssemblyError::InvalidParameterSchema {
                        node_type: spec.id.into(),
                        source,
                    }
                })?
            } else if spec.stage == Stage::Summary && !parameters.is_empty() {
                Parameters::new([ParameterGroup::new("configure", parameters)]).map_err(
                    |source| BuiltinAssemblyError::InvalidParameterSchema {
                        node_type: spec.id.into(),
                        source,
                    },
                )?
            } else {
                assembled_parameters(spec.id, parameters)?
            }
        },
        instance_display: NodeInstanceDisplaySpec::Static,
        execution: pure(),
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
    if spec.id == "yssbi.statistics.panel.compare" {
        let mut ports = regression_inputs(Family::Panel)?;
        ports.push(data_output(
            "result",
            "Model comparison",
            result_type(Family::Panel)?,
        )?);
        return Ok(ports);
    }
    let mut ports = Vec::new();
    if matches!(spec.family, Family::Vec | Family::Var) {
        ports.push(user_data_input(
            "variables",
            "Variables",
            numeric_data_series_type(),
            2,
        )?);
    } else {
        ports.extend(regression_inputs(spec.family)?);
    }
    if spec.family == Family::Var {
        ports.push(user_data_input(
            "exogenous",
            "Exogenous",
            numeric_data_series_type(),
            0,
        )?);
    }
    if matches!(spec.family, Family::Iv2sls | Family::IvLiml) {
        ports.push(bounded_user_data_input(
            "endogenous",
            "Endogenous",
            numeric_data_series_type(),
            1,
            None,
        )?);
        ports.push(bounded_user_data_input(
            "instruments",
            "Instruments",
            numeric_data_series_type(),
            1,
            None,
        )?);
    }
    if spec.family == Family::PanelDid {
        ports.push(data_input(
            "treatment",
            "Treatment",
            numeric_data_series_type(),
        )?);
    }
    if spec.family == Family::Linear {
        ports.push(bounded_user_data_input(
            "weights",
            "Weights (WLS)",
            numeric_data_series_type(),
            0,
            Some(1),
        )?);
        ports.push(user_data_input(
            "sigma",
            "Covariance column (GLS)",
            numeric_data_series_type(),
            0,
        )?);
    }
    if spec.family == Family::Linear {
        ports.push(bounded_user_data_input(
            "clusters",
            "Cluster IDs",
            label_series()?,
            0,
            Some(1),
        )?);
    }
    if spec.family == Family::PanelDid {
        ports.push(data_output("result", "Result", result_type(spec.family)?)?);
    } else {
        ports.push(data_output("model", "Model", model_type(spec.family)?)?);
    }
    if !matches!(spec.family, Family::PanelDid | Family::Var | Family::Vec) {
        ports.push(data_output("fitted", "Fitted", numeric_data_series_type())?);
        ports.push(data_output(
            "residuals",
            "Residuals",
            numeric_data_series_type(),
        )?);
    }
    Ok(ports)
}

fn summary_ports(spec: &NodeSpec) -> Result<Vec<PortSpec>, BuiltinAssemblyError> {
    Ok(vec![
        data_input("model", "Model", model_type(spec.family)?)?,
        data_output("result", "Result", result_type(spec.family)?)?,
    ])
}

fn prediction_ports(family: Family) -> Result<Vec<PortSpec>, BuiltinAssemblyError> {
    Ok(vec![
        data_input("model", "Model", prediction_model_type(family)?)?,
        user_data_input("x", "X", numeric_data_series_type(), 1)?,
        data_output("prediction", "Prediction", numeric_data_series_type())?,
    ])
}

fn test_ports(family: Family) -> Result<Vec<PortSpec>, BuiltinAssemblyError> {
    let mut ports = Vec::new();
    match family {
        Family::PanelDid => {
            ports.extend(regression_inputs(family)?);
            ports.push(data_input(
                "treat",
                "Treated group (0/1)",
                numeric_data_series_type(),
            )?);
            ports.push(data_input(
                "post",
                "Post period (0/1)",
                numeric_data_series_type(),
            )?);
        }
        Family::Adf => ports.push(user_data_input(
            "series",
            "DataSeries",
            numeric_data_series_type(),
            1,
        )?),
        Family::Var | Family::VecRank => ports.push(user_data_input(
            "variables",
            "Variables",
            numeric_data_series_type(),
            2,
        )?),
        _ => ports.push(data_input(
            "series",
            "DataSeries",
            numeric_data_series_type(),
        )?),
    }
    ports.push(data_output("result", "Result", result_type(family)?)?);
    Ok(ports)
}

fn regression_inputs(family: Family) -> Result<Vec<PortSpec>, BuiltinAssemblyError> {
    let mut ports = vec![
        data_input("y", "Y", numeric_data_series_type())?,
        user_data_input(
            "x",
            "X",
            numeric_data_series_type(),
            if matches!(family, Family::Iv2sls | Family::IvLiml | Family::PanelDid) {
                0
            } else {
                1
            },
        )?,
    ];
    if matches!(family, Family::Panel | Family::PanelDid) {
        ports.push(data_input("entity", "Entity", numeric_data_series_type())?);
        ports.push(data_input("time", "Time", numeric_data_series_type())?);
    }
    Ok(ports)
}

fn parameters(spec: &NodeSpec) -> Result<Vec<Parameter>, BuiltinAssemblyError> {
    if spec.id == "yssbi.statistics.panel.compare" {
        let mut parameters = configure_parameters(Family::Panel)?;
        parameters.retain(|p| p.key.as_str() != "estimator");
        parameters.push(parameter(
            "estimators",
            concrete("core.text")?,
            ParameterEditorSpec::Text { multiline: false },
            DataValue::String("fixed_effects,random_effects,between".into()),
            vec![ParameterConstraint::Length {
                min: Some(1),
                max: Some(256),
            }],
        )?);
        return Ok(parameters);
    }
    if spec.stage == Stage::Summary {
        return if spec.family == Family::Linear {
            linear_summary_parameters()
        } else {
            model_summary_parameters(spec.family)
        };
    }
    let mut parameters = match spec.stage {
        Stage::Test if spec.family == Family::PanelDid => vec![
            bounded_integer_parameter("repetitions", 100, 10, 2000)?,
            nonnegative_integer_parameter("seed", 42)?,
            toggle_parameter("constant", true)?,
            choice_parameter(
                "covariance",
                "cluster",
                &["nonrobust", "HC0", "HC1", "HC2", "HC3", "cluster"],
            )?,
            toggle_parameter("use_observed_coefficient", false)?,
            decimal_parameter("observed_coefficient", "0")?,
        ],
        Stage::Test if spec.family == Family::Adf => vec![
            bounded_integer_parameter("lags", 1, 0, 1000)?,
            choice_parameter("regression", "constant", &["none", "constant", "trend"])?,
        ],
        Stage::Test if spec.family == Family::Var => {
            vec![bounded_integer_parameter("max_lags", 4, 1, 1000)?]
        }
        Stage::Test if spec.family == Family::VecRank => vec![
            bounded_integer_parameter("max_lags", 4, 1, 1000)?,
            choice_parameter("trend", "constant", &["none", "constant", "trend"])?,
        ],
        Stage::Fit if spec.family == Family::Vec => vec![
            positive_integer_parameter("rank", 1)?,
            bounded_integer_parameter("lags", 1, 1, 1000)?,
            choice_parameter("trend", "constant", &["none", "constant", "trend"])?,
        ],
        Stage::Fit if spec.family == Family::Var => {
            vec![
                bounded_integer_parameter("lags", 1, 1, 1000)?,
                toggle_parameter("constant", true)?,
                toggle_parameter("dfk", false)?,
                parameter(
                    "selected_lags",
                    concrete("core.text")?,
                    ParameterEditorSpec::Text { multiline: false },
                    DataValue::String("".into()),
                    vec![ParameterConstraint::Length {
                        min: None,
                        max: Some(4096),
                    }],
                )?,
            ]
        }
        Stage::Fit if spec.family == Family::PanelDid => vec![],
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
            linear_parameters()?
        } else {
            configure_parameters(spec.family)?
        };
        parameters.extend(schema);
    }
    Ok(parameters)
}

fn model_summary_parameters(family: Family) -> Result<Vec<Parameter>, BuiltinAssemblyError> {
    if matches!(family, Family::Logit | Family::Probit | Family::Prais) {
        let text_field = |key, default: &str| {
            parameter(
                key,
                concrete("core.text")?,
                ParameterEditorSpec::Text { multiline: false },
                DataValue::String(default.into()),
                vec![ParameterConstraint::Length {
                    min: None,
                    max: Some(4096),
                }],
            )
        };
        let mut parameters = vec![
            toggle_parameter("hypothesis_test", false)?,
            text_field("hypothesis", "x1 = 0")?,
        ];
        if family != Family::Prais {
            if family == Family::Logit {
                parameters.push(toggle_parameter("odds_ratios", true)?);
            }
            parameters.extend([
                toggle_parameter("marginal_effects", false)?,
                choice_parameter("marginal_evaluation", "average", &["average", "at_means"])?,
                choice_parameter("marginal_method", "dydx", &["dydx", "eyex", "eydx", "dyex"])?,
                text_field("marginal_at", "")?,
                toggle_parameter("classification", true)?,
                decimal_parameter("cutoff", "0.5")?,
            ]);
        }
        return Ok(parameters);
    }
    let toggles = match family {
        Family::Iv2sls | Family::IvLiml => {
            let defaults = yss_sci_contract::causal::iv::IvSummaryOptions::default();
            let mut toggles = vec![
                ("model_summary", defaults.model_summary),
                ("coefficient_table", defaults.coefficient_table),
                ("first_stage", defaults.first_stage),
                ("overidentification", defaults.overidentification),
            ];
            if family == Family::Iv2sls {
                toggles.push(("endogeneity", defaults.endogeneity));
            }
            toggles
        }
        Family::Panel => {
            let defaults = yss_sci_contract::panel::PanelSummaryOptions::default();
            vec![
                ("model_summary", defaults.model_summary),
                ("coefficient_table", defaults.coefficient_table),
                ("effects_statistics", defaults.effects_statistics),
                ("estimator_statistics", defaults.estimator_statistics),
            ]
        }
        Family::Var => {
            let defaults = yss_sci_contract::time_series::var::VarSummaryOptions::default();
            vec![
                ("model_summary", defaults.model_summary),
                ("coefficient_table", defaults.coefficient_table),
                ("lag_exclusion", defaults.lag_exclusion),
                ("serial_tests", defaults.serial_tests),
                ("stability", defaults.stability),
            ]
        }
        Family::Vec => {
            let defaults = yss_sci_contract::time_series::vec::VecSummaryOptions::default();
            vec![
                ("model_summary", defaults.model_summary),
                ("coefficient_table", defaults.coefficient_table),
                ("cointegration", defaults.cointegration),
                ("serial_tests", defaults.serial_tests),
                ("stability", defaults.stability),
            ]
        }
        _ => return Ok(vec![]),
    };
    let mut parameters = toggles
        .into_iter()
        .map(|(key, default)| toggle_parameter(key, default))
        .collect::<Result<Vec<_>, _>>()?;
    if matches!(family, Family::Iv2sls | Family::IvLiml) {
        parameters.push(toggle_parameter("hypothesis_test", false)?);
        parameters.push(parameter(
            "hypothesis",
            concrete("core.text")?,
            ParameterEditorSpec::Text { multiline: false },
            DataValue::String("x1 = 0".into()),
            vec![ParameterConstraint::Length {
                min: Some(1),
                max: Some(4096),
            }],
        )?);
    }
    let serial_lags = match family {
        Family::Var => {
            Some(yss_sci_contract::time_series::var::VarSummaryOptions::default().serial_lags)
        }
        Family::Vec => {
            Some(yss_sci_contract::time_series::vec::VecSummaryOptions::default().serial_lags)
        }
        _ => None,
    };
    if let Some(lags) = serial_lags {
        parameters.push(bounded_integer_parameter(
            "serial_lags",
            lags as i64,
            1,
            40,
        )?);
    }
    Ok(parameters)
}

fn linear_summary_parameters() -> Result<Vec<Parameter>, BuiltinAssemblyError> {
    let defaults = yss_sci_contract::regression::summary::LinearSummaryOptions::default();
    let mut parameters = vec![];
    for (key, enabled) in [
        ("model_summary", defaults.model_summary),
        ("coefficient_table", defaults.coefficient_table),
        ("coefficient_chart", defaults.coefficient_chart),
        ("equation", defaults.equation),
        ("anova", defaults.anova),
        ("diagnostics", defaults.diagnostics),
        ("residual_plot", defaults.residual_plot),
        ("observations", defaults.observations),
        ("acf_pacf", defaults.acf_pacf),
        ("serial_tests", defaults.serial_tests),
        ("hypothesis_test", defaults.hypothesis_test),
    ] {
        parameters.push(toggle_parameter(key, enabled)?);
    }
    let enabled = |key| -> Result<ParameterCondition, BuiltinAssemblyError> {
        Ok(ParameterCondition {
            key: sid(key, ParameterKey::new)?,
            values: vec![DataValue::Bool(true)].into_boxed_slice(),
        })
    };
    for (key, selector, default) in [
        ("acf_max_lag", "acf_pacf", defaults.acf_max_lag),
        ("serial_lags", "serial_tests", defaults.serial_lags),
    ] {
        let mut lag = positive_integer_parameter(key, default as i64)?;
        lag.constraints = vec![ParameterConstraint::IntegerRange {
            min: Some(1),
            max: Some(40),
        }];
        parameters.push(lag.when(enabled(selector)?));
    }
    parameters
        .push(toggle_parameter("bg_nomiss0", defaults.bg_nomiss0)?.when(enabled("serial_tests")?));
    parameters.push(
        parameter(
            "hypothesis",
            concrete("core.text")?,
            ParameterEditorSpec::Text { multiline: false },
            DataValue::String(defaults.hypothesis.into()),
            vec![
                ParameterConstraint::Required,
                ParameterConstraint::Length {
                    min: Some(1),
                    max: Some(4096),
                },
            ],
        )?
        .when(enabled("hypothesis_test")?),
    );
    Ok(parameters)
}

fn configure_parameters(family: Family) -> Result<Vec<Parameter>, BuiltinAssemblyError> {
    let mut parameters = vec![toggle_parameter("constant", true)?];
    match family {
        Family::Logit | Family::Probit => {
            parameters.push(positive_integer_parameter("max_iterations", 100)?);
            parameters.push(tolerance_parameter("0.00000001")?);
        }
        Family::Iv2sls | Family::IvLiml => {
            parameters.push(choice_parameter(
                "covariance",
                "nonrobust",
                &["nonrobust", "HC0", "HC1", "HC2", "HC3"],
            )?);
            parameters.push(toggle_parameter("small", false)?);
        }
        Family::Panel => {
            parameters.push(choice_parameter(
                "estimator",
                "fixed_effects",
                &[
                    "fixed_effects",
                    "lsdv",
                    "first_difference",
                    "random_effects",
                    "maximum_likelihood",
                    "between",
                ],
            )?);
            parameters.push(choice_parameter(
                "effects",
                "entity",
                &["entity", "time", "two_way"],
            )?);
            parameters.push(choice_parameter(
                "covariance",
                "nonrobust",
                &["nonrobust", "HC0", "HC1", "HC2", "HC3", "cluster"],
            )?);
        }
        Family::Prais => {
            parameters.push(choice_parameter(
                "transform",
                "prais_winsten",
                &["prais_winsten", "cochrane_orcutt"],
            )?);
            parameters.push(positive_integer_parameter("max_iterations", 100)?);
            parameters.push(tolerance_parameter("0.000001")?);
        }
        _ => {}
    }
    Ok(parameters)
}

fn linear_parameters() -> Result<Vec<Parameter>, BuiltinAssemblyError> {
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
    let conditional =
        |parameter: Parameter, covariance: &'static str| -> Result<_, BuiltinAssemblyError> {
            Ok(parameter.when(choice_condition("covariance", covariance)?))
        };
    let mut scale = decimal_parameter("scale", "1")?;
    scale.constraints.push(ParameterConstraint::Positive);
    Ok(vec![
        choice("method", "OLS", &["OLS", "WLS", "GLS"])?,
        toggle_parameter("constant", defaults.constant)?,
        choice(
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
                "cluster",
            ],
        )?,
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
    ])
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
        LiteralPolicy::Forbidden,
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
        LiteralPolicy::Forbidden,
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
        LiteralPolicy::Forbidden,
    )
}

fn choice_parameter(
    key: &'static str,
    default: &'static str,
    choices: &[&str],
) -> Result<Parameter, BuiltinAssemblyError> {
    let mut parameter = select_parameter(key, default)?;
    parameter.constraints.push(ParameterConstraint::OneOf(
        choices
            .iter()
            .map(|v| DataValue::String((*v).into()))
            .collect(),
    ));
    Ok(parameter)
}
fn alternative_parameter() -> Result<Parameter, BuiltinAssemblyError> {
    choice_parameter(
        "alternative",
        yss_sci_contract::hypothesis::Alternative::TwoSided.name(),
        &yss_sci_contract::hypothesis::Alternative::NAMES,
    )
}
fn nonnegative_integer_parameter(
    key: &'static str,
    default: i64,
) -> Result<Parameter, BuiltinAssemblyError> {
    let mut p = positive_integer_parameter(key, default)?;
    p.constraints = vec![ParameterConstraint::IntegerRange {
        min: Some(0),
        max: None,
    }];
    Ok(p)
}

fn minimum_integer_parameter(
    key: &'static str,
    default: i64,
    min: i64,
) -> Result<Parameter, BuiltinAssemblyError> {
    let mut parameter = positive_integer_parameter(key, default)?;
    parameter.constraints = vec![ParameterConstraint::IntegerRange {
        min: Some(min),
        max: None,
    }];
    Ok(parameter)
}

fn bounded_integer_parameter(
    key: &'static str,
    default: i64,
    min: i64,
    max: i64,
) -> Result<Parameter, BuiltinAssemblyError> {
    let mut parameter = positive_integer_parameter(key, default)?;
    parameter.constraints = vec![ParameterConstraint::IntegerRange {
        min: Some(min),
        max: Some(max),
    }];
    Ok(parameter)
}

fn tolerance_parameter(default: &'static str) -> Result<Parameter, BuiltinAssemblyError> {
    let mut parameter = decimal_parameter("tolerance", default)?;
    parameter.constraints.push(ParameterConstraint::Positive);
    Ok(parameter)
}

fn positive_integer_parameter(
    key: &'static str,
    default: i64,
) -> Result<Parameter, BuiltinAssemblyError> {
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
) -> Result<Parameter, BuiltinAssemblyError> {
    parameter(
        key,
        concrete("core.numeric")?,
        ParameterEditorSpec::Number,
        DataValue::Decimal(assembled_decimal("statistics.parameter", default)?),
        vec![],
    )
}
fn toggle_parameter(key: &'static str, default: bool) -> Result<Parameter, BuiltinAssemblyError> {
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
) -> Result<Parameter, BuiltinAssemblyError> {
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
) -> Result<Parameter, BuiltinAssemblyError> {
    Ok(Parameter {
        key: sid(key, ParameterKey::new)?,
        title_key: iid(format!("parameters.statistics.{key}.title"))?,
        description_key: Some(iid(format!("parameters.statistics.{key}.description"))?),
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
            "statistics.result.panel_did",
            "types.statistics_result_panel_did.title",
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
        ("statistics.model.cox", "types.statistics_model_cox.title"),
        (
            "statistics.design.spatial_weights",
            "types.statistics_spatial_weights.title",
        ),
        (
            "statistics.result.treatment_effect",
            "types.statistics_result_treatment_effect.title",
        ),
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
const CATEGORIES: &[&str] = &[
    "statistics.descriptive",
    "statistics.tests",
    "statistics.imputation",
    "statistics.association",
    "statistics.regression",
    "statistics.anova",
    "statistics.multivariate",
    "statistics.longitudinal",
    "statistics.panel",
    "statistics.causal",
    "statistics.timeseries",
    "statistics.survival",
    "statistics.spatial",
    "statistics.psychometrics",
    "statistics.decision",
    "statistics.machine_learning",
    "statistics.meta",
    "statistics.design_quality",
    "statistics.power",
    "statistics.survey",
    "statistics.inference",
    "statistics.diagnostics",
    "statistics.postestimation",
];

fn statistics_categories() -> Result<Vec<CategoryRegistration>, BuiltinAssemblyError> {
    let mut categories = vec![CategoryRegistration {
        id: sid("statistics", NodeCategoryId::new)?,
        title_key: iid("categories.statistics.title")?,
        parent: None,
        order: 70,
    }];
    for (index, &id) in CATEGORIES.iter().enumerate() {
        categories.push(CategoryRegistration {
            id: sid(id, NodeCategoryId::new)?,
            title_key: iid(format!("categories.{id}.title"))?,
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
fn model_type(family: Family) -> Result<TypeExpr, BuiltinAssemblyError> {
    let id = match family {
        Family::Linear => "statistics.model.linear",
        Family::Logit => "statistics.model.logit",
        Family::Probit => "statistics.model.probit",
        Family::Prais => "statistics.model.prais",
        Family::Vec => "statistics.model.vec",
        Family::Iv2sls => "statistics.model.iv_2sls",
        Family::IvLiml => "statistics.model.iv_liml",
        Family::Panel => "statistics.model.panel",
        Family::PanelDid => "statistics.result.panel_did",
        Family::Var => "statistics.model.var",
        Family::Adf | Family::VecRank => {
            return Err(BuiltinAssemblyError::UnsupportedBuiltinConfiguration {
                context: "statistics fit model family",
                value: format!("{family:?}").into(),
            });
        }
    };
    concrete(id)
}
fn prediction_model_type(family: Family) -> Result<TypeExpr, BuiltinAssemblyError> {
    if !matches!(
        family,
        Family::Linear | Family::Logit | Family::Probit | Family::Panel
    ) {
        return Err(
            BuiltinAssemblyError::UnsupportedStatisticsPredictionFamily {
                family: format!("{family:?}").into(),
            },
        );
    }
    model_type(family)
}
fn result_type(family: Family) -> Result<TypeExpr, BuiltinAssemblyError> {
    concrete(match family {
        Family::Adf => "statistics.result.adf",
        Family::Linear => "statistics.result.linear",
        Family::Logit => "statistics.result.logit",
        Family::Probit => "statistics.result.probit",
        Family::Prais => "statistics.result.prais",
        Family::PanelDid => "statistics.result.panel_did",
        Family::Iv2sls | Family::IvLiml | Family::Panel | Family::Var | Family::Vec => {
            "statistics.report"
        }
        Family::VecRank => "statistics.model.vec_rank",
    })
}
fn report_type() -> Result<TypeExpr, BuiltinAssemblyError> {
    concrete("statistics.report")
}
