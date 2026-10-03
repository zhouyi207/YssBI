use super::*;
pub(super) fn append(fragment: &mut ProviderFragment) -> Result<(), BuiltinAssemblyError> {
    for (id, en, zh, moderated) in [
        (
            "yssbi.statistics.workflow.mediation",
            "Mediation (one observed mediator)",
            "中介作用（单一观测中介）",
            false,
        ),
        (
            "yssbi.statistics.workflow.moderated_mediation",
            "Moderated mediation (one moderated stage)",
            "有调节的中介作用（单阶段调节）",
            true,
        ),
    ] {
        let mut ports = vec![
            data_input("response", "Response Y", series_type()?)?,
            data_input("predictor", "Predictor X", series_type()?)?,
            data_input("mediator", "Mediator M", series_type()?)?,
        ];
        let mut params = vec![
            nonnegative_integer_parameter("replications", 1000)?,
            nonnegative_integer_parameter("seed", 42)?,
        ];
        if moderated {
            ports.push(data_input("moderator", "Moderator W", series_type()?)?);
            params.extend([
                choice_parameter("stage", "first", &["first", "second"])?,
                decimal_parameter("probe_sd", "1")?,
            ]);
        }
        ports.extend([
            bounded_user_data_input("covariates", "Additive covariate", series_type()?, 0, None)?,
            data_output("result", "Mediation effects and equations", report_type()?)?,
            fixed_numeric_table(
                "observations",
                "Outcome and mediator fits",
                &[
                    "observation",
                    "response",
                    "fitted",
                    "residual",
                    "mediator",
                    "mediator_fitted",
                    "mediator_residual",
                ],
            )?,
        ]);
        append_node(fragment, id, en, zh, ports, params)?;
    }
    Ok(())
}
