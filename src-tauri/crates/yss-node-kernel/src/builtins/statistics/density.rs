use super::{Input, common::*, install};
use crate::{KernelError, KernelRegistryBuilder};

pub(super) fn register(builder: &mut KernelRegistryBuilder) {
    install(
        builder,
        "yssbi.plot.kde.view",
        vec![Input::fixed("values")],
        &[],
        1,
        |inv| {
            let values = columns(
                &[inv.inputs.first().ok_or(KernelError::InvalidNumericInput)?],
                inv,
                0,
            )?
            .remove(0);
            if values.len() < 2 {
                return Err(KernelError::InvalidNumericInput);
            }
            inv.control.check_bytes(
                values
                    .len()
                    .checked_mul(24)
                    .and_then(|n| n.checked_add(256 * 256)),
            )?;
            let density = yss_sci_runtime::density::compute_kernel_density(
                yss_sci_contract::density::KernelDensityInput {
                    values: &values,
                    grid_points: 256,
                    min_x: None,
                },
            );
            if density
                .points
                .iter()
                .any(|point| !point.x.is_finite() || !point.density.is_finite())
            {
                return Err(KernelError::NonFiniteResult);
            }
            value(serde_json::json!({ "method": "gaussian_kde", "points": density.points.iter().map(|p| serde_json::json!({"x":p.x,"density":p.density})).collect::<Vec<_>>() }), inv).map(|result| vec![result])
        },
    );
}
