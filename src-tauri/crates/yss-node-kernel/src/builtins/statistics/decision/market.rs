use super::{columns::*, *};
use yss_sci_contract::decision::market::PriceRangeDefinition;

pub(super) fn register(builder: &mut KernelRegistryBuilder) {
    install(
        builder,
        "yssbi.statistics.decision.turf",
        vec![Input::repeated("criteria", 1..=usize::MAX)],
        &["combination_size"],
        1,
        turf,
    );
    install(
        builder,
        "yssbi.statistics.decision.psm",
        ["too_cheap", "cheap", "expensive", "too_expensive"]
            .map(Input::fixed)
            .to_vec(),
        &["range_definition"],
        2,
        price,
    );
}
fn turf(inv: &KernelInvocation<'_>) -> Result<Vec<RuntimeValue>, KernelError> {
    let data = read_criteria(inv)?;
    admit(&data, 0, 0, false, inv)?;
    let control = Control::from_shared(inv.control.cancellation.clone(), inv.control.deadline);
    let result = yss_sci_runtime::decision::reach::turf(
        &data.columns,
        integer(inv, "combination_size")?,
        &control,
    )
    .map_err(computation_error)?;
    Ok(vec![named_report(&result, &data, inv)?])
}
fn price(inv: &KernelInvocation<'_>) -> Result<Vec<RuntimeValue>, KernelError> {
    let data = super::super::common::columns(&inv.inputs.iter().collect::<Vec<_>>(), inv, 0)?;
    inv.control.check_bytes(
        data[0]
            .len()
            .checked_mul(4 * 7 * (size_of::<RuntimeValue>() * 3 + 16) + 128)
            .and_then(|n| n.checked_add(65536)),
    )?;
    let definition = match text(inv, "range_definition")? {
        "original" => PriceRangeDefinition::Original,
        "narrower" => PriceRangeDefinition::Narrower,
        _ => return Err(KernelError::InvalidParameter),
    };
    let control = Control::from_shared(inv.control.cancellation.clone(), inv.control.deadline);
    let result = yss_sci_runtime::decision::pricing::price_sensitivity(&data, definition, &control)
        .map_err(computation_error)?;
    Ok(vec![
        value(&result.summary, inv)?,
        numeric_table(
            &result.rows,
            1,
            [
                "price",
                "too_cheap",
                "cheap",
                "expensive",
                "too_expensive",
                "not_cheap",
                "not_expensive",
            ],
            |r| {
                [
                    r.price,
                    r.too_cheap,
                    r.cheap,
                    r.expensive,
                    r.too_expensive,
                    1. - r.cheap,
                    1. - r.expensive,
                ]
            },
            inv,
        )?,
    ])
}
