use futures_util::StreamExt;
use yss_relational_contract::{
    RelationColumn, RelationControl, RelationError, RelationHandle, RelationPage,
};
use yss_tabular_contract::{TabularColumn, TabularColumnName, TabularScalar, TabularSnapshot};

pub(crate) async fn read_page(
    relation: &RelationHandle,
    offset: usize,
    limit: usize,
    control: &RelationControl,
) -> Result<RelationPage, RelationError> {
    control.check()?;
    if limit == 0 {
        return Err(RelationError::InvalidInput);
    }
    let probe = limit.checked_add(1).ok_or(RelationError::InvalidInput)?;
    let page = relation.limit(offset, probe)?;
    let schema = page.schema();
    let mut columns: Vec<Vec<TabularScalar>> = vec![Vec::new(); schema.fields().len()];
    let mut stream = page.stream(control.clone()).await?;
    let mut count = 0usize;
    let mut emitted = 0usize;
    let mut encoded_bytes = 0usize;
    while let Some(batch) = stream.next().await {
        let batch = batch?;
        control.check()?;
        if batch.get_array_memory_size() > control.max_input_bytes {
            return Err(RelationError::MemoryLimitExceeded);
        }
        count = count
            .checked_add(batch.num_rows())
            .ok_or(RelationError::MemoryLimitExceeded)?;
        let take = batch.num_rows().min(limit.saturating_sub(emitted));
        emitted += take;
        for (source, target) in batch.columns().iter().zip(&mut columns) {
            let values = yss_tabular_arrow::array_to_json(source.slice(0, take).as_ref())
                .map_err(|_| RelationError::InvalidInput)?;
            encoded_bytes = encoded_bytes
                .checked_add(
                    serde_json::to_vec(&values)
                        .map_err(|_| RelationError::InvalidInput)?
                        .len(),
                )
                .ok_or(RelationError::MemoryLimitExceeded)?;
            if encoded_bytes > control.max_input_bytes {
                return Err(RelationError::MemoryLimitExceeded);
            }
            for value in values {
                target
                    .push(serde_json::from_value(value).map_err(|_| RelationError::InvalidInput)?);
            }
        }
    }
    let data = TabularSnapshot::try_from_columns(
        schema
            .fields()
            .iter()
            .zip(columns)
            .map(|(field, values)| {
                Ok(TabularColumn::new(
                    TabularColumnName::try_from(field.name().as_str())
                        .map_err(|_| RelationError::InvalidInput)?,
                    values.into_boxed_slice(),
                ))
            })
            .collect::<Result<Box<[_]>, RelationError>>()?,
    )
    .map_err(|_| RelationError::InvalidInput)?;
    let columns = schema
        .fields()
        .iter()
        .map(|field| RelationColumn {
            name: field.name().as_str().into(),
            data_type: yss_tabular_arrow::data_type_name(field.data_type()).into(),
        })
        .collect();
    Ok(RelationPage {
        data,
        row_count: emitted,
        columns,
        has_more: count > limit,
    })
}
