//! Observed-variable interaction/path interfaces; latent-variable SEM stays separate.
use super::*;
pub(super) fn append(fragment: &mut ProviderFragment) -> Result<(), BuiltinAssemblyError> {
    for (id, en, zh, advanced) in [
        (
            "yssbi.statistics.workflow.moderation",
            "Moderation (two-way interaction)",
            "调节作用（二阶交互）",
            false,
        ),
        (
            "yssbi.statistics.workflow.moderation_advanced",
            "Moderation (three-way interaction)",
            "调节作用（双调节变量三阶交互）",
            true,
        ),
    ] {
        let mut ports = vec![
            data_input("response", "Response Y", series_type()?)?,
            data_input("predictor", "Focal predictor X", series_type()?)?,
            data_input("moderator", "Moderator W", series_type()?)?,
        ];
        if advanced {
            ports.push(data_input(
                "second_moderator",
                "Second moderator Z",
                series_type()?,
            )?);
        }
        ports.extend([
            bounded_user_data_input("covariates", "Additive covariate", series_type()?, 0, None)?,
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
            en,
            zh,
            ports,
            vec![decimal_parameter("probe_sd", "1")?],
        )?;
    }
    Ok(())
}
