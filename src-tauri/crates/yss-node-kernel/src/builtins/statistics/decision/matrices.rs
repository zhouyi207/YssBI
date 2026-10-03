use super::{columns::*, *};
use yss_sci_contract::decision::influence::InfluenceNormalization;
pub(super) fn register(builder: &mut KernelRegistryBuilder) {
    for (method, parameters, outputs) in [
        ("ahp", &["random_index"][..], 2),
        ("fahp", &[][..], 2),
        (
            "dematel",
            &["influence_normalization", "attenuation"][..],
            3,
        ),
        ("ism", &[][..], 3),
    ] {
        install(
            builder,
            &format!("yssbi.statistics.decision.{method}"),
            vec![Input::repeated("criteria", 1..=usize::MAX)],
            parameters,
            outputs,
            move |inv| execute(method, inv),
        );
    }
}
fn execute(method: &str, inv: &KernelInvocation<'_>) -> Result<Vec<RuntimeValue>, KernelError> {
    let data = read_criteria(inv)?;
    let n = data.columns.len();
    if data.rows() != n {
        return Err(KernelError::ShapeMismatch);
    }
    let cell_bytes = if matches!(method, "dematel" | "ism") {
        4 * (size_of::<RuntimeValue>() * 3 + 32) + 256
    } else {
        256
    };
    inv.control.check_bytes(
        n.checked_mul(n)
            .and_then(|v| v.checked_mul(cell_bytes))
            .and_then(|v| {
                v.checked_add(n.checked_mul(24 * STRUCTURED_VALUE_BYTES * STRUCTURED_VALUE_COPIES)?)
            })
            .and_then(|v| v.checked_add(65536)),
    )?;
    let c = Control::from_shared(inv.control.cancellation.clone(), inv.control.deadline);
    match method {
        "ahp" => {
            let result = yss_sci_runtime::decision::hierarchy::ahp(
                &data.columns,
                number(inv, "random_index")?,
                &c,
            )
            .map_err(computation_error)?;
            Ok(vec![
                named_report(&result, &data, inv)?,
                numeric_list(&result.weights, inv)?,
            ])
        }
        "fahp" => {
            let result = yss_sci_runtime::decision::hierarchy::fuzzy_ahp(&data.columns, &c)
                .map_err(computation_error)?;
            Ok(vec![
                named_report(&result, &data, inv)?,
                numeric_list(&result.weights, inv)?,
            ])
        }
        "dematel" => dematel(&data, &c, inv),
        "ism" => ism(&data, &c, inv),
        _ => Err(KernelError::InvalidParameter),
    }
}
fn dematel(
    data: &Criteria,
    c: &Control,
    inv: &KernelInvocation<'_>,
) -> Result<Vec<RuntimeValue>, KernelError> {
    let normalization = match text(inv, "influence_normalization")? {
        "max_sum" => InfluenceNormalization::MaxSum,
        "none" => InfluenceNormalization::None,
        _ => return Err(KernelError::InvalidParameter),
    };
    let result = yss_sci_runtime::decision::influence::dematel(
        &data.columns,
        normalization,
        number(inv, "attenuation")?,
        c,
    )
    .map_err(computation_error)?;
    Ok(vec![
        named_report(&result.summary, data, inv)?,
        numeric_table(
            &result.rows,
            1,
            [
                "criterion",
                "outgoing",
                "incoming",
                "prominence",
                "net_cause",
                "weight",
            ],
            |r| {
                [
                    Some(r.criterion as f64),
                    Some(r.outgoing),
                    Some(r.incoming),
                    Some(r.prominence),
                    Some(r.net_cause),
                    r.weight,
                ]
            },
            inv,
        )?,
        numeric_table(
            &result.matrix,
            2,
            ["source", "target", "direct", "total"],
            |r| [r.source as f64, r.target as f64, r.direct, r.total],
            inv,
        )?,
    ])
}
fn ism(
    data: &Criteria,
    c: &Control,
    inv: &KernelInvocation<'_>,
) -> Result<Vec<RuntimeValue>, KernelError> {
    let result =
        yss_sci_runtime::decision::influence::ism(&data.columns, c).map_err(computation_error)?;
    Ok(vec![
        named_report(&result.summary, data, inv)?,
        numeric_table(
            &result.rows,
            1,
            ["criterion", "level", "driving_power", "dependence"],
            |r| {
                [
                    r.criterion as f64,
                    r.level as f64,
                    r.driving_power as f64,
                    r.dependence as f64,
                ]
            },
            inv,
        )?,
        numeric_table(
            &result.matrix,
            2,
            ["source", "target", "reachable"],
            |r| {
                [
                    r.source as f64,
                    r.target as f64,
                    if r.reachable { 1. } else { 0. },
                ]
            },
            inv,
        )?,
    ])
}
