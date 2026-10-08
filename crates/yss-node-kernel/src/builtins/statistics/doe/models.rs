use super::*;
use yss_sci_contract::regression::models::IterationOptions;
use yss_sci_runtime::doe as sci;
pub(super) fn register(builder: &mut KernelRegistryBuilder) {
    install(
        builder,
        "yssbi.statistics.doe.response_surface",
        vec![
            Input::fixed("y"),
            Input::repeated("factors", 1..=usize::MAX),
        ],
        &[],
        2,
        |inv| execute(inv, true),
    );
    install(
        builder,
        "yssbi.statistics.doe.dose_response",
        vec![Input::fixed("y"), Input::fixed("dose")],
        &["max_iterations", "tolerance"],
        2,
        |inv| execute(inv, false),
    );
}
fn execute(inv: &KernelInvocation<'_>, surface: bool) -> Result<Vec<RuntimeValue>, KernelError> {
    let (data, retained) = materialize(inv)?;
    let n = data[0].values.len();
    let c = Control::from_shared(inv.control.cancellation.clone(), inv.control.deadline);
    let k = if surface {
        sci::response_surface_parameter_count(data.len() - 1).map_err(computation_error)?
    } else {
        4
    };
    inv.control.check_bytes((|| {
        n.checked_mul(k)?
            .checked_mul(256)?
            .checked_add(k.checked_mul(k)?.checked_mul(512)?)?
            .checked_add(n.checked_mul(4 * size_of::<RuntimeValue>() * 8)?)?
            .checked_add(retained.checked_mul(3)?)?
            .checked_add(65536)
    })())?;
    let response = numeric(&data[0], false, inv)?;
    let predictors = data
        .into_iter()
        .skip(1)
        .map(|c| numeric(&c, false, inv))
        .collect::<Result<Vec<_>, _>>()?;
    if surface {
        let result =
            sci::response_surface(&response, &predictors, &c).map_err(computation_error)?;
        regression_outputs(
            inv,
            &response,
            &result.model,
            inv.inputs[1..]
                .iter()
                .enumerate()
                .map(|(j, v)| input_label(v, format!("factor{}", j + 1)))
                .collect(),
            &result.geometry,
        )
    } else {
        let result = sci::dose_response(
            &response,
            &predictors[0],
            IterationOptions {
                max_iterations: integer(inv, "max_iterations")?,
                tolerance: number(inv, "tolerance")?,
            },
            &c,
        )
        .map_err(computation_error)?;
        regression_outputs(
            inv,
            &response,
            &result.model,
            inv.inputs[1..]
                .iter()
                .enumerate()
                .map(|(j, v)| input_label(v, format!("factor{}", j + 1)))
                .collect(),
            &result.parameters,
        )
    }
}
