use super::*;
use yss_relational_contract::{NumericOperation, NumericType, SeriesOperand};
pub(super) fn register(builder: &mut KernelRegistryBuilder) {
    install(
        builder,
        "yssbi.statistics.survey.weights",
        vec![Input::fixed("values")],
        &["input_kind"],
        2,
        weights,
    );
}
fn weights(inv: &KernelInvocation<'_>) -> Result<Vec<RuntimeValue>, KernelError> {
    let probabilities = match text(inv, "input_kind")? {
        "weights" => false,
        "inclusion_probabilities" => true,
        _ => return Err(KernelError::InvalidParameter),
    };
    let (data, retained) = materialize(inv)?;
    inv.control.check_bytes(
        data[0]
            .values
            .len()
            .checked_mul(size_of::<RuntimeValue>() * 12)
            .and_then(|n| n.checked_add(retained)),
    )?;
    let values = numeric(&data[0], false, inv)?;
    drop(data);
    let control = Control::from_shared(inv.control.cancellation.clone(), inv.control.deadline);
    let result = yss_sci_runtime::survey::sampling_weights(&values, probabilities, &control)
        .map_err(computation_error)?;
    let output = if !probabilities {
        inv.inputs[0].clone()
    } else {
        match inv.inputs[0].unannotated() {
            RuntimeValue::Series(series) => RuntimeValue::Series(
                series
                    .relation()
                    .numeric_series(
                        NumericOperation::Divide,
                        &[
                            SeriesOperand::Scalar(yss_data_contract::TabularScalar::Integer(1)),
                            SeriesOperand::Series(series.clone()),
                        ],
                        NumericType::Float64,
                    )
                    .map_err(crate::builtins::relational::kernel_error)?,
            ),
            RuntimeValue::List(_) => RuntimeValue::List(
                result
                    .weights
                    .iter()
                    .map(|&w| RuntimeValue::float64(w).map_err(|_| KernelError::NonFiniteResult))
                    .collect::<Result<Vec<_>, _>>()?
                    .into(),
            ),
            _ => return Err(KernelError::InvalidNumericInput),
        }
    };
    Ok(vec![value(result.summary, inv)?, output])
}
