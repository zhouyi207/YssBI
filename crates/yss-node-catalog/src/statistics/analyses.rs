//! Executable diagnostics and post-estimation protocols.
use super::*;
mod models;

const SPECS: &[(&str, &str, &str)] = &[
    (
        "yssbi.statistics.diagnostic.breusch_pagan",
        "statistics.diagnostics",
        "linear",
    ),
    (
        "yssbi.statistics.diagnostic.white",
        "statistics.diagnostics",
        "linear",
    ),
    (
        "yssbi.statistics.diagnostic.information_matrix",
        "statistics.diagnostics",
        "linear",
    ),
    (
        "yssbi.statistics.diagnostic.reset",
        "statistics.diagnostics",
        "linear",
    ),
    (
        "yssbi.statistics.diagnostic.vif",
        "statistics.diagnostics",
        "linear",
    ),
    (
        "yssbi.statistics.diagnostic.leverage",
        "statistics.diagnostics",
        "linear",
    ),
    (
        "yssbi.statistics.diagnostic.breusch_godfrey",
        "statistics.diagnostics",
        "linear",
    ),
    (
        "yssbi.statistics.diagnostic.wald",
        "statistics.diagnostics",
        "linear",
    ),
    (
        "yssbi.statistics.test.normality",
        "statistics.tests",
        "series",
    ),
    (
        "yssbi.statistics.diagnostic.durbin_watson",
        "statistics.diagnostics",
        "series",
    ),
    (
        "yssbi.statistics.diagnostic.ljung_box",
        "statistics.diagnostics",
        "series",
    ),
    (
        "yssbi.statistics.timeseries.acf",
        "statistics.timeseries",
        "series",
    ),
    (
        "yssbi.statistics.timeseries.pacf",
        "statistics.timeseries",
        "series",
    ),
    (
        "yssbi.statistics.timeseries.granger",
        "statistics.timeseries",
        "var",
    ),
    (
        "yssbi.statistics.timeseries.irf",
        "statistics.timeseries",
        "var",
    ),
    (
        "yssbi.statistics.timeseries.fevd",
        "statistics.timeseries",
        "var",
    ),
    (
        "yssbi.statistics.diagnostic.hausman",
        "statistics.diagnostics",
        "iv_2sls",
    ),
];

pub(super) fn append(fragment: &mut ProviderFragment) -> Result<(), BuiltinAssemblyError> {
    models::append(fragment)?;
    for &(id, category, input) in SPECS {
        let port = if input == "series" {
            data_input("series", "DataSeries", numeric_data_series_type())?
        } else {
            let model_type = match input {
                "linear" => "statistics.model.linear",
                "var" => "statistics.model.var",
                _ => "statistics.model.iv_2sls",
            };
            data_input("model", "Model", concrete(model_type)?)?
        };
        let parameters = if id.ends_with("breusch_pagan") {
            vec![
                toggle_parameter("rhs", false)?,
                toggle_parameter("koenker", false)?,
            ]
        } else if id.ends_with("reset") {
            vec![toggle_parameter("rhs", false)?]
        } else if id.ends_with("breusch_godfrey") {
            vec![lag()?, toggle_parameter("bg_nomiss0", true)?]
        } else if id.ends_with("wald") {
            vec![parameter(
                "hypothesis",
                concrete("core.text")?,
                ParameterEditorSpec::Text { multiline: false },
                DataValue::String("x1 = 0".into()),
                vec![
                    ParameterConstraint::Required,
                    ParameterConstraint::Length {
                        min: Some(1),
                        max: Some(4096),
                    },
                ],
            )?]
        } else if id.ends_with("irf") || id.ends_with("fevd") {
            vec![bounded_integer_parameter("steps", 8, 1, 1000)?]
        } else if id.ends_with("acf") || id.ends_with("ljung_box") {
            vec![lag()?]
        } else {
            vec![]
        };
        fragment.nodes.push(leaf(
            NodeProtocol {
                type_id: sid(id, NodeTypeId::new)?,
                catalog: NodeCatalogProtocol {
                    title_key: node_key(id, "title")?,
                    aliases_key: None,
                    category_id: sid(category, NodeCategoryId::new)?,
                    icon_id: sid("builtin.statistics", IconId::new)?,
                    style_id: sid("builtin.dataframe", NodeStyleId::new)?,
                    hidden: false,
                },
                interface: assembled_interface(
                    id,
                    vec![port, data_output("result", "Result", report_type()?)?],
                    vec![],
                    vec![],
                )?,
                parameters: assembled_parameters(id, parameters)?,
                instance_display: NodeInstanceDisplaySpec::Static,
                execution: pure(),
                typing: NodeTypingSpec::Fixed,
                scope: NodeScope::Any,
                managed_role: None,
            },
            id,
        ));
    }
    Ok(())
}
fn lag() -> Result<Parameter, BuiltinAssemblyError> {
    let mut parameter = positive_integer_parameter("lags", 1)?;
    parameter.constraints = vec![ParameterConstraint::IntegerRange {
        min: Some(1),
        max: Some(40),
    }];
    Ok(parameter)
}
