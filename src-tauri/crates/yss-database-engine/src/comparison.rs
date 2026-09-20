use crate::series::{DataFusionSeries, expression};
use arrow::compute::kernels::cmp;
use arrow::{
    array::{Array, ArrayRef, BooleanArray, Float64Array, Int64Array, UInt64Array},
    datatypes::{DataType, Field},
};
use datafusion::{
    common::{DataFusionError, ScalarValue},
    logical_expr::{ColumnarValue, Expr, Volatility, create_udf},
};
use std::sync::Arc;
use yss_relational_contract::{ComparisonOperand, ComparisonOperation, RelationError, SeriesPlan};
use yss_tabular_contract::TabularScalar;

pub(crate) fn compare(
    operation: ComparisonOperation,
    operands: &[ComparisonOperand],
) -> Result<Arc<dyn SeriesPlan>, RelationError> {
    let mut types = Vec::new();
    let mut expressions = Vec::new();
    let mut meaning = None;
    for operand in operands {
        let (dtype, expr) = match operand {
            ComparisonOperand::Series(series) => {
                let field = series.plan().field();
                let semantic = yss_database_arrow::column_semantic(field)
                    .map_err(|_| RelationError::InvalidInput)?
                    .kind;
                if meaning.is_some_and(|kind| kind != semantic) {
                    return Err(RelationError::InvalidInput);
                }
                meaning = Some(semantic);
                if !matches!(
                    operation,
                    ComparisonOperation::Equal | ComparisonOperation::NotEqual
                ) && !matches!(
                    semantic,
                    yss_data_contract::SemanticType::Numeric
                        | yss_data_contract::SemanticType::Text
                ) {
                    return Err(RelationError::InvalidInput);
                }
                (field.data_type().clone(), expression(series)?)
            }
            ComparisonOperand::Scalar(value) => {
                let value = match value {
                    TabularScalar::Null => ScalarValue::Null,
                    TabularScalar::Bool(v) => ScalarValue::Boolean(Some(*v)),
                    TabularScalar::Integer(v) => ScalarValue::Int64(Some(*v)),
                    TabularScalar::Unsigned(v) => ScalarValue::UInt64(Some(*v)),
                    TabularScalar::Decimal(v) => ScalarValue::Float64(Some(v.as_f64())),
                    TabularScalar::String(v) => ScalarValue::Utf8(Some(v.to_string())),
                };
                (value.data_type(), Expr::Literal(value, None))
            }
        };
        types.push(dtype);
        expressions.push(expr);
    }
    let function = create_udf(
        &format!("yss_compare_{operation:?}"),
        types,
        DataType::Boolean,
        Volatility::Immutable,
        Arc::new(move |args| {
            let arrays = ColumnarValue::values_to_arrays(args)?;
            let [left, right] = arrays.as_slice() else {
                return Err(failure());
            };
            let result = compare_arrays(operation, left, right)?;
            if args.iter().all(|v| matches!(v, ColumnarValue::Scalar(_))) {
                Ok(ColumnarValue::Scalar(ScalarValue::try_from_array(
                    &result, 0,
                )?))
            } else {
                Ok(ColumnarValue::Array(Arc::new(result)))
            }
        }),
    );
    let field = Field::new("result", DataType::Boolean, true);
    let semantic =
        yss_database_arrow::column_semantic(&field).map_err(|_| RelationError::InvalidInput)?;
    let field = yss_database_arrow::with_column_semantic(field, &semantic)
        .map_err(|_| RelationError::InvalidInput)?;
    Ok(Arc::new(DataFusionSeries {
        expression: function.call(expressions),
        field: Arc::new(field),
    }))
}

fn failure() -> DataFusionError {
    DataFusionError::External(Box::new(RelationError::InvalidInput))
}

fn normalize(array: &ArrayRef) -> Result<ArrayRef, DataFusionError> {
    let target = match array.data_type() {
        DataType::Dictionary(_, value) => Some(value.as_ref().clone()),
        DataType::LargeUtf8 | DataType::Utf8View => Some(DataType::Utf8),
        dtype if dtype.is_signed_integer() => Some(DataType::Int64),
        dtype if dtype.is_unsigned_integer() => Some(DataType::UInt64),
        dtype if dtype.is_floating() => Some(DataType::Float64),
        _ => None,
    };
    let result = if let Some(target) = target {
        arrow::compute::cast(array, &target)?
    } else {
        array.clone()
    };
    if let Some(values) = result.as_any().downcast_ref::<Float64Array>()
        && values.iter().flatten().any(|v| !v.is_finite())
    {
        return Err(failure());
    }
    // Dictionary values can themselves use a narrow numeric/string representation.
    if matches!(array.data_type(), DataType::Dictionary(..)) {
        normalize(&result)
    } else {
        Ok(result)
    }
}

fn compare_arrays(
    operation: ComparisonOperation,
    left: &ArrayRef,
    right: &ArrayRef,
) -> Result<BooleanArray, DataFusionError> {
    let mut left = normalize(left)?;
    let mut right = normalize(right)?;
    if left.data_type() == &DataType::Null || right.data_type() == &DataType::Null {
        return Ok(BooleanArray::new_null(left.len()));
    }
    if left.data_type().is_temporal() && right.data_type() == &DataType::Utf8 {
        right = yss_database_arrow::lossless_cast(right.as_ref(), left.data_type(), false)
            .map_err(|_| failure())?;
    } else if right.data_type().is_temporal() && left.data_type() == &DataType::Utf8 {
        left = yss_database_arrow::lossless_cast(left.as_ref(), right.data_type(), false)
            .map_err(|_| failure())?;
    }
    if left.data_type() != right.data_type()
        && (left.data_type().is_decimal() || right.data_type().is_decimal())
    {
        // Fixed-point/integer comparisons use an exact common decimal, never f64.
        fn precision(dtype: &DataType) -> Option<(i16, i16)> {
            match dtype {
                DataType::Decimal32(p, s)
                | DataType::Decimal64(p, s)
                | DataType::Decimal128(p, s)
                | DataType::Decimal256(p, s) => Some((i16::from(*p), i16::from(*s))),
                DataType::Int64 | DataType::UInt64 => Some((20, 0)),
                _ => None,
            }
        }
        let (lp, ls) = precision(left.data_type()).ok_or_else(failure)?;
        let (rp, rs) = precision(right.data_type()).ok_or_else(failure)?;
        let scale = ls.max(rs);
        let digits = (lp - ls).max(rp - rs) + scale;
        if !(1..=76).contains(&digits) {
            return Err(failure());
        }
        let dtype = DataType::Decimal256(digits as u8, scale as i8);
        left = yss_database_arrow::lossless_cast(left.as_ref(), &dtype, false)
            .map_err(|_| failure())?;
        right = yss_database_arrow::lossless_cast(right.as_ref(), &dtype, false)
            .map_err(|_| failure())?;
    }
    if left.data_type() == right.data_type() {
        return Ok(match operation {
            ComparisonOperation::Equal => cmp::eq(&left, &right)?,
            ComparisonOperation::NotEqual => cmp::neq(&left, &right)?,
            ComparisonOperation::Less => cmp::lt(&left, &right)?,
            ComparisonOperation::LessEqual => cmp::lt_eq(&left, &right)?,
            ComparisonOperation::Greater => cmp::gt(&left, &right)?,
            ComparisonOperation::GreaterEqual => cmp::gt_eq(&left, &right)?,
        });
    }
    // Mixed integers/floats never pass through DataFusion's implicit float coercion.
    fn numeric(array: &ArrayRef, row: usize) -> Result<TabularScalar, DataFusionError> {
        if array.is_null(row) {
            return Ok(TabularScalar::Null);
        }
        if let Some(v) = array.as_any().downcast_ref::<Int64Array>() {
            return Ok(TabularScalar::Integer(v.value(row)));
        }
        if let Some(v) = array.as_any().downcast_ref::<UInt64Array>() {
            return Ok(TabularScalar::Unsigned(v.value(row)));
        }
        if let Some(v) = array.as_any().downcast_ref::<Float64Array>() {
            return Ok(TabularScalar::Decimal(
                v.value(row).try_into().map_err(|_| failure())?,
            ));
        }
        Err(failure())
    }
    let values = (0..left.len())
        .map(|row| {
            let a = numeric(&left, row)?;
            let b = numeric(&right, row)?;
            if matches!(a, TabularScalar::Null) || matches!(b, TabularScalar::Null) {
                Ok(None)
            } else {
                a.compare(&b)
                    .map(|o| Some(operation.evaluate(o)))
                    .ok_or_else(failure)
            }
        })
        .collect::<Result<Vec<_>, DataFusionError>>()?;
    Ok(BooleanArray::from(values))
}
