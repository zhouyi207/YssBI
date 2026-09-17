use crate::relation::DataFusionRelation;
use arrow::datatypes::{Field, Schema, SchemaRef};
use datafusion::{
    common::{
        Column, ScalarValue,
        tree_node::{Transformed, TreeNode},
    },
    dataframe::DataFrame,
    logical_expr::{Expr, ExprFunctionExt, JoinType, expr::Sort},
};
use std::sync::Arc;
use yss_data_contract::table::{RowConcatMode, TableJoin, TableJoinKind, append_column_names};
use yss_relational_contract::{RelationBinding, RelationError, RelationHandle};

fn column(name: &str) -> Expr {
    Expr::Column(Column::from_name(name.to_owned()))
}

fn meaning(field: &Field) -> Result<yss_data_contract::ColumnSemantic, RelationError> {
    let mut semantic =
        yss_tabular_arrow::column_semantic(field).map_err(|_| RelationError::InvalidInput)?;
    if semantic.kind == yss_data_contract::SemanticType::Binary {
        if field.data_type() == &arrow::datatypes::DataType::Boolean
            && semantic.positive_value.is_none()
        {
            semantic.positive_value = Some("true".into());
        }
        semantic
            .values
            .sort_by(|left, right| left.value.cmp(&right.value));
    }
    Ok(semantic)
}
pub(crate) fn derived_field(field: &Field, name: &str) -> Field {
    let mut metadata = field.metadata().clone();
    metadata.remove("yssbi.column_id");
    field.clone().with_name(name).with_metadata(metadata)
}
fn adapter(handle: &RelationHandle) -> Result<&DataFusionRelation, RelationError> {
    handle
        .plan()
        .as_any()
        .downcast_ref::<DataFusionRelation>()
        .ok_or(RelationError::InvalidInput)
}

impl DataFusionRelation {
    fn with_position(&self, name: &str) -> Result<DataFrame, RelationError> {
        let mut internal = name.to_owned();
        while self
            .domain
            .schema()
            .index_of_column_by_name(None, &internal)
            .is_some()
        {
            internal.push('_');
        }
        let number = datafusion::functions_window::expr_fn::row_number()
            .order_by(self.domain_order.clone())
            .build()
            .map_err(|_| RelationError::InvalidPlan)?
            .alias(&internal);
        let frame = self
            .domain
            .as_ref()
            .clone()
            .window(vec![number])
            .map_err(|_| RelationError::InvalidPlan)?;
        let mut expressions = self
            .columns
            .iter()
            .zip(self.schema.fields())
            .map(|(expr, field)| expr.clone().alias(field.name()))
            .collect::<Vec<_>>();
        expressions.push(column(&internal).alias(name));
        frame
            .select(expressions)
            .map_err(|_| RelationError::InvalidPlan)
    }

    pub(crate) fn expand_expression(&self, expression: Expr) -> Result<Expr, RelationError> {
        expression
            .transform_up(|expression| {
                if let Expr::Column(column) = &expression {
                    let index = self.schema.index_of(&column.name)?;
                    Ok(Transformed::yes(self.columns[index].clone()))
                } else {
                    Ok(Transformed::no(expression))
                }
            })
            .map(|value| value.data)
            .map_err(|_| RelationError::InvalidPlan)
    }

    pub(crate) fn project_expressions(
        &self,
        fields: Vec<Field>,
        expressions: Vec<Expr>,
    ) -> Result<RelationHandle, RelationError> {
        let frame = self
            .domain
            .as_ref()
            .clone()
            .select(
                expressions
                    .iter()
                    .zip(&fields)
                    .map(|(expr, field)| expr.clone().alias(field.name())),
            )
            .map_err(|_| RelationError::InvalidPlan)?;
        Self {
            frame,
            schema: Arc::new(Schema::new_with_metadata(
                fields,
                self.schema.metadata().clone(),
            )),
            bindings: self.bindings.clone(),
            lease: self.lease.clone(),
            executor: self.executor.clone(),
            ordered_single_file: self.ordered_single_file,
            domain: self.domain.clone(),
            domain_order: self.domain_order.clone(),
            columns: expressions,
        }
        .into_handle()
    }

    fn combined(
        &self,
        frame: DataFrame,
        schema: SchemaRef,
        inputs: &[&Self],
        domain_order: Vec<Sort>,
    ) -> Result<RelationHandle, RelationError> {
        let mut bindings: Vec<RelationBinding> = Vec::new();
        let mut leases = Vec::new();
        let session = self
            .bindings
            .first()
            .ok_or(RelationError::InvalidInput)?
            .project_session
            .as_ref();
        for input in std::iter::once(self).chain(inputs.iter().copied()) {
            if !Arc::ptr_eq(&self.executor, &input.executor) {
                return Err(RelationError::InvalidInput);
            }
            for binding in input.bindings.iter() {
                if binding.project_session.as_ref() != session {
                    return Err(RelationError::InvalidInput);
                }
                if let Some(previous) = bindings
                    .iter()
                    .find(|previous| previous.dataset == binding.dataset)
                {
                    if previous != binding {
                        return Err(RelationError::InvalidInput);
                    }
                } else {
                    bindings.push(binding.clone());
                }
            }
            leases.push(input.lease.clone());
        }
        let columns = schema
            .fields()
            .iter()
            .map(|field| column(field.name()))
            .collect();
        let domain = Arc::new(frame);
        let frame = domain
            .as_ref()
            .clone()
            .select(schema.fields().iter().map(|field| column(field.name())))
            .map_err(|_| RelationError::InvalidPlan)?;
        Self {
            domain,
            domain_order,
            columns,
            frame,
            schema,
            bindings: bindings.into(),
            lease: Arc::new(leases),
            executor: self.executor.clone(),
            ordered_single_file: false,
        }
        .into_handle()
    }

    pub(crate) fn compose_columns(
        &self,
        others: &[RelationHandle],
    ) -> Result<RelationHandle, RelationError> {
        let inputs = others.iter().map(adapter).collect::<Result<Vec<_>, _>>()?;
        if inputs
            .iter()
            .any(|other| !Arc::ptr_eq(&self.domain, &other.domain))
        {
            return Err(RelationError::UnalignedSeries);
        }
        let mut fields = self
            .schema
            .fields()
            .iter()
            .map(|f| derived_field(f, f.name()))
            .collect::<Vec<_>>();
        let mut expressions = self.columns.clone();
        for (index, input) in inputs.iter().enumerate() {
            let names = append_column_names(
                &fields.iter().map(|f| f.name().clone()).collect::<Vec<_>>(),
                &input
                    .schema
                    .fields()
                    .iter()
                    .map(|f| f.name().clone())
                    .collect::<Vec<_>>(),
                &format!("_{}", index + 2),
            );
            fields.extend(
                input
                    .schema
                    .fields()
                    .iter()
                    .zip(names)
                    .map(|(field, name)| derived_field(field, &name)),
            );
            expressions.extend(input.columns.iter().cloned());
        }
        self.project_expressions(fields, expressions)
    }

    pub(crate) fn compose_rows(
        &self,
        others: &[RelationHandle],
        mode: RowConcatMode,
    ) -> Result<RelationHandle, RelationError> {
        let inputs = others.iter().map(adapter).collect::<Result<Vec<_>, _>>()?;
        let sources = std::iter::once(self)
            .chain(inputs.iter().copied())
            .collect::<Vec<_>>();
        let mut fields = self
            .schema
            .fields()
            .iter()
            .map(|f| derived_field(f, f.name()))
            .collect::<Vec<_>>();
        for input in &inputs {
            if mode == RowConcatMode::ByPosition && input.schema.fields().len() != fields.len() {
                return Err(RelationError::InvalidInput);
            }
            for (index, field) in input.schema.fields().iter().enumerate() {
                let target = match mode {
                    RowConcatMode::ByPosition => Some(index),
                    RowConcatMode::ByName => fields.iter().position(|f| f.name() == field.name()),
                };
                if let Some(target) = target {
                    if fields[target].data_type() != field.data_type()
                        || meaning(&fields[target])? != meaning(field)?
                    {
                        return Err(RelationError::InvalidInput);
                    }
                    fields[target] = fields[target]
                        .clone()
                        .with_nullable(fields[target].is_nullable() || field.is_nullable());
                } else {
                    fields.push(derived_field(field, field.name()));
                }
            }
        }
        if mode == RowConcatMode::ByName {
            for field in &mut fields {
                if sources
                    .iter()
                    .any(|source| source.schema.index_of(field.name()).is_err())
                {
                    *field = field.clone().with_nullable(true);
                }
            }
        }
        let mut frames = Vec::new();
        // Stable source positions are derived from the explicit row-domain ordering.
        let mut tag = "__yss_concat_source".to_owned();
        while fields.iter().any(|field| field.name() == &tag) {
            tag.push('_');
        }
        let mut position = format!("{tag}_row");
        while fields.iter().any(|field| field.name() == &position) {
            position.push('_');
        }
        for (ordinal, input) in sources.iter().enumerate() {
            let mut expressions = fields
                .iter()
                .enumerate()
                .map(|(index, field)| {
                    let source = match mode {
                        RowConcatMode::ByPosition => {
                            input.schema.fields().get(index).map(|f| f.name().as_str())
                        }
                        RowConcatMode::ByName => input
                            .schema
                            .field_with_name(field.name())
                            .ok()
                            .map(|f| f.name().as_str()),
                    };
                    let value = if let Some(source) = source {
                        column(source)
                    } else {
                        Expr::Literal(
                            ScalarValue::try_from(field.data_type())
                                .map_err(|_| RelationError::InvalidInput)?,
                            None,
                        )
                    };
                    Ok(value.alias(field.name()))
                })
                .collect::<Result<Vec<_>, RelationError>>()?;
            expressions
                .push(Expr::Literal(ScalarValue::UInt64(Some(ordinal as u64)), None).alias(&tag));
            expressions.push(column(&position));
            frames.push(
                input
                    .with_position(&position)?
                    .select(expressions)
                    .map_err(|_| RelationError::InvalidPlan)?,
            );
        }
        let mut frames = frames.into_iter();
        let mut frame = frames.next().ok_or(RelationError::InvalidInput)?;
        for other in frames {
            frame = frame.union(other).map_err(|_| RelationError::InvalidPlan)?;
        }
        let order = vec![
            column(&tag).sort(true, true),
            column(&position).sort(true, true),
        ];
        let frame = frame
            .sort(order.clone())
            .map_err(|_| RelationError::InvalidPlan)?;
        self.combined(frame, Arc::new(Schema::new(fields)), &inputs, order)
    }

    pub(crate) fn compose_join(
        &self,
        right: &RelationHandle,
        spec: &TableJoin,
    ) -> Result<RelationHandle, RelationError> {
        let right = adapter(right)?;
        if !spec.is_valid() {
            return Err(RelationError::InvalidInput);
        }
        for (left_key, right_key) in spec.left_keys.iter().zip(&spec.right_keys) {
            let left = self
                .schema
                .field_with_name(left_key)
                .map_err(|_| RelationError::InvalidInput)?;
            let right = right
                .schema
                .field_with_name(right_key)
                .map_err(|_| RelationError::InvalidInput)?;
            let lm = meaning(left)?;
            let rm = meaning(right)?;
            if left.data_type() != right.data_type()
                || lm.kind != rm.kind
                || lm.positive_value != rm.positive_value
            {
                return Err(RelationError::InvalidInput);
            }
        }
        let mut left_position = "__yss_left_row".to_owned();
        while self.schema.index_of(&left_position).is_ok()
            || right.schema.index_of(&left_position).is_ok()
        {
            left_position.push('_');
        }
        let mut right_position = "__yss_right_row".to_owned();
        while self.schema.index_of(&right_position).is_ok()
            || right.schema.index_of(&right_position).is_ok()
        {
            right_position.push('_');
        }
        let left_frame = self
            .with_position(&left_position)?
            .alias("__left")
            .map_err(|_| RelationError::InvalidPlan)?;
        let right_frame = right
            .with_position(&right_position)?
            .alias("__right")
            .map_err(|_| RelationError::InvalidPlan)?;
        let kind = match spec.kind {
            TableJoinKind::Inner => JoinType::Inner,
            TableJoinKind::Left => JoinType::Left,
            TableJoinKind::Right => JoinType::Right,
            TableJoinKind::Full => JoinType::Full,
        };
        let keys = spec
            .left_keys
            .iter()
            .zip(&spec.right_keys)
            .map(|(left, right)| {
                Expr::Column(Column::new(Some("__left"), left))
                    .eq(Expr::Column(Column::new(Some("__right"), right)))
            });
        let frame = left_frame
            .join_on(right_frame, kind, keys)
            .map_err(|_| RelationError::InvalidPlan)?;
        let right_names = append_column_names(
            &self
                .schema
                .fields()
                .iter()
                .map(|f| f.name().clone())
                .collect::<Vec<_>>(),
            &right
                .schema
                .fields()
                .iter()
                .map(|f| f.name().clone())
                .collect::<Vec<_>>(),
            &spec.right_suffix,
        );
        let mut fields = Vec::new();
        let mut expressions = Vec::new();
        for (schema, names, side, nullable) in [
            (
                &self.schema,
                self.schema
                    .fields()
                    .iter()
                    .map(|f| f.name().clone())
                    .collect::<Vec<_>>(),
                "__left",
                matches!(spec.kind, TableJoinKind::Right | TableJoinKind::Full),
            ),
            (
                &right.schema,
                right_names,
                "__right",
                matches!(spec.kind, TableJoinKind::Left | TableJoinKind::Full),
            ),
        ] {
            for (field, name) in schema.fields().iter().zip(names) {
                fields.push(
                    derived_field(field, &name).with_nullable(nullable || field.is_nullable()),
                );
                expressions.push(Expr::Column(Column::new(Some(side), field.name())).alias(&name));
            }
        }
        let mut order = Vec::new();
        for (side, position) in [("__left", left_position), ("__right", right_position)] {
            let mut name = position.clone();
            while fields.iter().any(|field| field.name() == &name) {
                name.push('_');
            }
            expressions.push(Expr::Column(Column::new(Some(side), position)).alias(&name));
            order.push(column(&name).sort(true, false));
        }
        let frame = frame
            .select(expressions)
            .and_then(|frame| frame.sort(order.clone()))
            .map_err(|_| RelationError::InvalidPlan)?;
        self.combined(frame, Arc::new(Schema::new(fields)), &[right], order)
    }
}
