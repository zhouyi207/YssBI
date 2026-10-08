use super::*;
use yss_sci_contract::path::{MediatedStage, MediationOptions};
pub(super) fn register(builder: &mut KernelRegistryBuilder) {
    for (id, moderated) in [
        ("yssbi.statistics.workflow.mediation", false),
        ("yssbi.statistics.workflow.moderated_mediation", true),
    ] {
        let mut inputs = vec![
            Input::fixed("y"),
            Input::fixed("x"),
            Input::fixed("mediator"),
        ];
        let mut params = vec!["replications", "seed"];
        if moderated {
            inputs.push(Input::fixed("moderator"));
            params.extend(["stage", "probe_sd"]);
        }
        inputs.push(Input::repeated("covariates", 0..=usize::MAX));
        install(builder, id, inputs, &params, 2, move |inv| {
            mediation(inv, moderated)
        });
    }
}
fn mediation(
    inv: &KernelInvocation<'_>,
    moderated: bool,
) -> Result<Vec<RuntimeValue>, KernelError> {
    let options = MediationOptions {
        moderated_stage: if moderated {
            match text(inv, "stage")? {
                "first" => MediatedStage::First,
                "second" => MediatedStage::Second,
                _ => return Err(KernelError::InvalidNumericInput),
            }
        } else {
            MediatedStage::None
        },
        probe_sd: if moderated {
            number(inv, "probe_sd")?
        } else {
            1.
        },
        replications: integer(inv, "replications")?,
        seed: integer(inv, "seed")? as u64,
    };
    let (data, retained) = materialize(inv)?;
    let (n, k) = (
        data[0].values.len(),
        data.len()
            .checked_add(2)
            .ok_or(KernelError::BudgetExceeded)?,
    );
    inv.control.check_bytes((|| {
        n.checked_mul(k)?
            .checked_mul(512)?
            .checked_add(k.checked_mul(k)?.checked_mul(4096)?)?
            .checked_add(n.checked_mul(7 * size_of::<RuntimeValue>() * 8)?)?
            .checked_add(options.replications.checked_mul(8 * size_of::<f64>())?)?
            .checked_add(retained.checked_mul(3)?)?
            .checked_add(65536)
    })())?;
    let y = numeric(&data[0], false, inv)?;
    let x = data
        .into_iter()
        .skip(1)
        .map(|v| numeric(&v, false, inv))
        .collect::<Result<Vec<_>, _>>()?;
    let control = Control::from_shared(inv.control.cancellation.clone(), inv.control.deadline);
    let result =
        yss_sci_runtime::path::mediation(&y, &x, options, &control).map_err(computation_error)?;
    let report = value(
        serde_json::json!({
            "method":"mediation", "observations":n,
            "input_names":inv.inputs.iter().enumerate().map(|(j,v)| input_label(v, format!("input{}",j+1))).collect::<Vec<_>>(),
            "mediator_equation":reports::equation(&result.mediator),
            "outcome_equation":reports::equation(&result.outcome),
            "details":result.diagnostics,
        }),
        inv,
    )?;
    let mut rows = inv.control.reserve(n)?;
    for (i, &response) in y.iter().enumerate() {
        if i.is_multiple_of(1024) {
            inv.check_control()?;
        }
        rows.push([
            (i + 1) as f64,
            response,
            result.outcome.fitted[i],
            result.outcome.residuals[i],
            x[1][i],
            result.mediator.fitted[i],
            result.mediator.residuals[i],
        ]);
    }
    Ok(vec![
        report,
        numeric_table(
            &rows,
            1,
            [
                "observation",
                "response",
                "fitted",
                "residual",
                "mediator",
                "mediator_fitted",
                "mediator_residual",
            ],
            |r| *r,
            inv,
        )?,
    ])
}
