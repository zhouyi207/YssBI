use super::EditorSchemaResolver;
use crate::GraphSchemaIssue;
use crate::parameter_projection::{effective_json_parameter, effective_text_parameter};
use yss_data_contract::{ValueType, table::append_column_names};
use yss_graph_document::{DynamicMemberLocator, DynamicPortBinding, NodeId, PortAddress, PortRef};
use yss_node_protocol::{
    NodeTypingSpec, ParameterKey, RelationalScalarType, SchemaColumnRef, SchemaField,
    SchemaFieldLineage, SemanticType, TypeExpr,
};

fn declared_semantic(value: &TypeExpr) -> Option<SemanticType> {
    match value {
        TypeExpr::Concrete(id) => SemanticType::ALL
            .into_iter()
            .find(|kind| kind.type_id() == id.as_str()),
        TypeExpr::Class(id) => SemanticType::ALL
            .into_iter()
            .find(|kind| kind.type_id() == id.as_str()),
        TypeExpr::Applied {
            constructor,
            arguments,
        } if constructor.as_str() == "core.data_series" => match arguments.as_slice() {
            [element] => declared_semantic(element),
            _ => None,
        },
        TypeExpr::Union(members) => {
            let first = declared_semantic(members.first()?)?;
            members
                .iter()
                .all(|member| declared_semantic(member) == Some(first))
                .then_some(first)
        }
        _ => None,
    }
}

fn scalar_semantic(value: &ValueType) -> Option<SemanticType> {
    let element = match value {
        ValueType::DataSeries(element) => element.as_ref(),
        _ => value,
    };
    match element {
        ValueType::Scalar(semantic) => Some(*semantic),
        _ => None,
    }
}

impl<'a> EditorSchemaResolver<'a> {
    fn composition_inputs(
        &self,
        node: NodeId,
        template: &str,
    ) -> Result<Vec<&'a PortAddress>, GraphSchemaIssue> {
        let mut ports = self.index.node_bindings(node).iter().filter_map(|(address, binding)| {
            if !matches!(&address.port, PortRef::Instance { template: key, .. } if key.as_str() == template) { return None; }
            match binding { DynamicPortBinding::UserCreated { order } => Some((order, *address)), _ => None }
        }).collect::<Vec<_>>();
        ports.sort_by(|a, b| a.0.cmp(b.0).then(a.1.cmp(b.1)));
        ports
            .into_iter()
            .map(|(_, input)| match self.index.input_connections(input) {
                [connection] => Ok(&connection.output),
                _ => Err(GraphSchemaIssue::UnconnectedInput),
            })
            .collect()
    }

    fn composition_fields(
        &mut self,
        source: &PortAddress,
    ) -> Result<Vec<SchemaField>, GraphSchemaIssue> {
        let state = self.resolve_output(source);
        state.exact().map(|s| s.fields.clone()).ok_or_else(|| {
            state
                .issue()
                .unwrap_or(GraphSchemaIssue::UnresolvedUpstream)
        })
    }

    pub(super) fn composition_series(
        &mut self,
        source: &PortAddress,
    ) -> Result<SchemaField, GraphSchemaIssue> {
        if let Some(field) = self.series_fields.get(source) {
            return field.clone();
        }
        let field = self.resolve_series_field(source);
        self.series_fields.insert(source.clone(), field.clone());
        field
    }

    fn resolve_series_field(
        &mut self,
        source: &PortAddress,
    ) -> Result<SchemaField, GraphSchemaIssue> {
        let node = self
            .document
            .nodes
            .get(&source.node_id)
            .ok_or(GraphSchemaIssue::MissingResource)?;
        let protocol = self
            .registry
            .protocol(&node.node_type)
            .ok_or(GraphSchemaIssue::MissingResource)?;
        if node.node_type.as_str() == "yssbi.dataframe.decompose" {
            let fields = self.resolve_input(node.id, &"dataframe".parse().unwrap())?;
            let Some(DynamicMemberLocator::SchemaField {
                source: origin,
                field,
            }) = self
                .document
                .port_bindings
                .get(source)
                .and_then(crate::port_projection::binding_origin)
            else {
                return Err(GraphSchemaIssue::MissingColumn);
            };
            return fields
                .into_iter()
                .find(|f| {
                    f.lineage
                        .as_ref()
                        .map_or(f.name.0.as_ref() == field.as_str(), |lineage| {
                            lineage.source.as_ref() == origin.as_str()
                                && lineage.field.as_ref() == field.as_str()
                        })
                })
                .ok_or(GraphSchemaIssue::MissingColumn);
        }
        if let NodeTypingSpec::ColumnOutput { input, column, .. } = &protocol.typing {
            let selected = effective_text_parameter(node, column, self.registry)
                .ok_or(GraphSchemaIssue::InvalidParameter)?;
            return self
                .resolve_input(node.id, input)?
                .into_iter()
                .find(|field| field.name.0.as_ref() == selected)
                .ok_or(GraphSchemaIssue::MissingColumn);
        }
        if node.node_type.as_str() == "yssbi.dataframe.series.choose" {
            let mut fields = Vec::new();
            for key in ["when_true", "when_false"] {
                let input = PortAddress::declared(node.id, key.parse().unwrap());
                let [connection] = self.index.input_connections(&input) else {
                    return Err(GraphSchemaIssue::UnconnectedInput);
                };
                fields.push(self.composition_series(&connection.output)?);
            }
            if fields[0].scalar_type != fields[1].scalar_type {
                return Err(GraphSchemaIssue::ConflictingInputs);
            }
            return Ok(SchemaField {
                name: SchemaColumnRef("result".into()),
                scalar_type: fields[0].scalar_type,
                lineage: None,
            });
        }
        let mut name = match &source.port {
            PortRef::Declared { key } => key.as_str().to_owned(),
            PortRef::Instance { template, .. } => template.as_str().to_owned(),
        };
        let kind = match &protocol.typing {
            NodeTypingSpec::ConstantOutput { .. } => {
                let constant = super::super::referenced_constant(self.document, node, protocol)
                    .ok_or(GraphSchemaIssue::MissingResource)?;
                if !constant.name.trim().is_empty() {
                    name = constant.name.clone();
                }
                scalar_semantic(&constant.data_type)
            }
            NodeTypingSpec::NumericFold { .. } | NodeTypingSpec::ShapePreservingNumeric { .. } => {
                Some(SemanticType::Numeric)
            }
            NodeTypingSpec::BinaryPredicate { .. } => Some(SemanticType::Binary),
            NodeTypingSpec::Identity { input, .. } => {
                let address = PortAddress::declared(node.id, input.clone());
                let [connection] = self.index.input_connections(&address) else {
                    return Err(GraphSchemaIssue::UnconnectedInput);
                };
                return self.composition_series(&connection.output);
            }
            NodeTypingSpec::ShapePreservingConversion { parameter, .. } => {
                match effective_text_parameter(node, parameter, self.registry) {
                    Some("auto") => self
                        .automatic_outputs
                        .get(source)
                        .and_then(|state| state.exact())
                        .and_then(yss_graph_type_mapping::data_type_from_resolved_type)
                        .as_ref()
                        .and_then(scalar_semantic),
                    target => target
                        .and_then(|id| SemanticType::ALL.into_iter().find(|s| s.type_id() == id)),
                }
            }
            _ => {
                let output = protocol
                    .interface
                    .ports
                    .iter()
                    .find(|port| match &source.port {
                        PortRef::Declared { key } => &port.key == key,
                        PortRef::Instance { template, .. } => &port.key == template,
                    });
                // A shared generic series parameter preserves element meaning through
                // operations such as Lag, even when the output has no table Schema.
                if let Some(output) = output
                    && matches!(&output.value_type, TypeExpr::Applied { constructor, arguments } if constructor.as_str() == "core.data_series" && matches!(arguments.as_slice(), [TypeExpr::Generic(_)]))
                {
                    for input in protocol.interface.ports.iter().filter(|port| {
                        port.direction == yss_node_protocol::PortDirection::Input
                            && port.value_type == output.value_type
                    }) {
                        let input = PortAddress::declared(node.id, input.key.clone());
                        if let [connection] = self.index.input_connections(&input) {
                            return self.composition_series(&connection.output);
                        }
                    }
                }
                output.and_then(|port| declared_semantic(&port.value_type))
            }
        };
        Ok(SchemaField {
            name: SchemaColumnRef(name.into()),
            scalar_type: kind.map_or(RelationalScalarType::Unknown, RelationalScalarType::Known),
            lineage: None,
        })
    }

    pub(super) fn resolve_composition(
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
            effective_json_parameter(node, &key.parse::<ParameterKey>().unwrap(), registry)
        };
        let text = |key: &str| {
            effective_text_parameter(node, &key.parse::<ParameterKey>().unwrap(), registry)
        };
        let mut fields = match node.node_type.as_str() {
            "yssbi.dataframe.combine" => {
                let sources = self.composition_inputs(node_id, "series")?;
                if sources.is_empty() {
                    return Err(GraphSchemaIssue::UnconnectedInput);
                }
                let mut fields: Vec<SchemaField> = Vec::new();
                for source in sources {
                    let mut field = self.composition_series(source)?;
                    let names = fields
                        .iter()
                        .map(|f| f.name.0.to_string())
                        .collect::<Vec<_>>();
                    field.name.0 = append_column_names(&names, &[field.name.0.to_string()], "_2")
                        .remove(0)
                        .into();
                    fields.push(field);
                }
                fields
            }
            "yssbi.dataframe.concat.rows" | "yssbi.dataframe.concat.columns" => {
                let sources = self.composition_inputs(node_id, "frames")?;
                if sources.len() < 2 {
                    return Err(GraphSchemaIssue::UnconnectedInput);
                }
                let mut fields = self.composition_fields(sources[0])?;
                let mode = text("column_match");
                for (index, source) in sources.iter().skip(1).enumerate() {
                    let next = self.composition_fields(source)?;
                    if node.node_type.as_str().ends_with("columns") {
                        let names = append_column_names(
                            &fields
                                .iter()
                                .map(|f| f.name.0.to_string())
                                .collect::<Vec<_>>(),
                            &next
                                .iter()
                                .map(|f| f.name.0.to_string())
                                .collect::<Vec<_>>(),
                            &format!("_{}", index + 2),
                        );
                        fields.extend(next.into_iter().zip(names).map(|(mut field, name)| {
                            field.name.0 = name.into();
                            field
                        }));
                    } else if mode == Some("by_position") {
                        if fields.len() != next.len()
                            || fields
                                .iter()
                                .zip(&next)
                                .any(|(a, b)| a.scalar_type != b.scalar_type)
                        {
                            return Err(GraphSchemaIssue::ConflictingInputs);
                        }
                    } else if mode == Some("by_name") {
                        for field in next {
                            if let Some(existing) = fields.iter().find(|f| f.name == field.name) {
                                if existing.scalar_type != field.scalar_type {
                                    return Err(GraphSchemaIssue::ConflictingInputs);
                                }
                            } else {
                                fields.push(field);
                            }
                        }
                    } else {
                        return Err(GraphSchemaIssue::InvalidParameter);
                    }
                }
                fields
            }
            "yssbi.dataframe.join" => {
                let mut left = self.resolve_input(node_id, &"left".parse().unwrap())?;
                let right = self.resolve_input(node_id, &"right".parse().unwrap())?;
                let keys = |name| {
                    parameter(name)
                        .and_then(|v| {
                            yss_node_protocol::dataframe::prepare_project_columns_json(&v).ok()
                        })
                        .ok_or(GraphSchemaIssue::InvalidParameter)
                };
                let left_keys = keys("left_keys")?;
                let right_keys = keys("right_keys")?;
                if left_keys.as_slice().len() != right_keys.as_slice().len() {
                    return Err(GraphSchemaIssue::InvalidParameter);
                }
                for (l, r) in left_keys.as_slice().iter().zip(right_keys.as_slice()) {
                    let l = left
                        .iter()
                        .find(|f| &f.name.0 == l)
                        .ok_or(GraphSchemaIssue::MissingColumn)?;
                    let r = right
                        .iter()
                        .find(|f| &f.name.0 == r)
                        .ok_or(GraphSchemaIssue::MissingColumn)?;
                    if l.scalar_type != r.scalar_type {
                        return Err(GraphSchemaIssue::ConflictingInputs);
                    }
                }
                let suffix = text("right_suffix").ok_or(GraphSchemaIssue::InvalidParameter)?;
                if suffix.is_empty() {
                    return Err(GraphSchemaIssue::InvalidParameter);
                }
                let names = append_column_names(
                    &left
                        .iter()
                        .map(|f| f.name.0.to_string())
                        .collect::<Vec<_>>(),
                    &right
                        .iter()
                        .map(|f| f.name.0.to_string())
                        .collect::<Vec<_>>(),
                    suffix,
                );
                if !matches!(text("join_type"), Some("semi" | "anti")) {
                    left.extend(right.into_iter().zip(names).map(|(mut field, name)| {
                        field.name.0 = name.into();
                        field
                    }));
                }
                left
            }
            _ => return Err(GraphSchemaIssue::UnsupportedResolver),
        };
        // Composite fields have graph-owned identities, preserved by downstream renames.
        for field in &mut fields {
            field.lineage = Some(SchemaFieldLineage {
                source: format!("graph:{node_id}:composition").into(),
                field: field.name.0.clone(),
            });
        }
        Ok(fields)
    }
}
