//! Coordinate schemas depend on parameters, never on scanning observations.
use super::*;

impl EditorSchemaResolver<'_> {
    pub(super) fn resolve_multivariate_coordinates(
        &self,
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
        let parameter = protocol
            .parameters
            .iter()
            .find(|p| p.key.as_str() == "components")
            .ok_or(GraphSchemaIssue::InvalidParameter)?;
        let count = crate::parameter_projection::effective_parameter_value(node, parameter)
            .and_then(|value| value.as_u64())
            .filter(|&n| (1..=16).contains(&n))
            .ok_or(GraphSchemaIssue::InvalidParameter)? as usize;
        let prefixes = if node.node_type.as_str() == "yssbi.statistics.association.canonical" {
            vec!["x_axis", "y_axis"]
        } else {
            vec!["axis"]
        };
        Ok(prefixes
            .into_iter()
            .flat_map(|prefix| {
                (1..=count).map(move |axis| SchemaField {
                    name: SchemaColumnRef(format!("{prefix}{axis}").into()),
                    scalar_type: RelationalScalarType::Known(
                        yss_data_contract::SemanticType::Numeric,
                    ),
                    lineage: None,
                })
            })
            .collect())
    }
}
