use super::*;
use yss_sci_contract::doe::DesignSpecification;
use yss_sci_runtime::doe as sci;

pub(super) fn register(builder: &mut KernelRegistryBuilder) {
    for (id, kind, parameters) in [
        ("yssbi.statistics.doe.family", 0, &["factors", "levels"][..]),
        (
            "yssbi.statistics.doe.orthogonal",
            1,
            &["factors", "levels"][..],
        ),
        (
            "yssbi.statistics.doe.uniform_design",
            2,
            &["factors", "runs", "candidates", "seed"][..],
        ),
    ] {
        install(builder, id, vec![], parameters, 2, move |inv| {
            execute(inv, kind)
        });
    }
}
fn execute(inv: &KernelInvocation<'_>, kind: u8) -> Result<Vec<RuntimeValue>, KernelError> {
    let factors = integer(inv, "factors")?;
    let specification = match kind {
        0 => DesignSpecification::FullFactorial {
            factors,
            levels: integer(inv, "levels")?,
        },
        1 => DesignSpecification::Orthogonal {
            factors,
            levels: integer(inv, "levels")?,
        },
        _ => DesignSpecification::Uniform {
            factors,
            runs: integer(inv, "runs")?,
            candidates: integer(inv, "candidates")?,
            seed: integer(inv, "seed")? as u64,
        },
    };
    let c = Control::from_shared(inv.control.cancellation.clone(), inv.control.deadline);
    let (runs, factors) = sci::design_dimensions(specification, &c).map_err(computation_error)?;
    inv.control.check_bytes(
        runs.checked_mul(factors.checked_add(1).ok_or(KernelError::BudgetExceeded)?)
            .and_then(|cells| cells.checked_mul(size_of::<RuntimeValue>() * 8 + 96))
            .and_then(|bytes| bytes.checked_add(65536)),
    )?;
    let result = sci::generate_design(specification, &c).map_err(computation_error)?;
    Ok(vec![
        value(result.summary, inv)?,
        matrix_table(result.rows, 1, inv)?,
    ])
}
