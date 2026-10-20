//! Observed-variable interaction/path interfaces; latent-variable SEM stays separate.
use super::*;
pub(super) fn append(fragment: &mut ProviderFragment) -> Result<(), BuiltinAssemblyError> {
    for (id, advanced) in [
        ("yssbi.statistics.workflow.moderation", false),
        ("yssbi.statistics.workflow.moderation_advanced", true),
    ] {
        let mut ports = vec![
            data_input("y", "Y", numeric_data_series_type())?,
            data_input("x", "X", numeric_data_series_type())?,
            data_input("moderator", "Moderator W", numeric_data_series_type())?,
        ];
        if advanced {
            ports.push(data_input(
                "second_moderator",
                "Second moderator Z",
                numeric_data_series_type(),
            )?);
        }
        ports.extend([
            bounded_user_data_input(
                "covariates",
                "Additive covariate",
                numeric_data_series_type(),
                0,
                None,
            )?,
            data_output(
                "result",
                "Interaction and conditional effects",
                report_type()?,
            )?,
            fixed_numeric_table(
                "observations",
                "Observed and fitted responses",
                &["observation", "response", "fitted", "residual"],
            )?,
        ]);
        append_node(
            fragment,
            id,
            ports,
            vec![decimal_parameter("probe_sd", "1")?],
        )?;
    }
    Ok(())
}
