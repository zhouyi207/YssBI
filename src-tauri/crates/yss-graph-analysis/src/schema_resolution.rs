use std::collections::{BTreeMap, BTreeSet};
mod aggregation;
mod composition;
mod generated_tables;
mod transforms;
pub(crate) use aggregation::column_names as aggregate_column_names;

use crate::parameter_projection::{effective_json_parameter, effective_text_parameter};
use crate::{GraphSchemaIssue, GraphSchemaState};
use yss_data_contract::ValueType;
use yss_graph_document::{
    DynamicMemberLocator, GraphDocument, NodeId, PortAddress, PortRef, SchemaFieldIdentity,
    SchemaSourceIdentity,
};
use yss_graph_resource_contract::{
    GraphDependencyManifest, GraphResourceId, ResourceCatalogSnapshot,
};
use yss_node_protocol::{
    ColumnSelectionExpr, ParameterKey, PortKey, RelationalScalarType, RenameExpr,
    ResolvedSchemaFact, SchemaColumnRef, SchemaExpr, SchemaField, SchemaFieldLineage, TypeExpr,
};
use yss_node_registry::NodeRegistry;

const DATAFRAME_RESOURCE_SCHEMA_RESOLVER: &str = "yssbi.dataframe.schema.resource";
const DATAFRAME_COLUMNS_INTERFACE_RESOLVER: &str = "yssbi.dataframe.interface.columns";
const DATAFRAME_INPUT_PORT: &str = "dataframe";

#[derive(Clone, Default)]
pub(crate) struct SchemaCache {
    outputs: BTreeMap<PortAddress, CachedSchemaOutput>,
    #[cfg(test)]
    pub(crate) reused_outputs: usize,
}

#[derive(Clone)]
struct CachedSchemaOutput {
    input_fingerprint: [u8; 32],
    dependencies: GraphDependencyManifest,
    state: GraphSchemaState,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DerivedSchemaPortMember {
    pub locator: DynamicMemberLocator,
    pub label: Box<str>,
    pub value_type: TypeExpr,
    pub schema: ResolvedSchemaFact,
}

pub(crate) fn resolve_graph_schemas(
    document: &GraphDocument,
    index: &crate::document_index::DocumentIndex<'_>,
    registry: &NodeRegistry,
    resources: &ResourceCatalogSnapshot,
    cache: &mut SchemaCache,
    observations: &crate::GraphSchemaObservations,
    arguments: &crate::function_arguments::BoundFunctionPorts,
) -> SchemaResolution {
    #[cfg(test)]
    {
        cache.reused_outputs = 0;
    }
    let mut output_addresses = document
        .nodes
        .values()
        .flat_map(|node| {
            registry
                .protocol(&node.node_type)
                .into_iter()
                .flat_map(move |protocol| {
                    protocol
                        .interface
                        .ports
                        .iter()
                        .filter(|port| {
                            port.direction == yss_node_protocol::PortDirection::Output
                                && (port.schema.is_some()
                                    || matches!(
                                        protocol.typing,
                                        yss_node_protocol::NodeTypingSpec::Identity { .. } | yss_node_protocol::NodeTypingSpec::ConstantOutput { .. }
                                    ))
                                && matches!(
                                    port.cardinality,
                                    yss_node_protocol::PortCardinality::Declared
                                )
                        })
                        .map(move |port| PortAddress::declared(node.id, port.key.clone()))
                })
        })
        .collect::<Vec<_>>();
    output_addresses.extend(observations.keys().cloned());
    output_addresses.extend(
        crate::function_structure::function_output_addresses_with_index(
            document, index, registry, resources,
        ),
    );
    let order = index.topological_order();
    let ranks = order
        .iter()
        .copied()
        .enumerate()
        .map(|(index, node_id)| (node_id, index))
        .collect::<BTreeMap<_, _>>();
    output_addresses
        .sort_by_key(|address| ranks.get(&address.node_id).copied().unwrap_or(usize::MAX));
    let mut resolver = EditorSchemaResolver {
        document,
        index,
        registry,
        resources: resources.clone(),
        observations,
        cache,
        resolved: SchemaResolution(
            arguments
                .iter()
                .map(|(address, argument)| (address.clone(), argument.schema.clone()))
                .collect(),
        ),
        series_fields: BTreeMap::new(),
        visiting: BTreeSet::new(),
    };
    for address in output_addresses {
        if ranks.contains_key(&address.node_id) {
            let _ = resolver.resolve_output(&address);
        } else {
            resolver.cache.outputs.remove(&address);
            resolver.resolved.insert(
                address,
                GraphSchemaState::Conflict(GraphSchemaIssue::DependencyCycle),
            );
        }
    }
    for (input, connections) in &index.incoming {
        let [connection] = connections.as_slice() else {
            resolver.resolved.insert(
                input.clone(),
                GraphSchemaState::Conflict(GraphSchemaIssue::ConflictingInputs),
            );
            continue;
        };
        let state = resolver.resolve_output(&connection.output);
        if let Some(schema) = state.exact() {
            let input_key = match &input.port {
                PortRef::Declared { key } => key.clone(),
                PortRef::Instance { template, .. } => template.clone(),
            };
            resolver.resolved.insert(
                input.clone(),
                GraphSchemaState::Exact(ResolvedSchemaFact {
                    expression: SchemaExpr::Input(input_key),
                    fields: schema.fields.clone(),
                }),
            );
        } else {
            resolver.resolved.insert(input.clone(), state);
        }
    }
    resolver
        .cache
        .outputs
        .retain(|address, _| resolver.resolved.state(address).is_some());
    resolver.resolved
}

pub(crate) fn derived_schema_port_members(
    node_id: NodeId,
    resolver_id: &str,
    schemas: &SchemaResolution,
) -> Vec<DerivedSchemaPortMember> {
    if resolver_id != DATAFRAME_COLUMNS_INTERFACE_RESOLVER {
        return Vec::new();
    }
    let input = PortAddress::declared(
        node_id,
        PortKey::new(DATAFRAME_INPUT_PORT).expect("built-in dataframe input key is valid"),
    );
    schemas
        .get(&input)
        .into_iter()
        .flat_map(|schema| schema.fields.iter())
        .map(|field| {
            let (source, identity) = field
                .lineage
                .as_ref()
                .map(|lineage| (lineage.source.clone(), lineage.field.clone()))
                .unwrap_or_else(|| {
                    (
                        format!("graph:{node_id}:{DATAFRAME_INPUT_PORT}").into(),
                        field.name.0.clone(),
                    )
                });
            DerivedSchemaPortMember {
                locator: DynamicMemberLocator::SchemaField {
                    source: SchemaSourceIdentity::new(source),
                    field: SchemaFieldIdentity::new(identity),
                },
                label: field.name.0.clone(),
                value_type: field_series_type(field.scalar_type),
                schema: ResolvedSchemaFact {
                    expression: SchemaExpr::Project {
                        input: Box::new(SchemaExpr::Input(
                            PortKey::new(DATAFRAME_INPUT_PORT)
                                .expect("built-in dataframe input key is valid"),
                        )),
                        columns: ColumnSelectionExpr::Explicit(vec![field.name.clone()]),
                    },
                    fields: vec![field.clone()],
                },
            }
        })
        .collect()
}

#[derive(Default)]
pub(crate) struct SchemaResolution(BTreeMap<PortAddress, GraphSchemaState>);

impl SchemaResolution {
    pub(crate) fn get(&self, address: &PortAddress) -> Option<&ResolvedSchemaFact> {
        self.0.get(address).and_then(GraphSchemaState::exact)
    }
    pub(crate) fn state(&self, address: &PortAddress) -> Option<&GraphSchemaState> {
        self.0.get(address)
    }
    pub(crate) fn internal_failure(&self) -> Option<&PortAddress> {
        self.0
            .iter()
            .find(|(_, state)| matches!(state, GraphSchemaState::InternalFailure(_)))
            .map(|(address, _)| address)
    }
    fn insert(&mut self, address: PortAddress, state: GraphSchemaState) {
        self.0.insert(address, state);
    }
}

struct EditorSchemaResolver<'a> {
    document: &'a GraphDocument,
    index: &'a crate::document_index::DocumentIndex<'a>,
    registry: &'a NodeRegistry,
    resources: ResourceCatalogSnapshot,
    cache: &'a mut SchemaCache,
    observations: &'a crate::GraphSchemaObservations,
    resolved: SchemaResolution,
    series_fields: BTreeMap<PortAddress, Result<SchemaField, GraphSchemaIssue>>,
    visiting: BTreeSet<PortAddress>,
}

impl EditorSchemaResolver<'_> {
    fn resolve_output(&mut self, address: &PortAddress) -> GraphSchemaState {
        if let Some(state) = self.resolved.state(address) {
            return state.clone();
        }
        if !self.visiting.insert(address.clone()) {
            return GraphSchemaState::Conflict(GraphSchemaIssue::DependencyCycle);
        }
        let result = match self.resolve_schema_output(address) {
            GraphSchemaState::Deferred if self.observations.contains_key(address) => {
                let observation = &self.observations[address];
                let mut fields = observation.fields.clone();
                for field in &mut fields {
                    // Data-dependent column removal preserves the input field's identity.
                    // Ambiguous names do not acquire an unrelated source's lineage.
                    let mut sources = self
                        .index
                        .node_input_connections(address.node_id)
                        .iter()
                        .filter_map(|connection| self.resolved.get(&connection.output))
                        .flat_map(|schema| &schema.fields)
                        .filter(|source| source.name == field.name);
                    if let Some(source) = sources.next().filter(|_| sources.next().is_none()) {
                        field.lineage = source.lineage.clone();
                    }
                    if field.lineage.is_none() {
                        field.lineage = Some(SchemaFieldLineage {
                            source: format!("graph:{address}").into(),
                            field: field.name.0.clone(),
                        });
                    }
                }
                GraphSchemaState::Observed {
                    version: observation.version,
                    schema: ResolvedSchemaFact {
                        expression: self.output_schema_expression(address).unwrap_or_else(|| {
                            SchemaExpr::Fixed {
                                fields: fields.clone(),
                            }
                        }),
                        fields,
                    },
                }
            }
            state => state,
        };
        self.visiting.remove(address);
        self.resolved.insert(address.clone(), result.clone());
        result
    }

    fn resolve_schema_output(&mut self, address: &PortAddress) -> GraphSchemaState {
        if self.function_returns_dataframe(address) {
            return GraphSchemaState::Deferred;
        }
        let Some(expression) = self.output_schema_expression(address) else {
            self.cache.outputs.remove(address);
            return GraphSchemaState::NotApplicable;
        };
        // Resolve upstream facts before checking this output. If a changed
        // branch produces the same Schema, downstream expressions stay cached.
        let sources = self.index.node_input_connections(address.node_id);
        for connection in sources {
            let _ = self.resolve_output(&connection.output);
        }
        // A scalar/series can change column meaning without declaring a table Schema.
        // Reuse each inferred field within this resolution, including recursive inputs.
        let series_fields = sources
            .iter()
            .map(|connection| self.composition_series(&connection.output))
            .collect::<Vec<_>>();
        let node = &self.document.nodes[&address.node_id];
        let constant = self
            .registry
            .protocol(&node.node_type)
            .and_then(|protocol| super::referenced_constant(self.document, node, protocol));
        let input_fingerprint = yss_canonical_hash::hash_canonical(
            "yssbi.graph-schema-input.v1",
            &(
                self.registry.fingerprint().as_bytes(),
                &node.node_type,
                &node.parameters,
                self.index.node_bindings(node.id),
                &expression,
                // Include target addresses as well: moving a source between
                // two inputs can change Project/Append/Rename semantics.
                sources
                    .iter()
                    .zip(&series_fields)
                    .map(|(connection, series_field)| {
                        (
                            &connection.input,
                            &connection.output,
                            self.resolved.state(&connection.output),
                            series_field,
                        )
                    })
                    .collect::<Vec<_>>(),
                constant.map(|constant| {
                    (
                        constant.id,
                        &constant.data_type,
                        &constant.data_value,
                        &constant.tabular,
                    )
                }),
            ),
        )
        .expect("schema inputs are serializable");
        if let Some(cached) = self.cache.outputs.get(address).filter(|cached| {
            cached.input_fingerprint == input_fingerprint
                && self.resources.matches_dependencies(&cached.dependencies)
        }) {
            self.resources.record_dependencies(&cached.dependencies);
            #[cfg(test)]
            {
                self.cache.reused_outputs += 1;
            }
            return cached.state.clone();
        }
        let tracked = self.resources.tracked();
        let parent = std::mem::replace(&mut self.resources, tracked);
        let state = match self.resolve_expression(address.node_id, &expression) {
            Ok(fields) => GraphSchemaState::Exact(ResolvedSchemaFact { expression, fields }),
            Err(issue) => GraphSchemaState::from_issue(issue),
        };
        let dependencies = self.resources.dependencies();
        self.resources = parent;
        self.resources.record_dependencies(&dependencies);
        self.cache.outputs.insert(
            address.clone(),
            CachedSchemaOutput {
                input_fingerprint,
                dependencies,
                state: state.clone(),
            },
        );
        state
    }

    fn function_returns_dataframe(&self, address: &PortAddress) -> bool {
        let Some(node) = self.document.nodes.get(&address.node_id) else {
            return false;
        };
        let Some(registered) = self.registry.get(&node.node_type) else {
            return false;
        };
        if !registered
            .structural_role()
            .is_some_and(yss_node_registry::StructuralNodeRole::calls_function)
        {
            return false;
        }
        let key = match &address.port {
            PortRef::Declared { key } | PortRef::Instance { template: key, .. } => key,
        };
        if !registered.protocol().interface.ports.iter().any(|port| {
            &port.key == key && port.direction == yss_node_protocol::PortDirection::Output
        }) {
            return false;
        }
        registered
            .function_reference(&node.parameters)
            .and_then(|path| yss_graph_document::GraphResourcePath::new(path).ok())
            .and_then(|path| self.resources.function_signature(&path))
            .is_some_and(|signature| signature.result() == Some(&ValueType::DataFrame))
    }

    fn output_schema_expression(&self, address: &PortAddress) -> Option<SchemaExpr> {
        let node = self.document.nodes.get(&address.node_id)?;
        let protocol = self.registry.protocol(&node.node_type)?;
        let key = match &address.port {
            PortRef::Declared { key } => key,
            PortRef::Instance { template, .. } => template,
        };
        let declared = protocol
            .interface
            .ports
            .iter()
            .find(|port| &port.key == key)?
            .schema
            .clone();
        declared.or_else(|| match &protocol.typing {
            yss_node_protocol::NodeTypingSpec::Identity { input, output } if output == key => {
                Some(SchemaExpr::Input(input.clone()))
            }
            yss_node_protocol::NodeTypingSpec::ConstantOutput { output, .. } if output == key => {
                let constant = super::referenced_constant(self.document, node, protocol)?;
                matches!(constant.data_type, ValueType::DataFrame).then(|| SchemaExpr::Derived {
                    resolver: "yssbi.constant.schema"
                        .parse()
                        .expect("constant schema resolver ID"),
                    dependencies: vec![],
                })
            }
            _ => None,
        })
    }

    fn resolve_expression(
        &mut self,
        node_id: NodeId,
        expression: &SchemaExpr,
    ) -> Result<Vec<SchemaField>, GraphSchemaIssue> {
        match expression {
            SchemaExpr::Input(port) => self.resolve_input(node_id, port),
            SchemaExpr::Fixed { fields } => Ok(fields.clone()),
            SchemaExpr::Project { input, columns } => {
                let fields = self.resolve_expression(node_id, input)?;
                if matches!(columns, ColumnSelectionExpr::All) {
                    return Ok(fields);
                }
                let selected = self
                    .selected_columns(node_id, columns)
                    .ok_or(GraphSchemaIssue::InvalidParameter)?;
                if matches!(columns, ColumnSelectionExpr::ExcludingParameter(_)) {
                    if selected
                        .iter()
                        .any(|name| !fields.iter().any(|field| &field.name.0 == name))
                    {
                        return Err(GraphSchemaIssue::MissingColumn);
                    }
                    let retained: Vec<_> = fields
                        .into_iter()
                        .filter(|field| !selected.contains(&field.name.0))
                        .collect();
                    return if retained.is_empty() {
                        Err(GraphSchemaIssue::InvalidParameter)
                    } else {
                        Ok(retained)
                    };
                }
                selected
                    .into_iter()
                    .map(|name| {
                        fields
                            .iter()
                            .find(|field| field.name.0 == name)
                            .cloned()
                            .ok_or(GraphSchemaIssue::MissingColumn)
                    })
                    .collect()
            }
            SchemaExpr::Append { inputs } => {
                let mut names = BTreeSet::new();
                let mut fields = Vec::new();
                for input in inputs {
                    for field in self.resolve_expression(node_id, input)? {
                        if names.insert(field.name.0.clone()) {
                            fields.push(field);
                        }
                    }
                }
                Ok(fields)
            }
            SchemaExpr::Rename { input, mapping } => {
                let mut fields = self.resolve_expression(node_id, input)?;
                let renames = self
                    .renames(node_id, mapping)
                    .ok_or(GraphSchemaIssue::InvalidParameter)?;
                if renames
                    .iter()
                    .any(|(name, _)| !fields.iter().any(|field| &field.name.0 == name))
                {
                    return Err(GraphSchemaIssue::MissingColumn);
                }
                for field in &mut fields {
                    if let Some((_, to)) = renames.iter().find(|(from, _)| *from == field.name.0) {
                        field.name = SchemaColumnRef(to.clone());
                    }
                }
                if fields
                    .iter()
                    .map(|field| &field.name.0)
                    .collect::<BTreeSet<_>>()
                    .len()
                    != fields.len()
                {
                    return Err(GraphSchemaIssue::InvalidParameter);
                }
                Ok(fields)
            }
            SchemaExpr::Filter { input, .. } => self.resolve_expression(node_id, input),
            SchemaExpr::Derived { resolver, .. }
                if resolver.as_str() == "yssbi.statistics.multivariate.schema.coordinates" =>
            {
                let prefixes = if self.document.nodes.get(&node_id).is_some_and(|node| {
                    node.node_type.as_str() == "yssbi.statistics.association.canonical"
                }) {
                    &["x_axis", "y_axis"][..]
                } else {
                    &["axis"][..]
                };
                self.resolve_generated_numeric_table(node_id, "components", prefixes, false)
            }
            SchemaExpr::Derived { resolver, .. }
                if resolver.as_str() == "yssbi.statistics.doe.schema.design" =>
            {
                self.resolve_generated_numeric_table(node_id, "factors", &["factor"], true)
            }
            SchemaExpr::Derived { resolver, .. }
                if resolver.as_str() == "yssbi.dataframe.schema.transform" =>
            {
                self.resolve_transform(node_id)
            }
            SchemaExpr::Derived { resolver, .. }
                if resolver.as_str() == "yssbi.dataframe.schema.aggregate" =>
            {
                self.resolve_aggregation(node_id)
            }
            SchemaExpr::Derived { resolver, .. }
                if resolver.as_str() == "yssbi.dataframe.schema.composition" =>
            {
                self.resolve_composition(node_id)
            }
            SchemaExpr::Derived { resolver, .. }
                if resolver.as_str() == DATAFRAME_RESOURCE_SCHEMA_RESOLVER =>
            {
                self.resolve_database_schema(node_id)
            }
            SchemaExpr::Derived { resolver, .. }
                if resolver.as_str() == "yssbi.constant.schema" =>
            {
                self.resolve_constant_schema(node_id)
            }
            SchemaExpr::Derived { resolver, .. }
                if resolver.as_str() == "yssbi.dataframe.schema.dropna" =>
            {
                self.resolve_input(node_id, &"source".parse().expect("static port"))?;
                Err(GraphSchemaIssue::DataDependent)
            }
            SchemaExpr::Derived { .. } => Err(GraphSchemaIssue::UnsupportedResolver),
        }
    }

    fn resolve_input(
        &mut self,
        node_id: NodeId,
        port: &PortKey,
    ) -> Result<Vec<SchemaField>, GraphSchemaIssue> {
        let input = PortAddress::declared(node_id, port.clone());
        let output = match self.index.input_connections(&input) {
            [] => return Err(GraphSchemaIssue::UnconnectedInput),
            [connection] => &connection.output,
            _ => return Err(GraphSchemaIssue::ConflictingInputs),
        };
        let state = self.resolve_output(output);
        let fields = state
            .exact()
            .map(|fact| fact.fields.clone())
            .ok_or_else(|| {
                state
                    .issue()
                    .unwrap_or(GraphSchemaIssue::UnresolvedUpstream)
            })?;
        self.resolved.insert(
            input,
            GraphSchemaState::Exact(ResolvedSchemaFact {
                expression: SchemaExpr::Input(port.clone()),
                fields: fields.clone(),
            }),
        );
        Ok(fields)
    }

    fn resolve_constant_schema(
        &self,
        node_id: NodeId,
    ) -> Result<Vec<SchemaField>, GraphSchemaIssue> {
        use yss_data_contract::TabularScalar;
        let node = self
            .document
            .nodes
            .get(&node_id)
            .ok_or(GraphSchemaIssue::MissingResource)?;
        let protocol = self
            .registry
            .protocol(&node.node_type)
            .ok_or(GraphSchemaIssue::MissingResource)?;
        if !matches!(
            &protocol.typing,
            yss_node_protocol::NodeTypingSpec::ConstantOutput { .. }
        ) {
            return Err(GraphSchemaIssue::UnsupportedResolver);
        }
        let constant = super::referenced_constant(self.document, node, protocol)
            .ok_or(GraphSchemaIssue::MissingResource)?;
        Ok(constant
            .tabular
            .iter()
            .flat_map(|snapshot| snapshot.columns())
            .map(|column| {
                let scalar_type = column
                    .values()
                    .iter()
                    .filter_map(|value| match value {
                        TabularScalar::Null => None,
                        TabularScalar::Bool(_) => Some(RelationalScalarType::Known(
                            yss_node_protocol::SemanticType::Binary,
                        )),
                        TabularScalar::Integer(_) => Some(RelationalScalarType::Known(
                            yss_node_protocol::SemanticType::Numeric,
                        )),
                        TabularScalar::Unsigned(value) if i64::try_from(*value).is_ok() => Some(
                            RelationalScalarType::Known(yss_node_protocol::SemanticType::Numeric),
                        ),
                        TabularScalar::Unsigned(_) | TabularScalar::Float64(_) => Some(
                            RelationalScalarType::Known(yss_node_protocol::SemanticType::Numeric),
                        ),
                        TabularScalar::String(_) => Some(RelationalScalarType::Known(
                            yss_node_protocol::SemanticType::Text,
                        )),
                    })
                    .reduce(|left, right| {
                        if left == right {
                            left
                        } else {
                            RelationalScalarType::Unknown
                        }
                    })
                    .unwrap_or(RelationalScalarType::Unknown);
                let scalar_type = match &constant.data_type {
                    ValueType::DataSeries(element) => {
                        yss_graph_type_mapping::relational_scalar_type_from_data_type(element)
                    }
                    _ => scalar_type,
                };

                SchemaField {
                    name: SchemaColumnRef(column.name().as_str().into()),
                    scalar_type,
                    lineage: Some(SchemaFieldLineage {
                        source: format!("constant:{}", constant.id).into(),
                        field: column.name().as_str().into(),
                    }),
                }
            })
            .collect())
    }

    fn resolve_database_schema(
        &self,
        node_id: NodeId,
    ) -> Result<Vec<SchemaField>, GraphSchemaIssue> {
        let node = self
            .document
            .nodes
            .get(&node_id)
            .ok_or(GraphSchemaIssue::UnresolvedUpstream)?;
        let resource = effective_text_parameter(
            node,
            &ParameterKey::new("dataframe").expect("built-in parameter key"),
            self.registry,
        )
        .ok_or(GraphSchemaIssue::InvalidParameter)?;
        let schema = self
            .resources
            .database_schema(&GraphResourceId::new(resource))
            .ok_or(GraphSchemaIssue::MissingResource)?;
        Ok(schema
            .columns
            .iter()
            .map(|column| SchemaField {
                name: SchemaColumnRef(column.name.clone().into()),
                scalar_type: yss_graph_type_mapping::relational_scalar_type_from_data_type(
                    &column.data_type,
                ),
                lineage: Some(SchemaFieldLineage {
                    source: resource.into(),
                    field: column.name.clone().into(),
                }),
            })
            .collect())
    }

    fn selected_columns(
        &self,
        node_id: NodeId,
        selection: &ColumnSelectionExpr,
    ) -> Option<Vec<Box<str>>> {
        match selection {
            ColumnSelectionExpr::All => Some(Vec::new()),
            ColumnSelectionExpr::Explicit(columns) => {
                Some(columns.iter().map(|column| column.0.clone()).collect())
            }
            ColumnSelectionExpr::FromParameter(parameter)
            | ColumnSelectionExpr::ExcludingParameter(parameter) => {
                let node = self.document.nodes.get(&node_id)?;
                effective_json_parameter(node, parameter, self.registry)?
                    .as_array()?
                    .iter()
                    .map(|value| value.as_str().map(Box::<str>::from))
                    .collect()
            }
        }
    }

    fn renames(&self, node_id: NodeId, mapping: &RenameExpr) -> Option<Vec<(Box<str>, Box<str>)>> {
        let node = self.document.nodes.get(&node_id)?;
        match mapping {
            RenameExpr::Explicit(values) => Some(
                values
                    .iter()
                    .map(|rename| (rename.from.0.clone(), rename.to.0.clone()))
                    .collect(),
            ),
            RenameExpr::FromParameter(parameter) => {
                let value = effective_json_parameter(node, parameter, self.registry)?;
                let object = value.as_object()?;
                Some(
                    object
                        .iter()
                        .map(|(from, to)| Some((from.as_str().into(), to.as_str()?.into())))
                        .collect::<Option<Vec<_>>>()?,
                )
            }
            RenameExpr::FromParameters { from, to } => Some(vec![(
                effective_text_parameter(node, from, self.registry)?.into(),
                effective_text_parameter(node, to, self.registry)?.into(),
            )]),
        }
    }
}

fn field_series_type(scalar_type: RelationalScalarType) -> TypeExpr {
    let element = match scalar_type {
        RelationalScalarType::Known(semantic) => ValueType::Scalar(semantic),
        RelationalScalarType::Unknown => return TypeExpr::Unknown,
    };
    yss_graph_type_mapping::type_expr_from_data_type(&ValueType::DataSeries(Box::new(element)))
        .unwrap_or(TypeExpr::Unknown)
}

#[cfg(test)]
mod tests;
