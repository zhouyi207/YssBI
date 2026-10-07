use super::{ResultPageKind, StoredResult};
use yss_data_contract::ValueType;
use yss_node_kernel::RuntimeValue;
use yss_relational_contract::RelationColumn;

/// A read projection shared by descriptors and pages. Constructing it never scans a relation.
pub(crate) struct ResultStructure {
    pub kind: ResultPageKind,
    pub total_count: Option<usize>,
    /// None means a deferred relation schema, not a table with zero columns.
    pub columns: Option<Box<[RelationColumn]>>,
}

impl ResultStructure {
    pub fn project(result: &StoredResult) -> Self {
        let column = |field: &arrow::datatypes::Field| RelationColumn {
            name: field.name().as_str().into(),
            data_type: yss_database_arrow::data_type_name(field.data_type()).into(),
        };
        match result.value().unannotated() {
            RuntimeValue::Relation(relation) => Self {
                kind: ResultPageKind::Sequence,
                total_count: None,
                columns: (!relation.schema_is_deferred()).then(|| {
                    relation
                        .schema()
                        .fields()
                        .iter()
                        .map(|f| column(f))
                        .collect()
                }),
            },
            RuntimeValue::Series(series) => Self {
                kind: ResultPageKind::Sequence,
                total_count: None,
                columns: Some(Box::new([column(series.plan().field())])),
            },
            RuntimeValue::List(values) => {
                let contract = result.output_contract();
                let field = contract
                    .and_then(|c| c.schema.as_deref())
                    .filter(|fields| fields.len() == 1)
                    .map(|fields| &fields[0]);
                let element = match contract.map(|c| &c.data_type) {
                    Some(ValueType::DataSeries(element) | ValueType::Array(element)) => {
                        element.as_ref()
                    }
                    _ => &ValueType::Any,
                };
                // Numeric is a semantic type, not a promise of Float64 storage. Keep the
                // declared type even for empty/all-null data; never sample a row to infer it.
                let data_type = result
                    .value()
                    .metadata()
                    .map(|metadata| metadata.semantic.kind.to_string())
                    .unwrap_or_else(|| field.map_or(element, |f| &f.data_type).to_string());
                Self {
                    kind: ResultPageKind::Sequence,
                    total_count: Some(values.len()),
                    columns: Some(Box::new([RelationColumn {
                        name: field.map_or("value", |f| f.name.as_ref()).into(),
                        data_type: data_type.into(),
                    }])),
                }
            }
            _ => Self {
                kind: ResultPageKind::Scalar,
                total_count: Some(1),
                columns: None,
            },
        }
    }
}
