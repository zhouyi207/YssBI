//! Executable survival-analysis inventory entries.
use super::*;

const METHODS: &[&str] = &[
    "survival.kaplan_meier",
    "survival.nelson_aalen",
    "survival.logrank",
    "survival.cox",
    "survival.exponential",
    "survival.weibull",
    "survival.lognormal",
    "survival.loglogistic",
    "survival.aft",
    "survival.competing_risks",
    "survival.time_dependent_cox",
    "workflow.subgroup",
    "plot.nomogram",
    "plot.calibration",
    "plot.decision_curve",
];
pub(super) fn append(fragment: &mut ProviderFragment) -> Result<(), BuiltinAssemblyError> {
    for &method in METHODS {
        let id = format!("yssbi.statistics.{method}");
        let curve = matches!(method, "survival.kaplan_meier" | "survival.nelson_aalen");
        let cox = matches!(
            method,
            "survival.cox" | "survival.time_dependent_cox" | "workflow.subgroup"
        );
        let parametric = matches!(
            method,
            "survival.exponential"
                | "survival.weibull"
                | "survival.lognormal"
                | "survival.loglogistic"
                | "survival.aft"
        );
        let evaluation = matches!(method, "plot.calibration" | "plot.decision_curve");
        let mut ports = if method == "plot.nomogram" {
            vec![data_input(
                "model",
                "Cox model",
                concrete("statistics.model.cox")?,
            )?]
        } else {
            let mut ports = vec![];
            if method == "survival.time_dependent_cox" {
                ports.push(data_input(
                    "start",
                    "Interval start",
                    numeric_data_series_type(),
                )?);
            }
            ports.push(data_input(
                if method == "survival.time_dependent_cox" {
                    "stop"
                } else {
                    "time"
                },
                "Observed time",
                numeric_data_series_type(),
            )?);
            ports.push(data_input(
                if method == "survival.competing_risks" {
                    "status"
                } else {
                    "event"
                },
                "Event status",
                if method == "survival.competing_risks" {
                    numeric_data_series_type()
                } else {
                    numeric_or_binary_series()?
                },
            )?);
            ports
        };
        if curve {
            ports.push(bounded_user_data_input(
                "groups",
                "Group (optional)",
                label_series()?,
                0,
                Some(1),
            )?);
        }
        if method == "workflow.subgroup" {
            ports.push(data_input(
                "treatment",
                "Treatment",
                numeric_or_binary_series()?,
            )?);
        }
        if matches!(method, "workflow.subgroup" | "survival.logrank") {
            ports.push(data_input("groups", "Group", label_series()?)?);
        }
        if method == "survival.time_dependent_cox" {
            ports.push(data_input(
                "subjects",
                "Subject identifier",
                label_series()?,
            )?);
        }
        if cox || parametric {
            ports.push(bounded_user_data_input(
                "x",
                "X",
                numeric_data_series_type(),
                if cox && method != "workflow.subgroup" {
                    1
                } else {
                    0
                },
                None,
            )?);
        }
        if evaluation {
            ports.push(data_input(
                "predicted_risk",
                "Predicted event probability",
                numeric_data_series_type(),
            )?);
        }
        ports.push(data_output(
            "result",
            "Result",
            if method == "survival.cox" {
                concrete("statistics.model.cox")?
            } else if method.starts_with("plot.") {
                concrete("plot.data")?
            } else {
                report_type()?
            },
        )?);
        if method == "survival.cox" || parametric {
            ports.push(fixed_numeric_table(
                "predictions",
                "Time, event and predicted risk",
                &["time", "event", "risk"],
            )?);
        }
        let mut parameters = vec![];
        if cox {
            parameters.push(choice_parameter(
                "survival_ties",
                "efron",
                &["efron", "breslow"],
            )?);
        }
        if cox || parametric {
            parameters.extend([
                minimum_integer_parameter("max_iterations", 500, 1)?,
                tolerance_parameter("0.0000001")?,
            ]);
        }
        if method == "survival.cox" || parametric || method.starts_with("plot.") {
            parameters.push(decimal_parameter("survival_horizon", "1")?);
        }
        match method {
            "survival.aft" => parameters.push(choice_parameter(
                "aft_distribution",
                "weibull",
                &["exponential", "weibull", "lognormal", "loglogistic"],
            )?),
            "plot.nomogram" => parameters.push(minimum_integer_parameter("nomogram_ticks", 5, 2)?),
            "plot.calibration" => {
                parameters.push(minimum_integer_parameter("calibration_bins", 10, 2)?)
            }
            "plot.decision_curve" => parameters.extend([
                decimal_parameter("decision_threshold_min", "0.01")?,
                decimal_parameter("decision_threshold_max", "0.99")?,
                minimum_integer_parameter("decision_points", 99, 2)?,
            ]),
            _ => {}
        }
        fragment.nodes.push(leaf(
            NodeProtocol {
                type_id: sid(id.as_str(), NodeTypeId::new)?,
                catalog: NodeCatalogProtocol {
                    title_key: node_key(&id, "title")?,
                    aliases_key: Some(node_key(&id, "aliases")?),
                    category_id: sid("statistics.survival", NodeCategoryId::new)?,
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
