//! Grid alignment is a native range/unnest/join plan.
use super::relational::kernel_error;
use super::transforms::{integer, text};
use crate::{
    KernelContract, KernelError, KernelId, KernelInputSpec, KernelInvocation, KernelParameterKey,
    KernelRegistryBuilder, RuntimeValue,
};

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
        builder
            .register(
                KernelId::new(id.into()).unwrap(),
                std::num::NonZeroU32::new(3).unwrap(),
                KernelContract::new(
                    [KernelInputSpec::fixed("dataframe")],
                    parameters
                        .iter()
                        .map(|p| KernelParameterKey::new((*p).into()).unwrap()),
                    1..=1,
                )
                .unwrap(),
                move |inv| align(panel, inv),
            )
            .expect("unique alignment kernel");
    }
}
fn align(panel: bool, inv: &KernelInvocation<'_>) -> Result<Vec<RuntimeValue>, KernelError> {
    inv.check_control()?;
    let [RuntimeValue::Relation(frame)] = inv.inputs else {
        return Err(KernelError::InputLayoutMismatch);
    };
    let entity = if panel {
        Some(text(inv, "entity_column")?)
    } else {
        None
    };
    let result = frame
        .align_grid(
            text(inv, "time_column")?,
            entity,
            integer(inv, "interval")?,
            inv.control.max_input_bytes,
        )
        .map_err(kernel_error)?;
    Ok(vec![RuntimeValue::Relation(result)])
}
