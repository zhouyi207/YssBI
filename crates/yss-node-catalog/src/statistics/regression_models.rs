//! Executable estimators and model-building analyses in the regression category.
use super::*;
const SPECS: &[&str] = &[
    "regression.robust",
    "regression.hierarchical",
    "regression.stepwise",
    "regression.curve",
    "regression.nonlinear",
    "regression.nonlinear_formula",
    "regression.ridge",
    "regression.lasso",
    "regression.pls",
    "regression.logit.multinomial",
    "regression.logit.ordinal",
    "regression.logit.firth",
    "regression.poisson",
    "regression.negative_binomial",
    "regression.zero_inflated_poisson",
    "regression.zero_inflated_negative_binomial",
    "regression.tobit",
    "regression.logit.conditional",
    "regression.deming",
    "regression.quantile",
    "workflow.regression.univariate_multivariable",
    "workflow.regression.grouped",
    "workflow.regression.baseline",
    "regression.threshold",
    "transform.rcs",
    "regression.glm",
    "regression.gamma",
    "regression.inverse_gaussian",
    "regression.cloglog",
    "regression.beta",
    "regression.fractional_response",
];
pub(super) fn append(fragment: &mut ProviderFragment) -> Result<(), BuiltinAssemblyError> {
    for &method in SPECS {
        let id = format!("yssbi.statistics.{method}");
        let (ports, parameters) = interface(method)?;
        let mut parameters = parameters;
        node_scoped_keys(&id, &mut parameters)?;
        fragment.nodes.push(leaf(
            NodeProtocol {
                type_id: sid(id.as_str(), NodeTypeId::new)?,
                catalog: NodeCatalogProtocol {
                    title_key: node_key(&id, "title")?,
                    aliases_key: Some(node_key(&id, "aliases")?),
                    category_id: sid("statistics.regression", NodeCategoryId::new)?,
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
fn list(key: &'static str, default: &[i64]) -> Result<Parameter, BuiltinAssemblyError> {
    parameter(
        key,
        numeric_data_series_type(),
        ParameterEditorSpec::Auto,
        DataValue::List(default.iter().copied().map(DataValue::Integer).collect()),
        vec![],
    )
}
fn text_field(key: &'static str, default: &'static str) -> Result<Parameter, BuiltinAssemblyError> {
    parameter(
        key,
        concrete("core.text")?,
        ParameterEditorSpec::Text { multiline: true },
        DataValue::String(default.into()),
        vec![],
    )
}
fn iteration(max: i64) -> Result<Vec<Parameter>, BuiltinAssemblyError> {
    Ok(vec![
        positive_integer_parameter("max_iterations", max)?,
        decimal_parameter("tolerance", "0.0000001")?,
    ])
}
fn union_series(ids: &[&'static str]) -> Result<TypeExpr, BuiltinAssemblyError> {
    normalize_type_expr(TypeExpr::Union(
        ids.iter()
            .map(|id| concrete(id).map(data_series_type))
            .collect::<Result<Vec<_>, _>>()?,
    ))
    .map_err(|e| BuiltinAssemblyError::UnsupportedBuiltinConfiguration {
        context: "regression series",
        value: e.to_string().into(),
    })
}
fn interface(method: &str) -> Result<(Vec<PortSpec>, Vec<Parameter>), BuiltinAssemblyError> {
    let response = match method {
        "regression.logit.multinomial" => label_series()?,
        "regression.logit.ordinal" => union_series(&["core.numeric", "core.ordinal"])?,
        "regression.logit.firth"
        | "regression.logit.conditional"
        | "regression.cloglog"
        | "regression.glm" => numeric_or_binary_series()?,
        _ => numeric_data_series_type(),
    };
    let mut ports = vec![data_input("y", "Y", response)?];
    if matches!(
        method,
        "regression.curve" | "regression.nonlinear" | "regression.deming" | "transform.rcs"
    ) {
        ports.push(data_input("x", "X", numeric_data_series_type())?);
    } else {
        ports.push(bounded_user_data_input(
            "x",
            "X",
            numeric_data_series_type(),
            1,
            None,
        )?);
    }
    if matches!(
        method,
        "regression.logit.conditional" | "workflow.regression.grouped"
    ) {
        ports.push(data_input("groups", "Group", label_series()?)?);
    }
    if method == "regression.threshold" {
        ports.push(data_input(
            "threshold_variable",
            "Threshold variable",
            numeric_data_series_type(),
        )?);
    }
    if matches!(
        method,
        "regression.zero_inflated_poisson" | "regression.zero_inflated_negative_binomial"
    ) {
        ports.push(bounded_user_data_input(
            "inflation_predictors",
            "Inflation predictor",
            numeric_data_series_type(),
            0,
            None,
        )?);
    }
    ports.push(data_output("result", "Result", report_type()?)?);
    let mut params = vec![];
    if !matches!(
        method,
        "regression.logit.ordinal"
            | "regression.logit.conditional"
            | "regression.curve"
            | "regression.nonlinear"
            | "regression.nonlinear_formula"
            | "regression.deming"
            | "regression.pls"
            | "transform.rcs"
    ) {
        params.push(toggle_parameter("constant", true)?);
    }
    match method {
        "regression.robust" => {
            params.extend([
                choice_parameter("robust_loss", "huber", &["huber", "tukey"])?,
                decimal_parameter("tuning", "1.345")?,
            ]);
            params.extend(iteration(500)?);
        }
        "regression.ridge" | "regression.lasso" => {
            params.extend([
                decimal_parameter("lambda", "1")?,
                toggle_parameter("standardize", true)?,
            ]);
            if method.ends_with("lasso") {
                params.extend(iteration(5000)?);
            }
        }
        "regression.pls" => params.extend([
            positive_integer_parameter("components", 1)?,
            toggle_parameter("standardize", true)?,
        ]),
        "regression.curve" => params.extend([
            choice_parameter(
                "curve_family",
                "polynomial",
                &[
                    "polynomial",
                    "logarithmic",
                    "inverse",
                    "exponential",
                    "power",
                ],
            )?,
            bounded_integer_parameter("degree", 2, 1, 8)?
                .when(choice_condition("curve_family", "polynomial")?),
        ]),
        "regression.nonlinear" => {
            params.extend([
                choice_parameter(
                    "nonlinear_family",
                    "exponential",
                    &["exponential", "logistic", "michaelis_menten", "gompertz"],
                )?,
                list("initial_values", &[])?,
            ]);
            params.extend(iteration(500)?);
        }
        "regression.nonlinear_formula" => {
            params.extend([
                text_field("formula", "b1 + b2*x1")?,
                list("initial_values", &[0, 1])?,
                list("lower_bounds", &[])?,
                list("upper_bounds", &[])?,
            ]);
            params.extend(iteration(500)?);
        }
        "regression.logit.multinomial"
        | "regression.logit.ordinal"
        | "regression.logit.firth"
        | "regression.logit.conditional"
        | "regression.poisson"
        | "regression.negative_binomial"
        | "regression.zero_inflated_poisson"
        | "regression.zero_inflated_negative_binomial"
        | "regression.gamma"
        | "regression.inverse_gaussian"
        | "regression.cloglog"
        | "regression.beta" => params.extend(iteration(500)?),
        "regression.glm" => {
            params.extend([
                choice_parameter(
                    "glm_family",
                    "poisson",
                    &[
                        "gaussian",
                        "binomial",
                        "poisson",
                        "gamma",
                        "inverse_gaussian",
                    ],
                )?,
                choice_parameter("gaussian_link", "identity", &["identity", "log"])?
                    .when(choice_condition("glm_family", "gaussian")?),
                choice_parameter("binomial_link", "logit", &["logit", "probit", "cloglog"])?
                    .when(choice_condition("glm_family", "binomial")?),
            ]);
            params.extend(iteration(500)?);
        }
        "regression.fractional_response" => {
            params.push(choice_parameter(
                "response_link",
                "logit",
                &["logit", "probit", "cloglog"],
            )?);
            params.extend(iteration(500)?);
        }
        "regression.tobit" => {
            params.extend([
                choice_parameter("censoring", "left", &["left", "both"])?,
                decimal_parameter("lower", "0")?,
                decimal_parameter("upper", "1")?.when(choice_condition("censoring", "both")?),
            ]);
            params.extend(iteration(500)?);
        }
        "regression.deming" => params.push(decimal_parameter("variance_ratio", "1")?),
        "regression.quantile" => {
            params.push(decimal_parameter("quantile", "0.5")?);
            params.extend(iteration(5000)?);
        }
        "regression.hierarchical" => params.push(list("block_sizes", &[])?),
        "regression.stepwise" => params.extend([
            choice_parameter("direction", "both", &["forward", "backward", "both"])?,
            choice_parameter("criterion", "aic", &["aic", "bic"])?,
        ]),
        "regression.threshold" => params.extend([
            decimal_parameter("trimming", "0.15")?,
            bounded_integer_parameter("max_candidates", 100, 1, 200)?,
        ]),
        "transform.rcs" => params.extend([
            choice_parameter("knot_mode", "auto", &["auto", "manual"])?,
            bounded_integer_parameter("knot_count", 4, 3, 8)?
                .when(choice_condition("knot_mode", "auto")?),
            list("knots", &[])?.when(choice_condition("knot_mode", "manual")?),
        ]),
        "workflow.regression.univariate_multivariable"
        | "workflow.regression.grouped"
        | "workflow.regression.baseline" => {}
        _ => unreachable!("known regression method"),
    }
    Ok((ports, params))
}
