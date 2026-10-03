use super::EditorSchemaResolver;
use crate::GraphSchemaIssue;
use crate::parameter_projection::effective_json_parameter;
use std::collections::BTreeSet;
use yss_data_contract::{SemanticType, TabularColumnName, aggregation::AggregateOperation};
use yss_graph_document::{NodeId, PortAddress};
use yss_node_protocol::{
    ParameterKey, RelationalScalarType, SchemaColumnRef, SchemaField, SchemaFieldLineage,
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
                .filter(|name| TabularColumnName::is_valid(name))
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
        let [connection] = self.index.input_connections(&input) else {
            return Err(GraphSchemaIssue::UnconnectedInput);
        };
        self.composition_series(&connection.output)
    }

    pub(super) fn resolve_aggregation(
        &mut self,
        node_id: NodeId,
    ) -> Result<Vec<SchemaField>, GraphSchemaIssue> {
        let node = self
            .document
            .nodes
            .get(&node_id)
            .ok_or(GraphSchemaIssue::MissingResource)?;
        let registry = self.registry;
        let parameter = |key: &str| {
            effective_json_parameter(
                node,
                &key.parse::<ParameterKey>().expect("parameter key"),
                registry,
            )
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
            "yssbi.dataframe.groupby" => {
                let input = self.resolve_input(node_id, &"source".parse().unwrap())?;
                let keys = column_names(parameter("keys").as_deref())?;
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
                    for name in column_names(parameter(operation.key()).as_deref())? {
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
