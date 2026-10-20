//! Executable definitions for the existing econometric/causal inventory entries.
use super::*;

const METHODS: &[&str] = &[
    "econometrics.gmm",
    "causal.rdd",
    "causal.psm",
    "econometrics.heckman_two_step",
    "test.heterogeneity",
    "econometrics.sfa",
    "econometrics.sur",
    "causal.ipw",
    "causal.regression_adjustment",
    "causal.aipw",
    "causal.ate",
    "causal.att",
    "causal.synthetic_control",
];

pub(super) fn append(fragment: &mut ProviderFragment) -> Result<(), BuiltinAssemblyError> {
    for &method in METHODS {
        let id = format!("yssbi.statistics.{method}");
        let effect = matches!(
            method,
            "causal.psm" | "causal.ipw" | "causal.regression_adjustment" | "causal.aipw"
        );
        let projection = matches!(method, "causal.ate" | "causal.att");
        let repeated = |key, label, min| {
            bounded_user_data_input(key, label, numeric_data_series_type(), min, None)
        };
        let mut ports = if projection {
            vec![data_input(
                "effects",
                "Treatment effects",
                concrete("statistics.result.treatment_effect")?,
            )?]
        } else if method == "econometrics.sur" {
            vec![repeated("y", "Y", 2)?, repeated("x", "X", 0)?]
        } else {
            vec![data_input("y", "Y", numeric_data_series_type())?]
        };
        if effect || method == "test.heterogeneity" {
            ports.push(data_input(
                "treatment",
                "Treatment",
                numeric_or_binary_series()?,
            )?);
        }
        if method == "test.heterogeneity" {
            ports.push(data_input("groups", "Group", label_series()?)?);
        }
        if method == "causal.rdd" {
            ports.push(data_input(
                "running",
                "Running variable",
                numeric_data_series_type(),
            )?);
        }
        if method == "econometrics.heckman_two_step" {
            ports.push(data_input(
                "selected",
                "Selected",
                numeric_or_binary_series()?,
            )?);
        }
        if !projection
            && !matches!(
                method,
                "econometrics.sur" | "causal.rdd" | "causal.synthetic_control"
            )
        {
            ports.push(repeated(
                "x",
                "X",
                if method == "econometrics.gmm" { 1 } else { 0 },
            )?);
        }
        match method {
            "econometrics.gmm" => ports.push(repeated("instruments", "Instrument", 1)?),
            "econometrics.heckman_two_step" => {
                ports.push(repeated("selection_predictors", "Selection predictor", 1)?)
            }
            "causal.synthetic_control" => ports.push(repeated("donors", "Donor outcome", 1)?),
            _ => {}
        }
        ports.push(data_output(
            "result",
            "Result",
            if effect {
                concrete("statistics.result.treatment_effect")?
            } else {
                report_type()?
            },
        )?);
        let mut parameters = vec![];
        if matches!(
            method,
            "econometrics.gmm" | "econometrics.sur" | "econometrics.sfa"
        ) {
            parameters.push(toggle_parameter("constant", true)?);
        }
        match method {
            "econometrics.gmm" => parameters.push(choice_parameter(
                "gmm_steps",
                "two_step",
                &["one_step", "two_step"],
            )?),
            "causal.rdd" => parameters.extend([
                decimal_parameter("rdd_cutoff", "0")?,
                decimal_parameter("rdd_bandwidth", "1")?,
                choice_parameter("rdd_kernel", "triangular", &["triangular", "uniform"])?,
            ]),
            "econometrics.sfa" => parameters.push(choice_parameter(
                "frontier_type",
                "production",
                &["production", "cost"],
            )?),
            "econometrics.sur" => parameters.push(parameter(
                "equation_predictors",
                concrete("core.text")?,
                ParameterEditorSpec::Text { multiline: false },
                DataValue::String("".into()),
                vec![],
            )?),
            "causal.synthetic_control" => {
                parameters.push(minimum_integer_parameter("pre_periods", 10, 1)?)
            }
            _ => {}
        }
        if matches!(method, "causal.psm" | "causal.ipw" | "causal.aipw") {
            parameters.push(decimal_parameter("ps_overlap", "0.000001")?);
        }
        if method == "causal.psm" {
            parameters.push(decimal_parameter("ps_caliper", "0.2")?);
        }
        if matches!(
            method,
            "causal.ipw"
                | "causal.aipw"
                | "causal.regression_adjustment"
                | "econometrics.heckman_two_step"
        ) {
            parameters.extend([
                nonnegative_integer_parameter("bootstrap_replications", 0)?,
                nonnegative_integer_parameter("seed", 42)?,
            ]);
        }
        if matches!(
            method,
            "causal.psm"
                | "causal.ipw"
                | "causal.aipw"
                | "econometrics.heckman_two_step"
                | "econometrics.sfa"
                | "causal.synthetic_control"
        ) {
            parameters.extend([
                minimum_integer_parameter(
                    "max_iterations",
                    if method == "causal.synthetic_control" {
                        5000
                    } else {
                        500
                    },
                    1,
                )?,
                tolerance_parameter("0.0000001")?,
            ]);
        }
        fragment.nodes.push(leaf(
            NodeProtocol {
                type_id: sid(id.as_str(), NodeTypeId::new)?,
                catalog: NodeCatalogProtocol {
                    title_key: node_key(&id, "title")?,
                    aliases_key: Some(node_key(&id, "aliases")?),
                    category_id: sid("statistics.causal", NodeCategoryId::new)?,
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
