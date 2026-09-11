use std::{any::Any, sync::Arc};

use arrow::array::{Array, ArrayRef, Float64Array};
use arrow::compute::kernels::numeric;
use arrow::datatypes::{DataType, Field};
use datafusion::common::{Column, DataFusionError, ScalarValue};
use datafusion::logical_expr::{ColumnarValue, Expr, Volatility, create_udf};
use yss_relational_contract::{
    NumericOperation, NumericType, RelationError, RelationLiteral, SeriesHandle, SeriesOperand,
    SeriesPlan,
};

pub(crate) struct DataFusionSeries {
    pub expression: Expr,
    pub field: Field,
}

impl SeriesPlan for DataFusionSeries {
    fn field(&self) -> &Field {
        &self.field
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn equals(&self, other: &dyn SeriesPlan) -> bool {
        other
            .as_any()
            .downcast_ref::<Self>()
            .is_some_and(|other| self.expression == other.expression && self.field == other.field)
    }
}

pub(crate) fn column(field: &Field) -> Arc<dyn SeriesPlan> {
    Arc::new(DataFusionSeries {
        expression: Expr::Column(Column::from_name(field.name().clone())),
        field: field.clone(),
    })
}

pub(crate) fn expression(series: &SeriesHandle) -> Result<Expr, RelationError> {
    series
        .plan()
        .as_any()
        .downcast_ref::<DataFusionSeries>()
        .map(|plan| plan.expression.clone())
        .ok_or(RelationError::InvalidInput)
}

fn failure(error: RelationError) -> DataFusionError {
    DataFusionError::External(Box::new(error))
}

fn finite(array: &ArrayRef) -> bool {
    array
        .as_any()
        .downcast_ref::<Float64Array>()
        .is_none_or(|array| array.values().iter().all(|value| value.is_finite()))
}

pub(crate) fn arithmetic(
    operation: NumericOperation,
    operands: &[SeriesOperand],
    output_type: NumericType,
) -> Result<Arc<dyn SeriesPlan>, RelationError> {
    if operands.len() < 2
        || (operation != NumericOperation::Add && operands.len() != 2)
        || (operation == NumericOperation::Divide && output_type != NumericType::Float64)
    {
        return Err(RelationError::InvalidInput);
    }
    let data_type = match output_type {
        NumericType::Int64 => DataType::Int64,
        NumericType::Float64 => DataType::Float64,
    };
    let expressions = operands
        .iter()
        .map(|operand| match operand {
            SeriesOperand::Series(series) => {
                let input = series.plan().field().data_type();
                if !input.is_numeric() || (output_type == NumericType::Int64 && !input.is_integer())
                {
                    return Err(RelationError::InvalidInput);
                }
                expression(series)
            }
            SeriesOperand::Scalar(RelationLiteral::Integer(value)) => {
                Ok(Expr::Literal(ScalarValue::Int64(Some(*value)), None))
            }
            SeriesOperand::Scalar(RelationLiteral::Decimal(value))
                if output_type == NumericType::Float64 =>
            {
                let value = value
                    .parse::<f64>()
                    .ok()
                    .filter(|value| value.is_finite())
                    .ok_or(RelationError::InvalidInput)?;
                Ok(Expr::Literal(ScalarValue::Float64(Some(value)), None))
            }
            _ => Err(RelationError::InvalidInput),
        })
        .collect::<Result<Vec<_>, _>>()?;
    let name = match operation {
        NumericOperation::Add => "yssbi_numeric_add",
        NumericOperation::Subtract => "yssbi_numeric_subtract",
        NumericOperation::Multiply => "yssbi_numeric_multiply",
        NumericOperation::Divide => "yssbi_numeric_divide",
    };
    // One native UDF retains strict numeric errors while Arrow supplies batch arithmetic
    // and DataFusion supplies scalar broadcasting, projection and controlled execution.
    let function = create_udf(
        name,
        vec![data_type.clone(); operands.len()],
        data_type.clone(),
        Volatility::Immutable,
        Arc::new(move |arguments| {
            let arrays = ColumnarValue::values_to_arrays(arguments)?;
            if arrays.len() < 2
                || arrays
                    .iter()
                    .any(|array| array.null_count() != 0 || !finite(array))
            {
                return Err(failure(RelationError::InvalidInput));
            }
            let mut result = arrays[0].clone();
            for right in &arrays[1..] {
                if operation == NumericOperation::Divide
                    && right
                        .as_any()
                        .downcast_ref::<Float64Array>()
                        .is_some_and(|array| array.values().contains(&0.0))
                {
                    return Err(failure(RelationError::DivisionByZero));
                }
                result = match operation {
                    NumericOperation::Add => numeric::add(&result, right),
                    NumericOperation::Subtract => numeric::sub(&result, right),
                    NumericOperation::Multiply => numeric::mul(&result, right),
                    NumericOperation::Divide => numeric::div(&result, right),
                }
                .map_err(|_| failure(RelationError::NonFiniteResult))?;
                if !finite(&result) {
                    return Err(failure(RelationError::NonFiniteResult));
                }
            }
            if arguments
                .iter()
                .all(|value| matches!(value, ColumnarValue::Scalar(_)))
            {
                Ok(ColumnarValue::Scalar(ScalarValue::try_from_array(
                    result.as_ref(),
                    0,
                )?))
            } else {
                Ok(ColumnarValue::Array(result))
            }
        }),
    );
    Ok(Arc::new(DataFusionSeries {
        expression: function.call(expressions),
        field: Field::new("result", data_type, false),
    }))
}
