use std::{any::Any, sync::Arc};

use arrow::array::{Array, ArrayRef, Float64Array};
use arrow::compute::kernels::numeric;
use arrow::datatypes::{DataType, Field};
use datafusion::common::{Column, DataFusionError, ScalarValue};
use datafusion::logical_expr::{ColumnarValue, Expr, Volatility, create_udf};
use yss_relational_contract::{
    BooleanOperand, BooleanOperation, NumericOperation, NumericType, RelationError,
    RelationLiteral, SeriesHandle, SeriesOperand, SeriesPlan,
};

pub(crate) struct DataFusionSeries {
    pub expression: Expr,
    pub field: Arc<Field>,
}

#[derive(Debug, PartialEq, Eq, Hash)]
struct SemanticConversionFunction {
    signature: datafusion::logical_expr::Signature,
    conversion: Arc<yss_tabular_arrow::PreparedConversion>,
}

impl datafusion::logical_expr::ScalarUDFImpl for SemanticConversionFunction {
    fn name(&self) -> &str {
        "yss_semantic_conversion"
    }
    fn signature(&self) -> &datafusion::logical_expr::Signature {
        &self.signature
    }
    fn return_type(&self, _: &[DataType]) -> datafusion::common::Result<DataType> {
        Ok(self.conversion.field().data_type().clone())
    }
    fn return_field_from_args(
        &self,
        _: datafusion::logical_expr::ReturnFieldArgs,
    ) -> datafusion::common::Result<Arc<Field>> {
        Ok(self.conversion.field().clone())
    }
    fn invoke_with_args(
        &self,
        arguments: datafusion::logical_expr::ScalarFunctionArgs,
    ) -> datafusion::common::Result<ColumnarValue> {
        let [input] = arguments.args.as_slice() else {
            return Err(failure(RelationError::InvalidInput));
        };
        let arrays = ColumnarValue::values_to_arrays(&arguments.args)?;
        let output = self
            .conversion
            .convert(arrays[0].as_ref())
            .map_err(|_| failure(RelationError::InvalidConversion))?;
        if matches!(input, ColumnarValue::Scalar(_)) {
            Ok(ColumnarValue::Scalar(ScalarValue::try_from_array(
                output.as_ref(),
                0,
            )?))
        } else {
            Ok(ColumnarValue::Array(output))
        }
    }
}

pub(crate) fn convert(
    series: &SeriesHandle,
    conversion: yss_data_contract::SemanticConversion,
) -> Result<Arc<dyn SeriesPlan>, RelationError> {
    let source = series.plan().field();
    let conversion = Arc::new(
        yss_tabular_arrow::PreparedConversion::new(source, &conversion)
            .map_err(|_| RelationError::InvalidConversion)?,
    );
    let field = conversion.field().clone();
    // Field metadata and policy participate in Eq/Hash, without exposing codes in a UDF name.
    let function = datafusion::logical_expr::ScalarUDF::from(SemanticConversionFunction {
        signature: datafusion::logical_expr::Signature::exact(
            vec![source.data_type().clone()],
            Volatility::Immutable,
        ),
        conversion,
    });
    Ok(Arc::new(DataFusionSeries {
        expression: function.call(vec![expression(series)?]),
        field,
    }))
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
        field: Arc::new(field.clone()),
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

pub(crate) fn boolean(
    operation: BooleanOperation,
    operands: &[BooleanOperand],
) -> Result<Arc<dyn SeriesPlan>, RelationError> {
    if operands.len() != operation.arity() {
        return Err(RelationError::InvalidInput);
    }
    let expressions = operands
        .iter()
        .map(|operand| match operand {
            BooleanOperand::Scalar(value) => Ok(Expr::Literal(ScalarValue::Boolean(*value), None)),
            BooleanOperand::Series(series) => {
                let field = series.plan().field();
                let semantic = yss_tabular_arrow::column_semantic(field)
                    .map_err(|_| RelationError::InvalidInput)?;
                if semantic.kind != yss_data_contract::SemanticType::Binary {
                    return Err(RelationError::InvalidInput);
                }
                if field.data_type() == &DataType::Boolean
                    && semantic
                        .positive_value
                        .as_deref()
                        .is_none_or(|value| value == "true")
                {
                    expression(series)
                } else {
                    let normalized = series.relation().convert_series(
                        series,
                        yss_data_contract::SemanticConversion::new(
                            yss_data_contract::SemanticType::Binary,
                            yss_data_contract::NumericRepresentation::Auto,
                        ),
                    )?;
                    expression(&normalized)
                }
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut expressions = expressions.into_iter();
    let first = expressions.next().ok_or(RelationError::InvalidInput)?;
    let expression = match operation {
        BooleanOperation::Not => Expr::Not(Box::new(first)),
        BooleanOperation::And => first.and(expressions.next().ok_or(RelationError::InvalidInput)?),
        BooleanOperation::Or => first.or(expressions.next().ok_or(RelationError::InvalidInput)?),
    };
    let field = Field::new("result", DataType::Boolean, true);
    let semantic =
        yss_tabular_arrow::column_semantic(&field).map_err(|_| RelationError::InvalidInput)?;
    let field = yss_tabular_arrow::with_column_semantic(field, &semantic)
        .map_err(|_| RelationError::InvalidInput)?;
    Ok(Arc::new(DataFusionSeries {
        expression,
        field: Arc::new(field),
    }))
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
    if !operation.accepts_arity(operands.len())
        || (operation.requires_float() && output_type != NumericType::Float64)
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
                if !yss_tabular_arrow::is_numeric_field(series.plan().field())
                    || (output_type == NumericType::Int64 && !input.is_integer())
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
        NumericOperation::Power => "yssbi_numeric_power",
        NumericOperation::Logarithm => "yssbi_numeric_log",
        NumericOperation::Ln => "yssbi_numeric_ln",
        NumericOperation::Log2 => "yssbi_numeric_log2",
        NumericOperation::Log10 => "yssbi_numeric_log10",
        NumericOperation::Square => "yssbi_numeric_square",
        NumericOperation::Sqrt => "yssbi_numeric_sqrt",
    };
    // One native UDF retains strict numeric errors while Arrow supplies batch arithmetic
    // and DataFusion supplies scalar broadcasting, projection and controlled execution.
    let input_types = operands
        .iter()
        .map(|operand| match operand {
            SeriesOperand::Series(series) => series.plan().field().data_type().clone(),
            SeriesOperand::Scalar(RelationLiteral::Integer(_)) => DataType::Int64,
            _ => DataType::Float64,
        })
        .collect();
    let result_type = data_type.clone();
    let function = create_udf(
        name,
        input_types,
        data_type.clone(),
        Volatility::Immutable,
        Arc::new(move |arguments| {
            let arrays = ColumnarValue::values_to_arrays(arguments)?;
            let arrays = arrays
                .iter()
                .map(|array| {
                    yss_tabular_arrow::lossless_cast(array.as_ref(), &result_type, false)
                        .map_err(|_| failure(RelationError::InvalidInput))
                })
                .collect::<Result<Vec<_>, _>>()?;
            if !operation.accepts_arity(arrays.len())
                || arrays
                    .iter()
                    .any(|array| array.null_count() != 0 || !finite(array))
            {
                return Err(failure(RelationError::InvalidInput));
            }
            let mut result = arrays[0].clone();
            if operation.is_unary() {
                let values = result
                    .as_any()
                    .downcast_ref::<Float64Array>()
                    .ok_or_else(|| failure(RelationError::InvalidInput))?;
                let values = values
                    .values()
                    .iter()
                    .map(|value| operation.evaluate_unary_float(*value).map_err(failure))
                    .collect::<Result<Vec<_>, _>>()?;
                result = Arc::new(Float64Array::from(values));
            }
            for right in &arrays[1..] {
                if operation == NumericOperation::Divide
                    && right
                        .as_any()
                        .downcast_ref::<Float64Array>()
                        .is_some_and(|array| array.values().contains(&0.0))
                {
                    return Err(failure(RelationError::DivisionByZero));
                }
                let native = match operation {
                    NumericOperation::Add => Some(numeric::add(&result, right)),
                    NumericOperation::Subtract => Some(numeric::sub(&result, right)),
                    NumericOperation::Multiply => Some(numeric::mul(&result, right)),
                    NumericOperation::Divide => Some(numeric::div(&result, right)),
                    NumericOperation::Power | NumericOperation::Logarithm => None,
                    NumericOperation::Ln
                    | NumericOperation::Log2
                    | NumericOperation::Log10
                    | NumericOperation::Square
                    | NumericOperation::Sqrt => None,
                };
                result = if let Some(native) = native {
                    native.map_err(|_| failure(RelationError::NonFiniteResult))?
                } else {
                    let left = result
                        .as_any()
                        .downcast_ref::<Float64Array>()
                        .ok_or_else(|| failure(RelationError::InvalidInput))?;
                    let right = right
                        .as_any()
                        .downcast_ref::<Float64Array>()
                        .ok_or_else(|| failure(RelationError::InvalidInput))?;
                    let values = left
                        .values()
                        .iter()
                        .zip(right.values().iter())
                        .map(|(left, right)| {
                            operation.evaluate_float(*left, *right).map_err(failure)
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    Arc::new(Float64Array::from(values))
                };
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
        field: Arc::new(Field::new("result", data_type, false)),
    }))
}
