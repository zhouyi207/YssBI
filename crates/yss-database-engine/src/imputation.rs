//! Deterministic single imputation as native expressions over the input domain.
use crate::series_transform::{checked, finite, over, plan, whole};
use arrow::datatypes::DataType;
use datafusion::{
    functions::expr_fn::coalesce,
    functions_aggregate::expr_fn as aggregate,
    logical_expr::{
        Expr,
        expr_fn::{cast, when},
    },
    prelude::lit,
};
use yss_relational_contract::{ImputationMethod, RelationError};

pub(crate) fn numeric(value: Expr, method: ImputationMethod) -> Result<Expr, RelationError> {
    let x = cast(value, DataType::Float64);
    let whole_sample = |expr| over(expr, vec![], vec![], whole());
    let replacement = match method {
        ImputationMethod::Mean => whole_sample(aggregate::avg(x.clone()))?,
        ImputationMethod::Median => whole_sample(aggregate::median(x.clone()))?,
        ImputationMethod::Mode => {
            let count = over(
                aggregate::count(x.clone()),
                vec![x.clone()],
                vec![],
                whole(),
            )?;
            let largest = whole_sample(aggregate::max(count.clone()))?;
            let candidates = when(count.eq(largest), x.clone()).end().map_err(plan)?;
            whole_sample(aggregate::min(candidates))?
        }
        ImputationMethod::Constant(value) if value.is_finite() => lit(value),
        _ => return Err(RelationError::InvalidInput),
    };
    let filled = finite(coalesce(vec![x, replacement]), &DataType::Float64, false);
    Ok(checked(
        filled.clone(),
        filled.is_not_null(),
        &DataType::Float64,
        RelationError::InvalidInput,
    ))
}
