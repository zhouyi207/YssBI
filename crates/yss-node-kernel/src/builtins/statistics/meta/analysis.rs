//! Pooling, moderator naming, sensitivity and plot dispatch.
use super::*;

pub(super) fn execute(
    method: &str,
    data: &[Vec<f64>],
    inv: &KernelInvocation<'_>,
    control: &Control,
) -> Result<Vec<RuntimeValue>, KernelError> {
    if method == "combine_p" {
        let stouffer = match text(inv, "p_method")? {
            "fisher" => false,
            "stouffer" => true,
            _ => return Err(KernelError::InvalidParameter),
        };
        return Ok(vec![value(
            sci::diagnostics::combine_p(
                &data[0],
                data.get(1).map(Vec::as_slice),
                stouffer,
                control,
            )
            .map_err(computation_error)?,
            inv,
        )?]);
    }
    let (y, v) = (&data[0], &data[1]);
    let options = options(inv)?;
    if matches!(method, "random_effect" | "tau_squared")
        && options.estimator == MetaEstimator::Fixed
    {
        return Err(KernelError::InvalidParameter);
    }
    let report = match method {
        "fixed_effect" | "random_effect" | "inverse_variance" | "regression" => {
            let mut fit =
                sci::model::fit(y, v, &data[2..], options, control).map_err(computation_error)?;
            for (j, term) in fit.summary.coefficients.iter_mut().skip(1).enumerate() {
                term.term = input_label(&inv.inputs[j + 2], format!("moderator{}", j + 1));
            }
            return Ok(vec![
                value(fit.summary, inv)?,
                output::studies(&fit.studies, inv)?,
            ]);
        }
        "cochran_q" | "i_squared" | "tau_squared" => value(
            sci::model::fit(y, v, &[], options, control)
                .map_err(computation_error)?
                .summary
                .heterogeneity,
            inv,
        )?,
        "egger" => value(
            sci::diagnostics::egger(y, v, options.confidence_level, control)
                .map_err(computation_error)?,
            inv,
        )?,
        "begg" => value(
            sci::diagnostics::begg(y, v, control).map_err(computation_error)?,
            inv,
        )?,
        "leave_one_out" => {
            let baseline = sci::model::fit(y, v, &[], options, control)
                .map_err(computation_error)?
                .summary;
            let rows = sci::diagnostics::leave_one_out(y, v, options, control)
                .map_err(computation_error)?;
            return Ok(vec![value(baseline, inv)?, output::omissions(&rows, inv)?]);
        }
        "sensitivity" => {
            let (summary, rows) =
                sci::diagnostics::sensitivity(y, v, options, control).map_err(computation_error)?;
            return Ok(vec![value(summary, inv)?, output::omissions(&rows, inv)?]);
        }
        "forest" => value(
            sci::plots::forest(y, v, options, boolean(inv, "exponentiate")?, control)
                .map_err(computation_error)?,
            inv,
        )?,
        "funnel" => value(
            sci::plots::funnel(y, v, options, control).map_err(computation_error)?,
            inv,
        )?,
        _ => return Err(KernelError::InvalidParameter),
    };
    Ok(vec![report])
}
