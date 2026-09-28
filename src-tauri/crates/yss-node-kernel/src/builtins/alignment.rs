use super::statistics::{Input, common, install};
use crate::{KernelError, KernelInvocation, KernelRegistryBuilder, RuntimeValue};

pub(super) fn register(builder: &mut KernelRegistryBuilder) {
    for (id, panel, parameters) in [
        (
            "yssbi.dataframe.timeseries.align",
            false,
            &["time_column", "interval"][..],
        ),
        (
            "yssbi.dataframe.panel.align",
            true,
            &["entity_column", "time_column", "interval"][..],
        ),
    ] {
        install(
            builder,
            id,
            vec![Input::fixed("dataframe")],
            parameters,
            1,
            move |inv| align(panel, inv),
        );
    }
}
fn align(panel: bool, inv: &KernelInvocation<'_>) -> Result<Vec<RuntimeValue>, KernelError> {
    let Some(RuntimeValue::Relation(frame)) = inv.inputs.first() else {
        return Err(KernelError::InvalidParameter);
    };
    let mut batches = Vec::new();
    let mut bytes = 0usize;
    frame
        .visit_batches(&inv.relation_control(), &mut |batch| {
            inv.relation_control().check()?;
            bytes = bytes
                .checked_add(batch.get_array_memory_size().saturating_mul(3))
                .filter(|v| *v <= inv.control.max_input_bytes)
                .ok_or(yss_relational_contract::RelationError::MemoryLimitExceeded)?;
            batches.push(batch.clone());
            Ok(())
        })
        .map_err(super::relational::kernel_error)?;
    let entity = if panel {
        Some(common::text(inv, "entity_column")?)
    } else {
        None
    };
    let time = common::text(inv, "time_column")?;
    let interval = i64::try_from(common::integer(inv, "interval")?)
        .map_err(|_| KernelError::InvalidParameter)?;
    let result = yss_sci_runtime::preprocessing::align_batches(
        &batches,
        time,
        entity,
        interval,
        inv.control.max_input_bytes,
    )
    .map_err(|e| match e {
        yss_sci_runtime::preprocessing::PreparationError::MemoryLimit => {
            KernelError::BudgetExceeded
        }
        _ => KernelError::InvalidParameter,
    })?;
    inv.check_control()?;
    Ok(vec![RuntimeValue::Relation(
        inv.relations
            .clone()
            .materialize(result, &inv.relation_control())
            .map_err(super::relational::kernel_error)?,
    )])
}
