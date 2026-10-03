use super::*;
mod labels;
pub(super) fn build(
    fragment: &mut ProviderFragment,
    id: &str,
    method: &str,
) -> Result<Vec<Parameter>, BuiltinAssemblyError> {
    let mut params = vec![
        choice_parameter("solve_for", "power", &["power", "sample_size"])?,
        positive_integer_parameter("sample_size", 100)?,
        decimal_parameter("target_power", "0.8")?,
        decimal_parameter("alpha", "0.05")?,
    ];
    if !matches!(
        method,
        "anova" | "linear_regression" | "noninferiority" | "equivalence"
    ) {
        params.push(choice_parameter(
            "alternative",
            "two_sided",
            &["two_sided", "greater", "less"],
        )?);
    }
    let decimals: &[(&str, &str)] = match method {
        "principles" | "mean_difference" | "paired" | "cluster_randomized" => {
            &[("effect_size", "0.5")]
        }
        "variance" => &[("variance_ratio", "1.5")],
        "proportion" => &[("null_proportion", "0.5"), ("proportion", "0.6")],
        "proportion_difference" => &[("proportion1", "0.6"), ("proportion2", "0.4")],
        "correlation" => &[("null_correlation", "0"), ("correlation", "0.3")],
        "anova" => &[("effect_f", "0.25")],
        "linear_regression" => &[("effect_f_squared", "0.15")],
        "generalized_model" => &[
            ("baseline_rate", "1"),
            ("rate_ratio", "1.5"),
            ("exposure", "1"),
        ],
        "logistic" => &[("baseline_probability", "0.2"), ("odds_ratio", "1.5")],
        "cox" | "logrank" => &[("hazard_ratio", "0.7"), ("event_fraction", "0.5")],
        "noninferiority" | "equivalence" => &[("difference", "0"), ("margin", "0.3")],
        _ => &[],
    };
    for &(key, default) in decimals {
        params.push(decimal_parameter(key, default)?);
    }
    match method {
        "mean_difference" => params.push(choice_parameter(
            "design",
            "independent",
            &["independent", "one_sample"],
        )?),
        "anova" => params.push(minimum_integer_parameter("groups", 3, 2)?),
        "linear_regression" => params.push(positive_integer_parameter("predictors", 3)?),
        "cox" => params.push(decimal_parameter("predictor_variance", "0.25")?),
        "logrank" => params.push(decimal_parameter("allocation", "0.5")?),
        "cluster_randomized" => params.extend([
            positive_integer_parameter("cluster_size", 20)?,
            decimal_parameter("icc", "0.05")?,
        ]),
        "noninferiority" => params.push(toggle_parameter("higher_is_better", true)?),
        _ => (),
    }
    labels::localize(fragment, id, &mut params)?;
    Ok(params)
}
