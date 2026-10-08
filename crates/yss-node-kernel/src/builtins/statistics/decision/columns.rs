use super::*;
pub(super) struct Criteria {
    pub columns: Vec<Vec<f64>>,
    names: Vec<String>,
    retained: usize,
}
impl Criteria {
    pub fn rows(&self) -> usize {
        self.columns[0].len()
    }
}
pub(super) fn read_criteria(inv: &KernelInvocation<'_>) -> Result<Criteria, KernelError> {
    let inputs = group(inv, "criteria");
    let columns = super::super::common::columns(&inputs, inv, 0)?;
    let retained = columns[0]
        .len()
        .checked_mul(columns.len())
        .and_then(|v| v.checked_mul(8))
        .ok_or(KernelError::BudgetExceeded)?;
    let names = inputs
        .iter()
        .enumerate()
        .map(|(j, v)| input_label(v, format!("criterion{}", j + 1)))
        .collect();
    Ok(Criteria {
        columns,
        names,
        retained,
    })
}
pub(super) fn admit(
    data: &Criteria,
    output_rows: usize,
    width: usize,
    svd: bool,
    inv: &KernelInvocation<'_>,
) -> Result<(), KernelError> {
    let p = data.columns.len();
    inv.control.check_bytes((|| {
        let k = p.checked_add(1)?;
        let decomposition = if svd {
            k.checked_mul(data.rows().min(k))?.checked_mul(128)?
        } else {
            0
        };
        data.retained
            .checked_mul(if svd { 12 } else { 4 })?
            .checked_add(
                output_rows
                    .checked_mul(width)?
                    .checked_mul(size_of::<RuntimeValue>() * 3 + 16)?,
            )?
            .checked_add(decomposition)?
            .checked_add(p.checked_mul(12 * STRUCTURED_VALUE_BYTES * STRUCTURED_VALUE_COPIES)?)?
            .checked_add(65536)
    })())?;
    Ok(())
}
pub(super) fn costs(inv: &KernelInvocation<'_>, p: usize) -> Result<Vec<bool>, KernelError> {
    let mut costs = vec![false; p];
    let Some(RuntimeValue::List(indices)) = inv.parameter("cost_criteria") else {
        return Err(KernelError::InvalidParameter);
    };
    for value in indices.iter() {
        let index = crate::builtins::numeric_input(Some(value))
            .map_err(|_| KernelError::InvalidParameter)?;
        if index < 1. || index > p as f64 || index.fract() != 0. {
            return Err(KernelError::InvalidParameter);
        }
        let slot = &mut costs[index as usize - 1];
        if *slot {
            return Err(KernelError::InvalidParameter);
        }
        *slot = true;
    }
    Ok(costs)
}
pub(super) fn explicit_weights(
    data: &Criteria,
    required: bool,
    inv: &KernelInvocation<'_>,
) -> Result<Vec<f64>, KernelError> {
    let weights = group(inv, "criterion_weights");
    if weights.len() != usize::from(required) {
        return Err(KernelError::InvalidParameter);
    }
    if weights.is_empty() {
        Ok(vec![])
    } else {
        Ok(super::super::common::columns(&weights, inv, data.retained)?.remove(0))
    }
}
pub(super) fn named_report(
    summary: &impl serde::Serialize,
    data: &Criteria,
    inv: &KernelInvocation<'_>,
) -> Result<RuntimeValue, KernelError> {
    #[derive(serde::Serialize)]
    struct Report<'a, T: serde::Serialize> {
        #[serde(flatten)]
        summary: &'a T,
        criterion_names: &'a [String],
    }
    value(
        Report {
            summary,
            criterion_names: &data.names,
        },
        inv,
    )
}
