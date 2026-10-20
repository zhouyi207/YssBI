use super::*;
pub(super) fn build(
    id: &str,
    method: &str,
    regression: bool,
) -> Result<Vec<Parameter>, BuiltinAssemblyError> {
    let mut parameters = if method == "weights" {
        vec![choice_parameter(
            "input_kind",
            "weights",
            &["weights", "inclusion_probabilities"],
        )?]
    } else {
        vec![choice_parameter(
            "lonely_psu",
            "fail",
            &["fail", "certainty"],
        )?]
    };
    if regression {
        parameters.extend([
            toggle_parameter("constant", true)?,
            positive_integer_parameter("max_iterations", 500)?,
            tolerance_parameter("0.0000001")?,
        ]);
    } else if method != "weights" {
        parameters.push(choice_parameter(
            "statistic",
            "mean",
            &["mean", "proportion"],
        )?);
    }
    node_scoped_keys(id, &mut parameters)?;
    Ok(parameters)
}
