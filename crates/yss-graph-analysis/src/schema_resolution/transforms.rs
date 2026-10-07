//! Static transformation schemas are derived from declared fields and parameters only.
use super::{EditorSchemaResolver, aggregation};
use crate::GraphSchemaIssue;
use std::collections::{BTreeMap, BTreeSet};
use yss_data_contract::{SemanticType, TabularColumnName};
use yss_graph_document::{NodeId, PortAddress};
use yss_node_protocol::{RelationalScalarType, SchemaColumnRef, SchemaField, SchemaFieldLineage};

fn field(name: &str, kind: SemanticType) -> SchemaField {
    SchemaField {
        name: SchemaColumnRef(name.into()),
        scalar_type: RelationalScalarType::Known(kind),
        lineage: None,
    }
}
impl EditorSchemaResolver<'_> {
    pub(super) fn resolve_transform(
        &mut self,
        node_id: NodeId,
    ) -> Result<Vec<SchemaField>, GraphSchemaIssue> {
        let node = self
            .document
            .nodes
            .get(&node_id)
            .ok_or(GraphSchemaIssue::MissingResource)?;
        let protocol = self
            .registry
            .protocol(&node.node_type)
            .ok_or(GraphSchemaIssue::MissingResource)?;
        let parameters = protocol
            .parameters
            .iter()
            .filter(|p| protocol.parameters.is_visible(p, &node.parameters))
            .filter_map(|p| {
                crate::parameter_projection::effective_parameter_value(node, p)
                    .map(|v| (p.key.as_str(), v))
            })
            .collect::<BTreeMap<_, _>>();
        let parameter = |key: &str| parameters.get(key).map(|value| value.as_ref());
        let column_name = |key: &str| {
            parameter(key)
                .and_then(serde_json::Value::as_str)
                .filter(|s| TabularColumnName::is_valid(s))
                .map(str::to_owned)
                .ok_or(GraphSchemaIssue::InvalidParameter)
        };
        let names = |key: &str| aggregation::column_names(parameter(key));
        let mut fields = if node.node_type.as_str() == "yssbi.dataframe.encode" {
            let source = self.frequency_source(node_id)?;
            if !matches!(
                source.scalar_type,
                RelationalScalarType::Known(
                    SemanticType::Categorical | SemanticType::Ordinal | SemanticType::Binary
                )
            ) {
                return Err(GraphSchemaIssue::InvalidParameter);
            }
            let levels = parameter("levels")
                .and_then(serde_json::Value::as_array)
                .ok_or(GraphSchemaIssue::InvalidParameter)?;
            let outputs = names("names")?;
            if levels.is_empty()
                || levels.len() != outputs.len()
                || levels.iter().any(|v| !v.is_string())
                || levels
                    .iter()
                    .enumerate()
                    .any(|(i, v)| levels[..i].contains(v))
            {
                return Err(GraphSchemaIssue::InvalidParameter);
            }
            let reference = if parameter("drop_reference")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false)
            {
                Some(column_name("reference")?)
            } else {
                None
            };
            if reference
                .as_ref()
                .is_some_and(|r| !outputs.iter().any(|n| n.as_ref() == r))
            {
                return Err(GraphSchemaIssue::InvalidParameter);
            }
            outputs
                .iter()
                .filter(|name| reference.as_ref().is_none_or(|r| name.as_ref() != r))
                .map(|name| field(name, SemanticType::Binary))
                .collect::<Vec<_>>()
        } else {
            let input = self.resolve_input(node_id, &"source".parse().unwrap())?;
            let select = |keys: &[Box<str>]| {
                keys.iter()
                    .map(|name| {
                        input
                            .iter()
                            .find(|f| f.name.0 == *name)
                            .cloned()
                            .ok_or(GraphSchemaIssue::MissingColumn)
                    })
                    .collect::<Result<Vec<_>, _>>()
            };
            match node.node_type.as_str() {
                "yssbi.dataframe.sort" => {
                    let columns = names("columns")?;
                    if columns.is_empty() {
                        return Err(GraphSchemaIssue::InvalidParameter);
                    }
                    select(&columns)?;
                    if names("descending_columns")?
                        .iter()
                        .any(|name| !columns.contains(name))
                    {
                        return Err(GraphSchemaIssue::InvalidParameter);
                    }
                    input
                }
                "yssbi.dataframe.deduplicate" => {
                    select(&names("keys")?)?;
                    input
                }
                "yssbi.dataframe.filter.mask" => input,
                "yssbi.dataframe.set_column" => {
                    let name = column_name("name")?;
                    let address = PortAddress::declared(node_id, "series".parse().unwrap());
                    let [connection] = self.index.input_connections(&address) else {
                        return Err(GraphSchemaIssue::UnconnectedInput);
                    };
                    let mut added = self.composition_series(&connection.output)?;
                    added.name = SchemaColumnRef(name.clone().into());
                    added.lineage = Some(SchemaFieldLineage {
                        source: format!("graph:{node_id}:transform").into(),
                        field: name.clone().into(),
                    });
                    let mut input = input;
                    if let Some(index) = input.iter().position(|f| f.name.0.as_ref() == name) {
                        input[index] = added;
                    } else {
                        input.push(added);
                    }
                    input
                }
                "yssbi.dataframe.unpivot" => {
                    let keys = names("keys")?;
                    let columns = names("columns")?;
                    if columns.is_empty() || columns.iter().any(|c| keys.contains(c)) {
                        return Err(GraphSchemaIssue::InvalidParameter);
                    }
                    let values = select(&columns)?;
                    if values
                        .iter()
                        .any(|f| f.scalar_type != values[0].scalar_type)
                    {
                        return Err(GraphSchemaIssue::ConflictingInputs);
                    }
                    let variable = column_name("variable_name")?;
                    let value = column_name("value_name")?;
                    let mut output = select(&keys)?;
                    output.push(field(&variable, SemanticType::Text));
                    output.push(SchemaField {
                        name: SchemaColumnRef(value.into()),
                        scalar_type: values[0].scalar_type,
                        lineage: None,
                    });
                    output
                }
                "yssbi.dataframe.pivot" => {
                    let keys = names("keys")?;
                    let category = column_name("category_column")?;
                    let value = column_name("value_column")?;
                    select(&[category.into()])?;
                    let source = select(&[value.into()])?.remove(0);
                    if parameter("aggregate").and_then(serde_json::Value::as_str) != Some("count")
                        && source.scalar_type != RelationalScalarType::Known(SemanticType::Numeric)
                    {
                        return Err(GraphSchemaIssue::InvalidParameter);
                    }
                    let levels = parameter("levels")
                        .and_then(serde_json::Value::as_array)
                        .ok_or(GraphSchemaIssue::InvalidParameter)?;
                    let outputs = names("names")?;
                    if levels.is_empty()
                        || levels.len() != outputs.len()
                        || levels.iter().any(|v| !v.is_string())
                        || levels
                            .iter()
                            .enumerate()
                            .any(|(i, v)| levels[..i].contains(v))
                    {
                        return Err(GraphSchemaIssue::InvalidParameter);
                    }
                    let mut output = select(&keys)?;
                    output.extend(
                        outputs
                            .iter()
                            .map(|name| field(name, SemanticType::Numeric)),
                    );
                    output
                }
                "yssbi.dataframe.resample" => {
                    let time = column_name("time_column")?;
                    let mut output = select(&[time.clone().into()])?;
                    if output[0].scalar_type != RelationalScalarType::Known(SemanticType::Datetime)
                    {
                        return Err(GraphSchemaIssue::InvalidParameter);
                    }
                    let keys = names("keys")?;
                    if keys.iter().any(|key| key.as_ref() == time) {
                        return Err(GraphSchemaIssue::InvalidParameter);
                    }
                    output.extend(select(&keys)?);
                    let columns = names("columns")?;
                    if columns.is_empty() {
                        return Err(GraphSchemaIssue::InvalidParameter);
                    }
                    let operation = parameter("aggregate")
                        .and_then(serde_json::Value::as_str)
                        .filter(|value| !value.is_empty() && value.trim() == *value)
                        .ok_or(GraphSchemaIssue::InvalidParameter)?;
                    for source in select(&columns)? {
                        if operation != "count"
                            && source.scalar_type
                                != RelationalScalarType::Known(SemanticType::Numeric)
                        {
                            return Err(GraphSchemaIssue::InvalidParameter);
                        }
                        output.push(field(
                            &format!("{}_{operation}", source.name.0),
                            SemanticType::Numeric,
                        ));
                    }
                    output
                }
                _ => return Err(GraphSchemaIssue::UnsupportedResolver),
            }
        };
        if fields.is_empty()
            || fields
                .iter()
                .map(|f| &f.name.0)
                .collect::<BTreeSet<_>>()
                .len()
                != fields.len()
        {
            return Err(GraphSchemaIssue::InvalidParameter);
        }
        for f in &mut fields {
            if f.lineage.is_none() {
                f.lineage = Some(SchemaFieldLineage {
                    source: format!("graph:{node_id}:transform").into(),
                    field: f.name.0.clone(),
                });
            }
        }
        Ok(fields)
    }
}
