//! Positional literal vectors share a coordinate domain; dataset row domains remain distinct.
use crate::{
    DataFusionRuntime,
    relation::DataFusionRelation,
    series_transform::{col, plan},
};
use arrow::{
    array::Array,
    datatypes::{DataType, Field, Schema},
};
use datafusion::{
    functions_nested::expr_fn::range,
    logical_expr::{ColumnarValue, Volatility, create_udf},
    prelude::lit,
};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use yss_relational_contract::{RelationControl, RelationError, SeriesHandle};

static NEXT_LITERAL: AtomicU64 = AtomicU64::new(1);
pub(crate) const POSITION: &str = "__yssbi_literal_position";
impl DataFusionRuntime {
    pub(crate) fn positional_relation(
        self: Arc<Self>,
        field: Field,
        expression: datafusion::logical_expr::Expr,
        length: usize,
    ) -> Result<yss_relational_contract::RelationHandle, RelationError> {
        let domain = {
            let mut domains = self
                .literal_domains
                .lock()
                .map_err(|_| RelationError::QueryFailed)?;
            if let Some(domain) = domains.get(&length).and_then(std::sync::Weak::upgrade) {
                domain
            } else {
                let n = i64::try_from(length).map_err(|_| RelationError::InvalidInput)?;
                let frame = self
                    .context()
                    .read_empty()
                    .map_err(plan)?
                    .select(vec![range(lit(0_i64), lit(n), lit(1_i64)).alias(POSITION)])
                    .map_err(plan)?
                    .unnest_columns(&[POSITION])
                    .map_err(plan)?;
                let domain = Arc::new(frame);
                domains.retain(|_, value| value.strong_count() > 0);
                domains.insert(length, Arc::downgrade(&domain));
                domain
            }
        };
        let frame = domain
            .as_ref()
            .clone()
            .select(vec![expression.clone().alias(field.name())])
            .map_err(plan)?;
        DataFusionRelation {
            row_identity: Default::default(),
            frame,
            schema: Arc::new(Schema::new(vec![field])),
            bindings: Arc::from([]),
            lease: Arc::new(()),
            executor: self,
            ordered_single_file: false,
            domain,
            columns: vec![expression],
            domain_order: vec![col(POSITION).sort(true, false)],
            positional_length: Some(length),
        }
        .into_handle()
    }
    pub(crate) fn literal_vector(
        self: Arc<Self>,
        field: Field,
        values: Arc<dyn Array>,
        control: &RelationControl,
    ) -> Result<SeriesHandle, RelationError> {
        control.check()?;
        let length = values.len();
        if values
            .get_array_memory_size()
            .checked_add(
                length
                    .checked_mul(64)
                    .ok_or(RelationError::MemoryLimitExceeded)?,
            )
            .is_none_or(|n| n > control.max_input_bytes)
        {
            return Err(RelationError::MemoryLimitExceeded);
        }
        let name = format!(
            "yssbi_literal_{}",
            NEXT_LITERAL.fetch_add(1, Ordering::Relaxed)
        );
        let dtype = field.data_type().clone();
        let function = create_udf(
            &name,
            vec![DataType::Int64],
            dtype,
            Volatility::Immutable,
            Arc::new(move |arguments| {
                let indices = ColumnarValue::values_to_arrays(arguments)?;
                let output = arrow::compute::take(values.as_ref(), indices[0].as_ref(), None)?;
                Ok(ColumnarValue::Array(output))
            }),
        );
        let expression = function.call(vec![col(POSITION)]);
        let relation = self.positional_relation(field.clone(), expression, length)?;
        relation.select_series(field.name())
    }
}
