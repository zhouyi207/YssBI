//! Repeated first differences, evaluated by a bounded DataFusion window operator.
use arrow::array::{Array, ArrayRef, Float64Array};
use arrow::datatypes::DataType;
use datafusion::common::{DataFusionError, ScalarValue};
use datafusion::logical_expr::{Expr, PartitionEvaluator, Volatility, expr_fn::create_udwf};
use std::{ops::Range, sync::Arc};
use yss_relational_contract::RelationError;

pub(crate) fn expression(value: Expr, order: usize) -> Result<Expr, RelationError> {
    if order == 0 {
        return Err(RelationError::InvalidInput);
    }
    Ok(create_udwf(
        &format!("yssbi_difference_{order}"),
        DataType::Float64,
        Arc::new(DataType::Float64),
        Volatility::Immutable,
        Arc::new(move || {
            Ok(Box::new(Difference {
                order,
                previous: Vec::new(),
            }))
        }),
    )
    .call(vec![value]))
}

#[derive(Debug)]
struct Difference {
    order: usize,
    previous: Vec<Option<f64>>,
}

impl PartitionEvaluator for Difference {
    fn supports_bounded_execution(&self) -> bool {
        true
    }
    fn is_causal(&self) -> bool {
        true
    }
    fn evaluate(
        &mut self,
        values: &[ArrayRef],
        range: &Range<usize>,
    ) -> datafusion::common::Result<ScalarValue> {
        let values = values[0]
            .as_any()
            .downcast_ref::<Float64Array>()
            .ok_or_else(|| DataFusionError::External(Box::new(RelationError::InvalidInput)))?;
        let mut current = (!values.is_null(range.start)).then(|| values.value(range.start));
        for previous in &mut self.previous {
            let next = current
                .zip(*previous)
                .map(|(current, previous)| current - previous);
            *previous = current;
            if next.is_some_and(|value| !value.is_finite()) {
                return Err(DataFusionError::External(Box::new(
                    RelationError::NonFiniteResult,
                )));
            }
            current = next;
        }
        // Grow with observed rows, not the requested order: order >= length yields Null.
        if self.previous.len() < self.order {
            self.previous.push(current);
            current = None;
        }
        Ok(ScalarValue::Float64(current))
    }
}
