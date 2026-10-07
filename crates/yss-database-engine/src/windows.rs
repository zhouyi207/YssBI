//! Lift nested window expressions into successive native Window operators.
use crate::relation::DataFusionRelation;
use datafusion::{
    common::{
        Column,
        tree_node::{Transformed, TreeNode, TreeNodeRecursion},
    },
    dataframe::DataFrame,
    logical_expr::{Expr, expr::Sort},
};
use yss_relational_contract::RelationError;

fn contains_window(expr: &Expr) -> bool {
    let mut found = false;
    let _ = expr.apply(|expr| {
        if matches!(expr, Expr::WindowFunction(_)) {
            found = true;
            Ok(TreeNodeRecursion::Stop)
        } else {
            Ok(TreeNodeRecursion::Continue)
        }
    });
    found
}

pub(crate) fn select(
    mut frame: DataFrame,
    mut expressions: Vec<Expr>,
    order: &[Sort],
) -> Result<DataFrame, RelationError> {
    loop {
        let mut windows = Vec::new();
        for expression in &expressions {
            expression
                .apply(|expr| {
                    if let Expr::WindowFunction(window) = expr {
                        let p = &window.params;
                        if !p
                            .args
                            .iter()
                            .chain(p.partition_by.iter())
                            .any(contains_window)
                            && !p.order_by.iter().any(|s| contains_window(&s.expr))
                            && !p.filter.as_deref().is_some_and(contains_window)
                            && !windows.contains(expr)
                        {
                            windows.push(expr.clone());
                        }
                    }
                    Ok(TreeNodeRecursion::Continue)
                })
                .map_err(|_| RelationError::InvalidPlan)?;
        }
        if windows.is_empty() {
            break;
        }
        let mut aliases = Vec::new();
        for (index, window) in windows.into_iter().enumerate() {
            let mut name = format!("__yssbi_window_{index}");
            while frame
                .schema()
                .index_of_column_by_name(None, &name)
                .is_some()
                || aliases.iter().any(|(_, n)| n == &name)
            {
                name.push('_');
            }
            aliases.push((window, name));
        }
        frame = frame
            .window(
                aliases
                    .iter()
                    .map(|(expr, name)| expr.clone().alias(name))
                    .collect(),
            )
            .map_err(|_| RelationError::InvalidPlan)?;
        expressions = expressions
            .into_iter()
            .map(|expr| {
                expr.transform_up(|expr| {
                    Ok(match aliases.iter().find(|(window, _)| window == &expr) {
                        Some((_, name)) => {
                            Transformed::yes(Expr::Column(Column::from_name(name.clone())))
                        }
                        None => Transformed::no(expr),
                    })
                })
                .map(|value| value.data)
                .map_err(|_| RelationError::InvalidPlan)
            })
            .collect::<Result<_, _>>()?;
    }
    if !order.is_empty() {
        frame = frame
            .sort(order.to_vec())
            .map_err(|_| RelationError::InvalidPlan)?;
    }
    frame
        .select(expressions)
        .map_err(|_| RelationError::InvalidPlan)
}

impl DataFusionRelation {
    pub(crate) fn select_native(&self, expressions: Vec<Expr>) -> Result<DataFrame, RelationError> {
        select(
            self.domain.as_ref().clone(),
            expressions,
            &self.domain_order,
        )
    }
}
