use crate::graph::value::DataType;
use crate::node_system::analysis::{
    EditorGraphProjectionDto, PortDirectionDto, PortKindDto, ResolvedPortDto,
    resolve_function_data_type,
};
use crate::node_system::catalog::{
    LocalizedCatalogDto, NodeCreationDescriptor, ResourceBoundCreateArgsDto,
};
use crate::node_system::document::{
    DynamicMemberLocator, FunctionParameterId, GraphDocument, GraphResourcePath,
    LastKnownPortMetadata, OrderKey, PortAddress, PortAddressDto, PortRef,
};
use crate::node_system::protocol::{
    ConnectionsPerPort, NodeProtocol, PortDirection, PortInstances, PortKey, PortKind,
    TypeConstructorId, TypeExpr, TypeId, TypeParameterId,
};
use crate::node_system::registry::NodeRegistry;
use crate::project::{CatalogMutationResource, CatalogMutationValidationSnapshot};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SourcePort {
    pub address: PortAddress,
    pub direction: PortDirection,
    pub kind: PortKind,
    pub value_type: TypeExpr,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CandidatePort {
    pub template: PortKey,
    pub direction: PortDirection,
    pub kind: PortKind,
    pub connections: ConnectionsPerPort,
    pub value_type: TypeExpr,
    pub type_parameters: Box<[TypeParameterId]>,
    pub dynamic: Option<DynamicCandidate>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DynamicCandidate {
    pub origin: DynamicMemberLocator,
    pub order: OrderKey,
    pub last_known: LastKnownPortMetadata,
}

pub(crate) fn source_from_projection(
    projection: &EditorGraphProjectionDto,
    source_port: PortAddressDto,
) -> Result<SourcePort, String> {
    let address: PortAddress = source_port.try_into()?;
    let port = projection
        .nodes
        .iter()
        .flat_map(|node| node.ports.iter())
        .find(|port| port.address == PortAddressDto::from(&address))
        .ok_or_else(|| format!("source port '{address}' is not present in the current graph"))?;
    if port.orphan || !port.connections.can_connect {
        return Err(format!(
            "source port '{address}' cannot accept another connection"
        ));
    }
    Ok(SourcePort {
        address,
        direction: direction(port),
        kind: kind(port),
        value_type: port
            .resolved_type
            .as_ref()
            .and_then(|summary| summary.internal_type_expr.clone())
            .unwrap_or(TypeExpr::Unknown),
    })
}

pub(crate) fn refine_source_type(
    source: &mut SourcePort,
    document: &GraphDocument,
    registry: &NodeRegistry,
    resources: &CatalogMutationValidationSnapshot,
) {
    if source.kind != PortKind::Data
        || !matches!(source.value_type, TypeExpr::Generic(_) | TypeExpr::Unknown)
    {
        return;
    }
    let Some(node) = document.nodes.get(&source.address.node_id) else {
        return;
    };
    let Some(protocol) = registry.protocol(&node.node_type) else {
        return;
    };
    let template = match &source.address.port {
        PortRef::Declared { key } => key,
        PortRef::Instance { template, .. } => template,
    };
    let Some(spec) = protocol
        .interface
        .ports
        .iter()
        .find(|port| &port.key == template)
    else {
        return;
    };
    source.value_type = spec.value_type.clone();
    let Some(path) = node
        .parameters
        .values()
        .find_map(|value| value.as_str())
        .map(crate::node_system::catalog::CatalogResourcePath::new)
    else {
        return;
    };
    let Some(resource) = resources.resources.get(&path) else {
        return;
    };
    match resource {
        CatalogMutationResource::Variable { data_type, .. } => {
            if let Ok(value_type) = data_type_to_type_expr(data_type) {
                source.value_type = value_type;
            }
        }
        CatalogMutationResource::Function { signature, .. } => {
            let Some(binding) = document.port_bindings.get(&source.address) else {
                return;
            };
            let origin = match binding {
                crate::node_system::document::DynamicPortBinding::Resolved { origin, .. }
                | crate::node_system::document::DynamicPortBinding::Orphan { origin, .. } => origin,
                crate::node_system::document::DynamicPortBinding::UserCreated { .. } => return,
            };
            let DynamicMemberLocator::FunctionParameter { parameter, .. } = origin else {
                return;
            };
            let type_name = if parameter.0.as_ref() == "return" {
                signature.return_type.as_deref()
            } else {
                signature
                    .parameters
                    .iter()
                    .find(|candidate| candidate.id == *parameter)
                    .map(|candidate| candidate.type_name.as_str())
            };
            if let Some(Ok(value_type)) = type_name.map(function_type_expr) {
                source.value_type = value_type;
            }
        }
        CatalogMutationResource::Database { .. } => {}
    }
}

pub(crate) fn compatible_catalog(
    snapshot: &crate::project::CatalogProjectSnapshot,
    graph_path: &GraphResourcePath,
    source: &SourcePort,
    locale: &str,
) -> LocalizedCatalogDto {
    let registry_fingerprint = snapshot.registry.fingerprint().to_string();
    let mut dto = snapshot
        .catalog
        .localize_with_resources(snapshot.registry.as_ref(), locale, &snapshot.resources)
        .into_dto(
            snapshot.project_instance_id.as_str(),
            registry_fingerprint,
            snapshot.resource_publication_revision,
        );
    dto.items.retain(|item| {
        candidate_ports(
            graph_path,
            &item.creation,
            snapshot.registry.as_ref(),
            &snapshot.validation,
        )
        .is_ok_and(|ports| ports.iter().any(|port| ports_are_compatible(source, port)))
    });
    let categories = dto
        .items
        .iter()
        .map(|item| item.category_id.as_ref())
        .collect::<std::collections::BTreeSet<_>>();
    dto.categories
        .retain(|category| categories.contains(category.category_id.as_ref()));
    dto
}

pub(crate) fn first_compatible_port(
    graph_path: &GraphResourcePath,
    descriptor: &NodeCreationDescriptor,
    registry: &NodeRegistry,
    resources: &CatalogMutationValidationSnapshot,
    source: &SourcePort,
) -> Result<CandidatePort, String> {
    candidate_ports(graph_path, descriptor, registry, resources)?
        .into_iter()
        .find(|candidate| ports_are_compatible(source, candidate))
        .ok_or_else(|| "created node has no compatible initial opposite-direction port".into())
}

fn candidate_ports(
    graph_path: &GraphResourcePath,
    descriptor: &NodeCreationDescriptor,
    registry: &NodeRegistry,
    resources: &CatalogMutationValidationSnapshot,
) -> Result<Vec<CandidatePort>, String> {
    let node_type = descriptor_node_type(descriptor);
    let protocol = registry
        .protocol(node_type)
        .ok_or_else(|| format!("unknown node type '{node_type}'"))?;
    validate_scope(graph_path, protocol)?;
    let resource = descriptor_resource(descriptor, resources)?;
    if let Some(CatalogMutationResource::Variable { scope, .. }) = resource {
        let in_scope = match scope {
            crate::variable::VariableScope::Global => true,
            crate::variable::VariableScope::Event { event_path }
            | crate::variable::VariableScope::Function {
                function_path: event_path,
            } => event_path.as_str() == graph_path.0.as_ref(),
        };
        if !in_scope {
            return Err("variable resource is out of graph scope".into());
        }
    }
    let mut ports = protocol
        .interface
        .ports
        .iter()
        .filter(|spec| match spec.instances {
            PortInstances::Declared => true,
            PortInstances::UserCreated { min, .. } => min > 0,
            PortInstances::Derived { .. } => false,
        })
        .map(|spec| {
            Ok(CandidatePort {
                template: spec.key.clone(),
                direction: spec.direction,
                kind: spec.kind,
                connections: spec.connections,
                value_type: resource_type_override(resource, node_type.as_str())?
                    .unwrap_or_else(|| spec.value_type.clone()),
                type_parameters: protocol.interface.type_parameters.clone(),
                dynamic: None,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    if let Some(CatalogMutationResource::Function { signature, .. }) = resource {
        let function = match descriptor {
            NodeCreationDescriptor::ResourceBound { resource_path, .. } => {
                GraphResourcePath(resource_path.as_str().into())
            }
            _ => unreachable!(),
        };
        let argument_spec = protocol
            .interface
            .ports
            .iter()
            .find(|port| port.key.as_str() == "arguments");
        if let Some(spec) = argument_spec {
            for (index, parameter) in signature.parameters.iter().enumerate() {
                ports.push(CandidatePort {
                    template: spec.key.clone(),
                    direction: spec.direction,
                    kind: spec.kind,
                    connections: spec.connections,
                    value_type: function_type_expr(&parameter.type_name)?,
                    type_parameters: Box::new([]),
                    dynamic: Some(DynamicCandidate {
                        origin: DynamicMemberLocator::FunctionParameter {
                            function: function.clone(),
                            parameter: parameter.id.clone(),
                        },
                        order: OrderKey(format!("{index:05}").into()),
                        last_known: LastKnownPortMetadata {
                            label: parameter.name.clone(),
                            value_type: Some(function_type_expr(&parameter.type_name)?),
                        },
                    }),
                });
            }
        }
        if let (Some(spec), Some(return_type)) = (
            protocol
                .interface
                .ports
                .iter()
                .find(|port| port.key.as_str() == "results"),
            signature.return_type.as_deref(),
        ) {
            ports.push(CandidatePort {
                template: spec.key.clone(),
                direction: spec.direction,
                kind: spec.kind,
                connections: spec.connections,
                value_type: function_type_expr(return_type)?,
                type_parameters: Box::new([]),
                dynamic: Some(DynamicCandidate {
                    origin: DynamicMemberLocator::FunctionParameter {
                        function,
                        parameter: FunctionParameterId("return".into()),
                    },
                    order: OrderKey("00000".into()),
                    last_known: LastKnownPortMetadata {
                        label: return_type.to_owned(),
                        value_type: Some(function_type_expr(return_type)?),
                    },
                }),
            });
        }
    }
    Ok(ports)
}

fn descriptor_node_type(
    descriptor: &NodeCreationDescriptor,
) -> &crate::node_system::protocol::NodeTypeId {
    match descriptor {
        NodeCreationDescriptor::Static { node_type_id }
        | NodeCreationDescriptor::ParameterizedStatic { node_type_id, .. }
        | NodeCreationDescriptor::ResourceBound { node_type_id, .. } => node_type_id,
    }
}

fn descriptor_resource<'a>(
    descriptor: &NodeCreationDescriptor,
    resources: &'a CatalogMutationValidationSnapshot,
) -> Result<Option<&'a CatalogMutationResource>, String> {
    let NodeCreationDescriptor::ResourceBound {
        node_type_id,
        resource_path,
        resource_revision,
        create_args,
    } = descriptor
    else {
        return Ok(None);
    };
    let resource = resources.resources.get(resource_path).ok_or_else(|| {
        format!(
            "catalog resource '{}' is unavailable",
            resource_path.as_str()
        )
    })?;
    let valid = match (create_args, resource) {
        (
            ResourceBoundCreateArgsDto::Function,
            CatalogMutationResource::Function {
                revision,
                allowed_node_type_id,
                ..
            },
        ) => revision == resource_revision && allowed_node_type_id == node_type_id,
        (
            ResourceBoundCreateArgsDto::Variable,
            CatalogMutationResource::Variable {
                revision,
                allowed_node_type_ids,
                ..
            },
        ) => {
            revision == resource_revision
                && allowed_node_type_ids
                    .iter()
                    .any(|allowed| allowed == node_type_id)
        }
        (
            ResourceBoundCreateArgsDto::Database,
            CatalogMutationResource::Database {
                authority_revision,
                allowed_node_type_id,
                ..
            },
        ) => authority_revision == resource_revision && allowed_node_type_id == node_type_id,
        _ => false,
    };
    valid
        .then_some(Some(resource))
        .ok_or_else(|| "catalog resource descriptor is stale or invalid".into())
}

fn resource_type_override(
    resource: Option<&CatalogMutationResource>,
    node_type: &str,
) -> Result<Option<TypeExpr>, String> {
    match resource {
        Some(CatalogMutationResource::Variable { data_type, .. }) => {
            data_type_to_type_expr(data_type).map(Some)
        }
        Some(CatalogMutationResource::Database { .. })
            if node_type == "yssbi.dataframe.source.get" =>
        {
            Ok(Some(concrete_type("tabular.dataframe")?))
        }
        _ => Ok(None),
    }
}

pub(crate) fn function_type_expr(type_name: &str) -> Result<TypeExpr, String> {
    data_type_to_type_expr(&resolve_function_data_type(type_name)?)
}

fn data_type_to_type_expr(data_type: &DataType) -> Result<TypeExpr, String> {
    match data_type {
        DataType::Boolean => concrete_type("core.bool"),
        DataType::Int64 => concrete_type("core.int64"),
        DataType::Float64 => concrete_type("core.float64"),
        DataType::String => concrete_type("core.string"),
        DataType::Date => concrete_type("core.date"),
        DataType::Datetime => concrete_type("core.datetime"),
        DataType::Time => concrete_type("core.time"),
        DataType::Categorical => concrete_type("core.categorical"),
        DataType::Object => concrete_type("core.object"),
        DataType::DataFrame => concrete_type("tabular.dataframe"),
        DataType::Struct(semantic_id) => concrete_type(semantic_id),
        DataType::Array(element) => applied_type("core.array", element),
        DataType::DataSeries(element) => applied_type("core.data_series", element),
        DataType::OneOf(values) => values
            .iter()
            .map(data_type_to_type_expr)
            .collect::<Result<Vec<_>, _>>()
            .map(TypeExpr::Union),
        DataType::Any => Ok(TypeExpr::Unknown),
    }
}

fn concrete_type(semantic_id: &str) -> Result<TypeExpr, String> {
    TypeId::new(semantic_id)
        .map(TypeExpr::Concrete)
        .map_err(|error| error.to_string())
}

fn applied_type(constructor: &str, element: &DataType) -> Result<TypeExpr, String> {
    Ok(TypeExpr::Applied {
        constructor: TypeConstructorId::new(constructor).map_err(|error| error.to_string())?,
        arguments: vec![data_type_to_type_expr(element)?],
    })
}

fn validate_scope(graph_path: &GraphResourcePath, protocol: &NodeProtocol) -> Result<(), String> {
    let scope = if graph_path.0.starts_with("events/") {
        crate::node_system::protocol::NodeScope::Event
    } else if graph_path.0.starts_with("functions/") {
        crate::node_system::protocol::NodeScope::Function
    } else {
        crate::node_system::protocol::NodeScope::Any
    };
    if protocol.scope != crate::node_system::protocol::NodeScope::Any
        && scope != crate::node_system::protocol::NodeScope::Any
        && protocol.scope != scope
    {
        Err(format!(
            "node type '{}' is out of graph scope",
            protocol.type_id
        ))
    } else {
        Ok(())
    }
}

fn ports_are_compatible(source: &SourcePort, candidate: &CandidatePort) -> bool {
    if source.direction == candidate.direction || source.kind != candidate.kind {
        return false;
    }
    if source.kind != PortKind::Data {
        return true;
    }
    match source.direction {
        PortDirection::Output => crate::node_system::compiler::type_exprs_assignable(
            &source.value_type,
            &candidate.value_type,
            &[],
            &candidate.type_parameters,
        ),
        PortDirection::Input => crate::node_system::compiler::type_exprs_assignable(
            &candidate.value_type,
            &source.value_type,
            &candidate.type_parameters,
            &[],
        ),
    }
}

fn direction(port: &ResolvedPortDto) -> PortDirection {
    match port.direction {
        PortDirectionDto::Input => PortDirection::Input,
        PortDirectionDto::Output => PortDirection::Output,
    }
}

fn kind(port: &ResolvedPortDto) -> PortKind {
    match port.kind {
        PortKindDto::Data => PortKind::Data,
        PortKindDto::Control => PortKind::Control,
        PortKindDto::Effect => PortKind::Effect,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node_system::catalog::{
        CatalogResourceEntry, CatalogResourcePath, build_builtin_node_system,
    };
    use crate::node_system::document::{FunctionParameter, FunctionSignature, ResourceRevision};
    use crate::node_system::protocol::NodeTypeId;
    use std::collections::BTreeMap;

    fn source(data_type: DataType) -> SourcePort {
        source_expr(data_type_to_type_expr(&data_type).unwrap())
    }

    fn source_expr(value_type: TypeExpr) -> SourcePort {
        SourcePort {
            address: PortAddress::declared(
                crate::node_system::document::NodeId::new(),
                PortKey::new("value").unwrap(),
            ),
            direction: PortDirection::Output,
            kind: PortKind::Data,
            value_type,
        }
    }

    fn candidate(value_type: TypeExpr) -> CandidatePort {
        CandidatePort {
            template: PortKey::new("input").unwrap(),
            direction: PortDirection::Input,
            kind: PortKind::Data,
            connections: ConnectionsPerPort::Single,
            value_type,
            type_parameters: Box::new([]),
            dynamic: None,
        }
    }

    fn snapshot(
        resources: Vec<CatalogResourceEntry>,
        validation_resources: BTreeMap<CatalogResourcePath, CatalogMutationResource>,
    ) -> crate::project::CatalogProjectSnapshot {
        let builtin = build_builtin_node_system().unwrap();
        let project_instance_id = crate::project::ProjectInstanceId::new();
        crate::project::CatalogProjectSnapshot {
            project_instance_id: project_instance_id.clone(),
            resource_publication_revision: 0,
            registry: builtin.registry,
            catalog: builtin.catalog,
            resources,
            validation: CatalogMutationValidationSnapshot {
                project_instance_id,
                authority_generation: 0,
                resources: validation_resources,
            },
            authority_generation: 0,
        }
    }

    #[test]
    fn compatible_catalog_filters_concrete_types_instead_of_returning_all_data_nodes() {
        let snapshot = snapshot(Vec::new(), BTreeMap::new());
        let catalog = compatible_catalog(
            &snapshot,
            &GraphResourcePath("events/main".into()),
            &source(DataType::Int64),
            "en-US",
        );
        let ids = catalog
            .items
            .iter()
            .map(|item| item.node_type_id.as_ref())
            .collect::<std::collections::BTreeSet<_>>();

        assert!(ids.contains("yssbi.numeric.add.int64"));
        assert!(!ids.contains("yssbi.logic.not"));
        assert!(!ids.contains("yssbi.numeric.add.float64"));
    }

    #[test]
    fn compatibility_uses_exact_type_expr_ids_and_compiler_source_union_semantics() {
        let tabular_series = source_expr(concrete_type("tabular.series").unwrap());
        let concrete_float_series = candidate(concrete_type("core.data_series.float64").unwrap());
        assert!(!ports_are_compatible(
            &tabular_series,
            &concrete_float_series
        ));

        let source_union = source_expr(TypeExpr::Union(vec![
            concrete_type("core.int64").unwrap(),
            concrete_type("core.string").unwrap(),
        ]));
        let int_target = candidate(concrete_type("core.int64").unwrap());
        assert!(!ports_are_compatible(&source_union, &int_target));

        let int_source = source(DataType::Int64);
        let union_target = candidate(TypeExpr::Union(vec![
            concrete_type("core.int64").unwrap(),
            concrete_type("core.string").unwrap(),
        ]));
        assert!(ports_are_compatible(&int_source, &union_target));
    }

    #[test]
    fn compatible_catalog_materializes_resource_bound_function_signature_ports() {
        let resource_path = CatalogResourcePath::new("functions/consume-int");
        let revision = ResourceRevision::new(3);
        let node_type_id = NodeTypeId::new("yssbi.project.function.call").unwrap();
        let signature = FunctionSignature {
            parameters: vec![FunctionParameter {
                id: FunctionParameterId("value".into()),
                name: "Value".into(),
                type_name: "core.int64".into(),
            }],
            return_type: Some("core.string".into()),
        };
        let snapshot = snapshot(
            vec![CatalogResourceEntry {
                name: "Consume Int".into(),
                node_type_id: node_type_id.clone(),
                resource_path: resource_path.clone(),
                resource_revision: revision,
                create_args: ResourceBoundCreateArgsDto::Function,
                technical_terms: vec!["function".into()],
            }],
            BTreeMap::from([(
                resource_path.clone(),
                CatalogMutationResource::Function {
                    revision,
                    signature,
                    allowed_node_type_id: node_type_id,
                    parameter_binding: "target".into(),
                },
            )]),
        );
        let int_catalog = compatible_catalog(
            &snapshot,
            &GraphResourcePath("events/main".into()),
            &source(DataType::Int64),
            "en-US",
        );
        let bool_catalog = compatible_catalog(
            &snapshot,
            &GraphResourcePath("events/main".into()),
            &source(DataType::Boolean),
            "en-US",
        );

        assert!(
            int_catalog
                .items
                .iter()
                .any(|item| { item.resource_path.as_ref() == Some(&resource_path) })
        );
        assert!(
            !bool_catalog
                .items
                .iter()
                .any(|item| { item.resource_path.as_ref() == Some(&resource_path) })
        );
    }
}
