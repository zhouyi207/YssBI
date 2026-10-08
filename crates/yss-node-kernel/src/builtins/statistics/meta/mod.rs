//! Aligned study inputs -> neutral SCI operations -> reports and reusable study relations.
use super::{Input, common::*, install};
use crate::{KernelError, KernelInvocation, KernelRegistryBuilder, RuntimeValue};
use yss_sci_contract::{execution::ScientificExecutionControl as Control, meta::*};
use yss_sci_runtime::meta as sci;
mod analysis;
mod effects;
mod output;
mod registration;
pub(super) use registration::register;

fn execute(method: &str, inv: &KernelInvocation<'_>) -> Result<Vec<RuntimeValue>, KernelError> {
    let data = columns(&inv.inputs.iter().collect::<Vec<_>>(), inv, 0)?;
    let n = data.first().map_or(0, Vec::len);
    let retained = n
        .checked_mul(data.len())
        .and_then(|values| values.checked_mul(size_of::<f64>()))
        .ok_or(KernelError::BudgetExceeded)?;
    let p = if method == "regression" {
        data.len().saturating_sub(1)
    } else {
        1
    };
    // Admit compact numeric columns, WLS workspaces, paged relations and plot/report trees
    // together; the study count has no fixed limit.
    inv.control.check_bytes((|| {
        let workspace = n
            .checked_mul(p.checked_add(16)?)?
            .checked_mul(32)?
            .checked_add(p.checked_mul(p)?.checked_mul(64)?)?;
        let table = n
            .checked_mul(9)?
            .checked_mul(size_of::<RuntimeValue>() * 3)?;
        let report = p
            .checked_mul(p.checked_add(10)?)?
            .checked_add(if matches!(method, "forest" | "funnel") {
                n.checked_mul(7)?
            } else {
                128
            })?
            .checked_mul(STRUCTURED_VALUE_BYTES * STRUCTURED_VALUE_COPIES)?;
        retained
            .checked_add(workspace)?
            .checked_add(table)?
            .checked_add(report)?
            .checked_add(65536)
    })())?;
    let control = Control::from_shared(inv.control.cancellation.clone(), inv.control.deadline);
    if matches!(
        method,
        "continuous" | "binary" | "single_proportion" | "mean" | "correlation" | "or_hr"
    ) {
        effects::execute(method, &data, inv, &control)
    } else {
        analysis::execute(method, &data, inv, &control)
    }
}

fn options(inv: &KernelInvocation<'_>) -> Result<MetaOptions, KernelError> {
    Ok(MetaOptions {
        estimator: if inv.parameter("estimator").is_some() {
            match text(inv, "estimator")? {
                "fixed" => MetaEstimator::Fixed,
                "der_simonian_laird" => MetaEstimator::DerSimonianLaird,
                "paule_mandel" => MetaEstimator::PauleMandel,
                _ => return Err(KernelError::InvalidParameter),
            }
        } else {
            MetaEstimator::Fixed
        },
        inference: if inv.parameter("inference").is_some() {
            match text(inv, "inference")? {
                "wald" => MetaInference::Wald,
                "knapp_hartung" => MetaInference::KnappHartung,
                _ => return Err(KernelError::InvalidParameter),
            }
        } else {
            MetaInference::Wald
        },
        confidence_level: if inv.parameter("confidence_level").is_some() {
            number(inv, "confidence_level")?
        } else {
            0.95
        },
    })
}
