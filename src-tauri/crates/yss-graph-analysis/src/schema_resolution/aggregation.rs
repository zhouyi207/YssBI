use super::*;
use yss_data_contract::{
    SemanticType,
    aggregation::{AggregateOperation, DESCRIPTION_FIELDS, supports_description},
};

pub(crate) fn column_names(
    value: Option<&serde_json::Value>,
) -> Result<Vec<Box<str>>, GraphSchemaIssue> {
    let Some(value) = value else {
        return Ok(vec![]);
    };
    let values = value.as_array().ok_or(GraphSchemaIssue::InvalidParameter)?;
    let mut names = BTreeSet::new();
    values
        .iter()
        .map(|value| {
            let name = value
                .as_str()
                .filter(|name| !name.is_empty() && name.trim() == *name)
                .ok_or(GraphSchemaIssue::InvalidParameter)?;
            if !names.insert(name) {
                return Err(GraphSchemaIssue::InvalidParameter);
            }
            Ok(name.into())
        })
        .collect()
}

fn field(name: &str, kind: SemanticType) -> SchemaField {
    SchemaField {
        name: SchemaColumnRef(name.into()),
        scalar_type: RelationalScalarType::Known(kind),
        lineage: None,
    }
}

impl EditorSchemaResolver<'_> {
    pub(super) fn frequency_source(
        &mut self,
        node_id: NodeId,
    ) -> Result<SchemaField, GraphSchemaIssue> {
        let input = PortAddress::declared(node_id, "series".parse().unwrap());
        let source = self
            .input_sources
            .get(&input)
            .and_then(|sources| match sources.as_slice() {
                [source] => Some(source.clone()),
                _ => None,
            })
            .ok_or(GraphSchemaIssue::UnconnectedInput)?;
        self.composition_series(&source)
    }

    pub(super) fn resolve_aggregation(
        &mut self,
        node_id: NodeId,
    ) -> Result<Vec<SchemaField>, GraphSchemaIssue> {
        let node = self
            .document
            .nodes
            .get(&node_id)
            .ok_or(GraphSchemaIssue::MissingResource)?
            .clone();
        let parameter = |key: &str| {
            node.parameters
                .get(&key.parse::<ParameterKey>().expect("parameter key"))
        };
        let mut fields = match node.node_type.as_str() {
            "yssbi.dataframe.series.frequency" => {
                let mut value = self.frequency_source(node_id)?;
                value.name = SchemaColumnRef("value".into());
                vec![
                    value,
                    field("frequency", SemanticType::Numeric),
                    field("proportion", SemanticType::Numeric),
                ]
            }
            "yssbi.dataframe.describe" | "yssbi.dataframe.series.describe" => {
                if node.node_type.as_str() == "yssbi.dataframe.describe" {
                    let input = self.resolve_input(node_id, &"source".parse().unwrap())?;
                    let columns = column_names(parameter("describe_columns"))?;
                    let supported = |f: &SchemaField| matches!(f.scalar_type, RelationalScalarType::Known(s) if supports_description(s));
                    if columns.is_empty() {
                        if !input.iter().any(supported) {
                            return Err(GraphSchemaIssue::InvalidParameter);
                        }
                    } else if columns
                        .iter()
                        .any(|name| !input.iter().any(|f| f.name.0 == *name && supported(f)))
                    {
                        return Err(GraphSchemaIssue::InvalidParameter);
                    }
                }
                DESCRIPTION_FIELDS
                    .iter()
                    .map(|(name, kind)| field(name, *kind))
                    .collect()
            }
            "yssbi.dataframe.groupby" => {
                let input = self.resolve_input(node_id, &"source".parse().unwrap())?;
                let keys = column_names(parameter("keys"))?;
                if keys.is_empty() {
                    return Err(GraphSchemaIssue::InvalidParameter);
                }
                let mut fields = keys
                    .iter()
                    .map(|name| {
                        input
                            .iter()
                            .find(|f| f.name.0 == *name)
                            .cloned()
                            .ok_or(GraphSchemaIssue::MissingColumn)
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                fields.push(field("row_count", SemanticType::Numeric));
                for operation in AggregateOperation::ALL {
                    for name in column_names(parameter(operation.key()))? {
                        let source = input
                            .iter()
                            .find(|f| f.name.0 == name)
                            .ok_or(GraphSchemaIssue::MissingColumn)?;
                        if !matches!(source.scalar_type, RelationalScalarType::Known(s) if operation.accepts(s))
                        {
                            return Err(GraphSchemaIssue::InvalidParameter);
                        }
                        fields.push(field(&operation.output_name(&name), SemanticType::Numeric));
                    }
                }
                if fields
                    .iter()
                    .map(|f| &f.name.0)
                    .collect::<BTreeSet<_>>()
                    .len()
                    != fields.len()
                {
                    return Err(GraphSchemaIssue::InvalidParameter);
                }
                fields
            }
            _ => return Err(GraphSchemaIssue::UnsupportedResolver),
        };
        for field in &mut fields {
            field.lineage = Some(SchemaFieldLineage {
                source: format!("graph:{node_id}:aggregate").into(),
                field: field.name.0.clone(),
            });
        }
        Ok(fields)
    }
}
