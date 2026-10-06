use super::*;
use arrow::datatypes::DataType;
use std::collections::{BTreeMap, BTreeSet};
use yss_database_arrow::ColumnSemantic;

pub struct DatasetColumnCast<'a> {
    pub column: &'a str,
    pub data_type: DataType,
    pub force: bool,
}

fn distinct_columns<'a>(
    names: impl Iterator<Item = &'a str>,
) -> Result<BTreeSet<&'a str>, DatasetStoreError> {
    let mut seen = BTreeSet::new();
    for name in names {
        if name.trim().is_empty() || !seen.insert(name) {
            return Err(DatasetStoreError::InvalidValue);
        }
    }
    if seen.is_empty() {
        return Err(DatasetStoreError::InvalidValue);
    }
    Ok(seen)
}

impl DatasetStore {
    pub fn prepare_column_semantic(
        self: &Arc<Self>,
        before: &Arc<DatasetSnapshot>,
        engine: &Arc<DataFusionRuntime>,
        operation: &str,
        name: &str,
        semantic: &ColumnSemantic,
        control: &RelationControl,
    ) -> Result<PreparedDataset, DatasetStoreError> {
        self.prepare_column_semantics(before, engine, operation, &[(name, semantic)], control)
    }

    pub fn prepare_column_semantics(
        self: &Arc<Self>,
        before: &Arc<DatasetSnapshot>,
        engine: &Arc<DataFusionRuntime>,
        operation: &str,
        columns: &[(&str, &ColumnSemantic)],
        control: &RelationControl,
    ) -> Result<PreparedDataset, DatasetStoreError> {
        control.check()?;
        distinct_columns(columns.iter().map(|(name, _)| *name))?;
        let replacements = columns
            .iter()
            .map(|(name, semantic)| {
                let field = yss_database_arrow::with_column_semantic(
                    user_column(before, name)?.clone(),
                    semantic,
                )
                .map_err(|_| DatasetStoreError::InvalidValue)?;
                Ok((*name, Arc::new(field)))
            })
            .collect::<Result<BTreeMap<_, _>, DatasetStoreError>>()?;
        let query = before.query(engine, "dataset-semantic")?;
        let mut invalid = false;
        let result = query.visit_columns(
            &replacements.keys().copied().collect::<Vec<_>>(),
            control,
            &mut |batch| {
                for (name, field) in &replacements {
                    let values = batch
                        .column_by_name(name)
                        .ok_or(yss_relational_contract::RelationError::InvalidInput)?;
                    yss_database_arrow::validate_semantic_array(field, values.as_ref()).map_err(
                        |_| {
                            invalid = true;
                            yss_relational_contract::RelationError::InvalidInput
                        },
                    )?;
                }
                Ok(())
            },
        );
        if invalid {
            return Err(DatasetStoreError::InvalidValue);
        }
        result?;
        let mut prepared = self.prepare_change(before, operation, false, true)?;
        prepared.metadata.schema = Arc::new(Schema::new_with_metadata(
            before
                .metadata
                .schema
                .fields()
                .iter()
                .map(|field| {
                    replacements
                        .get(field.name().as_str())
                        .unwrap_or(field)
                        .clone()
                })
                .collect::<Vec<_>>(),
            before.metadata.schema.metadata().clone(),
        ));
        Ok(prepared)
    }

    pub fn prepare_cast_column(
        self: &Arc<Self>,
        before: &Arc<DatasetSnapshot>,
        engine: &Arc<DataFusionRuntime>,
        operation: &str,
        cast: DatasetColumnCast<'_>,
        control: &RelationControl,
    ) -> Result<PreparedDataset, DatasetStoreError> {
        self.prepare_cast_columns(before, engine, operation, vec![cast], control)
    }

    pub fn prepare_cast_columns(
        self: &Arc<Self>,
        before: &Arc<DatasetSnapshot>,
        engine: &Arc<DataFusionRuntime>,
        operation: &str,
        casts: Vec<DatasetColumnCast<'_>>,
        control: &RelationControl,
    ) -> Result<PreparedDataset, DatasetStoreError> {
        control.check()?;
        distinct_columns(casts.iter().map(|cast| cast.column))?;
        for cast in &casts {
            user_column(before, cast.column)?;
        }
        let mut query = before.query(engine, "dataset-cast")?;
        let mut fields = before.metadata.schema.fields().to_vec();
        let mut schema = before.metadata.schema.clone();
        for cast in casts {
            control.check()?;
            let field = user_column(before, cast.column)?;
            let data_type = yss_database_arrow::timezone_free_data_type(&cast.data_type);
            let categories = if matches!(data_type, DataType::Dictionary(..)) {
                let mut labels = query.distinct_labels(cast.column, control)?;
                for value in yss_database_arrow::column_semantic(field)
                    .map_err(|_| DatasetStoreError::InvalidSchema)?
                    .values
                {
                    if !labels.contains(&value.value) {
                        labels.push(value.value);
                    }
                }
                Some(yss_database_arrow::CategoryDomain {
                    labels,
                    ordered: false,
                })
            } else {
                None
            };
            let id = yss_database_arrow::column_identity(field)
                .map_err(|_| DatasetStoreError::InvalidSchema)?;
            let target = yss_database_arrow::with_column_metadata(
                field.clone().with_data_type(data_type),
                id,
                categories.as_ref(),
            )
            .map_err(|_| DatasetStoreError::InvalidSchema)?;
            let semantic = yss_database_arrow::cast_column_semantic(field, &target)
                .map_err(|_| DatasetStoreError::InvalidValue)?;
            let target = yss_database_arrow::with_column_semantic(target, &semantic)
                .map_err(|_| DatasetStoreError::InvalidValue)?;
            let index = before
                .metadata
                .schema
                .index_of(cast.column)
                .map_err(|_| DatasetStoreError::InvalidValue)?;
            fields[index] = Arc::new(target);
            schema = Arc::new(Schema::new_with_metadata(
                fields.clone(),
                before.metadata.schema.metadata().clone(),
            ));
            // Each plan step retains its source type; all steps materialize and commit once.
            query = query.cast_column(cast.column, schema.clone(), cast.force)?;
        }
        self.materialize(before, operation, schema, query, true, control)
    }

    pub fn prepare_rename_column(
        self: &Arc<Self>,
        before: &Arc<DatasetSnapshot>,
        operation: &str,
        from: &str,
        to: &str,
    ) -> Result<PreparedDataset, DatasetStoreError> {
        self.prepare_rename_columns(before, operation, &[(from, to)])
    }

    pub fn prepare_rename_columns(
        self: &Arc<Self>,
        before: &Arc<DatasetSnapshot>,
        operation: &str,
        renames: &[(&str, &str)],
    ) -> Result<PreparedDataset, DatasetStoreError> {
        distinct_columns(renames.iter().map(|(from, _)| *from))?;
        distinct_columns(renames.iter().map(|(_, to)| *to))?;
        for (from, _) in renames {
            user_column(before, from)?;
        }
        let renames = renames.iter().copied().collect::<BTreeMap<_, _>>();
        let fields = before
            .metadata
            .schema
            .fields()
            .iter()
            .map(|field| match renames.get(field.name().as_str()) {
                Some(name) => Arc::new(field.as_ref().clone().with_name(*name)),
                None => field.clone(),
            })
            .collect::<Vec<_>>();
        // Validate the final namespace together, permitting swaps but never duplicate/internal names.
        distinct_columns(fields.iter().map(|field| field.name().as_str()))?;
        let mut prepared = self.prepare_change(before, operation, false, true)?;
        prepared.metadata.schema = Arc::new(Schema::new_with_metadata(
            fields,
            before.metadata.schema.metadata().clone(),
        ));
        Ok(prepared)
    }

    pub fn prepare_add_column(
        self: &Arc<Self>,
        before: &Arc<DatasetSnapshot>,
        operation: &str,
        name: &str,
        data_type: DataType,
    ) -> Result<PreparedDataset, DatasetStoreError> {
        self.prepare_add_columns(before, operation, &[(name, data_type)])
    }

    pub fn prepare_add_columns(
        self: &Arc<Self>,
        before: &Arc<DatasetSnapshot>,
        operation: &str,
        columns: &[(&str, DataType)],
    ) -> Result<PreparedDataset, DatasetStoreError> {
        distinct_columns(columns.iter().map(|(name, _)| *name))?;
        let mut fields = before.metadata.schema.fields().to_vec();
        for (name, data_type) in columns {
            if before.metadata.schema.index_of(name).is_ok() {
                return Err(DatasetStoreError::InvalidValue);
            }
            let field = yss_database_arrow::with_column_metadata(
                Field::new(
                    *name,
                    yss_database_arrow::timezone_free_data_type(data_type),
                    true,
                ),
                &Uuid::new_v4().to_string(),
                None,
            )
            .map_err(|_| DatasetStoreError::InvalidSchema)?;
            fields.push(Arc::new(field));
        }
        // Column membership changes the positional payload, even without rewriting files.
        let mut prepared = self.prepare_change(before, operation, true, true)?;
        prepared.metadata.schema = Arc::new(Schema::new_with_metadata(
            fields,
            before.metadata.schema.metadata().clone(),
        ));
        Ok(prepared)
    }

    pub fn prepare_delete_column(
        self: &Arc<Self>,
        before: &Arc<DatasetSnapshot>,
        operation: &str,
        name: &str,
    ) -> Result<PreparedDataset, DatasetStoreError> {
        self.prepare_delete_columns(before, operation, &[name])
    }

    pub fn prepare_delete_columns(
        self: &Arc<Self>,
        before: &Arc<DatasetSnapshot>,
        operation: &str,
        columns: &[&str],
    ) -> Result<PreparedDataset, DatasetStoreError> {
        let names = distinct_columns(columns.iter().copied())?;
        let ids = names
            .iter()
            .map(|name| {
                yss_database_arrow::column_identity(user_column(before, name)?)
                    .map_err(|_| DatasetStoreError::InvalidSchema)
            })
            .collect::<Result<BTreeSet<_>, _>>()?;
        if names.len() >= before.metadata.schema.fields().len().saturating_sub(2) {
            return Err(DatasetStoreError::InvalidValue);
        }
        let mut prepared = self.prepare_change(before, operation, true, true)?;
        prepared.metadata.schema = Arc::new(Schema::new_with_metadata(
            before
                .metadata
                .schema
                .fields()
                .iter()
                .filter(|field| !names.contains(field.name().as_str()))
                .cloned()
                .collect::<Vec<_>>(),
            before.metadata.schema.metadata().clone(),
        ));
        prepared.overlay.columns = prepared
            .overlay
            .columns
            .iter()
            .filter(|patch| !ids.contains(patch.column_id.as_ref()))
            .cloned()
            .collect();
        Ok(prepared)
    }
}
