//! Additional model diagnostics with explicit fitted-model or aligned-series inputs.
use super::super::*;

const METHODS: &[&str] = &[
    "collinearity",
    "nri_idi",
    "harman",
    "residual",
    "cooks_distance",
    "aic",
    "bic",
    "lr",
    "score_lm",
    "nested_comparison",
    "ph",
];
pub(super) fn append(fragment: &mut ProviderFragment) -> Result<(), BuiltinAssemblyError> {
    for &method in METHODS {
        let id = format!("yssbi.statistics.diagnostic.{method}");
        let mut ports = match method {
            "collinearity" | "harman" => vec![bounded_user_data_input(
                "variables",
                "Variable",
                numeric_data_series_type(),
                if method == "harman" { 2 } else { 1 },
                None,
            )?],
            "nri_idi" => vec![
                data_input("outcome", "Binary outcome", numeric_or_binary_series()?)?,
                data_input(
                    "reference",
                    "Reference probability",
                    numeric_data_series_type(),
                )?,
                data_input("new", "New probability", numeric_data_series_type())?,
            ],
            "ph" => vec![
                data_input("time", "Time", numeric_data_series_type())?,
                data_input("event", "Event", numeric_or_binary_series()?)?,
                bounded_user_data_input("x", "X", numeric_data_series_type(), 1, None)?,
            ],
            "lr" | "score_lm" | "nested_comparison" => vec![
                data_input("restricted", "Restricted model", fitted_regression_type()?)?,
                data_input("full", "Full model", fitted_regression_type()?)?,
            ],
            "residual" | "cooks_distance" => vec![data_input(
                "model",
                "Model",
                concrete("statistics.model.linear")?,
            )?],
            _ => vec![data_input("model", "Model", fitted_regression_type()?)?],
        };
        ports.push(data_output("result", "Result", report_type()?)?);
        if matches!(method, "residual" | "cooks_distance") {
            ports.push(fixed_numeric_table(
                "observations",
                "Observation diagnostics",
                &[
                    "observation",
                    "fitted",
                    "residual",
                    "weighted_residual",
                    "leverage",
                    "standardized_residual",
                    "studentized_residual",
                    "cooks_distance",
                ],
            )?);
        }
        let mut parameters = match method {
            "collinearity" => vec![toggle_parameter("constant", true)?],
            "nri_idi" => vec![
                choice_parameter("nri_mode", "continuous", &["continuous", "categorical"])?,
                parameter(
                    "risk_thresholds",
                    numeric_data_series_type(),
                    ParameterEditorSpec::Auto,
                    DataValue::List(vec![]),
                    vec![],
                )?,
            ],
            "ph" => vec![
                choice_parameter("survival_ties", "efron", &["efron", "breslow"])?,
                choice_parameter("time_transform", "rank", &["rank", "log", "identity"])?,
                positive_integer_parameter("max_iterations", 500)?,
                tolerance_parameter("0.0000001")?,
            ],
            _ => vec![],
        };
        node_scoped_keys(&id, &mut parameters)?;
        fragment.nodes.push(leaf(
            NodeProtocol {
                type_id: sid(id.as_str(), NodeTypeId::new)?,
                catalog: NodeCatalogProtocol {
                    title_key: node_key(&id, "title")?,
                    aliases_key: None,
                    category_id: sid("statistics.diagnostics", NodeCategoryId::new)?,
                    icon_id: sid("builtin.statistics", IconId::new)?,
                    style_id: sid("builtin.dataframe", NodeStyleId::new)?,
                    hidden: false,
                },
                interface: assembled_interface(&id, ports, vec![], vec![])?,
                parameters: assembled_parameters(&id, parameters)?,
                instance_display: NodeInstanceDisplaySpec::Static,
                execution: pure(),
                typing: NodeTypingSpec::Fixed,
                scope: NodeScope::Any,
                managed_role: None,
            },
            &id,
        ));
    }
    Ok(())
}
